use crate::error::{Error, Result};
use crate::model::{Claim, ManualBlock, Note, State, Task};
use serde_json::{Value, json};

#[derive(Clone, Debug, Default)]
pub struct Update {
    pub title: Option<String>,
    pub kind: Option<String>,
    pub priority: Option<String>,
    pub description: Option<String>,
    pub acceptance: Option<Vec<String>>,
    pub labels: Option<Vec<String>>,
    pub parent: Option<Option<String>>,
}
#[derive(Clone, Debug)]
pub enum Action {
    Create(Box<Task>),
    Claim(String),
    Close {
        id: String,
        cancelled: bool,
        note: Option<String>,
    },
    Note {
        id: String,
        body: String,
    },
    Update {
        id: String,
        patch: Update,
    },
    Block {
        id: String,
        by: Option<String>,
        reason: Option<String>,
    },
    Unblock {
        id: String,
        by: Option<String>,
    },
    Release {
        id: String,
        force: bool,
        reason: Option<String>,
    },
    Defer(String),
    Resume(String),
    Reopen(String),
}
fn transition(message: impl Into<String>) -> Error {
    Error::new("INVALID_STATE_TRANSITION", message)
}
pub fn apply(state: &mut State, action: &Action, actor: Option<&str>, now: &str) -> Result<Value> {
    let actor = actor.filter(|a| !a.trim().is_empty()).ok_or_else(|| {
        Error::new(
            "ACTOR_REQUIRED",
            "Set --actor, AYE_ACTOR, or a worktree actor",
        )
    })?;
    state.validate()?;
    let mut next = state.clone();
    let mut released_claim = None;
    let id = match action {
        Action::Create(task) => {
            if next.tasks.contains_key(&task.id) {
                return Err(Error::new("TASK_ID_COLLISION", &task.id));
            }
            if task.status != "open"
                || task.claim.is_some()
                || task.closed_at.is_some()
                || task.resolution.is_some()
            {
                return Err(Error::usage("New tasks must be open"));
            }
            let mut task = task.as_ref().clone();
            task.parent = task
                .parent
                .as_deref()
                .map(|v| resolve_relation(&next, v))
                .transpose()?;
            task.discovered_from = task
                .discovered_from
                .as_deref()
                .map(|v| resolve_relation(&next, v))
                .transpose()?;
            task.depends_on = task
                .depends_on
                .iter()
                .map(|v| resolve_relation(&next, v))
                .collect::<Result<_>>()?;
            task.labels.sort();
            task.depends_on.sort();
            let id = task.id.clone();
            next.tasks.insert(id.clone(), task);
            id
        }
        Action::Claim(input)
        | Action::Close { id: input, .. }
        | Action::Note { id: input, .. }
        | Action::Update { id: input, .. }
        | Action::Block { id: input, .. }
        | Action::Unblock { id: input, .. }
        | Action::Release { id: input, .. }
        | Action::Defer(input)
        | Action::Resume(input)
        | Action::Reopen(input) => {
            let id = next.resolve(input)?;
            let original = &next.tasks[&id];
            if matches!(action, Action::Claim(_)) && original.claim.is_some() {
                return Err(Error::new(
                    "TASK_ALREADY_CLAIMED",
                    format!("{id} is already claimed"),
                ));
            }
            let forced_release = matches!(action, Action::Release { force: true, .. });
            if !forced_release && original.claim.as_ref().is_some_and(|c| c.actor != actor) {
                return Err(Error::new("NOT_CLAIM_OWNER", &id));
            }
            let ready = next.effective(original) == "ready";
            let unblocked = original.manual_block.is_none() && next.blocked_by(original).is_empty();
            let by = match action {
                Action::Block { by, .. } | Action::Unblock { by, .. } => by
                    .as_deref()
                    .map(|v| resolve_relation(&next, v))
                    .transpose()?,
                _ => None,
            };
            if let Action::Block {
                by: input_by,
                reason,
                ..
            } = action
            {
                if input_by.is_some() == reason.is_some() {
                    return Err(Error::usage("Specify exactly one of --by and --reason"));
                }
                if reason.as_deref().is_some_and(|v| v.trim().is_empty()) {
                    return Err(Error::usage("Block reason cannot be empty"));
                }
                if let Some(target) = &by {
                    if target == &id {
                        return Err(Error::new(
                            "INVALID_DEPENDENCY",
                            "A task cannot block itself",
                        ));
                    }
                    let prerequisite = &next.tasks[target];
                    if prerequisite.status == "closed"
                        && prerequisite.resolution.as_deref() == Some("done")
                    {
                        return Err(Error::new("BLOCKER_ALREADY_SATISFIED", target));
                    }
                    if original.depends_on.contains(target) {
                        return Err(Error::new("DUPLICATE_DEPENDENCY", target));
                    }
                }
            }
            let parent = match action {
                Action::Update { patch, .. } => patch
                    .parent
                    .as_ref()
                    .map(|v| v.as_deref().map(|v| resolve_relation(&next, v)).transpose())
                    .transpose()?,
                _ => None,
            };
            if matches!(action, Action::Reopen(_))
                && next
                    .tasks
                    .values()
                    .any(|t| t.status == "in_progress" && t.depends_on.contains(&id))
            {
                return Err(transition(
                    "Release actively in-progress dependents before reopening their prerequisite",
                ));
            }
            let task = next.tasks.get_mut(&id).unwrap();
            match action {
                Action::Claim(_) => {
                    if task.status != "open" {
                        return Err(transition(&id));
                    }
                    if !ready {
                        return Err(Error::new("TASK_NOT_READY", &id));
                    }
                    task.status = "in_progress".into();
                    task.claim = Some(Claim {
                        actor: actor.into(),
                        claimed_at: now.into(),
                    });
                }
                Action::Close {
                    cancelled, note, ..
                } => {
                    if task.status == "closed"
                        || (!cancelled && (task.status == "deferred" || !unblocked))
                    {
                        return Err(transition("Task cannot close with this resolution"));
                    }
                    task.status = "closed".into();
                    task.resolution = Some(if *cancelled { "cancelled" } else { "done" }.into());
                    task.closed_at = Some(now.into());
                    task.claim = None;
                    if let Some(body) = note {
                        append_note(task, actor, now, body)?;
                    }
                }
                Action::Note { body, .. } => append_note(task, actor, now, body)?,
                Action::Update { patch, .. } => {
                    if let Some(v) = &patch.title {
                        task.title = v.clone();
                    }
                    if let Some(v) = &patch.kind {
                        task.kind = v.clone();
                    }
                    if let Some(v) = &patch.priority {
                        task.priority = v.clone();
                    }
                    if let Some(v) = &patch.description {
                        task.description = v.clone();
                    }
                    if let Some(v) = &patch.acceptance {
                        task.acceptance = v.clone();
                    }
                    if let Some(v) = &patch.labels {
                        task.labels = v.clone();
                        task.labels.sort();
                    }
                    if let Some(v) = parent {
                        task.parent = v;
                    }
                }
                Action::Block { reason, .. } => {
                    if task.status == "closed" {
                        return Err(transition("Reopen task before blocking"));
                    }
                    if let Some(by) = by {
                        task.depends_on.push(by);
                        task.depends_on.sort();
                    }
                    if let Some(reason) = reason {
                        task.manual_block = Some(ManualBlock {
                            reason: reason.clone(),
                            actor: actor.into(),
                            blocked_at: now.into(),
                        });
                    }
                    released_claim = task.claim.take().map(|c| c.actor);
                    if task.status == "in_progress" {
                        task.status = "open".into();
                    }
                }
                Action::Unblock { .. } => {
                    if task.status == "closed" {
                        return Err(transition("Reopen task before unblocking"));
                    }
                    if let Some(by) = by {
                        if !task.depends_on.contains(&by) {
                            return Err(Error::new("DEPENDENCY_NOT_FOUND", by));
                        }
                        task.depends_on.retain(|v| v != &by);
                    } else {
                        task.manual_block = None;
                    }
                }
                Action::Release { force, reason, .. } => {
                    if task.status != "in_progress" {
                        return Err(transition("Only in-progress tasks can be released"));
                    }
                    if *force {
                        let reason = reason
                            .as_deref()
                            .filter(|v| !v.trim().is_empty())
                            .ok_or_else(|| Error::usage("Forced release requires a reason"))?;
                        let old_actor = &task.claim.as_ref().unwrap().actor;
                        let body =
                            format!("Forced release of claim owned by {old_actor}: {reason}");
                        append_note(task, actor, now, &body)?;
                    } else if reason.is_some() {
                        return Err(Error::usage("--reason requires --force"));
                    }
                    task.claim = None;
                    task.status = "open".into();
                }
                Action::Defer(_) => {
                    if task.status != "open" && task.status != "in_progress" {
                        return Err(transition("Only open or in-progress tasks can be deferred"));
                    }
                    task.status = "deferred".into();
                    task.claim = None;
                }
                Action::Resume(_) => {
                    if task.status != "deferred" {
                        return Err(transition("Only deferred tasks can be resumed"));
                    }
                    task.status = "open".into();
                }
                Action::Reopen(_) => {
                    if task.status != "closed" {
                        return Err(transition("Only closed tasks can be reopened"));
                    }
                    task.status = "open".into();
                    task.resolution = None;
                    task.closed_at = None;
                    task.claim = None;
                }
                Action::Create(_) => unreachable!(),
            }
            task.updated_at = now.into();
            id
        }
    };
    // The loaded state was valid, so a new invariant failure is invalid command input.
    next.validate().map_err(|e| match e.message.as_str() {
        "Dependency cycle" => Error::new("DEPENDENCY_CYCLE", e.message),
        "Parent cycle" => Error::new("PARENT_CYCLE", e.message),
        _ => Error::usage(e.message),
    })?;
    let task = &next.tasks[&id];
    let mut output = json!({"task": task, "computed": {"effective_state": next.effective(task), "blocked_by": next.blocked_by(task)}});
    if matches!(action, Action::Block { .. }) {
        output["released_claim"] = json!(released_claim);
    }
    *state = next;
    Ok(output)
}
fn append_note(task: &mut Task, actor: &str, now: &str, body: &str) -> Result<()> {
    if body.trim().is_empty() {
        return Err(Error::usage("Note body cannot be empty"));
    }
    task.notes.push(Note {
        actor: actor.into(),
        created_at: now.into(),
        body: body.into(),
    });
    Ok(())
}

