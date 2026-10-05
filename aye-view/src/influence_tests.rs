use crate::{
    app::App,
    graph::{Density, Edge},
    mouse_tests::{click, find, key, render},
};
use aye::{
    model::{Claim, ManualBlock, State, Task},
    reader::ReaderSnapshot,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{buffer::Buffer, style::Modifier};
use std::collections::BTreeSet;

const NOW: &str = "2026-10-05T00:00:00.000Z";
fn id(n: usize) -> String {
    format!("t-{n:020x}")
}
fn fixture() -> App {
    let mut state = State::empty();
    let dependencies: &[&[usize]] = &[
        &[16],
        &[0, 3],
        &[1],
        &[],
        &[0],
        &[4],
        &[0],
        &[6],
        &[0],
        &[8],
        &[0],
        &[1, 10, 4],
        &[2],
        &[2],
        &[],
        &[],
        &[],
        &[14],
    ];
    for (n, deps) in dependencies.iter().enumerate() {
        let mut task = Task::new(format!("NODE{n:02}"), NOW);
        task.id = id(n);
        task.depends_on = deps.iter().map(|&n| id(n)).collect();
        if [4, 6, 8].contains(&n) {
            task.status = "closed".into();
            task.resolution = Some(if n == 8 { "cancelled" } else { "done" }.into());
            task.closed_at = Some(NOW.into());
        }
        if n == 6 {
            task.labels.push(aye::BYPASSED_LABEL.into());
        }
        if n == 12 {
            task.manual_block = Some(ManualBlock {
                actor: "external".into(),
                reason: "waiting".into(),
                blocked_at: NOW.into(),
            });
        }
        if n == 13 {
            task.status = "deferred".into();
        }
        if n == 14 {
            task.status = "in_progress".into();
            task.claim = Some(Claim {
                actor: "worker".into(),
                claimed_at: NOW.into(),
            });
        }
        if n == 15 {
            task.parent = Some(id(0));
            task.discovered_from = Some(id(0));
        }
        state.tasks.insert(task.id.clone(), task);
    }
    state.validate().unwrap();
    App::new(ReaderSnapshot {
        oid: "initial".into(),
        state,
    })
}
fn point(app: &App, x: i64, y: i64) -> (u16, u16) {
    assert!(x >= app.graph_viewport.x && y >= app.graph_viewport.y);
    // Wide Graph's rendered interior begins at (1, 3).
    (
        (x - app.graph_viewport.x + 1) as u16,
        (y - app.graph_viewport.y + 3) as u16,
    )
}
fn assert_nodes(app: &App, frame: &Buffer, affected: &[usize]) {
    for node in app.graph.nodes.values() {
        let cell = &frame[point(app, node.x, node.y)];
        let chosen = app.selected_id.as_deref() == Some(node.id.as_str());
        assert_eq!(
            cell.modifier.contains(Modifier::REVERSED),
            chosen,
            "{} selection",
            node.id
        );
        assert_eq!(
            cell.modifier.contains(Modifier::BOLD),
            chosen || affected.iter().any(|&n| node.id == id(n)),
            "{} influence",
            node.id
        );
    }
}
pub(super) fn route_cells(edge: &Edge) -> BTreeSet<(i64, i64)> {
    edge.points
        .windows(2)
        .flat_map(|pair| {
            let (a, b) = (pair[0], pair[1]);
            if a.1 == b.1 {
                (a.0.min(b.0)..=a.0.max(b.0))
                    .map(|x| (x, a.1))
                    .collect::<Vec<_>>()
            } else {
                (a.1.min(b.1)..=a.1.max(b.1)).map(|y| (a.0, y)).collect()
            }
        })
        .collect()
}
fn assert_edges(app: &App, frame: &Buffer, eligible: &[(usize, usize)]) {
    let mut all = BTreeSet::new();
    let mut bold = BTreeSet::new();
    for edge in &app.graph.edges {
        let cells = route_cells(edge);
        all.extend(cells.iter().copied());
        if eligible
            .iter()
            .any(|&(a, b)| edge.prerequisite == id(a) && edge.dependent == id(b))
        {
            bold.extend(cells);
        }
    }
    for (x, y) in all {
        assert_eq!(
            frame[point(app, x, y)].modifier.contains(Modifier::BOLD),
            bold.contains(&(x, y)),
            "route cell ({x},{y})"
        );
    }
}
#[test]
fn influence_input_render_transitive_fanin_and_context() {
    let mut app = fixture();
    let before = app.snapshot.state.clone();
    let frame = render(&mut app, 500, 120);
    assert_nodes(&app, &frame, &[1, 2, 10, 11, 12, 13]);
    assert_edges(
        &app,
        &frame,
        &[(0, 1), (1, 2), (0, 10), (1, 11), (10, 11), (2, 12), (2, 13)],
    );
    assert_eq!(app.snapshot.state.project, before.project);
    assert_eq!(app.snapshot.state.tasks, before.tasks);
    for n in [1, 12] {
        assert_eq!(
            app.snapshot
                .state
                .effective(&app.snapshot.state.tasks[&id(n)]),
            "blocked"
        );
    }
    assert_eq!(
        app.snapshot
            .state
            .effective(&app.snapshot.state.tasks[&id(13)]),
        "deferred"
    );
    assert_eq!(
        app.snapshot
            .state
            .effective(&app.snapshot.state.tasks[&id(14)]),
        "in_progress"
    );
    let layout = app.graph.clone();
    key(&mut app, KeyCode::Right);
    assert_eq!(app.selected_id, Some(id(1)));
    let frame = render(&mut app, 500, 120);
    assert_nodes(&app, &frame, &[2, 11, 12, 13]);
    assert_edges(&app, &frame, &[(1, 2), (1, 11), (2, 12), (2, 13)]);
    assert_eq!(app.graph, layout);
    click(&mut app, find(&frame, "NODE03"));
    assert_eq!(app.selected_id, Some(id(3)));
    let frame = render(&mut app, 500, 120);
    assert_nodes(&app, &frame, &[1, 2, 11, 12, 13]);
    assert_edges(&app, &frame, &[(3, 1), (1, 2), (1, 11), (2, 12), (2, 13)]);
    click(&mut app, find(&frame, "NODE14"));
    let frame = render(&mut app, 500, 120);
    assert_nodes(&app, &frame, &[17]);
    assert_edges(&app, &frame, &[(14, 17)]);
}
#[test]
fn influence_closed_roots_cutoffs_and_alternate_paths_refresh() {
    for (resolution, bypass) in [("done", false), ("done", true), ("cancelled", false)] {
        let mut app = fixture();
        render(&mut app, 500, 120);
        let mut state = app.snapshot.state.clone();
        let task = state.tasks.get_mut(&id(1)).unwrap();
        task.status = "closed".into();
        task.resolution = Some(resolution.into());
        task.closed_at = Some(NOW.into());
        if bypass {
            task.labels.push(aye::BYPASSED_LABEL.into());
        }
        state.validate().unwrap();
        app.replace_snapshot(ReaderSnapshot {
            oid: format!("{resolution}{bypass}"),
            state,
        });
        let frame = render(&mut app, 500, 120);
        assert_eq!(app.selected_id, Some(id(0)));
        assert_nodes(&app, &frame, &[10, 11]);
        assert_edges(&app, &frame, &[(0, 10), (10, 11)]);
        app.select(&id(1));
        let frame = render(&mut app, 500, 120);
        if resolution == "cancelled" {
            assert_nodes(&app, &frame, &[2, 11, 12, 13]);
            assert_edges(&app, &frame, &[(1, 2), (1, 11), (2, 12), (2, 13)]);
            assert_eq!(
                app.snapshot
                    .state
                    .effective(&app.snapshot.state.tasks[&id(2)]),
                "blocked"
            );
        } else {
            assert_nodes(&app, &frame, &[]);
            assert_edges(&app, &frame, &[]);
        }
    }
    let mut app = fixture();
    for root in [4, 6, 8] {
        app.select(&id(root));
        let frame = render(&mut app, 500, 120);
        if root == 8 {
            assert_nodes(&app, &frame, &[9]);
            assert_edges(&app, &frame, &[(8, 9)]);
        } else {
            assert_nodes(&app, &frame, &[]);
            assert_edges(&app, &frame, &[]);
        }
    }
}
#[test]
fn influence_filter_focus_hidden_intermediary_does_not_change_scope() {
    let mut app = crate::mouse_tests::app(3, true);
    let mut state = app.snapshot.state.clone();
    for n in [0, 2] {
        state.tasks.get_mut(&id(n)).unwrap().priority = "P1".into();
    }
    app.replace_snapshot(ReaderSnapshot {
        oid: "priorities".into(),
        state,
    });
    key(&mut app, KeyCode::Char('F'));
    key(&mut app, KeyCode::Char('f'));
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Right);
    key(&mut app, KeyCode::Right);
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.query.filters.priority.as_deref(), Some("P1"));
    let ids = app.visible_ids.clone();
    let filters = app.query.filters.clone();
    let frame = render(&mut app, 200, 30);
    assert_eq!(app.focus_root, Some(id(0)));
    assert_eq!(app.selected_id, Some(id(0)));
    assert_eq!(app.visible_ids, ids);
    assert_eq!(app.query.filters, filters);
    assert_eq!(app.graph.nodes.len(), 2);
    assert!(!app.graph.nodes.contains_key(&id(1)));
    assert!(app.graph.edges.is_empty());
    assert_nodes(&app, &frame, &[2]);
    click(&mut app, find(&frame, "NODE02"));
    let frame = render(&mut app, 200, 30);
    assert_eq!(app.focus_root, Some(id(0)));
    assert_eq!(app.selected_id, Some(id(2)));
    assert_nodes(&app, &frame, &[]);
}
#[test]
fn influence_pan_zoom_resize_and_removed_empty_selection() {
    let mut app = crate::mouse_tests::app(12, true);
    render(&mut app, 110, 24);
    for _ in 0..12 {
        app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT));
    }
    let viewport = app.graph_viewport;
    assert!(viewport.x > 0);
    render(&mut app, 110, 24);
    assert_eq!(app.graph_viewport, viewport);
    for density in [Density::Standard, Density::Compact] {
        app.set_density(density);
        for (w, h) in [(110, 24), (130, 30), (20, 6)] {
            let frame = render(&mut app, w, h);
            assert_eq!(app.selected_id, Some(id(0)));
            assert_eq!(frame.content.len(), usize::from(w) * usize::from(h));
            for node in app.graph.nodes.values() {
                let x = node.x - app.graph_viewport.x + 1;
                let y = node.y - app.graph_viewport.y + if w < 70 { 4 } else { 3 };
                if node.id != id(0)
                    && x >= 1
                    && x < i64::from(app.graph_size.0) + 1
                    && y >= 3
                    && y < i64::from(h) - 2
                {
                    assert!(
                        frame[(x as u16, y as u16)]
                            .modifier
                            .contains(Modifier::BOLD)
                    );
                }
            }
        }
    }
    let mut state = app.snapshot.state.clone();
    state.tasks.remove(&id(0));
    state.tasks.get_mut(&id(1)).unwrap().depends_on.clear();
    app.replace_snapshot(ReaderSnapshot {
        oid: "removed".into(),
        state,
    });
    let frame = render(&mut app, 500, 100);
    assert_eq!(app.selected_id, Some(id(1)));
    assert!(!app.graph.nodes.contains_key(&id(0)));
    assert_nodes(&app, &frame, &(2..12).collect::<Vec<_>>());
    app.replace_snapshot(ReaderSnapshot {
        oid: "empty".into(),
        state: State::empty(),
    });
    let frame = render(&mut app, 20, 6);
    assert_eq!(app.selected_id, None);
    assert!(app.graph.nodes.is_empty());
    assert!(app.graph.edges.is_empty());
    assert!(
        !frame
            .content
            .iter()
            .any(|cell| cell.modifier.contains(Modifier::REVERSED))
    );
}
#[test]
fn influence_snapshot_reopen_and_missing_selected_clear_stale_routes() {
    let mut app = crate::mouse_tests::app(3, true);
    render(&mut app, 200, 30);
    let original = app.snapshot.state.clone();
    let mut state = original.clone();
    let b = state.tasks.get_mut(&id(1)).unwrap();
    b.status = "closed".into();
    b.resolution = Some("done".into());
    b.closed_at = Some(NOW.into());
    app.apply_update(crate::watch::Update::Snapshot(ReaderSnapshot {
        oid: "closed".into(),
        state,
    }));
    let frame = render(&mut app, 200, 30);
    assert_nodes(&app, &frame, &[]);
    assert_edges(&app, &frame, &[]);
    app.apply_update(crate::watch::Update::Snapshot(ReaderSnapshot {
        oid: "reopened".into(),
        state: original,
    }));
    let frame = render(&mut app, 200, 30);
    assert_nodes(&app, &frame, &[1, 2]);
    assert_edges(&app, &frame, &[(0, 1), (1, 2)]);
    app.selected_id = Some("t-missing".into());
    let frame = render(&mut app, 200, 30);
    assert_nodes(&app, &frame, &[]);
    assert_edges(&app, &frame, &[]);
}
