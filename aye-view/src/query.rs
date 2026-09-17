//! Read-only search and five simple intersecting filters, all session-local.
use crate::{
    app::{App, Pane},
    model::{current_ids, sanitize, status},
};
use aye::model::{State, Task};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Filters {
    pub state: Option<String>,
    pub priority: Option<String>,
    pub kind: Option<String>,
    pub label: Option<String>,
    pub claimant: Option<String>,
}
impl Filters {
    pub fn matches(&self, state: &State, task: &Task) -> bool {
        self.state.as_ref().is_none_or(|value| {
            value == status(state, task).1 || (value == "closed" && task.status == "closed")
        }) && self
            .priority
            .as_ref()
            .is_none_or(|value| value == &task.priority)
            && self.kind.as_ref().is_none_or(|value| value == &task.kind)
            && self
                .label
                .as_ref()
                .is_none_or(|value| task.labels.contains(value))
            && self.claimant.as_ref().is_none_or(|value| {
                task.claim
                    .as_ref()
                    .is_some_and(|claim| &claim.actor == value)
            })
    }
    pub fn active(&self) -> bool {
        self != &Self::default()
    }
    fn field(&self, field: usize) -> &Option<String> {
        match field {
            0 => &self.state,
            1 => &self.priority,
            2 => &self.kind,
            3 => &self.label,
            _ => &self.claimant,
        }
    }
    fn field_mut(&mut self, field: usize) -> &mut Option<String> {
        match field {
            0 => &mut self.state,
            1 => &mut self.priority,
            2 => &mut self.kind,
            3 => &mut self.label,
            _ => &mut self.claimant,
        }
    }
}
#[derive(Default, Debug)]
pub struct QueryState {
    pub filters: Filters,
    pub modal: Option<Modal>,
    /// One explicitly chosen search result may bypass current visibility/filters.
    /// Leaving it or applying filters ends the reveal.
    pub revealed_id: Option<String>,
}
#[derive(Debug)]
pub enum Modal {
    Search(Search),
    Filter { draft: Filters, field: usize },
}
#[derive(Default, Debug)]
pub struct Search {
    pub text: String,
    pub results: Vec<String>,
    pub selected: usize,
}
impl Search {
    fn refresh(&mut self, state: &State) {
        let previous = self.results.get(self.selected).cloned();
        let needle = self.text.to_lowercase();
        self.results = state
            .tasks
            .values()
            .filter(|task| {
                task.title.to_lowercase().contains(&needle)
                    || task.id.to_lowercase().contains(&needle)
                    || task
                        .labels
                        .iter()
                        .any(|label| label.to_lowercase().contains(&needle))
            })
            .map(|task| task.id.clone())
            .collect();
        sort_ids(state, &mut self.results);
        self.selected = previous
            .and_then(|id| self.results.iter().position(|value| value == &id))
            .unwrap_or(0);
    }
}
fn sort_ids(state: &State, ids: &mut [String]) {
    ids.sort_by(|a, b| {
        let a = &state.tasks[a];
        let b = &state.tasks[b];
        (&a.priority, &a.created_at, &a.id).cmp(&(&b.priority, &b.created_at, &b.id))
    });
}
impl App {
    /// Recompute presentation after filters or canonical snapshot replacement.
    /// Preserve selection when possible, otherwise choose its previous list neighbor.
    pub fn refresh_visible(&mut self) {
        let previous = self.selected_id.clone();
        let previous_index = previous
            .as_ref()
            .and_then(|id| self.visible_ids.iter().position(|value| value == id))
            .unwrap_or(0);
        if self
            .focus_root
            .as_ref()
            .is_some_and(|id| !self.snapshot.state.tasks.contains_key(id))
        {
            self.focus_root = None;
        }
        let history_ids = self.history_candidates();
        let state = &self.snapshot.state;
        let mut ids = if let Some(ids) = history_ids {
            ids
        } else if let Some(root) = &self.focus_root {
            crate::focus::ids(state, &self.relations, root)
        } else if matches!(
            self.query.filters.state.as_deref(),
            Some("closed" | "closed(done)" | "closed(cancelled)")
        ) {
            state.tasks.keys().cloned().collect()
        } else {
            current_ids(state)
        };
        ids.retain(|id| self.query.filters.matches(state, &state.tasks[id]));
        self.add_recent(&mut ids);
        let state = &self.snapshot.state;
        if let Some(id) = &self.query.revealed_id {
            self.recent.only.remove(id);
            if state.tasks.contains_key(id) {
                if !ids.contains(id) {
                    ids.push(id.clone());
                }
            } else {
                self.query.revealed_id = None;
            }
        }
        if self.history.is_none() {
            sort_ids(state, &mut ids);
        }
        self.selected_id = previous.clone().filter(|id| ids.contains(id)).or_else(|| {
            ids.get(previous_index.min(ids.len().saturating_sub(1)))
                .cloned()
        });
        if self.selected_id != previous {
            self.detail_scroll = 0;
        }
        self.visible_ids = ids;
        self.list_offset = self
            .list_offset
            .min(self.visible_ids.len().saturating_sub(1));
        if let Some(Modal::Search(search)) = &mut self.query.modal {
            search.refresh(state);
        }
    }
    pub fn open_search(&mut self) {
        let mut search = Search::default();
        search.refresh(&self.snapshot.state);
        self.query.modal = Some(Modal::Search(search));
    }
    pub fn open_filters(&mut self) {
        self.query.modal = Some(Modal::Filter {
            draft: self.query.filters.clone(),
            field: 0,
        });
    }
    /// Returns true when a modal consumed the key, including otherwise-global shortcuts.
    pub fn handle_query_key(&mut self, key: KeyEvent) -> bool {
        let Some(mut modal) = self.query.modal.take() else {
            return false;
        };
        if key.code == KeyCode::Esc {
            return true;
        }
        match &mut modal {
            Modal::Search(search) => match key.code {
                KeyCode::Char(c)
                    if !key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                {
                    search.text.push(c);
                    search.refresh(&self.snapshot.state);
                }
                KeyCode::Backspace => {
                    search.text.pop();
                    search.refresh(&self.snapshot.state);
                }
                KeyCode::Down => {
                    search.selected = search
                        .selected
                        .saturating_add(1)
                        .min(search.results.len().saturating_sub(1))
                }
                KeyCode::Up => search.selected = search.selected.saturating_sub(1),
                KeyCode::Enter => {
                    if let Some(id) = search.results.get(search.selected).cloned() {
                        self.close_history();
                        if self.focus_root.as_ref().is_some_and(|root| {
                            !crate::focus::ids(&self.snapshot.state, &self.relations, root)
                                .contains(&id)
                        }) {
                            self.focus_root = None;
                        }
                        self.query.revealed_id = Some(id.clone());
                        self.refresh_visible();
                        self.select(&id);
                        self.pane = Pane::Main;
                        return true;
                    }
                }
                _ => {}
            },
            Modal::Filter { draft, field } => match key.code {
                KeyCode::Down | KeyCode::Tab => *field = (*field + 1) % 5,
                KeyCode::Up | KeyCode::BackTab => *field = (*field + 4) % 5,
                KeyCode::Left | KeyCode::Right | KeyCode::Char(' ') => {
                    let options = options(&self.snapshot.state, *field);
                    let current = draft
                        .field(*field)
                        .as_ref()
                        .and_then(|value| options.iter().position(|v| v == value))
                        .map(|n| n + 1)
                        .unwrap_or(0);
                    let count = options.len() + 1;
                    let next = if key.code == KeyCode::Left {
                        (current + count - 1) % count
                    } else {
                        (current + 1) % count
                    };
                    *draft.field_mut(*field) =
                        next.checked_sub(1).and_then(|n| options.get(n).cloned());
                }
                KeyCode::Char('c') => *draft = Filters::default(),
                KeyCode::Enter => {
                    self.query.filters = draft.clone();
                    self.query.revealed_id = None;
                    self.refresh_visible();
                    self.pane = Pane::Main;
                    return true;
                }
                _ => {}
            },
        }
        self.query.modal = Some(modal);
        true
    }
}
fn options(state: &State, field: usize) -> Vec<String> {
    match field {
        0 => [
            "ready",
            "in_progress",
            "blocked",
            "deferred",
            "closed(done)",
            "closed(cancelled)",
            "closed",
        ]
        .into_iter()
        .map(String::from)
        .collect(),
        1 => ["P0", "P1", "P2", "P3", "P4"]
            .into_iter()
            .map(String::from)
            .collect(),
        2 => ["task", "bug", "feature", "chore"]
            .into_iter()
            .map(String::from)
            .collect(),
        3 => state
            .tasks
            .values()
            .flat_map(|task| task.labels.iter().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        _ => state
            .tasks
            .values()
            .filter_map(|task| task.claim.as_ref().map(|claim| claim.actor.clone()))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
    }
}
/// Draw only viewport-sized search rows, never every matching historical task.
pub fn render(frame: &mut Frame, app: &App) {
    let Some(modal) = &app.query.modal else {
        return;
    };
    let full = frame.area();
    let width = full.width.min(88);
    let height = full.height.min(22);
    let area = Rect::new(
        full.x + (full.width - width) / 2,
        full.y + (full.height - height) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, area);
    let title = match modal {
        Modal::Search(_) => "Search · all tasks",
        Modal::Filter { .. } => "Filter · five intersecting fields",
    };
    let block = Block::default().title(title).borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    match modal {
        Modal::Search(search) => {
            frame.render_widget(
                Paragraph::new(format!("> {}", sanitize(&search.text))),
                Rect::new(inner.x, inner.y, inner.width, 1),
            );
            let rows = inner.height.saturating_sub(3) as usize;
            let start = search.selected.saturating_sub(rows.saturating_sub(1));
            let list_area = Rect::new(inner.x, inner.y + 1, inner.width, rows as u16);
            if search.results.is_empty() {
                frame.render_widget(
                    Paragraph::new("No matches. Edit search or Esc to cancel."),
                    list_area,
                );
            } else {
                let items = search
                    .results
                    .iter()
                    .skip(start)
                    .take(rows)
                    .map(|id| {
                        let task = &app.snapshot.state.tasks[id];
                        ListItem::new(format!(
                            "{} {} {} · {}",
                            status(&app.snapshot.state, task).0,
                            task.priority,
                            sanitize(&task.title).replace('\n', " "),
                            id
                        ))
                    })
                    .collect::<Vec<_>>();
                let mut list_state =
                    ListState::default().with_selected(Some(search.selected - start));
                frame.render_stateful_widget(
                    List::new(items)
                        .highlight_symbol("> ")
                        .highlight_style(Style::default().add_modifier(Modifier::REVERSED)),
                    list_area,
                    &mut list_state,
                );
            }
            if inner.height >= 3 {
                frame.render_widget(
                    Paragraph::new(format!(
                        "{} matches · ↑/↓ Select · Enter Reveal\nEsc Cancel · Ctrl-C Quit",
                        search.results.len()
                    )),
                    Rect::new(inner.x, inner.y + inner.height - 2, inner.width, 2),
                );
            }
        }
        Modal::Filter { draft, field } => {
            let names = ["State", "Priority", "Type", "Label", "Claimant"];
            let lines = names
                .iter()
                .enumerate()
                .map(|(i, name)| {
                    let text = format!(
                        "{} {name}: {}",
                        if i == *field { ">" } else { " " },
                        sanitize(draft.field(i).as_deref().unwrap_or("Any")).replace('\n', " ")
                    );
                    if i == *field {
                        Line::styled(text, Style::default().add_modifier(Modifier::REVERSED))
                    } else {
                        Line::raw(text)
                    }
                })
                .chain([
                    Line::raw(""),
                    Line::raw("↑/↓ or Tab Field · ←/→ or Space Cycle"),
                    Line::raw("c Clear draft · Enter Apply · Esc Cancel"),
                ])
                .collect::<Vec<_>>();
            frame.render_widget(Paragraph::new(lines), inner);
        }
    }
}
