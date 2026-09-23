use crate::{
    app::{App, Mode, Pane},
    graph::{Density, Viewport},
    query::Modal,
    view,
};
use aye::{
    model::{State, Task},
    reader::ReaderSnapshot,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};
const NOW: &str = "2026-09-17T03:10:00.000Z";
fn id(n: usize) -> String {
    format!("t-{n:020x}")
}
fn chain(count: usize) -> App {
    let mut state = State::empty();
    for n in 0..count {
        let mut task = Task::new(
            format!("鏈{n} 👩‍💻 {} FULL-TITLE-END", "中文".repeat(20)),
            NOW,
        );
        task.id = id(n);
        if n > 0 {
            task.depends_on.push(id(n - 1));
        }
        state.tasks.insert(task.id.clone(), task);
    }
    App::new(ReaderSnapshot {
        oid: "first".into(),
        state,
    })
}
fn key(app: &mut App, c: char) {
    app.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
}
fn frame(app: &mut App, w: u16, h: u16) -> Buffer {
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    t.draw(|f| view::render(f, app)).unwrap();
    t.backend().buffer().clone()
}
fn text(buffer: &Buffer) -> String {
    buffer.content.iter().map(|c| c.symbol()).collect()
}
#[test]
fn zoom_compact_fits_more_complete_nodes_and_preserves_details_and_truth() {
    let mut app = chain(8);
    let standard = frame(&mut app, 88, 18);
    let old_graph = app.graph.clone();
    let selected = app.selected_id.clone();
    let detail = view::detail_text(&app);
    key(&mut app, '-');
    let compact = frame(&mut app, 88, 18);
    assert!(app.graph.width < old_graph.width);
    assert!(text(&compact).matches('┘').count() > text(&standard).matches('┘').count());
    assert!(text(&compact).contains("Compact"));
    assert!(text(&compact).contains("👩‍💻"));
    assert!(text(&compact).contains('…'));
    assert_eq!(view::detail_text(&app), detail);
    assert!(detail.contains("FULL-TITLE-END"));
    assert_eq!(app.selected_id, selected);
    assert_eq!(
        app.graph.nodes.keys().collect::<Vec<_>>(),
        old_graph.nodes.keys().collect::<Vec<_>>()
    );
    assert_eq!(
        app.graph
            .edges
            .iter()
            .map(|e| (&e.prerequisite, &e.dependent))
            .collect::<Vec<_>>(),
        old_graph
            .edges
            .iter()
            .map(|e| (&e.prerequisite, &e.dependent))
            .collect::<Vec<_>>()
    );
    for (id, node) in &app.graph.nodes {
        assert_eq!(node.layer, old_graph.nodes[id].layer);
    }
    key(&mut app, '0');
    assert_eq!(frame(&mut app, 88, 18), standard);
    assert_eq!(app.graph, old_graph);
}

#[test]
fn zoom_preserves_visible_selection_and_panned_anchor_without_render_snapback() {
    let mut app = chain(20);
    app.select(&id(5));
    frame(&mut app, 88, 18);
    let selected = app.selected_id.clone();
    let offset = app.graph.nodes[&id(5)].x - app.graph_viewport.x;
    key(&mut app, '-');
    assert_eq!(app.graph.nodes[&id(5)].x - app.graph_viewport.x, offset);
    assert_eq!(app.selected_id, selected);
    let compact = frame(&mut app, 88, 18);
    let viewport = app.graph_viewport;
    key(&mut app, '-');
    assert_eq!(app.graph_viewport, viewport);
    assert_eq!(frame(&mut app, 88, 18), compact);
    key(&mut app, '+');
    frame(&mut app, 88, 18);
    let node = &app.graph.nodes[&id(5)];
    assert!(node.x >= app.graph_viewport.x);
    assert!(node.x + 28 <= app.graph_viewport.x + i64::from(app.graph_size.0));

    // Pan away from the selected root using actual input, then keep the visible
    // middle-of-chain task at its screen offset instead of revealing the root.
    app.select(&id(0));
    frame(&mut app, 88, 18);
    for _ in 0..20 {
        app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT));
    }
    frame(&mut app, 88, 18);
    assert_eq!(app.graph_viewport.x, 160);
    let offset = app.graph.nodes[&id(5)].x - app.graph_viewport.x;
    key(&mut app, '-');
    assert_eq!(app.graph.nodes[&id(5)].x - app.graph_viewport.x, offset);
    assert_eq!(app.selected_id, Some(id(0)));
    assert!(app.graph_viewport.x > 0);
    let compact = frame(&mut app, 88, 18);
    assert_eq!(frame(&mut app, 88, 18), compact);
    key(&mut app, '0');
    assert_eq!(app.graph_viewport.x, 160);
    let standard = frame(&mut app, 88, 18);
    for c in ['0', '=', '+'] {
        key(&mut app, c);
        assert_eq!(app.graph_viewport.x, 160);
        assert_eq!(frame(&mut app, 88, 18), standard);
    }
    // Partially clipped selection preserves a negative signed screen offset.
    app.select(&id(4));
    frame(&mut app, 88, 18);
    for _ in 0..2 {
        app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT));
    }
    let offset = app.graph.nodes[&id(4)].x - app.graph_viewport.x;
    assert!(offset < 0);
    key(&mut app, '-');
    assert_eq!(app.graph.nodes[&id(4)].x - app.graph_viewport.x, offset);
    frame(&mut app, 88, 18);
    assert_eq!(app.graph.nodes[&id(4)].x - app.graph_viewport.x, offset);
}

