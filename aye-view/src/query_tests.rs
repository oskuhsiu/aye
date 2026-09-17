use crate::{
    app::{App, Pane},
    query::{Filters, Modal},
    view,
};
use aye::{
    model::{Claim, ManualBlock, State, Task},
    reader::ReaderSnapshot,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};
const NOW: &str = "2026-09-17T03:10:00.000Z";
fn task(id: u8, title: &str) -> Task {
    let mut t = Task::new(title.into(), NOW);
    t.id = format!("t-{id:020x}");
    t
}
fn fixture() -> App {
    let mut state = State::empty();
    let mut ready = task(1, "Ready refresh");
    ready.labels = vec!["Auth".into()];
    ready.priority = "P1".into();
    ready.kind = "bug".into();
    let mut claimed = task(2, "Claimed refresh");
    claimed.labels = vec!["Auth".into()];
    claimed.priority = "P1".into();
    claimed.kind = "bug".into();
    claimed.status = "in_progress".into();
    claimed.claim = Some(Claim {
        actor: "agent-a".into(),
        claimed_at: NOW.into(),
    });
    let mut blocked = task(3, "Blocked rollout");
    blocked.manual_block = Some(ManualBlock {
        reason: "external wait".into(),
        actor: "agent-b".into(),
        blocked_at: NOW.into(),
    });
    let mut deferred = task(4, "Deferred cleanup");
    deferred.status = "deferred".into();
    let mut done = task(5, "Historical UNIQUE archive");
    done.labels = vec!["ARCHIVE-LABEL".into()];
    done.status = "closed".into();
    done.resolution = Some("done".into());
    done.closed_at = Some(NOW.into());
    let mut cancelled = task(6, "Cancelled old migration");
    cancelled.status = "closed".into();
    cancelled.resolution = Some("cancelled".into());
    cancelled.closed_at = Some(NOW.into());
    for t in [ready, claimed, blocked, deferred, done, cancelled] {
        state.tasks.insert(t.id.clone(), t);
    }
    App::new(ReaderSnapshot {
        oid: "a".repeat(40),
        state,
    })
}
fn id(n: u8) -> String {
    format!("t-{n:020x}")
}
fn key(app: &mut App, code: KeyCode) {
    app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
}
fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        key(app, KeyCode::Char(c));
    }
}
fn frame(app: &mut App, w: u16, h: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
    terminal.draw(|f| view::render(f, app)).unwrap();
    let b = terminal.backend().buffer();
    (0..h)
        .map(|y| (0..w).map(|x| b[(x, y)].symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}
#[test]
fn search_matches_title_id_label_and_reveals_hidden_history() {
    for query in ["hIsToRiCaL", "T-00000000000000000005", "archive-label"] {
        let mut app = fixture();
        assert!(!app.visible_ids.contains(&id(5)));
        key(&mut app, KeyCode::Char('/'));
        type_text(&mut app, query);
        assert!(frame(&mut app, 100, 24).contains("Historical UNIQUE archive"));
        key(&mut app, KeyCode::Enter);
        assert_eq!(app.selected_id, Some(id(5)));
        assert!(app.visible_ids.contains(&id(5)));
        assert!(app.query.modal.is_none());
        key(&mut app, KeyCode::Enter);
        let rendered = frame(&mut app, 55, 22);
        assert!(rendered.contains("Historical UNIQUE archive"));
        assert!(rendered.contains("closed(done)"));
    }
}
#[test]
fn search_modal_owns_characters_no_match_cancel_and_ctrl_c() {
    let mut app = fixture();
    let original = app.selected_id.clone();
    key(&mut app, KeyCode::Char('/'));
    type_text(&mut app, "qfhr spaces");
    assert!(!app.quit);
    let text = frame(&mut app, 90, 20);
    assert!(text.contains("qfhr spaces"));
    assert!(text.contains("No matches"));
    key(&mut app, KeyCode::Enter);
    assert!(app.query.modal.is_some());
    key(&mut app, KeyCode::Esc);
    assert_eq!(app.selected_id, original);
    key(&mut app, KeyCode::Down);
    assert_ne!(app.selected_id, original);
    key(&mut app, KeyCode::Char('/'));
    app.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert!(app.quit);
}
#[test]
fn search_arrows_pick_visible_result_and_cancel_does_not_change_selection() {
    let mut app = fixture();
    key(&mut app, KeyCode::Char('/'));
    type_text(&mut app, "refresh");
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.selected_id, Some(id(2)));
    assert!(view::detail_text(&app).contains("Claimed refresh"));
    key(&mut app, KeyCode::Char('/'));
    type_text(&mut app, "archive");
    key(&mut app, KeyCode::Esc);
    assert_eq!(app.selected_id, Some(id(2)));
}
#[test]
fn each_filter_and_intersection_uses_canonical_effective_state() {
    let cases = [
        (
            Filters {
                state: Some("ready".into()),
                ..Default::default()
            },
            vec![id(1)],
        ),
        (
            Filters {
                state: Some("blocked".into()),
                ..Default::default()
            },
            vec![id(3)],
        ),
        (
            Filters {
                state: Some("deferred".into()),
                ..Default::default()
            },
            vec![id(4)],
        ),
        (
            Filters {
                priority: Some("P1".into()),
                ..Default::default()
            },
            vec![id(1), id(2)],
        ),
        (
            Filters {
                kind: Some("bug".into()),
                ..Default::default()
            },
            vec![id(1), id(2)],
        ),
        (
            Filters {
                label: Some("Auth".into()),
                ..Default::default()
            },
            vec![id(1), id(2)],
        ),
        (
            Filters {
                claimant: Some("agent-a".into()),
                ..Default::default()
            },
            vec![id(2)],
        ),
        (
            Filters {
                state: Some("in_progress".into()),
                priority: Some("P1".into()),
                kind: Some("bug".into()),
                label: Some("Auth".into()),
                claimant: Some("agent-a".into()),
            },
            vec![id(2)],
        ),
    ];
    for (filters, expected) in cases {
        let mut app = fixture();
        app.query.filters = filters;
        app.refresh_visible();
        assert_eq!(app.visible_ids, expected);
        assert!(
            app.selected_id
                .as_ref()
                .is_some_and(|id| app.visible_ids.contains(id))
        );
        frame(&mut app, 100, 22);
    }
}
#[test]
fn closed_filters_expose_history_clear_restores_default_and_selection() {
    let mut app = fixture();
    let original = app.visible_ids.clone();
    for (state, expected) in [
        ("closed(done)", vec![id(5)]),
        ("closed(cancelled)", vec![id(6)]),
        ("closed", vec![id(5), id(6)]),
    ] {
        app.query.filters.state = Some(state.into());
        app.refresh_visible();
        assert_eq!(app.visible_ids, expected);
    }
    key(&mut app, KeyCode::Char('f'));
    assert!(frame(&mut app, 100, 22).contains("Filter"));
    key(&mut app, KeyCode::Char('c'));
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.visible_ids, original);
    assert!(
        app.selected_id
            .as_ref()
            .is_some_and(|id| original.contains(id))
    );
}
#[test]
fn filter_modal_five_fields_apply_cancel_clear_and_empty_recovery() {
    let mut app = fixture();
    key(&mut app, KeyCode::Char('f'));
    for name in ["State", "Priority", "Type", "Label", "Claimant"] {
        assert!(frame(&mut app, 80, 20).contains(name));
    }
    // Cycle state to ready, priority to P1, type to bug, label Auth, claimant agent-a.
    key(&mut app, KeyCode::Right);
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Right);
    key(&mut app, KeyCode::Right);
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Right);
    key(&mut app, KeyCode::Right);
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Right);
    key(&mut app, KeyCode::Right);
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Right);
    key(&mut app, KeyCode::Enter);
    assert!(app.visible_ids.is_empty());
    assert!(app.selected_id.is_none());
    assert!(frame(&mut app, 100, 20).contains("No tasks match"));
    key(&mut app, KeyCode::Char('f'));
    key(&mut app, KeyCode::Char('c'));
    key(&mut app, KeyCode::Esc);
    assert!(app.visible_ids.is_empty());
    key(&mut app, KeyCode::Char('f'));
    key(&mut app, KeyCode::Char('c'));
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.visible_ids.len(), 4);
    assert_eq!(app.pane, Pane::Main);
}
#[test]
fn filter_preserves_selection_or_chooses_visible_neighbor_and_clears_reveal() {
    let mut app = fixture();
    app.select(&id(2));
    app.query.filters.priority = Some("P1".into());
    app.refresh_visible();
    assert_eq!(app.selected_id, Some(id(2)));
    app.query.filters.state = Some("ready".into());
    app.refresh_visible();
    assert_eq!(app.selected_id, Some(id(1)));
    key(&mut app, KeyCode::Char('/'));
    type_text(&mut app, "archive");
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.selected_id, Some(id(5)));
    key(&mut app, KeyCode::Char('f'));
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.visible_ids, vec![id(1)]);
}
#[test]
fn modal_tiny_unicode_backspace_and_release_are_safe() {
    let mut app = fixture();
    key(&mut app, KeyCode::Char('/'));
    type_text(&mut app, "中文");
    key(&mut app, KeyCode::Backspace);
    if let Some(Modal::Search(search)) = &app.query.modal {
        assert_eq!(search.text, "中");
    } else {
        panic!("search open");
    }
    for (w, h) in [(0, 0), (1, 1), (10, 3), (35, 15)] {
        frame(&mut app, w, h);
    }
    key(&mut app, KeyCode::Esc);
    key(&mut app, KeyCode::Char('f'));
    for (w, h) in [(0, 0), (1, 1), (10, 3), (35, 15)] {
        frame(&mut app, w, h);
    }
}

