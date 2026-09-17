//! Session-only closed-task views. Nothing here reads or writes repository files.
use crate::{
    app::{App, Pane},
    model::{sanitize, status},
};
use aye::model::State;
use chrono::{DateTime, Duration, Utc};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use std::collections::BTreeSet;
const BATCH: usize = 50;

fn closed_entries(state: &State) -> Vec<(DateTime<Utc>, String)> {
    let mut entries: Vec<_> = state
        .tasks
        .values()
        .filter(|task| task.status == "closed")
        .filter_map(|task| {
            task.closed_at
                .as_ref()?
                .parse::<DateTime<Utc>>()
                .ok()
                .map(|time| (time, task.id.clone()))
        })
        .collect();
    entries.sort_by(|(ta, ia), (tb, ib)| tb.cmp(ta).then_with(|| ia.cmp(ib)));
    entries
}

pub struct RecentState {
    pub enabled: bool,
    /// Optional fixed clock for deterministic embedding and verification.
    pub fixed_now: Option<DateTime<Utc>>,
    pub index_builds: usize,
    now: DateTime<Utc>,
    source_oid: Option<String>,
    entries: Vec<(DateTime<Utc>, String)>,
    ids: Vec<String>,
    checked_at: Option<DateTime<Utc>>,
    next_change: Option<DateTime<Utc>>,
    pub(crate) only: BTreeSet<String>,
}
impl Default for RecentState {
    fn default() -> Self {
        Self {
            enabled: false,
            fixed_now: None,
            index_builds: 0,
            now: Utc::now(),
            source_oid: None,
            entries: Vec::new(),
            ids: Vec::new(),
            checked_at: None,
            next_change: None,
            only: BTreeSet::new(),
        }
    }
}
impl RecentState {
    pub fn at(now: DateTime<Utc>) -> Self {
        Self {
            fixed_now: Some(now),
            now,
            ..Self::default()
        }
    }
    fn refresh(&mut self, state: &State, oid: &str) -> bool {
        if !self.enabled {
            return false;
        }
        let replaced = self.source_oid.as_deref() != Some(oid);
        if replaced {
            self.entries = closed_entries(state);
            self.source_oid = Some(oid.into());
            self.index_builds += 1;
        } else if self.checked_at.is_some_and(|last| self.now >= last)
            && self.next_change.is_none_or(|next| self.now < next)
        {
            return false;
        }
        let before = std::mem::take(&mut self.ids);
        self.next_change = None;
        for (closed, id) in &self.entries {
            let expiry = *closed + Duration::hours(24);
            if *closed <= self.now && self.now <= expiry {
                self.ids.push(id.clone());
            }
            let transition = if *closed > self.now {
                *closed
            } else {
                expiry + Duration::nanoseconds(1)
            };
            if transition > self.now {
                self.next_change = Some(
                    self.next_change
                        .map_or(transition, |next| next.min(transition)),
                );
            }
        }
        self.checked_at = Some(self.now);
        before != self.ids
    }
}

