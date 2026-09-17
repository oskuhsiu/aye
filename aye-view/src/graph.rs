//! Deterministic dependency layout and terminal-clipped drawing.
use crate::model::{sanitize, status};
use aye::model::{State, Task};
use ratatui::{
    Frame,
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Span,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use unicode_segmentation::UnicodeSegmentation;

pub const NODE_WIDTH: i64 = 28;
pub const NODE_HEIGHT: i64 = 4;
const COLUMN_STEP: i64 = 36;
const ROW_STEP: i64 = 6;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    pub id: String,
    pub layer: usize,
    pub x: i64,
    pub y: i64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edge {
    pub prerequisite: String,
    pub dependent: String,
    pub points: Vec<(i64, i64)>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Graph {
    pub nodes: BTreeMap<String, Node>,
    pub edges: Vec<Edge>,
    pub width: i64,
    pub height: i64,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Viewport {
    pub x: i64,
    pub y: i64,
}
impl Graph {
    pub fn new(state: &State, visible_ids: &[String]) -> Self {
        let included: BTreeSet<_> = visible_ids
            .iter()
            .filter(|id| state.tasks.contains_key(*id))
            .cloned()
            .collect();
        let mut ids: Vec<_> = included.iter().cloned().collect();
        ids.sort_by(|a, b| {
            let a = &state.tasks[a];
            let b = &state.tasks[b];
            (&a.priority, &a.created_at, &a.id).cmp(&(&b.priority, &b.created_at, &b.id))
        });
        let mut children: BTreeMap<String, Vec<String>> =
            ids.iter().map(|id| (id.clone(), vec![])).collect();
        let mut neighbors = children.clone();
        let mut degree: BTreeMap<_, usize> = ids.iter().map(|id| (id.clone(), 0)).collect();
        let mut pairs = Vec::new();
        for id in &ids {
            for prerequisite in &state.tasks[id].depends_on {
                if included.contains(prerequisite) {
                    pairs.push((prerequisite.clone(), id.clone()));
                    children.get_mut(prerequisite).unwrap().push(id.clone());
                    neighbors.get_mut(prerequisite).unwrap().push(id.clone());
                    neighbors.get_mut(id).unwrap().push(prerequisite.clone());
                    *degree.get_mut(id).unwrap() += 1;
                }
            }
        }
        pairs.sort();
        let mut ranks: BTreeMap<String, usize> = ids.iter().map(|id| (id.clone(), 0)).collect();
        let mut ready: VecDeque<_> = ids.iter().filter(|id| degree[*id] == 0).cloned().collect();
        while let Some(id) = ready.pop_front() {
            let next = ranks[&id] + 1;
            for child in &children[&id] {
                let rank = ranks.get_mut(child).unwrap();
                *rank = (*rank).max(next);
                let count = degree.get_mut(child).unwrap();
                *count -= 1;
                if *count == 0 {
                    ready.push_back(child.clone());
                }
            }
        }
        let mut graph = Self::default();
        let mut visited = BTreeSet::new();
        let mut base_y = 0;
        for root in &ids {
            if visited.contains(root) {
                continue;
            }
            let mut pending = vec![root.clone()];
            let mut members = BTreeSet::new();
            while let Some(id) = pending.pop() {
                if visited.insert(id.clone()) {
                    members.insert(id.clone());
                    pending.extend(neighbors[&id].iter().cloned());
                }
            }
            let edges: Vec<_> = pairs
                .iter()
                .filter(|(from, _)| members.contains(from))
                .collect();
            let long_count = edges
                .iter()
                .filter(|(from, to)| ranks[to] > ranks[from] + 1)
                .count() as i64;
            let node_y = base_y + long_count * 2;
            let mut rows: BTreeMap<usize, i64> = BTreeMap::new();
            for id in ids.iter().filter(|id| members.contains(*id)) {
                let layer = ranks[id];
                let row = rows.entry(layer).or_default();
                graph.nodes.insert(
                    id.clone(),
                    Node {
                        id: id.clone(),
                        layer,
                        x: layer as i64 * COLUMN_STEP,
                        y: node_y + *row * ROW_STEP,
                    },
                );
                *row += 1;
            }
            let mut long_index = 0;
            for (from, to) in edges {
                let a = &graph.nodes[from];
                let b = &graph.nodes[to];
                let start = (a.x + NODE_WIDTH, a.y + 2);
                let end = (b.x - 1, b.y + 2);
                let points = if b.layer == a.layer + 1 {
                    let mid = a.x + NODE_WIDTH + 3;
                    vec![start, (mid, start.1), (mid, end.1), end]
                } else {
                    let track = base_y + long_index * 2;
                    long_index += 1;
                    let left = a.x + NODE_WIDTH + 2;
                    let right = b.x - 3;
                    vec![
                        start,
                        (left, start.1),
                        (left, track),
                        (right, track),
                        (right, end.1),
                        end,
                    ]
                };
                graph.edges.push(Edge {
                    prerequisite: from.clone(),
                    dependent: to.clone(),
                    points,
                });
            }
            base_y = node_y + rows.values().max().copied().unwrap_or(0) * ROW_STEP + 2;
        }
        graph.width = graph
            .nodes
            .values()
            .map(|n| n.x + NODE_WIDTH)
            .max()
            .unwrap_or(0);
        graph.height = graph
            .nodes
            .values()
            .map(|n| n.y + NODE_HEIGHT)
            .max()
            .unwrap_or(0);
        graph
    }
    pub fn reveal(&self, id: &str, viewport: &mut Viewport, width: u16, height: u16) {
        if let Some(n) = self.nodes.get(id) {
            if n.x < viewport.x {
                viewport.x = n.x;
            } else if n.x + NODE_WIDTH > viewport.x + i64::from(width) {
                viewport.x = (n.x + NODE_WIDTH - i64::from(width)).min(n.x).max(0);
            }
            if n.y < viewport.y {
                viewport.y = n.y;
            } else if n.y + NODE_HEIGHT > viewport.y + i64::from(height) {
                viewport.y = (n.y + NODE_HEIGHT - i64::from(height)).min(n.y).max(0);
            }
        }
    }
}
pub fn colors_enabled() -> bool {
    std::env::var_os("NO_COLOR").is_none_or(|v| v.is_empty())
}
pub fn task_style(state: &State, task: &Task, colors: bool) -> Style {
    let label = status(state, task).1;
    let style = if colors {
        match label {
            "ready" => Style::default().fg(Color::Green),
            "in_progress" => Style::default().fg(Color::Cyan),
            "blocked" => Style::default().fg(Color::Red),
            "deferred" => Style::default().fg(Color::Yellow),
            _ => Style::default(),
        }
    } else {
        Style::default()
    };
    if task.status == "closed" {
        style.add_modifier(Modifier::DIM)
    } else {
        style
    }
}
/// Draw directly into the terminal-sized buffer. No buffer is sized from world coordinates.
pub fn draw(
    frame: &mut Frame,
    area: Rect,
    graph: &Graph,
    state: &State,
    selected: Option<&str>,
    viewport: Viewport,
    colors: bool,
) {
    let mut canvas = Canvas {
        buffer: frame.buffer_mut(),
        area,
        viewport,
    };
    for edge in &graph.edges {
        for segment in edge.points.windows(2) {
            canvas.line(segment[0], segment[1], Style::default(), true);
        }
        if let Some(&(x, y)) = edge.points.last() {
            canvas.cell(x, y, "→", Style::default());
        }
    }
    for node in graph.nodes.values() {
        if node.x + NODE_WIDTH <= viewport.x
            || node.x >= viewport.x + i64::from(area.width)
            || node.y + NODE_HEIGHT <= viewport.y
            || node.y >= viewport.y + i64::from(area.height)
        {
            continue;
        }
        let task = &state.tasks[&node.id];
        let chosen = selected == Some(node.id.as_str());
        let mut style = task_style(state, task, colors);
        if chosen {
            style = style.add_modifier(Modifier::BOLD | Modifier::REVERSED);
        }
        for dy in 0..NODE_HEIGHT {
            for dx in 0..NODE_WIDTH {
                canvas.cell(node.x + dx, node.y + dy, " ", style);
            }
        }
        canvas.line(
            (node.x + 1, node.y),
            (node.x + NODE_WIDTH - 2, node.y),
            style,
            false,
        );
        canvas.line(
            (node.x + 1, node.y + NODE_HEIGHT - 1),
            (node.x + NODE_WIDTH - 2, node.y + NODE_HEIGHT - 1),
            style,
            false,
        );
        canvas.line(
            (node.x, node.y + 1),
            (node.x, node.y + NODE_HEIGHT - 2),
            style,
            false,
        );
        canvas.line(
            (node.x + NODE_WIDTH - 1, node.y + 1),
            (node.x + NODE_WIDTH - 1, node.y + NODE_HEIGHT - 2),
            style,
            false,
        );
        for (dx, dy, symbol) in [
            (0, 0, "┌"),
            (NODE_WIDTH - 1, 0, "┐"),
            (0, NODE_HEIGHT - 1, "└"),
            (NODE_WIDTH - 1, NODE_HEIGHT - 1, "┘"),
        ] {
            canvas.cell(node.x + dx, node.y + dy, symbol, style);
        }
        let (symbol, label) = status(state, task);
        let title = format!(
            "{symbol} {} {}",
            task.priority,
            sanitize(&task.title).replace('\n', " ")
        );
        canvas.text(
            node.x + 1,
            node.y + 1,
            &truncate(&title, (NODE_WIDTH - 2) as usize),
            style,
        );
        let subtitle = task
            .claim
            .as_ref()
            .map(|c| format!("{label} · {}", sanitize(&c.actor).replace('\n', " ")))
            .unwrap_or_else(|| label.into());
        canvas.text(
            node.x + 1,
            node.y + 2,
            &truncate(&subtitle, (NODE_WIDTH - 2) as usize),
            style,
        );
    }
}
fn truncate(text: &str, width: usize) -> String {
    if Span::raw(text).width() <= width {
        return text.into();
    }
    let mut result = String::new();
    let mut used = 0;
    for g in text.graphemes(true) {
        let w = Span::raw(g).width();
        if used + w >= width {
            break;
        }
        result.push_str(g);
        used += w;
    }
    result.push('…');
    result
}
struct Canvas<'a> {
    buffer: &'a mut Buffer,
    area: Rect,
    viewport: Viewport,
}
impl Canvas<'_> {
    fn position(&self, x: i64, y: i64) -> Option<(u16, u16)> {
        let x = x - self.viewport.x;
        let y = y - self.viewport.y;
        if x < 0 || y < 0 || x >= i64::from(self.area.width) || y >= i64::from(self.area.height) {
            None
        } else {
            Some((self.area.x + x as u16, self.area.y + y as u16))
        }
    }
    fn cell(&mut self, x: i64, y: i64, symbol: &str, style: Style) {
        if let Some(p) = self.position(x, y) {
            self.buffer[p].set_symbol(symbol).set_style(style);
        }
    }
    fn line(&mut self, a: (i64, i64), b: (i64, i64), style: Style, join: bool) {
        if a.1 == b.1 {
            let lo = a.0.min(b.0).max(self.viewport.x);
            let hi =
                a.0.max(b.0)
                    .min(self.viewport.x + i64::from(self.area.width) - 1);
            for x in lo..=hi {
                self.stroke(x, a.1, "─", style, join);
            }
        } else {
            let lo = a.1.min(b.1).max(self.viewport.y);
            let hi =
                a.1.max(b.1)
                    .min(self.viewport.y + i64::from(self.area.height) - 1);
            for y in lo..=hi {
                self.stroke(a.0, y, "│", style, join);
            }
        }
    }
    fn stroke(&mut self, x: i64, y: i64, symbol: &str, style: Style, join: bool) {
        if let Some(p) = self.position(x, y) {
            let old = self.buffer[p].symbol();
            let symbol = if join
                && ((symbol == "─" && old == "│") || (symbol == "│" && old == "─") || old == "┼")
            {
                "┼"
            } else {
                symbol
            };
            self.buffer[p].set_symbol(symbol).set_style(style);
        }
    }
    fn text(&mut self, mut x: i64, y: i64, text: &str, style: Style) {
        for g in text.graphemes(true) {
            let width = Span::raw(g).width();
            if let Some((sx, sy)) = self.position(x, y)
                && x + width as i64 <= self.viewport.x + i64::from(self.area.width)
            {
                self.buffer.set_stringn(sx, sy, g, width, style);
            }
            x += width as i64;
        }
    }
}
#[cfg(test)]
mod tests;
