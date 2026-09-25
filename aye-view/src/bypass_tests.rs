use crate::{app::App, query::Filters, view};
use aye::{
    BYPASSED_LABEL,
    model::{State, Task},
    reader::ReaderSnapshot,
};

const NOW: &str = "2026-09-17T03:10:00.000Z";

fn closed(id: u8, title: &str, bypassed: bool) -> Task {
    let mut task = Task::new(title.into(), NOW);
    task.id = format!("t-{id:020x}");
    task.status = "closed".into();
    task.resolution = Some("done".into());
    task.closed_at = Some(NOW.into());
    if bypassed {
        task.labels.push(BYPASSED_LABEL.into());
    }
    task
}

#[test]
fn bypass_filter_and_detail_distinguish_accepted_risk() {
    let mut state = State::empty();
    let bypassed = closed(1, "Device verification waived", true);
    let done = closed(2, "Fully verified", false);
    state.tasks.insert(bypassed.id.clone(), bypassed.clone());
    state.tasks.insert(done.id.clone(), done);

    let mut app = App::new(ReaderSnapshot {
        oid: "a".repeat(40),
        state,
    });
    app.query.filters = Filters {
        state: Some("closed(bypassed)".into()),
        ..Default::default()
    };
    app.refresh_visible();

    assert_eq!(app.visible_ids, vec![bypassed.id.clone()]);
    assert_eq!(app.selected_id, Some(bypassed.id));
    let detail = view::detail_text(&app);
    assert!(detail.contains("⚠ closed(bypassed)"));
    assert!(detail.contains(BYPASSED_LABEL));
}
