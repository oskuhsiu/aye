use crate::{
    app::{App, Mode, Pane},
    graph::Density,
    view,
};
use aye::{
    model::{State, Task},
    reader::ReaderSnapshot,
};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, text::Span};

fn app(count: usize, chain: bool) -> App {
    let mut state = State::empty();
    for n in 0..count {
        let mut task = Task::new(format!("NODE{n:02} 中文 👩‍💻"), "2026-10-01T00:00:00.000Z");
        task.id = format!("t-{n:020x}");
        task.description = format!("DETAIL{n:02}\n{}", "long description\n".repeat(60));
        if chain && n > 0 {
            task.depends_on.push(format!("t-{:020x}", n - 1));
        }
        state.tasks.insert(task.id.clone(), task);
    }
    App::new(ReaderSnapshot {
        oid: "a".repeat(40),
        state,
    })
}
fn render(app: &mut App, width: u16, height: u16) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|f| view::render(f, app)).unwrap();
    terminal.backend().buffer().clone()
}
fn text(buffer: &Buffer) -> String {
    (0..buffer.area.height)
        .map(|y| {
            let mut line = String::new();
            let mut x = 0;
            while x < buffer.area.width {
                let symbol = buffer[(x, y)].symbol();
                line.push_str(symbol);
                x += Span::raw(symbol).width().max(1) as u16;
            }
            line
        })
        .collect::<Vec<_>>()
        .join("\n")
}
fn find(buffer: &Buffer, needle: &str) -> (u16, u16) {
    for (y, row) in text(buffer).lines().enumerate() {
        if let Some(start) = row.find(needle) {
            return (Span::raw(&row[..start]).width() as u16, y as u16);
        }
    }
    panic!("{needle:?} not displayed\n{}", text(buffer));
}
fn mouse(app: &mut App, kind: MouseEventKind, (column, row): (u16, u16)) {
    app.handle_mouse(MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    });
}
fn click(app: &mut App, point: (u16, u16)) {
    mouse(app, MouseEventKind::Down(MouseButton::Left), point);
    mouse(app, MouseEventKind::Up(MouseButton::Left), point);
}
fn key(app: &mut App, code: KeyCode) {
    app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
}

#[test]
fn current_click_selects_displayed_graph_node_at_both_densities() {
    for density in [Density::Standard, Density::Compact] {
        let mut app = app(4, false);
        app.set_density(density);
        let frame = render(&mut app, 130, 30);
        click(&mut app, find(&frame, "NODE02"));
        assert_eq!(app.selected_id.as_deref(), Some("t-00000000000000000002"));
        assert_eq!(app.pane, Pane::Main);
        assert!(text(&render(&mut app, 130, 30)).contains("DETAIL02"));
    }
}

#[test]
fn current_clipped_node_footprint_selects_but_pane_border_does_not() {
    let mut app = app(4, true);
    render(&mut app, 110, 18);
    let second = "t-00000000000000000001";
    app.graph_viewport.x = app.graph.nodes[second].x + 9;
    app.graph_anchor = app.selected_id.clone();
    render(&mut app, 110, 18);
    click(&mut app, (0, 3));
    assert_eq!(app.selected_id.as_deref(), Some("t-00000000000000000000"));
    click(&mut app, (1, 3));
    assert_eq!(app.selected_id.as_deref(), Some(second));
    assert!(text(&render(&mut app, 110, 18)).contains("t-00000000000000000001"));
}

#[test]
fn list_click_uses_final_rendered_offset_and_unicode_cells() {
    let mut app = app(60, false);
    key(&mut app, KeyCode::Tab);
    for _ in 0..35 {
        key(&mut app, KeyCode::Down);
    }
    let frame = render(&mut app, 110, 16);
    let point = find(&frame, "NODE34");
    // Hit after the wide glyphs too: the entire visible one-line row is clickable.
    click(&mut app, (point.0 + 20, point.1));
    assert_eq!(app.selected_id.as_deref(), Some("t-00000000000000000022"));
    assert!(text(&render(&mut app, 110, 16)).contains("t-00000000000000000022"));
    assert_eq!(app.mode, Mode::List);
}