#[test]
fn leaving_revealed_history_restores_default_visibility() {
    let mut app = fixture();
    let original = app.visible_ids.clone();
    key(&mut app, KeyCode::Char('/'));
    type_text(&mut app, "archive");
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Up);
    assert_eq!(app.visible_ids, original);
    assert!(app.query.revealed_id.is_none());
    assert!(
        app.selected_id
            .as_ref()
            .is_some_and(|id| app.visible_ids.contains(id))
    );
}

#[test]
fn snapshot_recompute_removes_stale_search_results_and_preserves_filter() {
    let mut app = fixture();
    app.query.filters.priority = Some("P1".into());
    app.refresh_visible();
    key(&mut app, KeyCode::Char('/'));
    type_text(&mut app, "archive");
    app.snapshot.state.tasks.remove(&id(5));
    app.refresh_visible();
    assert!(frame(&mut app, 100, 24).contains("No matches"));
    key(&mut app, KeyCode::Enter);
    assert!(app.query.modal.is_some());
    key(&mut app, KeyCode::Esc);
    assert_eq!(app.visible_ids, vec![id(1), id(2)]);
}

#[test]
fn released_search_key_does_not_type_or_quit() {
    let mut app = fixture();
    key(&mut app, KeyCode::Char('/'));
    let mut event = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
    event.kind = crossterm::event::KeyEventKind::Release;
    app.handle_key(event);
    assert!(!app.quit);
    if let Some(Modal::Search(search)) = &app.query.modal {
        assert!(search.text.is_empty());
    } else {
        panic!("search stays open");
    }
}

