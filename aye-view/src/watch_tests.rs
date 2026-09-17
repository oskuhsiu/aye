use super::*;
use aye::{
    error::Error,
    model::{State, Task},
};
use std::{cell::Cell, collections::BTreeMap, rc::Rc};

struct Fake {
    oid: Option<String>,
    snapshots: BTreeMap<String, ReaderSnapshot>,
    loads: Rc<Cell<usize>>,
    advance_on_load: Option<String>,
}
impl Source for Fake {
    fn current_oid(&mut self) -> aye::error::Result<Option<String>> {
        Ok(self.oid.clone())
    }
    fn load(&mut self, oid: &str) -> aye::error::Result<ReaderSnapshot> {
        self.loads.set(self.loads.get() + 1);
        if let Some(next) = self.advance_on_load.take() {
            self.oid = Some(next);
        }
        self.snapshots
            .get(oid)
            .cloned()
            .ok_or_else(|| Error::new("STATE_CORRUPT", "broken snapshot"))
    }
}
fn snapshot(oid: &str) -> ReaderSnapshot {
    ReaderSnapshot {
        oid: oid.into(),
        state: State::empty(),
    }
}
#[test]
fn unchanged_oid_skips_loading_and_changed_reads_stay_pinned() {
    let loads = Rc::new(Cell::new(0));
    let source = Fake {
        oid: Some("a".into()),
        snapshots: [("b".into(), snapshot("b")), ("c".into(), snapshot("c"))].into(),
        loads: loads.clone(),
        advance_on_load: None,
    };
    let mut poller = Poller::new(source, "a".into());
    assert!(poller.poll(false).is_none());
    assert_eq!(loads.get(), 0);
    poller.source.oid = Some("b".into());
    poller.source.advance_on_load = Some("c".into());
    assert!(matches!(poller.poll(false),Some(Update::Snapshot(s)) if s.oid == "b"));
    assert!(matches!(poller.poll(false),Some(Update::Snapshot(s)) if s.oid == "c"));
    assert!(poller.poll(false).is_none());
    assert_eq!(loads.get(), 2);
}
#[test]
fn failed_oid_is_not_reparsed_without_manual_refresh_and_recovers() {
    let loads = Rc::new(Cell::new(0));
    let source = Fake {
        oid: Some("bad".into()),
        snapshots: [("good".into(), snapshot("good"))].into(),
        loads: loads.clone(),
        advance_on_load: None,
    };
    let mut poller = Poller::new(source, "good".into());
    assert!(matches!(poller.poll(false),Some(Update::Failed(e)) if e.contains("STATE_CORRUPT")));
    assert!(poller.poll(false).is_none());
    assert_eq!(loads.get(), 1);
    assert!(matches!(poller.poll(true), Some(Update::Failed(_))));
    assert_eq!(loads.get(), 2);
    poller.source.oid = Some("good".into());
    assert!(matches!(poller.poll(false),Some(Update::Snapshot(s)) if s.oid == "good"));
    poller.source.oid = None;
    assert!(matches!(poller.poll(false),Some(Update::Failed(e)) if e.contains("aye init")));
    assert!(poller.poll(false).is_none());
    poller.source.oid = Some("good".into());
    assert!(matches!(poller.poll(false),Some(Update::Snapshot(s)) if s.oid == "good"));
    assert_eq!(loads.get(), 4);
}
#[test]
fn refreshed_selection_prefers_surviving_context_and_errors_keep_last_good_frame() {
    use crate::{app::App, view};
    use ratatui::{Terminal, backend::TestBackend};
    fn task(n: u8, title: &str) -> Task {
        let mut t = Task::new(title.into(), "2026-09-17T00:00:00.000Z");
        t.id = format!("t-{n:020x}");
        t
    }
    let prerequisite = task(1, "Shared prerequisite");
    let mut selected = task(2, "Finishing selected task");
    selected.depends_on.push(prerequisite.id.clone());
    let mut sibling = task(3, "Continuing sibling");
    sibling.depends_on.push(prerequisite.id.clone());
    let unrelated = task(4, "Unrelated work");
    let mut state = State::empty();
    for t in [&prerequisite, &selected, &sibling, &unrelated] {
        state.tasks.insert(t.id.clone(), t.clone());
    }
    state.validate().unwrap();
    let mut app = App::new(ReaderSnapshot {
        oid: "a".into(),
        state: state.clone(),
    });
    app.select(&selected.id);
    app.apply_update(Update::Failed("STATE_CORRUPT: broken\u{1b}[2J".into()));
    assert_eq!(app.snapshot.oid, "a");
    assert_eq!(app.selected_id.as_deref(), Some(selected.id.as_str()));
    let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
    terminal.draw(|f| view::render(f, &mut app)).unwrap();
    let text = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();
    assert!(text.contains("last good"));
    assert!(text.contains("Finishing selected task"));
    assert!(!text.contains('\u{1b}'));
    for id in [&prerequisite.id, &selected.id] {
        let t = state.tasks.get_mut(id).unwrap();
        t.status = "closed".into();
        t.resolution = Some("done".into());
        t.closed_at = Some(t.updated_at.clone());
    }
    state.validate().unwrap();
    app.apply_update(Update::Snapshot(ReaderSnapshot {
        oid: "b".into(),
        state: state.clone(),
    }));
    assert_eq!(app.selected_id.as_deref(), Some(prerequisite.id.as_str()));
    assert!(!app.visible_ids.contains(&selected.id));
    assert!(app.refresh_error.is_none());
    app.apply_update(Update::Snapshot(ReaderSnapshot {
        oid: "c".into(),
        state,
    }));
    assert_eq!(app.selected_id.as_deref(), Some(prerequisite.id.as_str()));
}
