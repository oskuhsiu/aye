use crate::{
    app::{App, Pane},
    detail::Detail,
    mouse_tests::{app, click, find, key, mouse, render},
};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::{
    layout::{Position, Rect},
    style::Modifier,
    text::Span,
};

fn fixture(title: &str, description: &str) -> App {
    let mut app = app(1, false);
    let task = app.snapshot.state.tasks.values_mut().next().unwrap();
    task.title = title.into();
    task.description = description.into();
    app.pane = Pane::Detail;
    app
}
fn press(app: &mut App, point: (u16, u16)) {
    mouse(app, MouseEventKind::Down(MouseButton::Left), point);
}
fn drag(app: &mut App, point: (u16, u16)) {
    mouse(app, MouseEventKind::Drag(MouseButton::Left), point);
}
fn release(app: &mut App, point: (u16, u16)) {
    mouse(app, MouseEventKind::Up(MouseButton::Left), point);
}

#[test]
fn forward_and_reverse_drag_render_and_copy_in_wide_and_narrow_detail() {
    for width in [70, 130] {
        for reverse in [false, true] {
            let mut app = fixture("Copy title", "plain description");
            let before = render(&mut app, width, 30);
            let point = find(&before, "t-00000000000000000000");
            let end = (point.0 + 6, point.1);
            let (start, end) = if reverse { (end, point) } else { (point, end) };
            press(&mut app, start);
            assert!(app.clipboard_requested.is_none());
            render(&mut app, width, 30);
            drag(&mut app, end);
            let selected = render(&mut app, width, 30);
            for x in point.0..=point.0 + 6 {
                assert!(selected[(x, point.1)].modifier.contains(Modifier::REVERSED));
            }
            assert!(
                !selected[(point.0 + 7, point.1)]
                    .modifier
                    .contains(Modifier::REVERSED)
            );
            assert!(app.clipboard_requested.is_none());
            release(&mut app, end);
            assert_eq!(app.clipboard_requested.take().as_deref(), Some("t-00000"));
            render(&mut app, width, 30);
            for modifier in [KeyModifiers::CONTROL, KeyModifiers::SUPER] {
                app.handle_key(KeyEvent::new(KeyCode::Char('c'), modifier));
                assert_eq!(app.clipboard_requested.take().as_deref(), Some("t-00000"));
                assert!(!app.quit);
            }
        }
    }
}

#[test]
fn unicode_continuation_cells_select_whole_cjk_emoji_and_combining_graphemes() {
    let mut app = fixture("Copy 中文 👩‍💻 e\u{301} tail", "");
    let before = render(&mut app, 70, 30);
    let point = find(&before, "中文");
    let end = (point.0 + Span::raw("中文 👩‍💻 ").width() as u16, point.1);
    press(&mut app, (point.0 + 1, point.1));
    render(&mut app, 70, 30);
    drag(&mut app, end);
    let selected = render(&mut app, 70, 30);
    assert!(selected[point].modifier.contains(Modifier::REVERSED));
    assert!(selected[end].modifier.contains(Modifier::REVERSED));
    release(&mut app, end);
    assert_eq!(
        app.clipboard_requested.take().as_deref(),
        Some("中文 👩‍💻 e\u{301}")
    );
}

#[test]
fn soft_wraps_do_not_add_newlines_to_copied_text() {
    let title = "START-abcdefghijklmnopqrstuvwxyz-END";
    let mut app = fixture(title, "");
    let before = render(&mut app, 20, 20);
    let start = find(&before, "START-abc");
    let last = find(&before, "mnopqrstuvwxyz-END");
    let end = (last.0 + 17, last.1);
    press(&mut app, start);
    render(&mut app, 20, 20);
    drag(&mut app, end);
    render(&mut app, 20, 20);
    release(&mut app, end);
    assert_eq!(app.clipboard_requested.take().as_deref(), Some(title));
}

#[test]
fn hard_and_blank_lines_and_trailing_spaces_remain_exact() {
    let mut app = fixture("Copy title", "ALPHA  \n\n中文 OMEGA");
    let before = render(&mut app, 70, 50);
    let start = find(&before, "ALPHA");
    let last = find(&before, "OMEGA");
    let end = (last.0 + 4, last.1);
    press(&mut app, end);
    render(&mut app, 70, 50);
    drag(&mut app, start);
    let selected = render(&mut app, 70, 50);
    assert!(
        selected[(start.0, start.1 + 1)]
            .modifier
            .contains(Modifier::REVERSED)
    );
    release(&mut app, start);
    assert_eq!(
        app.clipboard_requested.take().as_deref(),
        Some("ALPHA  \n\n中文 OMEGA")
    );
}