#[test]
fn graph_query_integration_reveals_search_and_removes_filtered_edges() {
    let mut app = fixture();
    app.snapshot.state.tasks.get_mut(&id(3)).unwrap().depends_on = vec![id(1)];
    app.snapshot.oid = "b".repeat(40);
    app.snapshot.state.validate().unwrap();
    frame(&mut app, 180, 30);
    assert!(
        app.graph
            .edges
            .iter()
            .any(|e| e.prerequisite == id(1) && e.dependent == id(3))
    );
    app.query.filters.state = Some("blocked".into());
    app.refresh_visible();
    frame(&mut app, 180, 30);
    assert_eq!(
        app.graph.nodes.keys().cloned().collect::<Vec<_>>(),
        vec![id(3)]
    );
    assert!(app.graph.edges.is_empty());
    app.query.filters = Filters::default();
    app.refresh_visible();
    key(&mut app, KeyCode::Char('/'));
    type_text(&mut app, "historical");
    key(&mut app, KeyCode::Enter);
    let text = frame(&mut app, 180, 30);
    assert_eq!(app.selected_id, Some(id(5)));
    assert!(app.graph.nodes.contains_key(&id(5)));
    assert!(text.contains("Historical UNIQUE archive"));
    app.query.revealed_id = None;
    app.query.filters.label = Some("not-present".into());
    app.refresh_visible();
    assert!(frame(&mut app, 180, 30).contains("No tasks match filters"));
    assert!(app.graph.nodes.is_empty());
}
