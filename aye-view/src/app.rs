//! Transient application state and keyboard reducer shared by the terminal and tests.
use crate::model::{Relations, current_ids};
use aye::reader::ReaderSnapshot;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pane {
    Main,
    Detail,
}

pub struct App {
    pub history: Option<crate::history::HistoryState>,
    pub recent: crate::history::RecentState,
    pub query: crate::query::QueryState,
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
}
impl App {
    pub fn new(snapshot: ReaderSnapshot) -> Self {
        let visible_ids = current_ids(&snapshot.state);
        Self {
            history: None,
            recent: crate::history::RecentState::default(),
            query: crate::query::QueryState::default(),
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
        }
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
            if matches!(key.code, KeyCode::Esc | KeyCode::Char('?')) {
                self.help = false;
            }
            return;
        }
        if self.handle_history_key(key) {
            return;
        }
        match key.code {
            KeyCode::Char('/') => self.open_search(),
            KeyCode::Char('f') => self.open_filters(),
            KeyCode::Char('?') => self.help = true,
            KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => self.pane = Pane::Detail,
            KeyCode::Esc | KeyCode::Left | KeyCode::Backspace => self.pane = Pane::Main,
            KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.pane = Pane::Main
            }
            KeyCode::Tab => {
                self.pane = if self.pane == Pane::Main {
                    Pane::Detail
                } else {
                    Pane::Main
                }
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
