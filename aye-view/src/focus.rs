//! Focus membership and logical graph navigation remain session-local.
use crate::{
    app::{App, Mode, Pane},
    model::Relations,
    query::Filters,
};
use aye::model::State;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::BTreeSet;

/// Traverse upward and downward separately: descendants' other prerequisites and
/// ancestors' other children are not part of the captured root's context.
pub fn ids(state: &State, relations: &Relations, root: &str) -> Vec<String> {
    if !state.tasks.contains_key(root) {
        return vec![];
    }
    let mut ancestors = BTreeSet::new();
    let mut pending = vec![root.to_string()];
    while let Some(id) = pending.pop() {
        if ancestors.insert(id.clone()) {
            pending.extend(state.tasks[&id].depends_on.iter().cloned());
        }
    }
    let mut descendants = BTreeSet::new();
    pending.push(root.to_string());
    while let Some(id) = pending.pop() {
        if descendants.insert(id.clone())
            && let Some(next) = relations.blocks.get(&id)
        {
            pending.extend(next.iter().cloned());
        }
    }
    ancestors.extend(descendants);
    let mut ids: Vec<_> = ancestors.into_iter().collect();
    ids.sort_by(|a, b| {
        let a = &state.tasks[a];
        let b = &state.tasks[b];
        (&a.priority, &a.created_at, &a.id).cmp(&(&b.priority, &b.created_at, &b.id))
    });
    ids
}
impl App {
    pub fn focus_selected(&mut self) {
        let Some(id) = self
            .selected_id
            .clone()
            .filter(|id| self.snapshot.state.tasks.contains_key(id))
        else {
            return;
        };
        self.history = None;
        self.query.filters = Filters::default();
        self.query.revealed_id = None;
        self.focus_root = Some(id.clone());
        self.mode = Mode::Graph;
        self.pane = Pane::Main;
        self.refresh_visible();
        self.select(&id);
    }
    pub fn show_current_graph(&mut self) {
        self.history = None;
        self.focus_root = None;
        self.query.filters = Filters::default();
        self.query.revealed_id = None;
        self.mode = Mode::Graph;
        self.pane = Pane::Main;
        self.refresh_visible();
    }
    pub(crate) fn handle_focus_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char('F') => {
                self.focus_selected();
                return true;
            }
            KeyCode::Char('g') => {
                self.show_current_graph();
                return true;
            }
            KeyCode::Esc
                if self.focus_root.is_some()
                    && self.history.is_none()
                    && self.pane == Pane::Main =>
            {
                self.show_current_graph();
                return true;
            }
            _ => {}
        }
        if self.history.is_some() || self.mode != Mode::Graph || self.pane != Pane::Main {
            return false;
        }
        let direction = match key.code {
            KeyCode::Left | KeyCode::Backspace => (-1, 0),
            KeyCode::Right | KeyCode::Char('l') => (1, 0),
            KeyCode::Up | KeyCode::Char('k') => (0, -1),
            KeyCode::Down | KeyCode::Char('j') => (0, 1),
            KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => (-1, 0),
            _ => return false,
        };
        self.ensure_graph();
        if key.modifiers.contains(KeyModifiers::SHIFT)
            && matches!(
                key.code,
                KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down
            )
        {
            self.graph_viewport.x = (self.graph_viewport.x + direction.0 * 8)
                .clamp(0, (self.graph.width - i64::from(self.graph_size.0)).max(0));
            self.graph_viewport.y = (self.graph_viewport.y + direction.1 * 4)
                .clamp(0, (self.graph.height - i64::from(self.graph_size.1)).max(0));
            self.graph_anchor = self.selected_id.clone();
            return true;
        }
        let Some(selected) = self
            .selected_id
            .as_ref()
            .and_then(|id| self.graph.nodes.get(id))
        else {
            return true;
        };
        let mut candidates: Vec<_> = if direction.0 != 0 {
            self.graph
                .edges
                .iter()
                .filter_map(|edge| {
                    if direction.0 < 0 && edge.dependent == selected.id {
                        Some(&edge.prerequisite)
                    } else if direction.0 > 0 && edge.prerequisite == selected.id {
                        Some(&edge.dependent)
                    } else {
                        None
                    }
                })
                .filter_map(|id| self.graph.nodes.get(id))
                .collect()
        } else {
            self.graph
                .nodes
                .values()
                .filter(|node| {
                    node.layer == selected.layer && (node.y - selected.y) * direction.1 > 0
                })
                .collect()
        };
        candidates.sort_by_key(|node| {
            let task = &self.snapshot.state.tasks[&node.id];
            (
                (node.y - selected.y).abs(),
                &task.priority,
                &task.created_at,
                &task.id,
            )
        });
        let next = candidates.first().map(|node| node.id.clone());
        if let Some(id) = next {
            self.select(&id);
        }
        true
    }
}
