//! Derived presentation data; canonical tasks remain owned by aye.
use aye::model::{State, Task};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default, Debug)]
pub struct Relations {
    pub blocks: BTreeMap<String, Vec<String>>,
    pub children: BTreeMap<String, Vec<String>>,
    pub discoveries: BTreeMap<String, Vec<String>>,
}
impl Relations {
    pub fn new(state: &State) -> Self {
        let mut relations = Self::default();
        for task in state.tasks.values() {
            for id in &task.depends_on {
                relations
                    .blocks
                    .entry(id.clone())
                    .or_default()
                    .push(task.id.clone());
            }
            if let Some(id) = &task.parent {
                relations
                    .children
                    .entry(id.clone())
                    .or_default()
                    .push(task.id.clone());
            }
            if let Some(id) = &task.discovered_from {
                relations
                    .discoveries
                    .entry(id.clone())
                    .or_default()
                    .push(task.id.clone());
            }
        }
        relations
    }
}
/// Session-local selection and pending downstream dependency influence.
/// Derived from canonical tasks, independently of the visible graph scope.
#[derive(Default)]
pub struct PendingInfluence<'a> {
    pub selected: Option<&'a str>,
    pub affected: BTreeSet<&'a str>,
}
impl<'a> PendingInfluence<'a> {
    pub fn new(state: &'a State, relations: &Relations, selected: Option<&'a str>) -> Self {
        let mut influence = Self {
            selected,
            ..Self::default()
        };
        let Some(root) = selected.and_then(|id| state.tasks.get(id)) else {
            return influence;
        };
        // Only done satisfies a prerequisite; a cancelled selection can still
        // influence pending work. Closed downstream targets always stop a branch.
        if root.status == "closed" && root.resolution.as_deref() == Some("done") {
            return influence;
        }
        let mut pending = vec![root.id.as_str()];
        while let Some(id) = pending.pop() {
            if let Some(dependents) = relations.blocks.get(id) {
                for id in dependents {
                    let task = &state.tasks[id];
                    if task.status != "closed" && influence.affected.insert(task.id.as_str()) {
                        pending.push(task.id.as_str());
                    }
                }
            }
        }
        influence
    }
    pub fn includes_edge(&self, prerequisite: &str, dependent: &str) -> bool {
        self.affected.contains(dependent)
            && (self.selected == Some(prerequisite) || self.affected.contains(prerequisite))
    }
}
/// All non-closed tasks and their prerequisite ancestors, in stable list order.
pub fn current_ids(state: &State) -> Vec<String> {
    let mut included = BTreeSet::new();
    let mut pending: Vec<_> = state
        .tasks
        .values()
        .filter(|t| t.status != "closed")
        .map(|t| t.id.clone())
        .collect();
    while let Some(id) = pending.pop() {
        if included.insert(id.clone()) {
            pending.extend(state.tasks[&id].depends_on.iter().cloned());
        }
    }
    let mut ids: Vec<_> = included.into_iter().collect();
    ids.sort_by(|a, b| {
        let a = &state.tasks[a];
        let b = &state.tasks[b];
        (&a.priority, &a.created_at, &a.id).cmp(&(&b.priority, &b.created_at, &b.id))
    });
    ids
}
pub fn status<'a>(state: &'a State, task: &'a Task) -> (&'static str, &'a str) {
    match state.effective(task) {
        "closed" if task.resolution.as_deref() == Some("cancelled") => ("×", "closed(cancelled)"),
        "closed" if task.labels.iter().any(|label| label == aye::BYPASSED_LABEL) => {
            ("⚠", "closed(bypassed)")
        }
        "closed" => ("✓", "closed(done)"),
        "in_progress" => ("▶", "in_progress"),
        "blocked" => ("!", "blocked"),
        "deferred" => ("⏸", "deferred"),
        _ => ("●", "ready"),
    }
}
/// Replace terminal controls and directional formatting while preserving Unicode text.
pub fn sanitize(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '\n' => '\n',
            '\t' => ' ',
            c if c.is_control()
                || matches!(
                    c,
                    '\u{061c}'
                        | '\u{200e}'
                        | '\u{200f}'
                        | '\u{202a}'..='\u{202e}'
                        | '\u{2066}'..='\u{2069}'
                ) =>
            {
                '�'
            }
            c => c,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: &str = "2026-09-17T03:10:00.000Z";

    #[test]
    fn bypass_marker_has_distinct_closed_status() {
        let mut state = State::empty();
        let mut task = Task::new("implemented without device".into(), NOW);
        task.status = "closed".into();
        task.resolution = Some("done".into());
        task.closed_at = Some(NOW.into());
        task.labels.push(aye::BYPASSED_LABEL.into());
        state.tasks.insert(task.id.clone(), task.clone());

        assert_eq!(status(&state, &task), ("⚠", "closed(bypassed)"));
        assert_eq!(state.effective(&task), "closed");
    }
}