#[test]
fn click_does_not_copy_and_release_position_completes_a_drag_without_motion() {
    let mut app = fixture("Copy title", "");
    let before = render(&mut app, 70, 30);
    let point = find(&before, "Copy title");
    click(&mut app, point);
    assert!(app.detail.selected_text().is_none());
    assert!(app.clipboard_requested.is_none());
    render(&mut app, 70, 30);
    press(&mut app, point);
    render(&mut app, 70, 30);
    release(&mut app, (point.0 + 3, point.1));
    assert_eq!(app.clipboard_requested.take().as_deref(), Some("Copy"));
}

#[test]
fn wrong_button_release_does_not_copy_or_transfer_capture() {
    let mut app = fixture("Copy title", "");
    let before = render(&mut app, 130, 30);
    let point = find(&before, "t-00000000000000000000");
    let viewport = app.graph_viewport;
    press(&mut app, point);
    render(&mut app, 130, 30);
    drag(&mut app, (point.0 + 3, point.1));
    mouse(&mut app, MouseEventKind::Up(MouseButton::Right), (0, 0));
    assert!(app.clipboard_requested.is_none());
    assert!(app.detail.dragging());
    release(&mut app, (0, 0));
    assert_eq!(
        app.clipboard_requested.take().as_deref(),
        Some("Copy title\nt")
    );
    assert_eq!(app.graph_viewport, viewport);
    assert_eq!(app.pane, Pane::Detail);
}

#[test]
fn navigation_wheel_resize_reload_focus_and_overlays_cancel_selection_and_release() {
    for cancel in 0..9 {
        let mut app = fixture("Copy title", &"long description\n".repeat(40));
        let before = render(&mut app, 70, 30);
        let point = find(&before, "Copy title");
        press(&mut app, point);
        render(&mut app, 70, 30);
        drag(&mut app, (point.0 + 3, point.1));
        render(&mut app, 70, 30);
        match cancel {
            0 => key(&mut app, KeyCode::Esc),
            1 => key(&mut app, KeyCode::Down),
            2 => mouse(&mut app, MouseEventKind::ScrollDown, point),
            3 => {
                app.handle_event(Event::Resize(60, 25));
            }
            4 => {
                let mut snapshot = app.snapshot.clone();
                snapshot.oid = "b".repeat(40);
                app.replace_snapshot(snapshot);
            }
            5 => {
                app.handle_event(Event::FocusLost);
            }
            6 => key(&mut app, KeyCode::Char('?')),
            7 => key(&mut app, KeyCode::Char('/')),
            8 => key(&mut app, KeyCode::Char('f')),
            _ => unreachable!(),
        }
        assert!(app.detail.selected_text().is_none(), "case {cancel}");
        release(&mut app, (point.0 + 6, point.1));
        assert!(app.clipboard_requested.is_none(), "case {cancel}");
    }
}

#[test]
fn blank_padding_clipped_graphemes_and_empty_geometry_do_not_select_phantom_text() {
    let mut detail = Detail::default();
    detail.layout("中".into(), Rect::new(0, 0, 1, 4), 0);
    detail.press(Position::new(0, 0));
    detail.drag(Position::new(0, 1));
    assert!(detail.selected_text().is_none());
    detail.layout("abc".into(), Rect::new(0, 0, 8, 4), 0);
    detail.press(Position::new(0, 2));
    detail.drag(Position::new(5, 3));
    assert!(detail.selected_text().is_none());
    detail.layout("abc".into(), Rect::default(), 0);
    detail.press(Position::new(0, 0));
    detail.drag(Position::new(1, 1));
    assert!(detail.selected_text().is_none());
}

#[test]
fn scrolling_uses_new_detail_source_ranges_and_quit_shortcuts_remain_available() {
    let mut app = fixture("Copy title", &"line of detail\n".repeat(50));
    render(&mut app, 70, 18);
    for _ in 0..3 {
        key(&mut app, KeyCode::PageDown);
        render(&mut app, 70, 18);
    }
    let before = render(&mut app, 70, 18);
    let point = find(&before, "line of detail");
    press(&mut app, point);
    render(&mut app, 70, 18);
    release(&mut app, (point.0 + 3, point.1));
    assert_eq!(app.clipboard_requested.take().as_deref(), Some("line"));
    key(&mut app, KeyCode::Char('q'));
    assert!(app.quit);
    let mut app = fixture("Copy title", "");
    app.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert!(app.quit);
}
