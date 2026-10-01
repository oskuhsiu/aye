use crate::{
    app::{App, Mode, Pane},
    graph,
    model::{sanitize, status},
    pointer::{Surface, Target},
};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Text},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};

pub fn render(frame: &mut Frame, app: &mut App) {
    if app.rendered_area.is_some_and(|area| area != frame.area()) {
        app.retain_visible_selection();
        app.invalidate_pointer();
    }
    app.rendered_area = Some(frame.area());
    app.tick_clock();
    app.pointer_hits.clear();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(if frame.area().width < 70 { 2 } else { 1 }),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(frame.area());
    frame.render_widget(
        Paragraph::new(format!(
            "aye-view · {} · {} visible tasks{}{} · read-only",
            if app.history.is_some() {
                "History"
            } else if app.focus_root.is_some() {
                if app.mode == Mode::Graph {
                    "Focus Graph"
                } else {
                    "Focus List"
                }
            } else if app.mode == Mode::Graph {
                "Graph"
            } else {
                "List"
            },
            app.visible_ids.len(),
            if app.query.filters.active() {
                " · filtered"
            } else {
                ""
            },
            if app.query.revealed_id.is_some() {
                " · search reveal"
            } else {
                ""
            }
        )),
        chunks[0],
    );
    frame.render_widget(
        Paragraph::new(status_summary(app)).wrap(ratatui::widgets::Wrap { trim: false }),
        chunks[1],
    );
    let main = chunks[2];
    if main.width >= 90 {
        let main_percent = if app.mode == Mode::Graph && app.history.is_none() {
            65
        } else {
            45
        };
        let panes = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(main_percent),
                Constraint::Percentage(100 - main_percent),
            ])
            .split(main);
        render_main(frame, app, panes[0]);
        render_detail(frame, app, panes[1]);
    } else if app.pane == Pane::Detail {
        render_detail(frame, app, main);
    } else {
        render_main(frame, app, main);
    }
    frame.render_widget(
        Paragraph::new(if frame.area().width<60 {
            "? Help · q Quit · Esc Back"
        } else if frame.area().width<110 {
            "? Help · q Quit · Esc Back · Tab Graph/List · F Focus"
        } else {
            "? Help · q Quit · F Focus · g Current · Tab Graph/List · / Search · f Filter · c Recent · h History · r Refresh"
        }),
        chunks[3],
    );
    if let Some(error) = &app.refresh_error {
        frame.render_widget(Clear, chunks[3]);
        frame.render_widget(
            Paragraph::new(format!(
                "! Showing last good state (r retry): {}",
                sanitize(error).replace('\n', " ")
            ))
            .style(if graph::colors_enabled() {
                Style::default().fg(Color::Red)
            } else {
                Style::default()
            }),
            chunks[3],
        );
    }
    if app.help {
        app.pointer_hits.clear();
        render_help(frame, app);
    }
    crate::query::render(frame, app);
    if app.query.modal.is_some() {
        app.pointer_hits.clear();
    }
    app.reveal_selection = false;
}
fn block(title: &str, focused: bool) -> Block<'_> {
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(if focused && graph::colors_enabled() {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        })
}
fn render_main(frame: &mut Frame, app: &mut App, area: Rect) {
    if app.history.is_some() {
        crate::history::render_history(frame, app, area);
        navigation_control(frame, app, area, Target::Details);
        return;
    }
    if app.mode == Mode::Graph {
        app.ensure_graph();
    }
    let area = crate::history::render_recent(frame, app, area);
    if app.mode == Mode::List {
        render_list(frame, app, area);
        navigation_control(frame, app, area, Target::Details);
        return;
    }
    let title = if let Some(root) = &app.focus_root {
        format!(
            "Focus · {} · g/Esc Current",
            sanitize(&app.snapshot.state.tasks[root].title).replace('\n', " ")
        )
    } else if app.visible_ids.len() > 500 {
        "Current Graph · Large graph; Tab for List".into()
    } else {
        "Current Graph · prerequisite → dependent".into()
    };
    let title = format!("{} · {title}", app.density.label());
    let block = block(&title, app.pane == Pane::Main);
    let inner = block.inner(area);
    if app.graph_drag.is_some_and(|drag| drag.area != inner) {
        app.graph_drag = None;
    }
    frame.render_widget(block, area);
    app.pointer_hits.add(inner, Target::Surface(Surface::Graph));
    navigation_control(frame, app, area, Target::Details);
    if app.graph.nodes.is_empty() {
        frame.render_widget(
            Paragraph::new(if app.query.filters.active() {
                "No tasks match filters. f opens filters; c clears, Enter applies."
            } else {
                "No current tasks. Create work with aye create <title>."
            })
            .wrap(ratatui::widgets::Wrap { trim: false }),
            inner,
        );
        return;
    }
    if app.graph_anchor != app.selected_id || app.graph_size != (inner.width, inner.height) {
        if let Some(id) = &app.selected_id {
            app.graph
                .reveal(id, &mut app.graph_viewport, inner.width, inner.height);
        }
        app.graph_anchor = app.selected_id.clone();
        app.graph_size = (inner.width, inner.height);
    }
    // Clip in wide world coordinates before converting to terminal cells.
    for node in app.graph.nodes.values() {
        let x = node.x - app.graph_viewport.x;
        let y = node.y - app.graph_viewport.y;
        let left = x.max(0).min(i64::from(inner.width));
        let top = y.max(0).min(i64::from(inner.height));
        let right = (x + app.density.node_width()).clamp(0, i64::from(inner.width));
        let bottom = (y + app.density.node_height()).clamp(0, i64::from(inner.height));
        if right > left && bottom > top {
            app.pointer_hits.add(
                Rect::new(
                    inner.x + left as u16,
                    inner.y + top as u16,
                    (right - left) as u16,
                    (bottom - top) as u16,
                ),
                Target::Task(node.id.clone()),
            );
        }
    }
    graph::draw(
        frame,
        inner,
        &app.graph,
        &app.snapshot.state,
        app.selected_id.as_deref(),
        app.graph_viewport,
        graph::colors_enabled(),
    );
}
fn render_list(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = block("List", app.pane == Pane::Main);
    let ids = app.graph_ids();
    let inner = block.inner(area);
    app.pointer_hits.add(inner, Target::Surface(Surface::List));
    if ids.is_empty() {
        frame.render_widget(
            Paragraph::new(if app.query.filters.active() {
                "No tasks match filters. f opens filters; c clears, Enter applies."
            } else {
                "No current tasks. Create work with aye create <title>."
            })
            .block(block)
            .wrap(ratatui::widgets::Wrap { trim: false }),
            area,
        );
        return;
    }
    let selected = app
        .selected_id
        .as_ref()
        .and_then(|id| ids.iter().position(|v| v == id));
    let start = crate::pointer::row_start(
        app.list_offset,
        selected,
        usize::from(inner.height),
        ids.len(),
        app.reveal_selection,
    );
    let end = start
        .saturating_add(usize::from(inner.height))
        .min(ids.len());
    let items = ids[start..end]
        .iter()
        .map(|id| {
            let task = &app.snapshot.state.tasks[id];
            let (symbol, label) = status(&app.snapshot.state, task);
            let title = sanitize(&task.title).replace('\n', " ");
            ListItem::new(format!("{symbol} {} {title} [{label}]", task.priority)).style(
                graph::task_style(&app.snapshot.state, task, graph::colors_enabled()),
            )
        })
        .collect::<Vec<_>>();
    let mut state = ListState::default().with_selected(
        selected
            .and_then(|index| index.checked_sub(start))
            .filter(|index| *index < items.len()),
    );
    frame.render_widget(block, area);
    frame.render_stateful_widget(
        List::new(items)
            .highlight_symbol("> ")
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED)),
        inner,
        &mut state,
    );
    app.list_offset = start + state.offset();
    app.pointer_hits.rows(inner, &ids[app.list_offset..end]);
}
fn relation_lines(app: &App, ids: &[String]) -> String {
    if ids.is_empty() {
        return "—".into();
    }
    ids.iter()
        .map(|id| {
            let t = &app.snapshot.state.tasks[id];
            format!("{} ({})", t.title, id)
        })
        .collect::<Vec<_>>()
        .join("\n")
}
pub fn detail_text(app: &App) -> String {
    let Some(t) = app
        .selected_id
        .as_ref()
        .and_then(|id| app.snapshot.state.tasks.get(id))
    else {
        return "No task selected".into();
    };
    let (symbol, state) = status(&app.snapshot.state, t);
    let mut sections = vec![format!(
        "{}\n{}\n{symbol} {state} · {} · {}",
        t.title, t.id, t.priority, t.kind
    )];
    let mut add = |name: &str, value: String| sections.push(format!("{name}\n{value}"));
    add(
        "Claim",
        t.claim
            .as_ref()
            .map(|c| format!("{} · {}", c.actor, c.claimed_at))
            .unwrap_or_else(|| "—".into()),
    );
    add(
        "Manual block",
        t.manual_block
            .as_ref()
            .map(|b| format!("{}\n{} · {}", b.reason, b.actor, b.blocked_at))
            .unwrap_or_else(|| "—".into()),
    );
    add(
        "Blocked by",
        relation_lines(app, &app.snapshot.state.blocked_by(t)),
    );
    add(
        "Blocks",
        relation_lines(
            app,
            app.relations
                .blocks
                .get(&t.id)
                .map(Vec::as_slice)
                .unwrap_or(&[]),
        ),
    );
    add("Depends on", relation_lines(app, &t.depends_on));
    add("Description", t.description.clone());
    add(
        "Acceptance",
        t.acceptance
            .iter()
            .map(|v| format!("• {v}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    add(
        "Parent",
        relation_lines(app, &t.parent.iter().cloned().collect::<Vec<_>>()),
    );
    add(
        "Discovered from",
        relation_lines(app, &t.discovered_from.iter().cloned().collect::<Vec<_>>()),
    );
    add(
        "Children",
        relation_lines(
            app,
            app.relations
                .children
                .get(&t.id)
                .map(Vec::as_slice)
                .unwrap_or(&[]),
        ),
    );
    add(
        "Discovered tasks",
        relation_lines(
            app,
            app.relations
                .discoveries
                .get(&t.id)
                .map(Vec::as_slice)
                .unwrap_or(&[]),
        ),
    );
    add("Labels", t.labels.join(", "));
    add(
        "Notes",
        t.notes
            .iter()
            .map(|n| format!("{} · {}\n{}", n.actor, n.created_at, n.body))
            .collect::<Vec<_>>()
            .join("\n\n"),
    );
    add(
        "Timestamps",
        format!(
            "Created {}\nUpdated {}\nClosed {}",
            t.created_at,
            t.updated_at,
            t.closed_at.as_deref().unwrap_or("—")
        ),
    );
    sanitize(&sections.join("\n\n"))
}
fn render_detail(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = block(
        "Detail · Enter focus · PgUp/PgDown scroll",
        app.pane == Pane::Detail,
    );
    let inner = block.inner(area);
    app.pointer_hits
        .add(inner, Target::Surface(Surface::Detail));
    let text = detail_text(app);
    // Wrap ourselves to retain usize scrolling beyond Paragraph's u16 offset.
    // Ratatui Text spans supply terminal cell widths, never byte slicing.
    let lines = wrapped_lines(&text, usize::from(inner.width));
    app.detail_page = usize::from(inner.height).max(1).saturating_sub(1).max(1);
    app.detail_max_scroll = lines.len().saturating_sub(usize::from(inner.height));
    app.detail_scroll = app.detail_scroll.min(app.detail_max_scroll);
    let visible = lines
        .into_iter()
        .skip(app.detail_scroll)
        .take(usize::from(inner.height))
        .map(Line::raw)
        .collect::<Vec<_>>();
    frame.render_widget(block, area);
    frame.render_widget(Paragraph::new(Text::from(visible)), inner);
    navigation_control(frame, app, area, Target::Back);
}
fn navigation_control(frame: &mut Frame, app: &mut App, area: Rect, target: Target) {
    if app.selected_id.is_none() {
        return;
    }
    let label = if matches!(target, Target::Back) {
        "[Back]"
    } else {
        "[Details]"
    };
    let width = label.len() as u16;
    if area.width < width + 2 || area.height < 2 {
        return;
    }
    let control = Rect::new(area.right() - width - 1, area.y, width, 1);
    frame.render_widget(
        Paragraph::new(label).style(Style::default().add_modifier(Modifier::BOLD)),
        control,
    );
    app.pointer_hits.add(control, target);
}
fn wrapped_lines(text: &str, width: usize) -> Vec<String> {
    use ratatui::text::Span;
    use unicode_segmentation::UnicodeSegmentation;
    let width = width.max(1);
    let mut output = Vec::new();
    for line in text.split('\n') {
        let mut row = String::new();
        let mut used = 0;
        for c in line.graphemes(true) {
            let w = Span::raw(c).width();
            if used + w > width && !row.is_empty() {
                output.push(std::mem::take(&mut row));
                used = 0;
            }
            row.push_str(c);
            used += w;
        }
        output.push(row);
    }
    output
}
pub fn status_summary(app: &App) -> String {
    let mut counts = [0usize; 4];
    for task in app.snapshot.state.tasks.values() {
        match app.snapshot.state.effective(task) {
            "ready" => counts[0] += 1,
            "in_progress" => counts[1] += 1,
            "blocked" => counts[2] += 1,
            "deferred" => counts[3] += 1,
            _ => {}
        }
    }
    format!(
        "{}{} ready · {} working · {} blocked · {} deferred",
        if counts[0] == 0 && counts.iter().sum::<usize>() > 0 {
            "No ready tasks · "
        } else {
            ""
        },
        counts[0],
        counts[1],
        counts[2],
        counts[3]
    )
}
fn render_help(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    frame.render_widget(Clear, area);
    let block = block("Help · j/k PgUp/Dn Scroll · Esc Back", true);
    let inner = block.inner(area);
    app.pointer_hits.add(inner, Target::Surface(Surface::Help));
    let text = "Mouse: click Graph/List/Recent/History tasks\nClick a pane to activate keyboard controls\nWheel: scroll the pointed surface, keep selection\nGraph: horizontal or Shift-wheel pans sideways\nGraph background: left drag pans like a map\nDrag ends on release; navigation/resize cancels\n[Details] opens selected task; [Back] returns\nMouse reporting needs terminal support/settings.\nCopy: use your terminal reporting override\n(iTerm2: hold Option). Keyboard keys still work.\n\nGraph: Left / Ctrl-h = prerequisite\nGraph: Right / l = dependent\nGraph: Up/Down or k/j = same layer\nShift-arrows: pan without changing selection\nGraph Main: - Compact; +/= Standard; 0 reset\nZoom preserves selection and viewport context.\nEnter: Detail; Esc: return to main pane\nF: focus selected ancestors and descendants\ng: full Current Graph; Esc leaves focused Main\nFocus clears filters; later filters intersect.\nTab: Graph/List (History: pane switch)\nDetail: j/k or arrows scroll; PgUp/PgDown page\n?: Help; q / Ctrl-c: Quit\n\nr: Refresh local state (poll every 500 ms)\n/ Search all tasks; arrows select; Enter reveal\nSearch outside Focus exits that focus.\nf Filters: up/down field, left/right cycle\nc clears filter draft; Enter applies; Esc cancels\nc: Recent 24h; ]: next unrelated recent task\nRecent is hidden during Focus.\nh: all closed History; Esc returns\nHistory arrows/PgUp/PgDown load more rows.\n\n● ready    ▶ in progress    ! blocked\n⏸ deferred    ✓ done    × cancelled\nCancelled prerequisites do not unlock tasks.\n\nB → A means A depends on B.\nB completion unlocks A.\nParent/discovery are detail context only.\n╳: lines cross without joining.\n\nCurrent Graph includes closed prerequisites.\nFocus root stays fixed as selection moves.";
    let lines = wrapped_lines(text, usize::from(inner.width));
    app.help_page = usize::from(inner.height).saturating_sub(1).max(1);
    app.help_max_scroll = lines.len().saturating_sub(usize::from(inner.height));
    app.help_scroll = app.help_scroll.min(app.help_max_scroll);
    let visible = lines
        .into_iter()
        .skip(app.help_scroll)
        .take(usize::from(inner.height))
        .map(Line::raw)
        .collect::<Vec<_>>();
    frame.render_widget(block, area);
    frame.render_widget(Paragraph::new(Text::from(visible)), inner);
}
