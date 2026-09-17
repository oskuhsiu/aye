use crate::{
    app::{App, Pane},
    history::RecentState,
    query::Filters,
    view,
};
use aye::{
    model::{State, Task},
    reader::ReaderSnapshot,
};
use chrono::{DateTime, Duration, Utc};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};
fn now() -> DateTime<Utc> {
    "2026-09-17T12:00:00.000Z".parse().unwrap()
}
fn id(n: usize) -> String {
    format!("t-{n:020x}")
}
fn task(n: usize, title: &str, closed: Option<DateTime<Utc>>, cancelled: bool) -> Task {
    let mut t = Task::new(title.into(), "2026-09-01T00:00:00.000Z");
    t.id = id(n);
    if let Some(time) = closed {
        t.status = "closed".into();
        t.closed_at = Some(time.to_rfc3339_opts(chrono::SecondsFormat::Millis, true));
        t.resolution = Some(if cancelled { "cancelled" } else { "done" }.into());
    }
    t
}
fn app(tasks: Vec<Task>) -> App {
    let mut state = State::empty();
    for t in tasks {
        state.tasks.insert(t.id.clone(), t);
    }
    let mut app = App::new(ReaderSnapshot {
        oid: "a".repeat(40),
        state,
    });
    app.recent = RecentState::at(now());
    app
}
fn key(app: &mut App, code: KeyCode) {
    app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
}
fn frame(app: &mut App, w: u16, h: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
    terminal.draw(|f| view::render(f, app)).unwrap();
    let buffer = terminal.backend().buffer();
    (0..h)
        .map(|y| (0..w).map(|x| buffer[(x, y)].symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}
fn many() -> App {
    let mut tasks = vec![task(1000, "CURRENT WORK", None, false)];
    for n in 0..125 {
        tasks.push(task(
            n,
            &format!("CLOSED ROW {n:03}"),
            Some(now() - Duration::minutes(n as i64)),
            n % 2 == 0,
        ));
    }
    app(tasks)
}
#[test]
fn recent_fixed_boundaries_done_cancelled_and_context_deduplication() {
    let boundary = now() - Duration::hours(24);
    let mut active = task(10, "ACTIVE", None, false);
    active.depends_on.push(id(1));
    let mut app = app(vec![
        active,
        task(1, "CONTEXT", Some(now()), false),
        task(2, "AT BOUNDARY", Some(boundary), true),
        task(
            3,
            "INSIDE",
            Some(boundary + Duration::milliseconds(1)),
            false,
        ),
        task(
            4,
            "OUTSIDE",
            Some(boundary - Duration::milliseconds(1)),
            false,
        ),
        task(5, "FUTURE", Some(now() + Duration::milliseconds(1)), false),
    ]);
    assert!(!app.recent.enabled);
    assert_eq!(app.visible_ids, vec![id(1), id(10)]);
    key(&mut app, KeyCode::Char('c'));
    assert_eq!(app.visible_ids, vec![id(1), id(2), id(3), id(10)]);
    assert_eq!(app.graph_ids(), vec![id(1), id(10)]);
    assert_eq!(app.recent_only_ids(), vec![id(3), id(2)]);
    let text = frame(&mut app, 130, 28);
    assert!(text.contains("Recently closed"));
    assert!(text.contains("AT BOUNDARY"));
    assert!(text.contains('×'));
    key(&mut app, KeyCode::Char(']'));
    assert_eq!(app.selected_id, Some(id(3)));
    key(&mut app, KeyCode::Char(']'));
    assert_eq!(app.selected_id, Some(id(2)));
    key(&mut app, KeyCode::Enter);
    assert!(frame(&mut app, 45, 20).contains("AT BOUNDARY"));
    key(&mut app, KeyCode::Char('c'));
    assert_eq!(app.visible_ids, vec![id(1), id(10)]);
}
#[test]
fn recent_unchanged_clock_is_cached_and_expiration_needs_no_oid_change() {
    let mut app = app(vec![
        task(1, "EXPIRING", Some(now() - Duration::hours(24)), false),
        task(2, "CURRENT", None, false),
    ]);
    key(&mut app, KeyCode::Char('c'));
    let oid = app.snapshot.oid.clone();
    let scans = app.recent.index_builds;
    for _ in 0..10 {
        app.tick_time(now());
        frame(&mut app, 80, 18);
    }
    assert_eq!(app.recent.index_builds, scans);
    assert!(app.visible_ids.contains(&id(1)));
    app.recent.fixed_now = Some(now() + Duration::milliseconds(1));
    app.tick_time(now() + Duration::milliseconds(1));
    assert!(!app.visible_ids.contains(&id(1)));
    assert_eq!(app.snapshot.oid, oid);
    assert_eq!(app.recent.index_builds, scans);
}
#[test]
fn history_orders_ties_exposes_fifty_then_batches_and_renders_viewport_only() {
    let mut app = many();
    let tasks = app.snapshot.state.tasks.clone();
    key(&mut app, KeyCode::Char('h'));
    assert_eq!(app.visible_ids.len(), 50);
    assert_eq!(app.selected_id, Some(id(0)));
    assert_eq!(app.history.as_ref().unwrap().ids.len(), 125);
    let rendered = frame(&mut app, 110, 24);
    assert!(rendered.contains("History"));
    assert!(rendered.contains("50/125"));
    assert!(rendered.contains("CLOSED ROW 000"));
    assert!(!rendered.contains("CLOSED ROW 049"));
    assert!(app.history.as_ref().unwrap().rendered_rows <= 20);
    for _ in 0..115 {
        key(&mut app, KeyCode::Down);
        frame(&mut app, 110, 24);
    }
    assert_eq!(app.selected_id, Some(id(115)));
    assert_eq!(app.visible_ids.len(), 125);
    key(&mut app, KeyCode::Enter);
    let detail = frame(&mut app, 42, 18);
    assert!(detail.contains("CLOSED ROW 115"));
    assert!(detail.contains("closed(done)"));
    assert_eq!(app.snapshot.state.tasks, tasks);
    key(&mut app, KeyCode::Esc);
    assert!(app.history.is_some());
    key(&mut app, KeyCode::Esc);
    assert!(app.history.is_none());
    assert_eq!(app.selected_id, Some(id(1000)));
    let mut ties = app_for_ties();
    key(&mut ties, KeyCode::Char('h'));
    assert_eq!(ties.visible_ids, vec![id(1), id(2), id(3)]);
}
fn app_for_ties() -> App {
    app(vec![
        task(3, "THREE", Some(now() - Duration::seconds(1)), false),
        task(2, "TWO", Some(now()), true),
        task(1, "ONE", Some(now()), false),
    ])
}
#[test]
fn history_filters_intersect_empty_reset_and_search_exits_to_current() {
    let mut app = many();
    key(&mut app, KeyCode::Char('h'));
    app.query.filters = Filters {
        state: Some("closed(cancelled)".into()),
        ..Default::default()
    };
    app.refresh_visible();
    assert!(
        app.visible_ids
            .iter()
            .all(|id| app.snapshot.state.tasks[id].resolution.as_deref() == Some("cancelled"))
    );
    app.query.filters.priority = Some("P0".into());
    app.refresh_visible();
    assert!(app.visible_ids.is_empty());
    assert!(frame(&mut app, 100, 22).contains("No closed tasks match"));
    key(&mut app, KeyCode::Char('f'));
    key(&mut app, KeyCode::Char('c'));
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.visible_ids.len(), 50);
    key(&mut app, KeyCode::Char('/'));
    for c in "CURRENT WORK".chars() {
        key(&mut app, KeyCode::Char(c));
    }
    key(&mut app, KeyCode::Enter);
    assert!(app.history.is_none());
    assert_eq!(app.selected_id, Some(id(1000)));
    assert!(frame(&mut app, 100, 22).contains("CURRENT WORK"));
}
#[test]
fn history_preserves_late_selection_across_new_closed_insert_and_reuses_index() {
    let mut app = many();
    key(&mut app, KeyCode::Char('h'));
    for _ in 0..75 {
        key(&mut app, KeyCode::Down);
    }
    let selected = app.selected_id.clone();
    let builds = app.history.as_ref().unwrap().index_builds;
    for _ in 0..5 {
        frame(&mut app, 100, 22);
        app.refresh_visible();
    }
    assert_eq!(app.history.as_ref().unwrap().index_builds, builds);
    let new = task(
        2000,
        "JUST CLOSED",
        Some(now() + Duration::seconds(1)),
        false,
    );
    app.snapshot.state.tasks.insert(new.id.clone(), new);
    app.snapshot.oid = "b".repeat(40);
    app.refresh_visible();
    assert_eq!(app.selected_id, selected);
    assert_eq!(app.visible_ids.first(), Some(&id(2000)));
    assert_eq!(app.history.as_ref().unwrap().index_builds, builds + 1);
}
#[test]
fn empty_history_back_tiny_sizes_and_control_h_do_not_open_history() {
    let mut app = app(vec![task(1, "ACTIVE", None, false)]);
    app.handle_key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::CONTROL));
    assert!(app.history.is_none());
    key(&mut app, KeyCode::Char('h'));
    assert!(app.visible_ids.is_empty());
    assert!(frame(&mut app, 80, 20).contains("No closed tasks"));
    for (w, h) in [(0, 0), (1, 1), (10, 3), (35, 15)] {
        frame(&mut app, w, h);
    }
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Esc);
    assert_eq!(app.selected_id, Some(id(1)));
    assert_eq!(app.pane, Pane::Main);
}
#[test]
fn recent_filters_and_search_reveal_do_not_duplicate_or_hide_explicit_graph_result() {
    let mut app = app(vec![
        task(1, "ARCHIVED", Some(now()), false),
        task(2, "ACTIVE", None, false),
    ]);
    key(&mut app, KeyCode::Char('c'));
    app.query.filters.state = Some("ready".into());
    app.refresh_visible();
    assert_eq!(app.visible_ids, vec![id(2)]);
    assert!(app.recent_only_ids().is_empty());
    key(&mut app, KeyCode::Char('/'));
    for c in "ARCHIVED".chars() {
        key(&mut app, KeyCode::Char(c));
    }
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.selected_id, Some(id(1)));
    assert!(app.graph_ids().contains(&id(1)));
    assert!(app.recent_only_ids().is_empty());
}
