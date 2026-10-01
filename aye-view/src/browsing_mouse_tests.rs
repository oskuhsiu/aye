use crate::{
    app::{App, Mode, Pane},
    graph::Density,
    history::RecentState,
    mouse_tests::{app, click, find, key, mouse, render, text},
};
use chrono::{DateTime, Duration, Utc};
use crossterm::event::{Event, KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

fn closed_app(count: usize) -> App {
    let mut app = app(count + 1, false);
    let mut snapshot = app.snapshot.clone();
    let now: DateTime<Utc> = "2026-10-01T00:00:00.000Z".parse().unwrap();
    for n in 1..=count {
        let task = snapshot
            .state
            .tasks
            .get_mut(&format!("t-{n:020x}"))
            .unwrap();
        task.status = "closed".into();
        task.resolution = Some(if n % 2 == 0 { "cancelled" } else { "done" }.into());
        task.closed_at = Some(
            (now - Duration::seconds(n as i64))
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        );
        if n % 2 == 0 {
            task.labels.push("even".into());
        }
    }
    snapshot.oid = "b".repeat(40);
    app.replace_snapshot(snapshot);
    app.recent = RecentState::at(now);
    app
}
fn paired_app() -> App {
    let mut app = app(30, false);
    let mut snapshot = app.snapshot.clone();
    for n in 15..30 {
        snapshot
            .state
            .tasks
            .get_mut(&format!("t-{n:020x}"))
            .unwrap()
            .depends_on
            .push(format!("t-{:020x}", n - 15));
    }
    snapshot.oid = "b".repeat(40);
    app.replace_snapshot(snapshot);
    app
}
fn wheel(app: &mut App, kind: MouseEventKind, point: (u16, u16), modifiers: KeyModifiers) {
    app.handle_mouse(MouseEvent {
        kind,
        column: point.0,
        row: point.1,
        modifiers,
    });
}
fn down(app: &mut App, point: (u16, u16)) {
    mouse(app, MouseEventKind::Down(MouseButton::Left), point);
}
fn drag(app: &mut App, point: (u16, u16)) {
    mouse(app, MouseEventKind::Drag(MouseButton::Left), point);
}
fn up(app: &mut App, point: (u16, u16)) {
    mouse(app, MouseEventKind::Up(MouseButton::Left), point);
}

#[test]
fn recent_click_and_narrow_back_keep_the_recent_context() {
    let mut app = closed_app(10);
    key(&mut app, KeyCode::Char('c'));
    let frame = render(&mut app, 50, 22);
    click(&mut app, find(&frame, "NODE02"));
    assert_eq!(app.selected_id.as_deref(), Some("t-00000000000000000002"));
    assert!(app.recent.enabled);
    let frame = render(&mut app, 50, 22);
    click(&mut app, find(&frame, "[Details]"));
    let frame = render(&mut app, 50, 22);
    assert!(text(&frame).contains("closed(cancelled)"));
    click(&mut app, find(&frame, "[Back]"));
    assert!(text(&render(&mut app, 50, 22)).contains("Recently closed"));
    assert!(app.recent.enabled);
}

#[test]
fn history_click_after_batch_and_filter_retains_history_and_return_selection() {
    let mut app = closed_app(150);
    let original = app.selected_id.clone();
    app.query.filters.label = Some("even".into());
    key(&mut app, KeyCode::Char('h'));
    for _ in 0..6 {
        render(&mut app, 110, 16);
        key(&mut app, KeyCode::PageDown);
    }
    let selected = app.selected_id.clone().unwrap();
    let position = app
        .visible_ids
        .iter()
        .position(|id| id == &selected)
        .unwrap();
    assert!(position > 50);
    let target = app.visible_ids[position - 1].clone();
    let title = app.snapshot.state.tasks[&target].title.clone();
    let frame = render(&mut app, 110, 16);
    click(&mut app, find(&frame, &title));
    assert_eq!(app.selected_id.as_deref(), Some(target.as_str()));
    let frame = render(&mut app, 50, 18);
    click(&mut app, find(&frame, "[Details]"));
    let frame = render(&mut app, 50, 18);
    assert!(text(&frame).contains(&target));
    click(&mut app, find(&frame, "[Back]"));
    assert!(app.history.is_some());
    assert_eq!(app.query.filters.label.as_deref(), Some("even"));
    key(&mut app, KeyCode::Esc);
    // The retained filter excludes the original active task, as in keyboard History.
    assert!(app.history.is_none());
    assert_ne!(app.selected_id, original);
    app.show_current_graph();
    assert_eq!(app.selected_id, original);
}

#[test]
fn list_wheel_retains_selection_and_focus_without_render_snapback_then_clicks_actual_row() {
    let mut app = app(100, false);
    key(&mut app, KeyCode::Tab);
    render(&mut app, 110, 16);
    app.pane = Pane::Detail;
    wheel(
        &mut app,
        MouseEventKind::ScrollDown,
        (2, 5),
        KeyModifiers::NONE,
    );
    let frame = render(&mut app, 110, 16);
    assert_eq!(app.list_offset, 3);
    assert_eq!(app.selected_id.as_deref(), Some("t-00000000000000000000"));
    assert_eq!(app.pane, Pane::Detail);
    assert!(text(&frame).contains("t-00000000000000000000"));
    assert!(text(&frame).lines().nth(3).unwrap().contains("NODE03"));
    click(&mut app, find(&frame, "NODE05"));
    assert_eq!(app.selected_id.as_deref(), Some("t-00000000000000000005"));
    for _ in 0..12 {
        render(&mut app, 110, 16);
        wheel(
            &mut app,
            MouseEventKind::ScrollDown,
            (2, 5),
            KeyModifiers::NONE,
        );
    }
    let scrolled = render(&mut app, 110, 16);
    assert!(app.list_offset > 20);
    assert!(!text(&scrolled).lines().take(14).any(|line| {
        line.split('│')
            .nth(1)
            .is_some_and(|main| main.contains("NODE05"))
    }));
    key(&mut app, KeyCode::Down);
    let frame = render(&mut app, 110, 16);
    assert!(text(&frame).contains("> ● P2 NODE06"));
    assert_eq!(app.selected_id.as_deref(), Some("t-00000000000000000006"));
}

#[test]
fn detail_wheel_uses_pointed_pane_and_reaches_wrapped_unicode_tail() {
    let mut app = app(1, false);
    let id = app.selected_id.clone().unwrap();
    app.snapshot.state.tasks.get_mut(&id).unwrap().description =
        format!("{}\nEND-DETAIL", "中文 👩‍💻 wrapped content ".repeat(400));
    let frame = render(&mut app, 110, 16);
    let point = find(&frame, &id);
    wheel(
        &mut app,
        MouseEventKind::ScrollDown,
        point,
        KeyModifiers::NONE,
    );
    assert_eq!(app.detail_scroll, 3);
    assert_eq!(app.pane, Pane::Main);
    let mut tail = false;
    for _ in 0..500 {
        let frame = render(&mut app, 110, 16);
        tail |= text(&frame).contains("END-DETAIL");
        wheel(
            &mut app,
            MouseEventKind::ScrollDown,
            point,
            KeyModifiers::NONE,
        );
    }
    render(&mut app, 110, 16);
    assert!(tail);
    assert_eq!(app.detail_scroll, app.detail_max_scroll);
    assert_eq!(app.selected_id.as_deref(), Some(id.as_str()));
    wheel(
        &mut app,
        MouseEventKind::ScrollLeft,
        point,
        KeyModifiers::NONE,
    );
    assert_eq!(app.detail_scroll, app.detail_max_scroll);
    for _ in 0..500 {
        render(&mut app, 110, 16);
        wheel(
            &mut app,
            MouseEventKind::ScrollUp,
            point,
            KeyModifiers::NONE,
        );
    }
    assert_eq!(app.detail_scroll, 0);
}

#[test]
fn recent_scroll_is_independent_then_keyboard_selection_reveals() {
    let mut app = closed_app(20);
    key(&mut app, KeyCode::Char('c'));
    let frame = render(&mut app, 110, 22);
    let point = find(&frame, "NODE01");
    wheel(
        &mut app,
        MouseEventKind::ScrollDown,
        point,
        KeyModifiers::NONE,
    );
    let frame = render(&mut app, 110, 22);
    assert!(text(&frame).contains("NODE04"));
    assert!(!text(&frame).contains("NODE01"));
    assert_eq!(app.selected_id.as_deref(), Some("t-00000000000000000000"));
    click(&mut app, find(&frame, "NODE05"));
    for _ in 0..3 {
        render(&mut app, 110, 22);
        wheel(
            &mut app,
            MouseEventKind::ScrollDown,
            point,
            KeyModifiers::NONE,
        );
    }
    render(&mut app, 110, 22);
    key(&mut app, KeyCode::Char(']'));
    let frame = render(&mut app, 110, 22);
    assert!(text(&frame).contains("> ✓ P2 NODE06") || text(&frame).contains("> × P2 NODE06"));
    assert_eq!(app.selected_id.as_deref(), Some("t-00000000000000000006"));
}

#[test]
fn history_wheel_exposes_final_matching_row_with_bounded_rendering_and_selection() {
    for filtered in [false, true] {
        let mut app = closed_app(120);
        if filtered {
            app.query.filters.label = Some("even".into());
        }
        key(&mut app, KeyCode::Char('h'));
        let selected = app.selected_id.clone();
        for _ in 0..100 {
            render(&mut app, 110, 16);
            assert!(app.history.as_ref().unwrap().rendered_rows <= 11);
            wheel(
                &mut app,
                MouseEventKind::ScrollDown,
                (2, 5),
                KeyModifiers::NONE,
            );
        }
        let frame = render(&mut app, 110, 16);
        assert_eq!(app.selected_id, selected);
        assert_eq!(app.visible_ids.len(), if filtered { 60 } else { 120 });
        assert!(text(&frame).contains("NODE120"));
        click(&mut app, find(&frame, "NODE120"));
        assert_eq!(app.selected_id.as_deref(), Some("t-00000000000000000078"));
        assert!(text(&render(&mut app, 110, 16)).contains("t-00000000000000000078"));
    }
}

#[test]
fn wheel_graph_axes_shift_and_bounds_preserve_task_and_pane() {
    for density in [Density::Standard, Density::Compact] {
        let mut app = paired_app();
        app.set_density(density);
        render(&mut app, 40, 18);
        wheel(
            &mut app,
            MouseEventKind::ScrollDown,
            (30, 8),
            KeyModifiers::NONE,
        );
        assert_eq!(app.graph_viewport.y, 3);
        render(&mut app, 40, 18);
        wheel(
            &mut app,
            MouseEventKind::ScrollRight,
            (30, 8),
            KeyModifiers::NONE,
        );
        assert_eq!(app.graph_viewport.x, 3);
        render(&mut app, 40, 18);
        wheel(
            &mut app,
            MouseEventKind::ScrollDown,
            (30, 8),
            KeyModifiers::SHIFT,
        );
        assert_eq!(app.graph_viewport.x, 6);
        assert_eq!(app.graph_viewport.y, 3);
        let selected = app.selected_id.clone();
        for _ in 0..200 {
            render(&mut app, 40, 18);
            wheel(
                &mut app,
                MouseEventKind::ScrollDown,
                (30, 8),
                KeyModifiers::NONE,
            );
        }
        assert_eq!(
            app.graph_viewport.y,
            (app.graph.height - i64::from(app.graph_size.1)).max(0)
        );
        assert_eq!(app.selected_id, selected);
        key(&mut app, KeyCode::Up);
        render(&mut app, 40, 18);
        assert_eq!(app.graph_viewport.y, 0);
    }
}

#[test]
fn help_and_query_wheels_cannot_reach_hidden_background() {
    for modal in ['?', '/', 'f'] {
        let mut app = paired_app();
        render(&mut app, 50, 18);
        key(&mut app, KeyCode::Char(modal));
        render(&mut app, 50, 18);
        wheel(
            &mut app,
            MouseEventKind::ScrollDown,
            (2, 5),
            KeyModifiers::NONE,
        );
        assert_eq!(app.graph_viewport.y, 0);
        if modal == '?' {
            assert_eq!(app.help_scroll, 3);
            for _ in 0..100 {
                render(&mut app, 50, 18);
                wheel(
                    &mut app,
                    MouseEventKind::ScrollDown,
                    (2, 5),
                    KeyModifiers::NONE,
                );
            }
            assert_eq!(app.help_scroll, app.help_max_scroll);
        } else {
            assert_eq!(app.help_scroll, 0);
        }
    }
}

#[test]
fn background_drag_moves_content_both_axes_and_never_selects_on_release() {
    for density in [Density::Standard, Density::Compact] {
        let mut app = paired_app();
        app.set_density(density);
        let before = render(&mut app, 40, 18);
        let selected = app.selected_id.clone();
        down(&mut app, (35, 8));
        render(&mut app, 40, 18);
        drag(&mut app, (30, 5));
        let moved = render(&mut app, 40, 18);
        assert_eq!((app.graph_viewport.x, app.graph_viewport.y), (5, 3));
        assert_ne!(text(&before), text(&moved));
        assert_eq!(app.selected_id, selected);
        assert_eq!(app.pane, Pane::Main);
        up(&mut app, (200, 10));
        let viewport = app.graph_viewport;
        drag(&mut app, (20, 2));
        assert_eq!(app.graph_viewport, viewport);
    }
}

#[test]
fn node_press_selects_without_pan_and_drag_crossing_detail_keeps_main() {
    let mut app = paired_app();
    let frame = render(&mut app, 130, 30);
    down(&mut app, find(&frame, "NODE02"));
    render(&mut app, 130, 30);
    let viewport = app.graph_viewport;
    drag(&mut app, (20, 20));
    assert_eq!(app.graph_viewport, viewport);
    up(&mut app, (20, 20));
    assert_eq!(app.selected_id.as_deref(), Some("t-00000000000000000002"));
    app.graph_viewport = Default::default();
    app.graph_anchor = app.selected_id.clone();
    render(&mut app, 130, 30);
    down(&mut app, (50, 7));
    render(&mut app, 130, 30);
    drag(&mut app, (45, 5));
    render(&mut app, 130, 30);
    drag(&mut app, (100, 15));
    render(&mut app, 130, 30);
    up(&mut app, (100, 15));
    assert_eq!(app.pane, Pane::Main);
    assert_eq!(app.selected_id.as_deref(), Some("t-00000000000000000002"));
    let frame = render(&mut app, 130, 30);
    click(&mut app, find(&frame, "t-00000000000000000002"));
    assert_eq!(app.pane, Pane::Detail);
}

#[test]
fn navigation_and_geometry_changes_cancel_drag_and_next_gesture_starts_cleanly() {
    for action in 0..9 {
        let mut app = paired_app();
        render(&mut app, 40, 18);
        down(&mut app, (35, 8));
        render(&mut app, 40, 18);
        drag(&mut app, (32, 6));
        render(&mut app, 40, 18);
        match action {
            0 => key(&mut app, KeyCode::Esc),
            1 => app.set_density(Density::Compact),
            2 => {
                app.handle_event(Event::Resize(40, 18));
            }
            3 => {
                app.handle_event(Event::FocusLost);
            }
            4 => {
                let mut s = app.snapshot.clone();
                s.oid = "c".repeat(40);
                app.replace_snapshot(s);
            }
            5 => key(&mut app, KeyCode::Char('g')),
            6 => key(&mut app, KeyCode::Tab),
            7 => key(&mut app, KeyCode::Char('f')),
            _ => mouse(&mut app, MouseEventKind::Down(MouseButton::Right), (32, 6)),
        }
        render(&mut app, 40, 18);
        let viewport = app.graph_viewport;
        drag(&mut app, (20, 2));
        assert_eq!(app.graph_viewport, viewport, "cancellation {action}");
    }
    let mut app = paired_app();
    render(&mut app, 40, 18);
    down(&mut app, (35, 8));
    app.handle_event(Event::Resize(40, 18));
    render(&mut app, 40, 18);
    down(&mut app, (35, 8));
    render(&mut app, 40, 18);
    drag(&mut app, (30, 5));
    assert_eq!((app.graph_viewport.x, app.graph_viewport.y), (5, 3));
}

#[test]
fn passive_motion_no_redraw_and_non_graph_surfaces_do_not_start_pan() {
    let mut app = paired_app();
    render(&mut app, 130, 30);
    for _ in 0..1000 {
        assert!(!app.handle_event(Event::Mouse(MouseEvent {
            kind: MouseEventKind::Moved,
            column: 2,
            row: 4,
            modifiers: KeyModifiers::NONE
        })));
    }
    for modal in ['?', '/', 'f'] {
        key(&mut app, KeyCode::Char(modal));
        render(&mut app, 130, 30);
        down(&mut app, (50, 7));
        drag(&mut app, (40, 5));
        up(&mut app, (40, 5));
        assert_eq!(app.graph_viewport, Default::default());
        key(&mut app, KeyCode::Esc);
    }
    key(&mut app, KeyCode::Tab);
    render(&mut app, 130, 30);
    down(&mut app, (50, 7));
    drag(&mut app, (40, 5));
    assert_eq!(app.mode, Mode::List);
    assert_eq!(app.graph_viewport, Default::default());
}

#[test]
fn graph_pointer_drag_uses_wide_world_coordinates() {
    let mut app = app(2000, true);
    app.select("t-000000000000000007cf");
    render(&mut app, 50, 18);
    assert!(app.graph_viewport.x > i64::from(u16::MAX));
    let before = app.graph_viewport.x;
    down(&mut app, (20, 12));
    render(&mut app, 50, 18);
    drag(&mut app, (25, 12));
    render(&mut app, 50, 18);
    assert_eq!(app.graph_viewport.x, before - 5);
    up(&mut app, (25, 12));
    assert_eq!(app.selected_id.as_deref(), Some("t-000000000000000007cf"));
}

#[test]
fn wheel_and_drag_preserve_focus_filters_and_search_reveal() {
    let mut app = paired_app();
    app.select("t-00000000000000000000");
    app.focus_selected();
    app.query.filters.priority = Some("P2".into());
    app.refresh_visible();
    let root = app.focus_root.clone();
    let filters = app.query.filters.clone();
    render(&mut app, 50, 18);
    down(&mut app, (35, 12));
    drag(&mut app, (40, 12));
    up(&mut app, (40, 12));
    assert_eq!(app.focus_root, root);
    assert_eq!(app.query.filters, filters);
    app.show_current_graph();
    app.query.revealed_id = app.selected_id.clone();
    app.refresh_visible();
    let reveal = app.query.revealed_id.clone();
    render(&mut app, 50, 18);
    wheel(
        &mut app,
        MouseEventKind::ScrollDown,
        (40, 8),
        KeyModifiers::NONE,
    );
    render(&mut app, 50, 18);
    down(&mut app, (35, 12));
    drag(&mut app, (40, 12));
    up(&mut app, (40, 12));
    assert_eq!(app.query.revealed_id, reveal);
}

#[test]
fn redraw_geometry_alone_cancels_capture_and_reload_clamps_independent_list() {
    let mut graph = paired_app();
    render(&mut graph, 40, 18);
    down(&mut graph, (35, 8));
    render(&mut graph, 110, 22);
    let viewport = graph.graph_viewport;
    drag(&mut graph, (20, 2));
    assert_eq!(graph.graph_viewport, viewport);
    let mut list = app(100, false);
    key(&mut list, KeyCode::Tab);
    for _ in 0..20 {
        render(&mut list, 110, 16);
        wheel(
            &mut list,
            MouseEventKind::ScrollDown,
            (2, 5),
            KeyModifiers::NONE,
        );
    }
    render(&mut list, 110, 16);
    let mut snapshot = list.snapshot.clone();
    snapshot.oid = "c".repeat(40);
    snapshot
        .state
        .tasks
        .retain(|id, _| id.as_str() <= "t-0000000000000000000b");
    list.replace_snapshot(snapshot);
    let frame = render(&mut list, 110, 16);
    assert_eq!(list.selected_id.as_deref(), Some("t-00000000000000000000"));
    assert!(list.list_offset <= 1);
    click(&mut list, find(&frame, "NODE07"));
    assert_eq!(list.selected_id.as_deref(), Some("t-00000000000000000007"));
}

#[test]
fn tiny_and_empty_pointer_scrolling_and_gestures_are_safe() {
    for count in [0, 3] {
        let mut app = app(count, false);
        let selected = app.selected_id.clone();
        for (w, h) in [(0, 0), (1, 1), (10, 3), (20, 6)] {
            render(&mut app, w, h);
            for point in [(0, 0), (1, 4), (100, 100)] {
                wheel(
                    &mut app,
                    MouseEventKind::ScrollDown,
                    point,
                    KeyModifiers::NONE,
                );
                down(&mut app, point);
                drag(&mut app, (120, 120));
                up(&mut app, (120, 120));
            }
            assert_eq!(app.selected_id, selected);
        }
    }
}

#[test]
fn resize_preserves_visible_final_history_selection_through_details_back() {
    for reported in [false, true] {
        let mut app = closed_app(120);
        key(&mut app, KeyCode::Char('h'));
        for _ in 0..50 {
            render(&mut app, 110, 16);
            wheel(
                &mut app,
                MouseEventKind::ScrollDown,
                (2, 5),
                KeyModifiers::NONE,
            );
        }
        let frame = render(&mut app, 110, 16);
        click(&mut app, find(&frame, "NODE120"));
        render(&mut app, 110, 16);
        if reported {
            app.handle_event(Event::Resize(50, 10));
        }
        let frame = render(&mut app, 50, 10);
        assert!(text(&frame).contains("> × P2 NODE120"));
        click(&mut app, find(&frame, "[Details]"));
        let frame = render(&mut app, 50, 10);
        assert!(text(&frame).contains("t-00000000000000000078"));
        click(&mut app, find(&frame, "[Back]"));
        assert!(text(&render(&mut app, 50, 10)).contains("> × P2 NODE120"));
    }
}

#[test]
fn resize_does_not_reveal_a_manually_hidden_selection_through_details_back() {
    let mut app = app(60, false);
    key(&mut app, KeyCode::Tab);
    for _ in 0..20 {
        render(&mut app, 110, 16);
        wheel(
            &mut app,
            MouseEventKind::ScrollDown,
            (2, 5),
            KeyModifiers::NONE,
        );
    }
    render(&mut app, 110, 16);
    let offset = app.list_offset;
    app.handle_event(Event::Resize(50, 10));
    let frame = render(&mut app, 50, 10);
    assert_eq!(app.list_offset, offset);
    assert!(!text(&frame).contains("NODE00"));
    click(&mut app, find(&frame, "[Details]"));
    let frame = render(&mut app, 50, 10);
    assert!(text(&frame).contains("t-00000000000000000000"));
    click(&mut app, find(&frame, "[Back]"));
    let frame = render(&mut app, 50, 10);
    assert_eq!(app.list_offset, offset);
    assert!(!text(&frame).contains("NODE00"));
}

#[test]
fn resize_in_detail_retains_row_intent_through_detail_wheel_and_back() {
    for hidden in [false, true] {
        let mut app = closed_app(120);
        key(&mut app, KeyCode::Char('h'));
        for _ in 0..50 {
            render(&mut app, 110, 16);
            wheel(
                &mut app,
                MouseEventKind::ScrollDown,
                (2, 5),
                KeyModifiers::NONE,
            );
        }
        let frame = render(&mut app, 110, 16);
        if !hidden {
            click(&mut app, find(&frame, "NODE120"));
        }
        let frame = render(&mut app, 110, 16);
        let selected = app.selected_id.clone();
        click(&mut app, find(&frame, "[Details]"));
        render(&mut app, 110, 16);
        app.handle_event(Event::Resize(50, 10));
        render(&mut app, 50, 10);
        wheel(
            &mut app,
            MouseEventKind::ScrollDown,
            (2, 4),
            KeyModifiers::NONE,
        );
        let frame = render(&mut app, 50, 10);
        click(&mut app, find(&frame, "[Back]"));
        let frame = render(&mut app, 50, 10);
        assert_eq!(app.selected_id, selected);
        assert_eq!(app.pane, Pane::Main);
        if hidden {
            assert!(!text(&frame).contains("NODE01"));
        } else {
            assert!(text(&frame).contains("> × P2 NODE120"));
        }
    }
}
