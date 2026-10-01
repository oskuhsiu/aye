//! Frame-local hit geometry and session-only pointer navigation.
use crate::{
    app::{App, Pane},
    graph::Viewport,
};
use crossterm::event::{Event, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Position, Rect};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Surface {
    Graph,
    List,
    Detail,
    Recent,
    History,
    Help,
}
#[derive(Clone, Debug)]
pub(crate) enum Target {
    Surface(Surface),
    Task(String),
    Details,
    Back,
}
#[derive(Default)]
pub(crate) struct HitMap(Vec<(Rect, Target)>);
impl HitMap {
    pub(crate) fn clear(&mut self) {
        self.0.clear();
    }
    pub(crate) fn add(&mut self, area: Rect, target: Target) {
        if !area.is_empty() {
            self.0.push((area, target));
        }
    }
    pub(crate) fn rows(&mut self, area: Rect, ids: &[String]) {
        for (row, id) in ids.iter().take(usize::from(area.height)).enumerate() {
            self.add(
                Rect::new(area.x, area.y + row as u16, area.width, 1),
                Target::Task(id.clone()),
            );
        }
    }
    fn contains_task(&self, id: &str) -> bool {
        self.0
            .iter()
            .any(|(_, target)| matches!(target,Target::Task(task) if task==id))
    }
    fn at(&self, column: u16, row: u16) -> Option<(Target, Rect)> {
        self.0
            .iter()
            .rev()
            .find(|(area, _)| area.contains(Position::new(column, row)))
            .map(|(area, target)| (target.clone(), *area))
    }
    fn surface_at(&self, column: u16, row: u16) -> Option<(Surface, Rect)> {
        self.0.iter().rev().find_map(|(area, target)| match target {
            Target::Surface(surface) if area.contains(Position::new(column, row)) => {
                Some((*surface, *area))
            }
            _ => None,
        })
    }
}
#[derive(Clone, Copy)]
pub(crate) struct GraphDrag {
    pub(crate) area: Rect,
    column: u16,
    row: u16,
    viewport: Viewport,
}

