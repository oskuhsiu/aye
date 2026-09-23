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

fn crossing_fixture() -> State {
    let mut state = State::empty();
    let mut tasks: Vec<_> = (0..5)
        .map(|i| {
            let mut t = Task::new(char::from(b'A' + i).to_string(), NOW);
            t.id = format!("t-{i:020x}");
            t
        })
        .collect();
    tasks[3].depends_on = vec![tasks[0].id.clone()];
    tasks[2].depends_on = vec![tasks[1].id.clone()];
    tasks[4].depends_on = vec![tasks[2].id.clone(), tasks[3].id.clone()];
    for task in tasks {
        state.tasks.insert(task.id.clone(), task);
    }
    state.validate().unwrap();
    state
}
fn segments_share_straight(a: &Edge, b: &Edge) -> bool {
    a.points.windows(2).any(|a| {
        b.points.windows(2).any(|b| {
            let horizontal = a[0].1 == a[1].1 && b[0].1 == b[1].1 && a[0].1 == b[0].1;
            let vertical = a[0].0 == a[1].0 && b[0].0 == b[1].0 && a[0].0 == b[0].0;
            if horizontal {
                a[0].0.min(a[1].0).max(b[0].0.min(b[1].0))
                    < a[0].0.max(a[1].0).min(b[0].0.max(b[1].0))
            } else if vertical {
                a[0].1.min(a[1].1).max(b[0].1.min(b[1].1))
                    < a[0].1.max(a[1].1).min(b[0].1.max(b[1].1))
            } else {
                false
            }
        })
    })
}
#[test]
fn crossed_dependencies_remain_visually_distinct_paths() {
    for density in [Density::Standard, Density::Compact] {
        let state = crossing_fixture();
        let graph = Graph::with_density(&state, &current_ids(&state), density);
        let mut terminal = Terminal::new(TestBackend::new(130, 16)).unwrap();
        terminal
            .draw(|f| {
                draw(
                    f,
                    f.area(),
                    &graph,
                    &state,
                    None,
                    Viewport::default(),
                    false,
                )
            })
            .unwrap();
        let frame = terminal.backend().buffer();
        let text = frame
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(
            text.contains('╳'),
            "independent paths must visibly cross without joining: {text}"
        );
        for a in &graph.edges {
            for b in &graph.edges {
                if a.prerequisite != b.prerequisite && a.dependent != b.dependent {
                    assert!(
                        !segments_share_straight(a, b),
                        "independent paths share a straight segment: {a:?}, {b:?}"
                    );
                }
            }
        }
        for edge in &graph.edges {
            let source = &graph.nodes[&edge.prerequisite];
            let target = &graph.nodes[&edge.dependent];
            assert_ne!(
                edge.points[0].1 - source.y,
                edge.points.last().unwrap().1 - target.y
            );
            for points in edge.points.windows(3) {
                let (previous, corner, next) = (points[0], points[1], points[2]);
                let expected = match (
                    previous.0 < corner.0,
                    previous.1 < corner.1,
                    next.0 > corner.0,
                    next.1 > corner.1,
                ) {
                    (true, false, false, true) => "┐",
                    (true, false, false, false) => "┘",
                    (false, true, true, false) => "└",
                    (false, false, true, false) => "┌",
                    _ => panic!("unexpected route bend"),
                };
                let actual = frame[(corner.0 as u16, corner.1 as u16)].symbol();
                // Shared target branches may form a real junction, never an independent fake junction.
                assert!(
                    actual == expected
                        || actual == "├"
                        || actual == "┤"
                        || actual == "┬"
                        || actual == "┴"
                        || actual == "╳",
                    "bend expected {expected}, got {actual}"
                );
            }
            let end = *edge.points.last().unwrap();
            assert_eq!(frame[(end.0 as u16, end.1 as u16)].symbol(), "→");
        }
    }
}

#[test]
fn long_edge_departure_arrival_tracks_never_share_independent_segments() {
    for density in [Density::Standard, Density::Compact] {
        let mut state = crossing_fixture();
        let a = "t-00000000000000000000".to_string();
        let b = "t-00000000000000000001".to_string();
        let c = "t-00000000000000000002".to_string();
        let d = "t-00000000000000000003".to_string();
        let e = "t-00000000000000000004".to_string();
        state.tasks.get_mut(&e).unwrap().depends_on.push(a);
        let mut f = Task::new("F".into(), NOW);
        f.id = "t-00000000000000000005".into();
        f.depends_on = vec![b, c, d];
        state.tasks.insert(f.id.clone(), f);
        state.validate().unwrap();
        let graph = Graph::with_density(&state, &current_ids(&state), density);
        assert_eq!(
            graph.edges.iter().filter(|e| e.points.len() == 6).count(),
            2
        );
        for a in &graph.edges {
            for b in &graph.edges {
                if a.prerequisite != b.prerequisite && a.dependent != b.dependent {
                    assert!(
                        !segments_share_straight(a, b),
                        "independent long/short paths overlap: {a:?} {b:?}"
                    );
                }
            }
        }
        let mut terminal = Terminal::new(TestBackend::new(150, 24)).unwrap();
        terminal
            .draw(|f| {
                draw(
                    f,
                    f.area(),
                    &graph,
                    &state,
                    None,
                    Viewport::default(),
                    false,
                )
            })
            .unwrap();
        let frame = terminal.backend().buffer();
        assert!(frame.content.iter().any(|cell| cell.symbol() == "╳"));
        for edge in &graph.edges {
            for pair in edge.points.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                if a.1 == b.1 {
                    for x in a.0.min(b.0) + 1..a.0.max(b.0) {
                        assert!(
                            ["─", "┬", "┴", "┼", "╳"]
                                .contains(&frame[(x as u16, a.1 as u16)].symbol()),
                            "horizontal path interrupted"
                        );
                    }
                } else {
                    for y in a.1.min(b.1) + 1..a.1.max(b.1) {
                        assert!(
                            ["│", "├", "┤", "┼", "╳"]
                                .contains(&frame[(a.0 as u16, y as u16)].symbol()),
                            "vertical path interrupted"
                        );
                    }
                }
            }
        }
    }
}

