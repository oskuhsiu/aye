use super::*;
use crate::{
    app::{App, Mode},
    model::current_ids,
    view,
};
use aye::{
    model::{ManualBlock, State, Task},
    reader::ReaderSnapshot,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};
const NOW: &str = "2026-09-17T03:10:00.000Z";
fn fixture() -> (State, Vec<String>) {
    let mut s = State::empty();
    let mut ts: Vec<_> = (0..9)
        .map(|i| {
            let mut t = Task::new(format!("task {i}"), NOW);
            t.id = format!("t-{i:020x}");
            t
        })
        .collect();
    ts[0].status = "closed".into();
    ts[0].resolution = Some("done".into());
    ts[0].closed_at = Some(NOW.into());
    ts[1].depends_on = vec![ts[0].id.clone()];
    ts[2].depends_on = vec![ts[0].id.clone()];
    ts[3].depends_on = vec![ts[1].id.clone(), ts[2].id.clone()];
    ts[4].status = "closed".into();
    ts[4].resolution = Some("cancelled".into());
    ts[4].closed_at = Some(NOW.into());
    let cancelled = ts[4].id.clone();
    ts[3].depends_on.push(cancelled);
    ts[5].manual_block = Some(ManualBlock {
        actor: "a".into(),
        reason: "external".into(),
        blocked_at: NOW.into(),
    });
    ts[5].parent = Some(ts[0].id.clone());
    ts[5].discovered_from = Some(ts[0].id.clone());
    ts[7].depends_on = vec![ts[6].id.clone()];
    ts[8].status = "closed".into();
    ts[8].resolution = Some("done".into());
    ts[8].closed_at = Some(NOW.into());
    let ids = ts.iter().map(|t| t.id.clone()).collect();
    for t in ts {
        s.tasks.insert(t.id.clone(), t);
    }
    (s, ids)
}
#[test]
fn dependency_semantics_components_stability_and_filtered_edges() {
    let (s, ids) = fixture();
    s.validate().unwrap();
    let visible = current_ids(&s);
    let g = Graph::new(&s, &visible);
    assert_eq!(g.nodes.len(), 8);
    assert!(!g.nodes.contains_key(&ids[8]));
    assert_eq!(g.edges.len(), 6);
    for e in &g.edges {
        assert!(s.tasks[&e.dependent].depends_on.contains(&e.prerequisite));
        assert!(g.nodes[&e.prerequisite].layer < g.nodes[&e.dependent].layer);
        assert!(e.points.last().unwrap().0 > e.points.first().unwrap().0);
    }
    assert!(
        !g.edges
            .iter()
            .any(|e| e.prerequisite == ids[5] || e.dependent == ids[5])
    );
    assert!(g.nodes[&ids[5]].y > g.nodes[&ids[3]].y);
    assert!(g.nodes[&ids[6]].y > g.nodes[&ids[5]].y);
    let mut reverse = visible.clone();
    reverse.reverse();
    assert_eq!(g, Graph::new(&s, &reverse));
    assert_eq!(g, Graph::new(&s, &visible));
    let filtered = Graph::new(&s, &[ids[1].clone(), ids[3].clone()]);
    assert_eq!(filtered.edges.len(), 1);
}
#[test]
fn real_graph_render_modes_symbols_unicode_and_long_chain_clip() {
    let (s, ids) = fixture();
    let mut app = App::new(ReaderSnapshot {
        oid: "x".into(),
        state: s,
    });
    assert_eq!(app.mode, Mode::Graph);
    let mut terminal = Terminal::new(TestBackend::new(150, 50)).unwrap();
    terminal.draw(|f| view::render(f, &mut app)).unwrap();
    let text = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();
    assert!(text.contains("Graph"));
    assert!(text.contains('✓'));
    assert!(text.contains('×'));
    assert!(text.contains('!'));
    assert!(text.contains('→'));
    app.select(&ids[3]);
    app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.mode, Mode::List);
    assert_eq!(app.selected_id.as_ref(), Some(&ids[3]));
    app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.mode, Mode::Graph);
    assert_eq!(app.selected_id.as_ref(), Some(&ids[3]));
    let mut s = State::empty();
    let mut previous = None;
    for i in 0..2500 {
        let mut t = Task::new(
            format!("鏈{i} 👩‍💻 {} FULL-TITLE-END", "中文".repeat(30)),
            NOW,
        );
        t.id = format!("t-{i:020x}");
        if let Some(id) = previous {
            t.depends_on.push(id);
        }
        previous = Some(t.id.clone());
        s.tasks.insert(t.id.clone(), t);
    }
    let visible = current_ids(&s);
    let g = Graph::new(&s, &visible);
    assert!(g.width > u16::MAX as i64);
    let mut app = App::new(ReaderSnapshot {
        oid: "chain".into(),
        state: s,
    });
    app.select(previous.as_ref().unwrap());
    let mut terminal = Terminal::new(TestBackend::new(70, 15)).unwrap();
    terminal.draw(|f| view::render(f, &mut app)).unwrap();
    assert_eq!(terminal.backend().buffer().content.len(), 1050);
    assert!(app.graph_viewport.x > 0);
    let clipped = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();
    assert!(clipped.contains("👩‍💻"));
    assert!(clipped.contains('…'));
    assert!(!clipped.contains("FULL-TITLE-END"));
    assert!(view::detail_text(&app).contains("FULL-TITLE-END"));
}

#[test]
fn every_symbol_no_color_and_repeat_frame_are_meaningful() {
    let (mut state, _) = fixture();
    let mut active = Task::new("active".into(), NOW);
    active.status = "in_progress".into();
    active.claim = Some(aye::model::Claim {
        actor: "viewer\n\x1b[2J".into(),
        claimed_at: NOW.into(),
    });
    let mut deferred = Task::new("deferred".into(), NOW);
    deferred.status = "deferred".into();
    for task in [active, deferred] {
        state.tasks.insert(task.id.clone(), task);
    }
    let graph = Graph::new(&state, &current_ids(&state));
    let selected = graph.nodes.keys().next().unwrap();
    let mut terminal = Terminal::new(TestBackend::new(120, 100)).unwrap();
    for colors in [true, false] {
        terminal
            .draw(|f| {
                draw(
                    f,
                    f.area(),
                    &graph,
                    &state,
                    Some(selected),
                    Viewport::default(),
                    colors,
                )
            })
            .unwrap();
        let first = terminal.backend().buffer().clone();
        let text = first.content.iter().map(|c| c.symbol()).collect::<String>();
        assert!(!text.chars().any(char::is_control));
        for symbol in ["●", "▶", "!", "⏸", "✓", "×"] {
            assert!(text.contains(symbol), "missing {symbol}");
        }
        assert!(
            first
                .content
                .iter()
                .any(|c| c.modifier.contains(Modifier::REVERSED))
        );
        if !colors {
            assert!(first.content.iter().all(|c| c.fg == Color::Reset));
        }
        terminal
            .draw(|f| {
                draw(
                    f,
                    f.area(),
                    &graph,
                    &state,
                    Some(selected),
                    Viewport::default(),
                    colors,
                )
            })
            .unwrap();
        assert_eq!(terminal.backend().buffer(), &first);
    }
}
