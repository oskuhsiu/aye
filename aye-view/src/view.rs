use crate::{
    app::{App, Mode, Pane},
    graph,
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
            "aye-view · {} · {} current tasks · read-only",
            if app.mode == Mode::Graph {
                "Graph"
            } else {
                "List"
            },
            app.visible_ids.len()
        )),
        chunks[0],
    );
    let main = chunks[1];
    if main.width >= 90 {
        let main_percent = if app.mode == Mode::Graph { 65 } else { 45 };
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
        Paragraph::new("Tab Graph/List · j/k ↑↓ Move · Enter Detail · Esc Back · ? Help · q Quit"),
        chunks[2],
    );
    if app.help {
        render_help(frame);
    }
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
    if app.mode == Mode::List {
        render_list(frame, app, area);
        return;
    }
    app.ensure_graph();
    let title = if app.visible_ids.len() > 500 {
        "Current Graph · Large graph; Tab for List"
    } else {
        "Current Graph · prerequisite → dependent"
    };
    let block = block(title, app.pane == Pane::Main);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if app.visible_ids.is_empty() {
        frame.render_widget(
            Paragraph::new("No current tasks. Create work with aye create <title>.")
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
    if app.visible_ids.is_empty() {
        frame.render_widget(
            Paragraph::new("No current tasks. Create work with aye create <title>.")
                .block(block)
                .wrap(ratatui::widgets::Wrap { trim: false }),
            area,
        );
        return;
    }
    let items = app
        .visible_ids
        .iter()
        .map(|id| {
            let t = &app.snapshot.state.tasks[id];
            let (symbol, label) = status(&app.snapshot.state, t);
            let title = sanitize(&t.title).replace('\n', " ");
            ListItem::new(format!("{symbol} {} {title} [{label}]", t.priority)).style(
                graph::task_style(&app.snapshot.state, t, graph::colors_enabled()),
            )
        })
        .collect::<Vec<_>>();
    let selected = app
        .selected_id
        .as_ref()
        .and_then(|id| app.visible_ids.iter().position(|v| v == id));
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
        "Detail · Enter focus · PgUp/PgDown scroll",
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
    frame.render_widget(Paragraph::new("j/k or ↑/↓: select task; scroll when Detail focused\nEnter / l / →: focus Detail     Esc / ← / Ctrl-h: return\nTab: Graph/List               PgUp/PgDown: scroll Detail\n?: toggle Help                 q / Ctrl-c: quit\n\n● ready   ▶ in progress   ! blocked   ⏸ deferred\n✓ completed   × cancelled (does not satisfy a dependency)\n\nDependency direction B → A means A depends on B.\nB completion unlocks A. Parent/discovery are detail context.\n\nCurrent List includes closed prerequisite ancestors.\nGraph arrows point from prerequisite to dependent. Tab opens List.")
        .block(block("Help · ? or Esc to return",true)).wrap(ratatui::widgets::Wrap {trim:false}),area);
}
