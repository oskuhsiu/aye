use crate::model::Task;

pub const BYPASSED_LABEL: &str = "aye:bypassed";

pub fn is_bypassed(task: &Task) -> bool {
    task.status == "closed"
        && task.resolution.as_deref() == Some("done")
        && task.labels.iter().any(|label| label == BYPASSED_LABEL)
}

pub(crate) fn audit_note(
    reason: &str,
    missing_checks: &[String],
    previous_manual_block: Option<&str>,
) -> String {
    let mut note = format!(
        "[aye:bypass]\nReason: {}\nMissing checks that were not verified:",
        reason.trim()
    );
    for check in missing_checks {
        note.push_str("\n- ");
        note.push_str(check.trim());
    }
    if let Some(block) = previous_manual_block.filter(|value| !value.trim().is_empty()) {
        note.push_str("\nPrevious manual blocker: ");
        note.push_str(block.trim());
    }
    note.push_str(
        "\nThis explicit authorization satisfies downstream dependencies without claiming those checks passed.",
    );
    note
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: &str = "2026-09-17T03:10:00.000Z";

    #[test]
    fn marker_requires_closed_done_state() {
        let mut task = Task::new("test".into(), NOW);
        task.labels.push(BYPASSED_LABEL.into());
        assert!(!is_bypassed(&task));
        task.status = "closed".into();
        task.resolution = Some("done".into());
        task.closed_at = Some(NOW.into());
        assert!(is_bypassed(&task));
        task.resolution = Some("cancelled".into());
        assert!(!is_bypassed(&task));
    }

    #[test]
    fn audit_note_names_each_unverified_check() {
        let note = audit_note(
            "No target device",
            &["Physical-device smoke test".into(), "Reconnect test".into()],
            Some("Hardware unavailable"),
        );
        assert!(note.contains("No target device"));
        assert!(note.contains("Physical-device smoke test"));
        assert!(note.contains("Reconnect test"));
        assert!(note.contains("Hardware unavailable"));
    }
}
