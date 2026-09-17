//! Transient application state and keyboard reducer shared by the terminal and tests.
use crate::graph::{Graph, Viewport};
use crate::model::{Relations, current_ids};
use aye::reader::ReaderSnapshot;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pane {
    Main,
    Detail,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Graph,
    List,
}

pub struct App {
    pub focus_root: Option<String>,
    pub help_scroll: usize,
    pub help_max_scroll: usize,
    pub help_page: usize,
    pub history: Option<crate::history::HistoryState>,
    pub recent: crate::history::RecentState,
    pub query: crate::query::QueryState,
    pub mode: Mode,
    pub graph: Graph,
    pub graph_viewport: Viewport,
    graph_key: (String, Vec<String>),
    pub graph_anchor: Option<String>,
    pub graph_size: (u16, u16),
    pub snapshot: ReaderSnapshot,
    pub relations: Relations,
    pub visible_ids: Vec<String>,
    pub selected_id: Option<String>,
    pub pane: Pane,
    pub detail_scroll: usize,
    pub detail_max_scroll: usize,
    pub detail_page: usize,
    pub list_offset: usize,
    pub help: bool,
    pub quit: bool,
    pub refresh_requested: bool,
    pub refresh_error: Option<String>,
}
impl App {
    pub fn new(snapshot: ReaderSnapshot) -> Self {
        let visible_ids = current_ids(&snapshot.state);
        Self {
            focus_root: None,
            help_scroll: 0,
            help_max_scroll: 0,
            help_page: 1,
            history: None,
            recent: crate::history::RecentState::default(),
            query: crate::query::QueryState::default(),
            mode: Mode::Graph,
            graph: Graph::new(&snapshot.state, &visible_ids),
            graph_key: (snapshot.oid.clone(), visible_ids.clone()),
            graph_viewport: Viewport::default(),
            graph_anchor: None,
            graph_size: (0, 0),
            relations: Relations::new(&snapshot.state),
            selected_id: visible_ids.first().cloned(),
            snapshot,
            visible_ids,
            pane: Pane::Main,
            detail_scroll: 0,
            detail_max_scroll: 0,
            detail_page: 1,
            list_offset: 0,
            help: false,
            quit: false,
            refresh_requested: false,
            refresh_error: None,
        }
    }
    pub fn ensure_graph(&mut self) {
        let ids = self.graph_ids();
        if self.graph_key.0 != self.snapshot.oid || self.graph_key.1 != ids {
            self.graph = Graph::new(&self.snapshot.state, &ids);
            self.graph_key = (self.snapshot.oid.clone(), ids);
            self.graph_anchor = None;
        }
    }
    pub fn apply_update(&mut self, update: crate::watch::Update) {
        match update {
            crate::watch::Update::Failed(error) => self.refresh_error = Some(error),
            crate::watch::Update::Snapshot(snapshot) => self.replace_snapshot(snapshot),
        }
    }
    pub fn replace_snapshot(&mut self, snapshot: ReaderSnapshot) {
        let selected = self.selected_id.clone();
        let index = selected
            .as_ref()
            .and_then(|id| self.visible_ids.iter().position(|v| v == id))
            .unwrap_or(0);
        let mut neighbors = Vec::new();
        if let Some(id) = &selected {
            if let Some(task) = self.snapshot.state.tasks.get(id) {
                neighbors.extend(task.depends_on.iter().cloned());
            }
            if let Some(dependents) = self.relations.blocks.get(id) {
                neighbors.extend(dependents.iter().cloned());
            }
        }
        self.snapshot = snapshot;
        self.relations = Relations::new(&self.snapshot.state);
        self.refresh_visible();
        let next = selected
            .filter(|id| self.visible_ids.contains(id))
            .or_else(|| {
                neighbors
                    .into_iter()
                    .find(|id| self.visible_ids.contains(id))
            })
            .or_else(|| {
                self.visible_ids
                    .get(index.min(self.visible_ids.len().saturating_sub(1)))
                    .cloned()
            });
        if next != self.selected_id {
            self.detail_scroll = 0;
        }
        self.selected_id = next;
        self.list_offset = self
            .list_offset
            .min(self.visible_ids.len().saturating_sub(1));
        self.refresh_error = None;
    }
    pub fn select(&mut self, id: &str) {
        if self
            .query
            .revealed_id
            .as_deref()
            .is_some_and(|revealed| revealed != id)
        {
            self.query.revealed_id = None;
            self.refresh_visible();
        }
        if self.snapshot.state.tasks.contains_key(id) && self.selected_id.as_deref() != Some(id) {
            self.selected_id = Some(id.into());
            self.detail_scroll = 0;
        }
    }
    pub fn move_selection(&mut self, delta: isize) {
        if self.visible_ids.is_empty() {
            self.selected_id = None;
            return;
        }
        let index = self
            .selected_id
            .as_ref()
            .and_then(|id| self.visible_ids.iter().position(|v| v == id))
            .unwrap_or(0);
        let next = index
            .saturating_add_signed(delta)
            .min(self.visible_ids.len() - 1);
        let id = self.visible_ids[next].clone();
        self.select(&id);
    }
    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.kind == KeyEventKind::Release {
            return;
        }
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.quit = true;
            return;
        }
        if self.handle_query_key(key) {
            return;
        }
        if key.code == KeyCode::Char('q') {
            self.quit = true;
            return;
        }
        if self.help {
            match key.code {
                KeyCode::Esc | KeyCode::Char('?') => self.help = false,
                KeyCode::Down | KeyCode::Char('j') => {
                    self.help_scroll = self.help_scroll.saturating_add(1).min(self.help_max_scroll)
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.help_scroll = self.help_scroll.saturating_sub(1)
                }
                KeyCode::PageDown => {
                    self.help_scroll = self
                        .help_scroll
                        .saturating_add(self.help_page)
                        .min(self.help_max_scroll)
                }
                KeyCode::PageUp => {
                    self.help_scroll = self.help_scroll.saturating_sub(self.help_page)
                }
                _ => {}
            }
            return;
        }
        if self.handle_focus_key(key) {
            return;
        }
        if self.handle_history_key(key) {
            return;
        }
        match key.code {
            KeyCode::Char('/') => self.open_search(),
            KeyCode::Char('f') => self.open_filters(),
            KeyCode::Char('?') => {
                self.help = true;
                self.help_scroll = 0;
            }
            KeyCode::Char('r') => self.refresh_requested = true,
            KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => self.pane = Pane::Detail,
            KeyCode::Esc | KeyCode::Left | KeyCode::Backspace => self.pane = Pane::Main,
            KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.pane = Pane::Main
            }
            KeyCode::Tab => {
                self.mode = if self.mode == Mode::Graph {
                    Mode::List
                } else {
                    Mode::Graph
                };
                self.pane = Pane::Main;
            }
            KeyCode::PageDown if self.pane == Pane::Detail => {
                self.detail_scroll = self
                    .detail_scroll
                    .saturating_add(self.detail_page)
                    .min(self.detail_max_scroll)
            }
            KeyCode::PageUp if self.pane == Pane::Detail => {
                self.detail_scroll = self.detail_scroll.saturating_sub(self.detail_page)
            }
            KeyCode::Down | KeyCode::Char('j') if self.pane == Pane::Detail => {
                self.detail_scroll = self
                    .detail_scroll
                    .saturating_add(1)
                    .min(self.detail_max_scroll)
            }
            KeyCode::Up | KeyCode::Char('k') if self.pane == Pane::Detail => {
                self.detail_scroll = self.detail_scroll.saturating_sub(1)
            }
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
            _ => {}
        }
    }
}
