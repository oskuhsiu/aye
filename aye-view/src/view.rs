use crate::{
    app::{App, Pane},
    model::{sanitize, status},
};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Text},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};

pub fn render(frame: &mut Frame, app: &mut App) {
    app.tick_clock();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(frame.area());
    frame.render_widget(
        Paragraph::new(format!(
            "aye-view · {} · {} visible tasks{}{} · read-only",
            if app.history.is_some() {
                "History"
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
    let main = chunks[1];
    if main.width >= 90 {
        let panes = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
            .split(main);
        render_main(frame, app, panes[0]);
        render_detail(frame, app, panes[1]);
    } else if app.pane == Pane::Detail {
        render_detail(frame, app, main);
    } else {
        render_main(frame, app, main);
    }
    frame.render_widget(
        Paragraph::new(
            "/ Search · f Filter · c Recent · h History · ] Recent task · Enter Detail · Esc Back · ? Help · q Quit",
        ),
        chunks[2],
    );
    if app.help {
        render_help(frame);
    }
    crate::query::render(frame, app);
}
fn block(title: &str, focused: bool) -> Block<'_> {
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(if focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        })
}
fn render_main(frame: &mut Frame, app: &mut App, area: Rect) {
    if app.history.is_some() {
        crate::history::render_history(frame, app, area);
    } else {
        let current = crate::history::render_recent(frame, app, area);
        render_list(frame, app, current);
    }
}
fn render_list(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = block("List", app.pane == Pane::Main);
    let ids = app.graph_ids();
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
    let items = ids
        .iter()
        .map(|id| {
            let t = &app.snapshot.state.tasks[id];
            let (symbol, label) = status(&app.snapshot.state, t);
            let title = sanitize(&t.title).replace('\n', " ");
            ListItem::new(format!("{symbol} {} {title} [{label}]", t.priority)).style(match label {
                "ready" => Style::default().fg(Color::Green),
                "in_progress" => Style::default().fg(Color::Cyan),
                "blocked" => Style::default().fg(Color::Red),
                "deferred" => Style::default().fg(Color::Yellow),
                _ => Style::default().add_modifier(Modifier::DIM),
            })
        })
        .collect::<Vec<_>>();
    let selected = app
        .selected_id
        .as_ref()
        .and_then(|id| ids.iter().position(|v| v == id));
    let mut state = ListState::default()
        .with_offset(app.list_offset)
        .with_selected(selected);
    frame.render_stateful_widget(
        List::new(items)
            .block(block)
            .highlight_symbol("> ")
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED)),
        area,
        &mut state,
    );
    app.list_offset = state.offset();
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
        "Detail · Enter/Tab focus · PgUp/PgDown scroll",
        app.pane == Pane::Detail,
    );
    let inner = block.inner(area);
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
fn render_help(frame: &mut Frame) {
    let area = frame.area();
    frame.render_widget(Clear, area);
    frame.render_widget(Paragraph::new("j/k or ↑/↓: select task; scroll when Detail focused\nEnter / l / →: focus Detail     Esc / ← / Ctrl-h: return\nTab: switch pane               PgUp/PgDown: scroll Detail\n?: toggle Help                 q / Ctrl-c: quit\n/ Search all tasks; ↑/↓ results, Enter reveal, Esc cancel\nf Filter: ↑/↓ field, ←/→ cycle, c clear draft, Enter apply\nSearch reveal ends on leaving the task or applying filters.\nc: Recent 24h toggle; ]: next task in the secondary region\nh: all closed History; Esc: return (from Detail, press twice)\nHistory respects filters; arrows/PgUp/PgDown load more rows.\n\n● ready   ▶ in progress   ! blocked   ⏸ deferred\n✓ completed   × cancelled (does not satisfy a dependency)\n\nDependency direction B → A means A depends on B.\nB completion unlocks A. Parent/discovery are detail context.\n\nCurrent List includes closed prerequisite ancestors.\nThis foundation opens in List; further modes arrive separately.")
        .block(block("Help · ? or Esc to return",true)).wrap(ratatui::widgets::Wrap {trim:false}),area);
}