#[test]
fn zoom_keeps_focus_filter_search_reveal_and_detail_scroll() {
    let mut app = chain(12);
    for n in (1..12).step_by(2) {
        app.snapshot.state.tasks.get_mut(&id(n)).unwrap().priority = "P1".into();
    }
    app.select(&id(2));
    key(&mut app, 'F');
    key(&mut app, 'f');
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    for _ in 0..2 {
        app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
    }
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.query.filters.priority.as_deref(), Some("P1"));
    key(&mut app, '/');
    for c in "鏈2".chars() {
        key(&mut app, c);
    }
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.query.revealed_id, Some(id(2)));
    let ids = app.visible_ids.clone();
    let filters = app.query.filters.clone();
    frame(&mut app, 88, 18);
    app.detail_scroll = 3;
    for c in ['-', '=', '-', '0'] {
        key(&mut app, c);
        assert_eq!(app.focus_root, Some(id(2)));
        assert_eq!(app.selected_id, Some(id(2)));
        assert_eq!(app.query.revealed_id, Some(id(2)));
        assert_eq!(app.query.filters, filters);
        assert_eq!(app.visible_ids, ids);
        assert_eq!(app.detail_scroll, 3);
        let rendered = text(&frame(&mut app, 50, 18));
        assert!(rendered.contains(app.density.label()));
    }
}

#[test]
fn zoom_keys_are_isolated_from_other_modes_and_modals() {
    for surface in ["list", "detail", "history", "help", "search", "filter"] {
        let mut app = chain(3);
        frame(&mut app, 88, 18);
        match surface {
            "list" => app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
            "detail" => app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            "history" => key(&mut app, 'h'),
            "help" => key(&mut app, '?'),
            "search" => key(&mut app, '/'),
            "filter" => key(&mut app, 'f'),
            _ => unreachable!(),
        }
        let graph = app.graph.clone();
        let viewport = app.graph_viewport;
        for c in ['-', '+', '=', '0'] {
            key(&mut app, c);
        }
        assert_eq!(app.density, Density::Standard, "{surface}");
        assert_eq!(app.graph, graph, "{surface}");
        assert_eq!(app.graph_viewport, viewport, "{surface}");
        if let Some(Modal::Search(search)) = &app.query.modal {
            assert_eq!(search.text, "-+=0");
        }
        if surface == "list" {
            assert_eq!(app.mode, Mode::List);
        }
        if surface == "detail" {
            assert_eq!(app.pane, Pane::Detail);
        }
    }
}

#[test]
fn zoom_empty_tiny_resize_reload_and_large_world_coordinates() {
    let mut empty = chain(0);
    empty.graph_viewport = Viewport { x: 100, y: 100 };
    for (w, h) in [(0, 0), (1, 1), (20, 6), (88, 18)] {
        frame(&mut empty, w, h);
        key(&mut empty, '-');
        assert_eq!(empty.graph_viewport, Viewport::default());
        key(&mut empty, '0');
    }
    let mut app = chain(2500);
    app.select(&id(2499));
    for c in ['-', '0'] {
        key(&mut app, c);
        assert!(app.graph.width > i64::from(u16::MAX));
        for (w, h) in [(20, 6), (1, 1), (70, 15), (120, 30)] {
            let rendered = frame(&mut app, w, h);
            assert_eq!(rendered.content.len(), usize::from(w) * usize::from(h));
            assert!(app.graph_viewport.x >= 0);
            assert_eq!(app.selected_id, Some(id(2499)));
        }
    }
    key(&mut app, '-');
    let mut state = app.snapshot.state.clone();
    state.tasks.get_mut(&id(2499)).unwrap().title = "Reloaded unicode 👩‍💻".into();
    app.replace_snapshot(ReaderSnapshot {
        oid: "second".into(),
        state,
    });
    let rendered = frame(&mut app, 88, 18);
    assert_eq!(app.graph.density, Density::Compact);
    assert_eq!(app.selected_id, Some(id(2499)));
    assert!(text(&rendered).contains("Reloaded"));
    assert!(view::detail_text(&app).contains("Reloaded unicode 👩‍💻"));
}

#[test]
fn zoom_expansion_minimally_reveals_selection_and_anchor_ties_use_id() {
    let mut app = chain(20);
    key(&mut app, '-');
    app.select(&id(5));
    frame(&mut app, 50, 18);
    let node = &app.graph.nodes[&id(5)];
    assert_eq!(node.x + 20 - app.graph_viewport.x, 48);
    key(&mut app, '+');
    let node = &app.graph.nodes[&id(5)];
    assert_eq!(node.x + 28 - app.graph_viewport.x, 48);
    let viewport = app.graph_viewport;
    frame(&mut app, 50, 18);
    assert_eq!(app.graph_viewport, viewport);

    app.select(&id(0));
    frame(&mut app, 88, 18);
    // Center 104 is equidistant from source centers 86 (ID 2) and 122
    // (ID 3). Both intersect; ID 2 must win, regardless of selected ID 0.
    app.graph_viewport.x = 61;
    key(&mut app, '-');
    assert_eq!(app.graph_viewport.x, 45);
    assert_eq!(app.selected_id, Some(id(0)));
    frame(&mut app, 88, 18);
    assert_eq!(app.graph_viewport.x, 45);
}