fn resolve_relation(state: &State, input: &str) -> Result<String> {
    state.resolve(input).map_err(|e| {
        if e.code == "TASK_NOT_FOUND" {
            Error::new("RELATION_TARGET_NOT_FOUND", input)
        } else {
            e
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    const NOW: &str = "2026-09-17T03:10:00.000Z";
    fn fixture() -> (State, String) {
        let mut s = State::empty();
        let t = Task::new("Ship bootstrap".into(), NOW);
        let id = t.id.clone();
        apply(&mut s, &Action::Create(Box::new(t)), Some("a"), NOW).unwrap();
        (s, id)
    }
    #[test]
    fn complete_task_and_preserve_notes() {
        let (mut s, id) = fixture();
        assert_eq!(s.effective(&s.tasks[&id]), "ready");
        apply(&mut s, &Action::Claim(id.clone()), Some("a"), NOW).unwrap();
        apply(
            &mut s,
            &Action::Note {
                id: id.clone(),
                body: "Verified behavior".into(),
            },
            Some("a"),
            NOW,
        )
        .unwrap();
        apply(
            &mut s,
            &Action::Close {
                id: id.clone(),
                cancelled: false,
                note: Some("Passed".into()),
            },
            Some("a"),
            NOW,
        )
        .unwrap();
        assert_eq!(s.tasks[&id].resolution.as_deref(), Some("done"));
        assert!(s.tasks[&id].claim.is_none());
        assert_eq!(s.tasks[&id].notes.len(), 2);
        s.validate().unwrap();
    }
    #[test]
    fn failed_mutations_and_claim_retry_preserve_winner() {
        let (mut s, id) = fixture();
        assert_eq!(
            apply(&mut s, &Action::Claim(id.clone()), None, NOW)
                .unwrap_err()
                .code,
            "ACTOR_REQUIRED"
        );
        apply(&mut s, &Action::Claim(id.clone()), Some("a"), NOW).unwrap();
        let before = serde_json::to_value(&s.tasks).unwrap();
        assert_eq!(
            apply(&mut s, &Action::Claim(id.clone()), Some("b"), NOW)
                .unwrap_err()
                .code,
            "TASK_ALREADY_CLAIMED"
        );
        for action in [
            Action::Note {
                id: id.clone(),
                body: "other".into(),
            },
            Action::Close {
                id,
                cancelled: false,
                note: None,
            },
        ] {
            assert_eq!(
                apply(&mut s, &action, Some("b"), NOW).unwrap_err().code,
                "NOT_CLAIM_OWNER"
            );
        }
        assert_eq!(serde_json::to_value(&s.tasks).unwrap(), before);
    }
    #[test]
    fn only_done_unblocks_and_parent_is_organizational() {
        let (mut s, id) = fixture();
        let mut child = Task::new("child".into(), NOW);
        child.parent = Some(id.clone());
        child.discovered_from = Some(id.clone());
        let child_id = child.id.clone();
        apply(&mut s, &Action::Create(Box::new(child)), Some("a"), NOW).unwrap();
        assert_eq!(s.effective(&s.tasks[&child_id]), "ready");
        s.tasks
            .get_mut(&child_id)
            .unwrap()
            .depends_on
            .push(id.clone());
        assert_eq!(s.effective(&s.tasks[&child_id]), "blocked");
        assert!(apply(&mut s, &Action::Claim(child_id.clone()), Some("a"), NOW).is_err());
        assert!(
            apply(
                &mut s,
                &Action::Close {
                    id: child_id.clone(),
                    cancelled: false,
                    note: None
                },
                Some("a"),
                NOW
            )
            .is_err()
        );
        apply(
            &mut s,
            &Action::Close {
                id: id.clone(),
                cancelled: true,
                note: None,
            },
            Some("a"),
            NOW,
        )
        .unwrap();
        assert_eq!(s.blocked_by(&s.tasks[&child_id]), vec![id.clone()]);
        s.tasks.get_mut(&id).unwrap().resolution = Some("done".into());
        assert_eq!(s.effective(&s.tasks[&child_id]), "ready");
    }

    #[test]
    fn protocol_errors_distinguish_relation_failures() {
        let (mut s, id) = fixture();
        let b = Task::new("b".into(), NOW);
        let bid = b.id.clone();
        run(&mut s, Action::Create(Box::new(b)));
        run(
            &mut s,
            Action::Block {
                id: id.clone(),
                by: Some(bid.clone()),
                reason: None,
            },
        );
        assert_eq!(
            apply(
                &mut s,
                &Action::Block {
                    id: bid.clone(),
                    by: Some(id.clone()),
                    reason: None
                },
                Some("a"),
                NOW
            )
            .unwrap_err()
            .code,
            "DEPENDENCY_CYCLE"
        );
        run(
            &mut s,
            Action::Update {
                id: bid.clone(),
                patch: Update {
                    parent: Some(Some(id.clone())),
                    ..Default::default()
                },
            },
        );
        assert_eq!(
            apply(
                &mut s,
                &Action::Update {
                    id: id.clone(),
                    patch: Update {
                        parent: Some(Some(bid)),
                        ..Default::default()
                    }
                },
                Some("a"),
                NOW
            )
            .unwrap_err()
            .code,
            "PARENT_CYCLE"
        );
        assert_eq!(
            apply(
                &mut s,
                &Action::Block {
                    id,
                    by: Some("t-00000000000000000000".into()),
                    reason: None
                },
                Some("a"),
                NOW
            )
            .unwrap_err()
            .code,
            "RELATION_TARGET_NOT_FOUND"
        );
    }
    fn run(s: &mut State, action: Action) -> Value {
        apply(s, &action, Some("a"), NOW).unwrap()
    }
    fn unchanged(s: &mut State, action: Action) {
        let before = s.tasks.clone();
        assert!(apply(s, &action, Some("a"), NOW).is_err());
        assert_eq!(s.tasks, before);
    }
    #[test]
    fn lifecycle_and_forced_release_audit() {
        let (mut s, id) = fixture();
        unchanged(
            &mut s,
            Action::Release {
                id: id.clone(),
                force: false,
                reason: None,
            },
        );
        run(&mut s, Action::Claim(id.clone()));
        let release = Action::Release {
            id: id.clone(),
            force: false,
            reason: None,
        };
        assert_eq!(
            apply(&mut s, &release, Some("b"), NOW).unwrap_err().code,
            "NOT_CLAIM_OWNER"
        );
        unchanged(
            &mut s,
            Action::Release {
                id: id.clone(),
                force: true,
                reason: None,
            },
        );
        apply(
            &mut s,
            &Action::Release {
                id: id.clone(),
                force: true,
                reason: Some("agent crashed".into()),
            },
            Some("b"),
            NOW,
        )
        .unwrap();
        let note = s.tasks[&id].notes.last().unwrap();
        assert_eq!(note.actor, "b");
        assert!(note.body.contains("a") && note.body.contains("agent crashed"));
        run(&mut s, Action::Claim(id.clone()));
        run(&mut s, release);
        assert_eq!(s.effective(&s.tasks[&id]), "ready");
        run(&mut s, Action::Claim(id.clone()));
        run(&mut s, Action::Defer(id.clone()));
        assert!(s.tasks[&id].claim.is_none());
        unchanged(&mut s, Action::Defer(id.clone()));
        unchanged(
            &mut s,
            Action::Close {
                id: id.clone(),
                cancelled: false,
                note: None,
            },
        );
        run(&mut s, Action::Resume(id.clone()));
        unchanged(&mut s, Action::Resume(id.clone()));
        run(
            &mut s,
            Action::Close {
                id: id.clone(),
                cancelled: true,
                note: None,
            },
        );
        run(&mut s, Action::Reopen(id.clone()));
        assert_eq!(s.tasks[&id].status, "open");
        assert!(s.tasks[&id].closed_at.is_none() && s.tasks[&id].resolution.is_none());
    }
    #[test]
    fn blocking_discovery_completion_and_unsafe_reopen() {
        let (mut s, id) = fixture();
        run(&mut s, Action::Claim(id.clone()));
        let mut blocker = Task::new("discovered".into(), NOW);
        blocker.discovered_from = Some(id[2..10].into());
        let bid = blocker.id.clone();
        run(&mut s, Action::Create(Box::new(blocker)));
        assert_eq!(s.tasks[&bid].discovered_from.as_deref(), Some(id.as_str()));
        let result = run(
            &mut s,
            Action::Block {
                id: id.clone(),
                by: Some(bid[2..10].into()),
                reason: None,
            },
        );
        assert_eq!(result["released_claim"], "a");
        assert!(s.tasks[&id].claim.is_none());
        unchanged(
            &mut s,
            Action::Block {
                id: id.clone(),
                by: Some(bid.clone()),
                reason: None,
            },
        );
        unchanged(
            &mut s,
            Action::Block {
                id: bid.clone(),
                by: Some(id.clone()),
                reason: None,
            },
        );
        run(
            &mut s,
            Action::Block {
                id: id.clone(),
                by: None,
                reason: Some("credentials".into()),
            },
        );
        run(
            &mut s,
            Action::Unblock {
                id: id.clone(),
                by: None,
            },
        );
        assert_eq!(s.blocked_by(&s.tasks[&id]), vec![bid.clone()]);
        let dependent_before = s.tasks[&id].clone();
        run(
            &mut s,
            Action::Close {
                id: bid.clone(),
                cancelled: false,
                note: None,
            },
        );
        assert_eq!(s.tasks[&id], dependent_before);
        assert_eq!(s.effective(&s.tasks[&id]), "ready");
        unchanged(
            &mut s,
            Action::Block {
                id: id.clone(),
                by: Some(bid.clone()),
                reason: None,
            },
        );
        run(&mut s, Action::Claim(id.clone()));
        unchanged(&mut s, Action::Reopen(bid.clone()));
        run(
            &mut s,
            Action::Release {
                id: id.clone(),
                force: false,
                reason: None,
            },
        );
        run(&mut s, Action::Reopen(bid.clone()));
        run(
            &mut s,
            Action::Close {
                id: bid.clone(),
                cancelled: true,
                note: None,
            },
        );
        assert_eq!(s.effective(&s.tasks[&id]), "blocked");
        run(
            &mut s,
            Action::Unblock {
                id: id.clone(),
                by: Some(bid),
            },
        );
        assert_eq!(s.effective(&s.tasks[&id]), "ready");
    }
    #[test]
    fn metadata_parent_validation_and_preserved_provenance() {
        let (mut s, id) = fixture();
        let mut child = Task::new("child".into(), NOW);
        child.parent = Some(id[2..10].into());
        child.discovered_from = Some(id.clone());
        let cid = child.id.clone();
        run(&mut s, Action::Create(Box::new(child)));
        let patch = Update {
            title: Some("fixed".into()),
            kind: Some("bug".into()),
            priority: Some("P1".into()),
            description: Some("body".into()),
            acceptance: Some(vec!["success".into(), "failure".into()]),
            labels: Some(vec!["z".into(), "a".into()]),
            ..Default::default()
        };
        run(
            &mut s,
            Action::Update {
                id: cid.clone(),
                patch,
            },
        );
        assert_eq!(s.tasks[&cid].labels, vec!["a", "z"]);
        assert_eq!(s.tasks[&cid].acceptance, vec!["success", "failure"]);
        assert_eq!(s.tasks[&cid].kind, "bug");
        assert_eq!(s.tasks[&cid].discovered_from.as_deref(), Some(id.as_str()));
        unchanged(
            &mut s,
            Action::Update {
                id: id.clone(),
                patch: Update {
                    parent: Some(Some(cid.clone())),
                    ..Default::default()
                },
            },
        );
        unchanged(
            &mut s,
            Action::Update {
                id: cid.clone(),
                patch: Update {
                    title: Some(" ".into()),
                    ..Default::default()
                },
            },
        );
        for target in [cid.clone(), "t-00000000000000000000".into()] {
            unchanged(
                &mut s,
                Action::Block {
                    id: cid.clone(),
                    by: Some(target),
                    reason: None,
                },
            );
        }
        run(
            &mut s,
            Action::Update {
                id: cid.clone(),
                patch: Update {
                    parent: Some(None),
                    ..Default::default()
                },
            },
        );
        assert!(s.tasks[&cid].parent.is_none());
    }
}
