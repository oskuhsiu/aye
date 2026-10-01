//! Hit regions of the last rendered frame. All coordinates are terminal cells.
use crate::app::{App, Pane};
use crossterm::event::{Event, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Position, Rect};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Surface {
    Graph,
    List,
    Detail,
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
    fn at(&self, column: u16, row: u16) -> Option<Target> {
        self.0
            .iter()
            .rev()
            .find(|(area, _)| area.contains(Position::new(column, row)))
            .map(|(_, target)| target.clone())
    }
}
impl App {
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
                self.pointer_hits.clear();
                true
            }
            Event::FocusLost => {
                self.pointer_hits.clear();
                true
            }
            _ => false,
        }
    }
    pub fn handle_mouse(&mut self, mouse: MouseEvent) -> bool {
        if self.help
            || self.query.modal.is_some()
            || mouse.kind != MouseEventKind::Down(MouseButton::Left)
        {
            return false;
        }
        let Some(target) = self.pointer_hits.at(mouse.column, mouse.row) else {
            return false;
        };
        match target {
            Target::Task(id) => {
                self.select(&id);
                self.pane = Pane::Main;
            }
            Target::Surface(Surface::Detail) | Target::Details => self.pane = Pane::Detail,
            Target::Surface(_) | Target::Back => self.pane = Pane::Main,
        }
        self.pointer_hits.clear();
        true
    }
}
