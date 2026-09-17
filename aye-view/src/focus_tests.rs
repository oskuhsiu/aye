use crate::{
    app::{App, Mode, Pane},
    view,
};
use aye::{
    model::{State, Task},
    reader::ReaderSnapshot,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};
const NOW: &str = "2026-09-17T03:10:00.000Z";
fn id(n: u8) -> String {
    format!("t-{n:020x}")
}
fn fixture() -> App {
    let mut state = State::empty();
    for n in 0..8 {
        let mut t = Task::new(format!("task{n}"), NOW);
        t.id = id(n);
        t.depends_on = match n {
            1 => vec![id(0)],
            2 | 3 => vec![id(1)],
            4 => vec![id(2), id(5)],
            6 => vec![id(4)],
            _ => vec![],
        };
        state.tasks.insert(t.id.clone(), t);
    }
    state.validate().unwrap();
    App::new(ReaderSnapshot {
        oid: "first".into(),
        state,
    })
}
fn key(app: &mut App, code: KeyCode) {
    app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
}
fn frame(app: &mut App, w: u16, h: u16) -> String {
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    t.draw(|f| view::render(f, app)).unwrap();
    t.backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect()
}
#[test]
fn focus_traversals_exclude_siblings_and_descendant_extra_prerequisites() {
    let mut app = fixture();
    app.select(&id(2));
    key(&mut app, KeyCode::Char('F'));
    assert_eq!(app.focus_root, Some(id(2)));
    assert_eq!(app.graph_ids(), vec![id(0), id(1), id(2), id(4), id(6)]);
    app.select(&id(4));
    assert_eq!(app.focus_root, Some(id(2)));
    key(&mut app, KeyCode::Tab);
    assert_eq!(app.mode, Mode::List);
    assert_eq!(app.selected_id, Some(id(4)));
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Esc);
    assert!(app.focus_root.is_some());
    assert_eq!(app.pane, Pane::Main);
    key(&mut app, KeyCode::Esc);
    assert!(app.focus_root.is_none());
    assert!(app.graph_ids().contains(&id(3)));
}
#[test]
fn logical_navigation_and_pan_preserve_selection_across_frames() {
    let mut app = fixture();
    app.select(&id(1));
    frame(&mut app, 55, 18);
    key(&mut app, KeyCode::Right);
    assert_eq!(app.selected_id, Some(id(2)));
    assert_eq!(app.pane, Pane::Main);
    key(&mut app, KeyCode::Down);
    assert_eq!(app.selected_id, Some(id(3)));
    key(&mut app, KeyCode::Backspace);
    assert_eq!(app.selected_id, Some(id(1)));
    key(&mut app, KeyCode::Char('l'));
    assert_eq!(app.selected_id, Some(id(2)));
    frame(&mut app, 55, 18);
    let before = app.graph_viewport;
    let selected = app.selected_id.clone();
    app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT));
    let panned = app.graph_viewport;
    assert_ne!(before, panned);
    frame(&mut app, 55, 18);
    assert_eq!(app.graph_viewport, panned);
    assert_eq!(app.selected_id, selected);
    key(&mut app, KeyCode::Char('h'));
    assert!(app.history.is_some());
}
#[test]
fn history_focus_clears_filters_suppresses_recent_and_g_restores() {
    let mut app = fixture();
    for n in [7, 8] {
        let mut t = Task::new(format!("closed{n}"), NOW);
        t.id = id(n);
        t.status = "closed".into();
        t.resolution = Some("done".into());
        t.closed_at = Some(NOW.into());
        app.snapshot.state.tasks.insert(t.id.clone(), t);
    }
    app.recent = crate::history::RecentState::at(NOW.parse().unwrap());
    app.recent.enabled = true;
    app.query.filters.state = Some("closed".into());
    app.open_history();
    app.select(&id(7));
    key(&mut app, KeyCode::Char('F'));
    assert_eq!(app.focus_root, Some(id(7)));
    assert!(app.history.is_none());
    assert!(!app.query.filters.active());
    assert_eq!(app.selected_id, Some(id(7)));
    assert_eq!(app.visible_ids, vec![id(7)]);
    assert!(app.recent_only_ids().is_empty());
    assert!(!frame(&mut app, 100, 25).contains("Recently closed"));
    key(&mut app, KeyCode::Char('g'));
    assert!(app.focus_root.is_none());
    assert!(app.history.is_none());
    assert_eq!(app.mode, Mode::Graph);
    assert!(!app.graph_ids().contains(&id(7)));
}
#[test]
fn snapshot_focus_root_is_stable_and_removed_root_exits() {
    let mut app = fixture();
    app.select(&id(2));
    key(&mut app, KeyCode::Char('F'));
    let mut state = app.snapshot.state.clone();
    let mut child = Task::new("new child".into(), NOW);
    child.id = id(9);
    child.depends_on = vec![id(2)];
    state.tasks.insert(child.id.clone(), child);
    app.replace_snapshot(ReaderSnapshot {
        oid: "second".into(),
        state: state.clone(),
    });
    assert_eq!(app.focus_root, Some(id(2)));
    assert!(app.visible_ids.contains(&id(9)));
    state.tasks.remove(&id(2));
    for task in state.tasks.values_mut() {
        task.depends_on.retain(|v| v != &id(2));
    }
    state.validate().unwrap();
    app.replace_snapshot(ReaderSnapshot {
        oid: "third".into(),
        state,
    });
    assert!(app.focus_root.is_none());
    assert_eq!(app.selected_id, Some(id(1)));
    let mut empty = App::new(ReaderSnapshot {
        oid: "empty".into(),
        state: State::empty(),
    });
    key(&mut empty, KeyCode::Char('F'));
    assert!(empty.focus_root.is_none());
}
#[test]
fn narrow_help_scrolls_and_no_ready_summary_keeps_graph() {
    let mut app = fixture();
    for task in app.snapshot.state.tasks.values_mut() {
        task.status = "deferred".into();
    }
    let text = frame(&mut app, 150, 30);
    assert!(text.contains("No ready tasks"));
    assert!(text.contains("8 deferred"));
    assert!(text.contains("Current Graph"));
    key(&mut app, KeyCode::Char('?'));
    let mut all = String::new();
    for _ in 0..30 {
        all.push_str(&frame(&mut app, 40, 15));
        key(&mut app, KeyCode::PageDown);
    }
    assert!(all.contains("without joining"));
    assert!(all.contains("Ctrl-h"));
    assert!(all.contains("Shift"));
    assert!(app.help_scroll > 0);
    key(&mut app, KeyCode::Esc);
    assert!(!app.help);
}