pub struct HistoryState {
    /// Lightweight sorted IDs only; row strings are materialized for the viewport.
    pub ids: Vec<String>,
    pub loaded: usize,
    pub total: usize,
    pub rendered_rows: usize,
    pub index_builds: usize,
    source_oid: String,
    return_selection: Option<String>,
    offset: usize,
    page: usize,
}
impl HistoryState {
    fn new(app: &App) -> Self {
        Self {
            ids: closed_entries(&app.snapshot.state)
                .into_iter()
                .map(|(_, id)| id)
                .collect(),
            loaded: BATCH,
            total: 0,
            rendered_rows: 0,
            index_builds: 1,
            source_oid: app.snapshot.oid.clone(),
            return_selection: app.selected_id.clone(),
            offset: 0,
            page: 20,
        }
    }
}
impl App {
    /// IDs for the primary DAG: a Recent-only extra never distorts its layout.
    pub fn graph_ids(&self) -> Vec<String> {
        self.visible_ids
            .iter()
            .filter(|id| !self.recent.only.contains(*id))
            .cloned()
            .collect()
    }
    pub fn recent_only_ids(&self) -> Vec<String> {
        self.recent
            .ids
            .iter()
            .filter(|id| self.recent.only.contains(*id))
            .cloned()
            .collect()
    }
    pub fn tick_clock(&mut self) {
        self.tick_time(self.recent.fixed_now.unwrap_or_else(Utc::now));
    }
    /// Advance time without a ref change. Most frames only compare the next boundary.
    pub fn tick_time(&mut self, now: DateTime<Utc>) -> bool {
        self.recent.now = now;
        let changed = self
            .recent
            .refresh(&self.snapshot.state, &self.snapshot.oid);
        if changed {
            self.refresh_visible();
        }
        changed
    }
    pub(crate) fn add_recent(&mut self, ids: &mut Vec<String>) {
        self.recent
            .refresh(&self.snapshot.state, &self.snapshot.oid);
        self.recent.only.clear();
        if !self.recent.enabled || self.history.is_some() {
            return;
        }
        let existing: BTreeSet<_> = ids.iter().cloned().collect();
        for id in &self.recent.ids {
            if !existing.contains(id)
                && self
                    .query
                    .filters
                    .matches(&self.snapshot.state, &self.snapshot.state.tasks[id])
            {
                ids.push(id.clone());
                self.recent.only.insert(id.clone());
            }
        }
    }
    pub(crate) fn history_candidates(&mut self) -> Option<Vec<String>> {
        let history = self.history.as_mut()?;
        if history.source_oid != self.snapshot.oid {
            history.ids = closed_entries(&self.snapshot.state)
                .into_iter()
                .map(|(_, id)| id)
                .collect();
            history.source_oid = self.snapshot.oid.clone();
            history.index_builds += 1;
        }
        let mut ids: Vec<_> = history
            .ids
            .iter()
            .filter(|id| {
                self.query
                    .filters
                    .matches(&self.snapshot.state, &self.snapshot.state.tasks[*id])
            })
            .cloned()
            .collect();
        history.total = ids.len();
        if let Some(index) = self
            .selected_id
            .as_ref()
            .and_then(|selected| ids.iter().position(|id| id == selected))
        {
            history.loaded = history.loaded.max((index / BATCH + 1) * BATCH);
        }
        ids.truncate(history.loaded);
        Some(ids)
    }
    pub fn open_history(&mut self) {
        if self.history.is_some() {
            return;
        }
        self.history = Some(HistoryState::new(self));
        self.query.revealed_id = None;
        self.pane = Pane::Main;
        self.refresh_visible();
    }
    pub fn close_history(&mut self) {
        if let Some(history) = self.history.take() {
            self.selected_id = history.return_selection;
            self.pane = Pane::Main;
            self.detail_scroll = 0;
            self.refresh_visible();
        }
    }
    fn move_history(&mut self, delta: isize) {
        let position = self
            .selected_id
            .as_ref()
            .and_then(|selected| self.visible_ids.iter().position(|id| id == selected))
            .unwrap_or(0);
        if delta > 0 && position.saturating_add(delta as usize + 5) >= self.visible_ids.len() {
            if let Some(history) = &mut self.history {
                history.loaded = history
                    .loaded
                    .saturating_add(BATCH)
                    .min(history.total.max(BATCH));
            }
            self.refresh_visible();
        }
        self.move_selection(delta);
    }
    /// Called after query/help interception, before Graph/List controls.
    pub fn handle_history_key(&mut self, key: KeyEvent) -> bool {
        if key.code == KeyCode::Char('h') && !key.modifiers.contains(KeyModifiers::CONTROL) {
            self.open_history();
            return true;
        }
        if key.code == KeyCode::Char('c') {
            self.recent.enabled = !self.recent.enabled;
            self.recent.now = self.recent.fixed_now.unwrap_or_else(Utc::now);
            self.refresh_visible();
            return true;
        }
        if self.history.is_none() {
            if key.code == KeyCode::Char(']') && self.recent.enabled {
                let ids = self.recent_only_ids();
                if !ids.is_empty() {
                    let next = self
                        .selected_id
                        .as_ref()
                        .and_then(|selected| ids.iter().position(|id| id == selected))
                        .map_or(0, |index| (index + 1) % ids.len());
                    self.select(&ids[next]);
                    self.pane = Pane::Main;
                }
                return true;
            }
            return false;
        }
        match key.code {
            KeyCode::Esc if self.pane == Pane::Main => self.close_history(),
            KeyCode::Tab => {
                self.pane = if self.pane == Pane::Main {
                    Pane::Detail
                } else {
                    Pane::Main
                }
            }
            KeyCode::Down | KeyCode::Char('j') if self.pane == Pane::Main => self.move_history(1),
            KeyCode::Up | KeyCode::Char('k') if self.pane == Pane::Main => self.move_history(-1),
            KeyCode::PageDown if self.pane == Pane::Main => {
                self.move_history(self.history.as_ref().unwrap().page as isize)
            }
            KeyCode::PageUp if self.pane == Pane::Main => {
                self.move_history(-(self.history.as_ref().unwrap().page as isize))
            }
            _ => return false,
        }
        true
    }
}
fn task_row(app: &App, id: &str) -> ListItem<'static> {
    let task = &app.snapshot.state.tasks[id];
    let (symbol, label) = status(&app.snapshot.state, task);
    ListItem::new(format!(
        "{symbol} {} {} [{label}] {}",
        task.priority,
        sanitize(&task.title).replace('\n', " "),
        task.closed_at.as_deref().unwrap_or("")
    ))
    .style(Style::default().add_modifier(Modifier::DIM))
}
pub fn render_history(frame: &mut Frame, app: &mut App, area: Rect) {
    let selected = app
        .selected_id
        .as_ref()
        .and_then(|selected| app.visible_ids.iter().position(|id| id == selected));
    let Some(history) = &mut app.history else {
        return;
    };
    let block = Block::default()
        .title(format!(
            "History · {}/{} · Esc Back",
            app.visible_ids.len(),
            history.total
        ))
        .borders(Borders::ALL);
    let inner = block.inner(area);
    let rows = usize::from(inner.height);
    history.page = rows.saturating_sub(1).max(1);
    if let Some(index) = selected {
        if index < history.offset {
            history.offset = index;
        } else if index >= history.offset + rows {
            history.offset = index.saturating_sub(rows.saturating_sub(1));
        }
    }
    history.offset = history.offset.min(app.visible_ids.len().saturating_sub(1));
    let start = history.offset;
    let end = start.saturating_add(rows).min(app.visible_ids.len());
    history.rendered_rows = end - start;
    frame.render_widget(block, area);
    if app.visible_ids.is_empty() {
        frame.render_widget(
            Paragraph::new(if app.query.filters.active() {
                "No closed tasks match filters. f to clear/apply; Esc Back."
            } else {
                "No closed tasks. Esc returns to current work."
            }),
            inner,
        );
        return;
    }
    let items = app.visible_ids[start..end]
        .iter()
        .map(|id| task_row(app, id))
        .collect::<Vec<_>>();
    let mut state =
        ListState::default().with_selected(selected.map(|index| index.saturating_sub(start)));
    frame.render_stateful_widget(
        List::new(items)
            .highlight_symbol("> ")
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED)),
        inner,
        &mut state,
    );
}
/// Paint a compact secondary region and return the remaining primary-pane area.
/// Call only outside History; graph/list renderers consume graph_ids in that area.
pub fn render_recent(frame: &mut Frame, app: &App, area: Rect) -> Rect {
    if !app.recent.enabled || area.height < 6 {
        return area;
    }
    let height = 5.min(area.height / 2);
    let main = Rect::new(area.x, area.y, area.width, area.height - height);
    let region = Rect::new(area.x, area.y + main.height, area.width, height);
    let ids = app.recent_only_ids();
    let block = Block::default()
        .title(format!("Recently closed · {} · ] Next", ids.len()))
        .borders(Borders::ALL);
    let inner = block.inner(region);
    frame.render_widget(block, region);
    if ids.is_empty() {
        frame.render_widget(Paragraph::new("No additional recent closed tasks"), inner);
        return main;
    }
    let selected = app
        .selected_id
        .as_ref()
        .and_then(|selected| ids.iter().position(|id| id == selected));
    let rows = usize::from(inner.height);
    let start = selected.unwrap_or(0).saturating_sub(rows.saturating_sub(1));
    let items = ids
        .iter()
        .skip(start)
        .take(rows)
        .map(|id| task_row(app, id))
        .collect::<Vec<_>>();
    let mut state = ListState::default().with_selected(selected.map(|index| index - start));
    frame.render_stateful_widget(
        List::new(items)
            .highlight_symbol("> ")
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED)),
        inner,
        &mut state,
    );
    main
}