/// Reveal deliberate selection once; a manually scrolled viewport stays independent.
pub(crate) fn row_start(
    offset: usize,
    selected: Option<usize>,
    rows: usize,
    len: usize,
    reveal: bool,
) -> usize {
    let rows = rows.max(1);
    let mut offset = offset.min(len.saturating_sub(rows));
    if reveal && let Some(index) = selected {
        if index < offset {
            offset = index;
        } else if index >= offset.saturating_add(rows) {
            offset = index.saturating_sub(rows - 1);
        }
    }
    offset.min(len.saturating_sub(rows))
}
impl App {
    pub(crate) fn retain_visible_selection(&mut self) {
        self.reveal_selection |= self
            .selected_id
            .as_ref()
            .is_some_and(|id| self.pointer_hits.contains_task(id));
    }
    pub(crate) fn invalidate_pointer(&mut self) {
        self.pointer_hits.clear();
        self.graph_drag = None;
    }
    /// Returns whether the event requires a new frame. Passive motion is inert.
    pub fn handle_event(&mut self, event: Event) -> bool {
        match event {
            Event::Key(key) => {
                if key.kind == crossterm::event::KeyEventKind::Release {
                    return false;
                }
                self.handle_key(key);
                true
            }
            Event::Mouse(mouse) => self.handle_mouse(mouse),
            Event::Resize(_, _) => {
                self.retain_visible_selection();
                self.invalidate_pointer();
                true
            }
            Event::FocusLost => {
                self.invalidate_pointer();
                true
            }
            _ => false,
        }
    }
    pub fn handle_mouse(&mut self, mouse: MouseEvent) -> bool {
        if self.query.modal.is_some() {
            self.graph_drag = None;
            return false;
        }
        if self.help || self.history.is_some() || self.mode != crate::app::Mode::Graph {
            self.graph_drag = None;
        }
        match mouse.kind {
            MouseEventKind::Up(_) => {
                self.graph_drag = None;
                return false;
            }
            MouseEventKind::Down(_) => self.graph_drag = None,
            MouseEventKind::Drag(_) => {
                let Some(origin) = self.graph_drag else {
                    return false;
                };
                let previous = self.graph_viewport;
                self.graph_viewport = Viewport {
                    x: origin
                        .viewport
                        .x
                        .saturating_sub(i64::from(mouse.column) - i64::from(origin.column)),
                    y: origin
                        .viewport
                        .y
                        .saturating_sub(i64::from(mouse.row) - i64::from(origin.row)),
                };
                self.clamp_graph_viewport();
                self.graph_anchor = self.selected_id.clone();
                let changed = previous != self.graph_viewport;
                if changed {
                    self.pointer_hits.clear();
                }
                return changed;
            }
            MouseEventKind::ScrollUp
            | MouseEventKind::ScrollDown
            | MouseEventKind::ScrollLeft
            | MouseEventKind::ScrollRight => {
                self.graph_drag = None;
                return self.wheel(mouse);
            }
            _ => return false,
        }
        if self.help || mouse.kind != MouseEventKind::Down(MouseButton::Left) {
            return false;
        }
        let Some((target, area)) = self.pointer_hits.at(mouse.column, mouse.row) else {
            return false;
        };
        match target {
            Target::Task(id) => {
                self.select(&id);
                self.pane = Pane::Main;
            }
            Target::Surface(Surface::Detail) | Target::Details => self.pane = Pane::Detail,
            Target::Surface(Surface::Graph) => {
                self.pane = Pane::Main;
                self.graph_drag = Some(GraphDrag {
                    area,
                    column: mouse.column,
                    row: mouse.row,
                    viewport: self.graph_viewport,
                });
            }
            Target::Surface(_) | Target::Back => self.pane = Pane::Main,
        }
        self.pointer_hits.clear();
        true
    }
    fn wheel(&mut self, mouse: MouseEvent) -> bool {
        let Some((surface, area)) = self.pointer_hits.surface_at(mouse.column, mouse.row) else {
            return false;
        };
        if self.help && surface != Surface::Help {
            return false;
        }
        let horizontal = matches!(
            mouse.kind,
            MouseEventKind::ScrollLeft | MouseEventKind::ScrollRight
        ) || (surface == Surface::Graph
            && mouse.modifiers.contains(KeyModifiers::SHIFT));
        if horizontal && surface != Surface::Graph {
            return false;
        }
        let delta = if matches!(
            mouse.kind,
            MouseEventKind::ScrollUp | MouseEventKind::ScrollLeft
        ) {
            -3
        } else {
            3
        };
        self.reveal_selection = false;
        let changed = match surface {
            Surface::Graph => {
                if horizontal {
                    self.pan_graph(delta as i64, 0)
                } else {
                    self.pan_graph(0, delta as i64)
                }
            }
            Surface::Detail => {
                let previous = self.detail_scroll;
                self.detail_scroll = self
                    .detail_scroll
                    .saturating_add_signed(delta)
                    .min(self.detail_max_scroll);
                previous != self.detail_scroll
            }
            Surface::Help => {
                let previous = self.help_scroll;
                self.help_scroll = self
                    .help_scroll
                    .saturating_add_signed(delta)
                    .min(self.help_max_scroll);
                previous != self.help_scroll
            }
            Surface::List => {
                let previous = self.list_offset;
                self.list_offset = self.list_offset.saturating_add_signed(delta).min(
                    self.graph_ids()
                        .len()
                        .saturating_sub(usize::from(area.height)),
                );
                previous != self.list_offset
            }
            Surface::Recent => {
                let previous = self.recent.offset;
                self.recent.offset = self.recent.offset.saturating_add_signed(delta).min(
                    self.recent_only_ids()
                        .len()
                        .saturating_sub(usize::from(area.height)),
                );
                previous != self.recent.offset
            }
            Surface::History => self.scroll_history(delta, usize::from(area.height)),
        };
        if changed {
            self.pointer_hits.clear();
        }
        changed
    }
}