#[test]
fn focused_filters_history_and_search_boundaries_are_explicit() {
    let mut app = fixture();
    app.snapshot.state.tasks.get_mut(&id(4)).unwrap().priority = "P1".into();
    app.select(&id(2));
    key(&mut app, KeyCode::Char('F'));
    key(&mut app, KeyCode::Char('f'));
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Right);
    key(&mut app, KeyCode::Right);
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.focus_root, Some(id(2)));
    assert_eq!(app.visible_ids, vec![id(4)]);
    assert!(frame(&mut app, 120, 25).contains("filtered"));
    key(&mut app, KeyCode::Char('c'));
    assert!(app.recent.enabled);
    assert!(app.recent_only_ids().is_empty());
    key(&mut app, KeyCode::Char('h'));
    assert!(app.history.is_some());
    key(&mut app, KeyCode::Esc);
    assert_eq!(app.focus_root, Some(id(2)));
    assert_eq!(app.visible_ids, vec![id(4)]);
    for (query, stays) in [("task2", true), ("task3", false)] {
        key(&mut app, KeyCode::Char('/'));
        for c in query.chars() {
            key(&mut app, KeyCode::Char(c));
        }
        key(&mut app, KeyCode::Enter);
        assert_eq!(app.focus_root.is_some(), stays);
        assert_eq!(app.selected_id, Some(id(if stays { 2 } else { 3 })));
    }
    key(&mut app, KeyCode::Char('g'));
    assert!(!app.query.filters.active());
    assert!(app.query.revealed_id.is_none());
}
#[test]
fn help_quit_back_page_keys_and_narrow_footer_are_reachable() {
    for (w, h) in [(40, 15), (55, 18)] {
        let mut app = fixture();
        let base = frame(&mut app, w, h);
        assert!(base.contains("? Help"));
        assert!(base.contains("q Quit"));
        key(&mut app, KeyCode::Char('?'));
        frame(&mut app, w, h);
        key(&mut app, KeyCode::Char('j'));
        assert_eq!(app.help_scroll, 1);
        key(&mut app, KeyCode::Up);
        assert_eq!(app.help_scroll, 0);
        key(&mut app, KeyCode::PageDown);
        assert!(app.help_scroll > 0);
        key(&mut app, KeyCode::PageUp);
        assert_eq!(app.help_scroll, 0);
        key(&mut app, KeyCode::Char('q'));
        assert!(app.quit);
    }
}
#[test]
fn no_color_refresh_error_is_visible_without_color() {
    if std::env::var_os("FOCUS_NO_COLOR_CHILD").is_none() {
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .env("NO_COLOR", "1")
            .env("FOCUS_NO_COLOR_CHILD", "1")
            .args([
                "--exact",
                "focus_tests::no_color_refresh_error_is_visible_without_color",
            ])
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stdout)
        );
        return;
    }
    let mut app = fixture();
    app.refresh_error = Some("STATE_CORRUPT".into());
    let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
    terminal.draw(|f| view::render(f, &mut app)).unwrap();
    let buffer = terminal.backend().buffer();
    let footer = (0..80)
        .map(|x| buffer[(x, 19)].symbol())
        .collect::<String>();
    assert!(footer.contains("last good state"));
    assert!((0..80).all(|x| buffer[(x, 19)].fg == ratatui::style::Color::Reset));
}