fn color_frame(graph: &Graph, state: &State, colors: bool) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(160, 160)).unwrap();
    terminal
        .draw(|f| {
            draw(
                f,
                f.area(),
                graph,
                state,
                graph.nodes.keys().next().map(String::as_str),
                Viewport::default(),
                colors,
            )
        })
        .unwrap();
    terminal.backend().buffer().clone()
}
fn point_color(frame: &Buffer, point: (i64, i64)) -> Color {
    frame[(point.0 as u16, point.1 as u16)].fg
}
#[test]
fn source_colors_crossings_convergence_and_monochrome() {
    for density in [Density::Standard, Density::Compact] {
        let state = crossing_fixture();
        let ids = current_ids(&state);
        let graph = Graph::with_density(&state, &ids, density);
        let frame = color_frame(&graph, &state, true);
        let sources: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| graph.nodes[&e.prerequisite].layer == 0)
            .collect();
        assert_eq!(point_color(&frame, sources[0].points[0]), Color::Cyan);
        assert_eq!(point_color(&frame, sources[1].points[0]), Color::Magenta);
        for cell in frame.content.iter().filter(|c| c.symbol() == "╳") {
            assert_eq!(cell.fg, Color::Reset);
        }
        let incoming: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| graph.nodes[&e.dependent].layer == 2)
            .collect();
        assert_eq!(incoming.len(), 2);
        assert_eq!(point_color(&frame, incoming[0].points[0]), Color::Cyan);
        assert_eq!(point_color(&frame, incoming[1].points[0]), Color::Magenta);
        let end = *incoming[0].points.last().unwrap();
        assert_eq!(point_color(&frame, (end.0 - 1, end.1)), Color::Reset);
        assert_eq!(incoming[0].points.last(), incoming[1].points.last());
        assert_eq!(
            point_color(&frame, *incoming[0].points.last().unwrap()),
            Color::Reset
        );
        let mut reordered = state.clone();
        for task in reordered.tasks.values_mut() {
            task.depends_on.reverse();
        }
        let mut reversed_ids = ids.clone();
        reversed_ids.reverse();
        let reordered_graph = Graph::with_density(&reordered, &reversed_ids, density);
        assert_eq!(graph, reordered_graph);
        assert_eq!(frame, color_frame(&reordered_graph, &reordered, true));
        let mut reversed_edges = graph.clone();
        reversed_edges.edges.reverse();
        assert_eq!(frame, color_frame(&reversed_edges, &state, true));
        assert_eq!(frame, color_frame(&graph, &state, true));
        let mono = color_frame(&graph, &state, false);
        assert!(mono.content.iter().all(|c| c.fg == Color::Reset));
        for (colored, plain) in frame.content.iter().zip(&mono.content) {
            assert_eq!(colored.symbol(), plain.symbol());
            assert_eq!(colored.modifier, plain.modifier);
        }
        for node in graph.nodes.values() {
            assert_eq!(
                point_color(&frame, (node.x, node.y)),
                task_style(&state, &state.tasks[&node.id], true)
                    .fg
                    .unwrap_or(Color::Reset)
            );
        }
    }
}
#[test]
fn source_colors_fanout_and_crowded_layer_cycle_by_full_id() {
    for density in [Density::Standard, Density::Compact] {
        let mut state = State::empty();
        for i in 0..9 {
            let mut source = Task::new(format!("source {i}"), NOW);
            source.id = format!("t-{i:020x}");
            // Layout order differs from full-ID order; palette must not follow priority.
            source.priority = if i % 2 == 0 { "P1" } else { "P2" }.into();
            for branch in 0..2 {
                let mut target = Task::new(format!("target {i}/{branch}"), NOW);
                target.id = format!("t-{:020x}", 100 + i * 2 + branch);
                target.depends_on = vec![source.id.clone()];
                state.tasks.insert(target.id.clone(), target);
            }
            state.tasks.insert(source.id.clone(), source);
        }
        let graph = Graph::with_density(&state, &current_ids(&state), density);
        let frame = color_frame(&graph, &state, true);
        let palette = [
            Color::Cyan,
            Color::Magenta,
            Color::Yellow,
            Color::Blue,
            Color::Green,
            Color::Red,
        ];
        for edge in &graph.edges {
            let index = usize::from_str_radix(&edge.prerequisite[2..], 16).unwrap();
            let expected = palette[index % palette.len()];
            assert_eq!(point_color(&frame, edge.points[0]), expected);
            assert_eq!(point_color(&frame, *edge.points.last().unwrap()), expected);
        }
    }
}