#[test]
fn narrow_details_and_back_controls_preserve_browsing_and_selection() {
    let mut app = app(3, false);
    let frame = render(&mut app, 50, 18);
    click(&mut app, find(&frame, "NODE01"));
    let frame = render(&mut app, 50, 18);
    click(&mut app, find(&frame, "[Details]"));
    let frame = render(&mut app, 50, 18);
    assert_eq!(app.pane, Pane::Detail);
    assert!(text(&frame).contains("t-00000000000000000001"));
    click(&mut app, find(&frame, "[Back]"));
    assert_eq!(app.pane, Pane::Main);
    assert_eq!(app.selected_id.as_deref(), Some("t-00000000000000000001"));
    assert!(text(&render(&mut app, 50, 18)).contains("Current Graph"));
}

#[test]
fn pane_activation_and_background_click_do_not_change_task() {
    let mut app = app(1, false);
    let frame = render(&mut app, 130, 22);
    click(&mut app, find(&frame, "t-00000000000000000000"));
    assert_eq!(app.pane, Pane::Detail);
    render(&mut app, 130, 22);
    key(&mut app, KeyCode::Down);
    assert_eq!(app.detail_scroll, 1);
    render(&mut app, 130, 22);
    click(&mut app, (2, 18));
    assert_eq!(app.pane, Pane::Main);
    assert_eq!(app.selected_id.as_deref(), Some("t-00000000000000000000"));
    render(&mut app, 130, 22);
    mouse(&mut app, MouseEventKind::Down(MouseButton::Right), (90, 4));
    assert_eq!(app.pane, Pane::Main);
}

#[test]
fn overlays_isolate_clicks_and_keep_keyboard_routes() {
    for overlay in ['?', '/', 'f'] {
        let mut app = app(3, false);
        let frame = render(&mut app, 130, 30);
        let point = find(&frame, "NODE02");
        key(&mut app, KeyCode::Char(overlay));
        render(&mut app, 130, 30);
        click(&mut app, point);
        mouse(
            &mut app,
            MouseEventKind::Drag(MouseButton::Left),
            (point.0 + 1, point.1),
        );
        assert_eq!(app.selected_id.as_deref(), Some("t-00000000000000000000"));
        assert_eq!(app.pane, Pane::Main);
        key(&mut app, KeyCode::Esc);
        let frame = render(&mut app, 130, 30);
        click(&mut app, find(&frame, "NODE02"));
        assert_eq!(app.selected_id.as_deref(), Some("t-00000000000000000002"));
    }
}

#[test]
fn geometry_changes_invalidate_old_hits_until_redraw() {
    let mut app = app(3, false);
    let frame = render(&mut app, 130, 30);
    let point = find(&frame, "NODE02");
    app.handle_event(Event::Resize(30, 10));
    click(&mut app, point);
    assert_eq!(app.selected_id.as_deref(), Some("t-00000000000000000000"));
    render(&mut app, 130, 30);
    app.set_density(Density::Compact);
    click(&mut app, point);
    assert_eq!(app.selected_id.as_deref(), Some("t-00000000000000000000"));
    render(&mut app, 130, 30);
    let mut snapshot = app.snapshot.clone();
    snapshot.state.tasks.remove("t-00000000000000000002");
    snapshot.oid = "b".repeat(40);
    app.replace_snapshot(snapshot);
    click(&mut app, point);
    assert_eq!(app.selected_id.as_deref(), Some("t-00000000000000000000"));
    let frame = render(&mut app, 130, 30);
    click(&mut app, find(&frame, "NODE01"));
    assert_eq!(app.selected_id.as_deref(), Some("t-00000000000000000001"));
}

#[test]
fn empty_tiny_and_zero_layouts_do_not_expose_hidden_targets() {
    let mut app = app(3, false);
    for (w, h) in [(0, 0), (1, 1), (10, 3), (20, 6)] {
        render(&mut app, w, h);
        for point in [(0, 0), (1, 4), (49, 15)] {
            click(&mut app, point);
        }
        assert_eq!(app.selected_id.as_deref(), Some("t-00000000000000000000"));
    }
    let mut empty = App::new(ReaderSnapshot {
        oid: "c".repeat(40),
        state: State::empty(),
    });
    render(&mut empty, 50, 18);
    click(&mut empty, (2, 4));
    assert!(empty.selected_id.is_none());
}
