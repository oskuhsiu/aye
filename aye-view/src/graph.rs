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
            // Reserve one vertical track per adjacent edge, and separate departure
            // and arrival tracks for long edges. Sharing a gap never shares a track.
            let mut track_counts: BTreeMap<usize, usize> = BTreeMap::new();
            let mut tracks = Vec::with_capacity(edges.len());
            for (from, to) in &edges {
                let count = track_counts.entry(ranks[from]).or_default();
                let departure = *count;
                *count += 1;
                let arrival = if ranks[to] > ranks[from] + 1 {
                    let count = track_counts.entry(ranks[to] - 1).or_default();
                    let slot = *count;
                    *count += 1;
                    Some(slot)
                } else {
                    None
                };
                tracks.push((departure, arrival));
            }
            let last_layer = members.iter().map(|id| ranks[id]).max().unwrap_or(0);
            let mut columns = vec![0; last_layer + 1];
            for layer in 0..last_layer {
                let gap = (track_counts.get(&layer).copied().unwrap_or(0) as i64 * 2 + 3).max(8);
                columns[layer + 1] = columns[layer] + NODE_WIDTH + gap;
            }
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
                        x: columns[layer],
                        y: node_y + *row * ROW_STEP,
                    },
                );
                *row += 1;
            }
            let mut long_index = 0;
            for ((from, to), (departure, arrival)) in edges.into_iter().zip(tracks) {
                let a = &graph.nodes[from];
                let b = &graph.nodes[to];
                let start = (a.x + NODE_WIDTH, a.y + 1);
                let end = (b.x - 1, b.y + 2);
                let points = if b.layer == a.layer + 1 {
                    let mid = a.x + NODE_WIDTH + 1 + departure as i64 * 2;
                    vec![start, (mid, start.1), (mid, end.1), end]
                } else {
                    let track = base_y + long_index * 2;
                    long_index += 1;
                    let left = a.x + NODE_WIDTH + 1 + departure as i64 * 2;
                    let right = columns[b.layer - 1]
                        + NODE_WIDTH
                        + 1
                        + arrival.expect("long edge arrival track") as i64 * 2;
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
    canvas.edges(graph, colors);
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
        );
        canvas.line(
            (node.x + 1, node.y + NODE_HEIGHT - 1),
            (node.x + NODE_WIDTH - 2, node.y + NODE_HEIGHT - 1),
            style,
        );
        canvas.line(
            (node.x, node.y + 1),
            (node.x, node.y + NODE_HEIGHT - 2),
            style,
        );
        canvas.line(
            (node.x + NODE_WIDTH - 1, node.y + 1),
            (node.x + NODE_WIDTH - 1, node.y + NODE_HEIGHT - 2),
            style,
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
// ANSI colors follow the terminal theme. Palette position is scoped to the
// visible layer and sorted by full source ID, independent of routing order.
const PATH_PALETTE: [Color; 6] = [
    Color::Cyan,
    Color::Magenta,
    Color::Yellow,
    Color::Blue,
    Color::Green,
    Color::Red,
];
fn source_styles(graph: &Graph, colors: bool) -> BTreeMap<&str, Style> {
    if !colors {
        return BTreeMap::new();
    }
    let sources: BTreeSet<_> = graph
        .edges
        .iter()
        .map(|edge| {
            (
                graph.nodes[&edge.prerequisite].layer,
                edge.prerequisite.as_str(),
            )
        })
        .collect();
    let mut counts = BTreeMap::<usize, usize>::new();
    sources
        .into_iter()
        .map(|(layer, id)| {
            let index = counts.entry(layer).or_default();
            let style = Style::default().fg(PATH_PALETTE[*index % PATH_PALETTE.len()]);
            *index += 1;
            (id, style)
        })
        .collect()
}
const NORTH: u8 = 1;
const EAST: u8 = 2;
const SOUTH: u8 = 4;
const WEST: u8 = 8;
#[derive(Clone, Copy, Default)]
struct Stroke {
    directions: u8,
    first_edge: Option<usize>,
    same_source: bool,
    same_target: bool,
}
impl Stroke {
    fn style(self, graph: &Graph, styles: &BTreeMap<&str, Style>) -> Style {
        if self.same_source {
            self.first_edge
                .and_then(|index| styles.get(graph.edges[index].prerequisite.as_str()))
                .copied()
                .unwrap_or_default()
        } else {
            Style::default()
        }
    }
    fn symbol(self) -> &'static str {
        if !self.same_source && !self.same_target {
            return "╳";
        }
        match self.directions {
            1 | 4 | 5 => "│",
            2 | 8 | 10 => "─",
            3 => "└",
            6 => "┌",
            9 => "┘",
            12 => "┐",
            7 => "├",
            11 => "┴",
            13 => "┤",
            14 => "┬",
            15 => "┼",
            _ => " ",
        }
    }
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
    fn line(&mut self, a: (i64, i64), b: (i64, i64), style: Style) {
        if a.1 == b.1 {
            let lo = a.0.min(b.0).max(self.viewport.x);
            let hi =
                a.0.max(b.0)
                    .min(self.viewport.x + i64::from(self.area.width) - 1);
            for x in lo..=hi {
                self.cell(x, a.1, "─", style);
            }
        } else {
            let lo = a.1.min(b.1).max(self.viewport.y);
            let hi =
                a.1.max(b.1)
                    .min(self.viewport.y + i64::from(self.area.height) - 1);
            for y in lo..=hi {
                self.cell(a.0, y, "│", style);
            }
        }
    }
    fn edges(&mut self, graph: &Graph, colors: bool) {
        let styles = source_styles(graph, colors);
        // Only visible cells carry routing metadata, regardless of world dimensions.
        let mut strokes =
            vec![Stroke::default(); usize::from(self.area.width) * usize::from(self.area.height)];
        for (index, edge) in graph.edges.iter().enumerate() {
            for segment in edge.points.windows(2) {
                let (a, b) = (segment[0], segment[1]);
                if a.1 == b.1 {
                    if a.1 < self.viewport.y || a.1 >= self.viewport.y + i64::from(self.area.height)
                    {
                        continue;
                    }
                    let low = a.0.min(b.0);
                    let high = a.0.max(b.0);
                    for x in low.max(self.viewport.x)
                        ..=high.min(self.viewport.x + i64::from(self.area.width) - 1)
                    {
                        let directions =
                            if x > low { WEST } else { 0 } | if x < high { EAST } else { 0 };
                        self.record(&mut strokes, graph, index, (x, a.1), directions);
                    }
                } else {
                    if a.0 < self.viewport.x || a.0 >= self.viewport.x + i64::from(self.area.width)
                    {
                        continue;
                    }
                    let low = a.1.min(b.1);
                    let high = a.1.max(b.1);
                    for y in low.max(self.viewport.y)
                        ..=high.min(self.viewport.y + i64::from(self.area.height) - 1)
                    {
                        let directions =
                            if y > low { NORTH } else { 0 } | if y < high { SOUTH } else { 0 };
                        self.record(&mut strokes, graph, index, (a.0, y), directions);
                    }
                }
            }
        }
        for y in 0..self.area.height {
            for x in 0..self.area.width {
                let stroke =
                    strokes[usize::from(y) * usize::from(self.area.width) + usize::from(x)];
                if stroke.directions != 0 {
                    self.cell(
                        self.viewport.x + i64::from(x),
                        self.viewport.y + i64::from(y),
                        stroke.symbol(),
                        stroke.style(graph, &styles),
                    );
                }
            }
        }
        for edge in &graph.edges {
            if let Some(&(x, y)) = edge.points.last()
                && let Some((sx, sy)) = self.position(x, y)
            {
                let stroke = strokes[usize::from(sy - self.area.y) * usize::from(self.area.width)
                    + usize::from(sx - self.area.x)];
                self.cell(x, y, "→", stroke.style(graph, &styles));
            }
        }
    }
    fn record(
        &self,
        strokes: &mut [Stroke],
        graph: &Graph,
        index: usize,
        point: (i64, i64),
        directions: u8,
    ) {
        if let Some((x, y)) = self.position(point.0, point.1) {
            let cell = &mut strokes[usize::from(y - self.area.y) * usize::from(self.area.width)
                + usize::from(x - self.area.x)];
            cell.directions |= directions;
            if let Some(first) = cell.first_edge {
                cell.same_source &=
                    graph.edges[first].prerequisite == graph.edges[index].prerequisite;
                cell.same_target &= graph.edges[first].dependent == graph.edges[index].dependent;
            } else {
                cell.first_edge = Some(index);
                cell.same_source = true;
                cell.same_target = true;
            }
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
