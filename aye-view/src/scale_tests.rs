//! Target-scale assertions at the real input/render boundary.
use crate::{app::App, query::Modal, view};
use aye::{
    model::{State, Task},
    reader::ReaderSnapshot,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};
use std::{collections::BTreeSet, time::Instant};

fn id(n: usize) -> String {
    format!("t-{:020x}", n + 1)
}
fn fixture() -> State {
    let mut state = State::empty();
    for n in 0..10_000 {
        let mut task = Task::new(format!("Scale {n:05}"), "2026-09-17T00:00:00.000Z");
        task.id = id(n);
        if n >= 1_000 {
            task.status = "closed".into();
            task.resolution = Some("done".into());
            task.closed_at = Some(task.created_at.clone());
        } else if n >= 100 {
            task.depends_on.push(id(n - 100));
        } else if n < 10 {
            task.depends_on.push(id(1_000 + n));
        }
        state.tasks.insert(task.id.clone(), task);
    }
    state.validate().unwrap();
    state
}
fn key(app: &mut App, code: KeyCode) {
    app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
}
fn draw(app: &mut App, terminal: &mut Terminal<TestBackend>) -> String {
    terminal.draw(|f| view::render(f, app)).unwrap();
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect()
}

#[test]
fn target_scale_keeps_all_context_and_bounds_history_materialization() {
    let state = fixture();
    assert_eq!(
        state
            .tasks
            .values()
            .filter(|t| state.effective(t) == "ready")
            .count(),
        100
    );
    let expected_ids: BTreeSet<_> = (0..1_010).map(id).collect();
    let expected_edges: BTreeSet<_> = state
        .tasks
        .values()
        .flat_map(|t| t.depends_on.iter().map(|p| (p.clone(), t.id.clone())))
        .collect();
    let original = state.tasks.clone();
    let started = Instant::now();
    let mut app = App::new(ReaderSnapshot {
        oid: "a".repeat(40),
        state,
    });
    let mut terminal = Terminal::new(TestBackend::new(190, 40)).unwrap();
    assert!(draw(&mut app, &mut terminal).contains("Large graph"));
    eprintln!("scale app + first frame: {:?}", started.elapsed());
    assert_eq!(
        app.visible_ids.iter().cloned().collect::<BTreeSet<_>>(),
        expected_ids
    );
    assert_eq!(
        app.graph.nodes.keys().cloned().collect::<BTreeSet<_>>(),
        expected_ids
    );
    assert_eq!(
        app.graph
            .edges
            .iter()
            .map(|e| (e.prerequisite.clone(), e.dependent.clone()))
            .collect::<BTreeSet<_>>(),
        expected_edges
    );
    let graph = app.graph.clone();
    let started = Instant::now();
    for _ in 0..10 {
        draw(&mut app, &mut terminal);
    }
    eprintln!("scale 10 unchanged frames: {:?}", started.elapsed());
    assert_eq!(app.graph, graph);

    key(&mut app, KeyCode::Char('/'));
    for c in "Scale 00999".chars() {
        key(&mut app, KeyCode::Char(c));
    }
    let Some(Modal::Search(search)) = &app.query.modal else {
        panic!("missing search");
    };
    assert_eq!(search.results, vec![id(999)]);
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.selected_id, Some(id(999)));
    key(&mut app, KeyCode::Char('F'));
    draw(&mut app, &mut terminal);
    assert_eq!(
        app.visible_ids.iter().cloned().collect::<BTreeSet<_>>(),
        (0..10).map(|n| id(99 + 100 * n)).collect()
    );
    assert_eq!(app.graph.edges.len(), 9);
    key(&mut app, KeyCode::Char('g'));
    key(&mut app, KeyCode::Tab);
    assert!(draw(&mut app, &mut terminal).contains("Scale 00999"));
    key(&mut app, KeyCode::Char('h'));
    assert!(draw(&mut app, &mut terminal).contains("50/9000"));
    assert_eq!(
        app.history
            .as_ref()
            .unwrap()
            .ids
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>(),
        (1_000..10_000).map(id).collect()
    );
    assert!(app.history.as_ref().unwrap().rendered_rows <= 35);
    let started = Instant::now();
    for _ in 0..75 {
        key(&mut app, KeyCode::Down);
        draw(&mut app, &mut terminal);
    }
    eprintln!("scale 75 history input + frames: {:?}", started.elapsed());
    assert_eq!(app.selected_id, Some(id(1_075)));
    assert_eq!(app.history.as_ref().unwrap().loaded, 100);
    assert_eq!(app.history.as_ref().unwrap().index_builds, 1);
    assert!(app.history.as_ref().unwrap().rendered_rows <= 35);
    key(&mut app, KeyCode::Enter);
    terminal = Terminal::new(TestBackend::new(50, 18)).unwrap();
    assert!(draw(&mut app, &mut terminal).contains("Scale 01075"));
    for (w, h) in [(20, 6), (1, 1), (0, 0)] {
        terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        draw(&mut app, &mut terminal);
        assert_eq!(
            terminal.backend().buffer().content.len(),
            usize::from(w) * usize::from(h)
        );
    }
    assert_eq!(
        app.snapshot.state.tasks, original,
        "Input/render changed canonical tasks"
    );
}
