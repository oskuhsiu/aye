use crate::error::Result;
use crate::is_bypassed;
use crate::model::{State, Task};
use crate::store::{self, Files, Snapshot};
use serde_json::{Value, json};

fn ordered(state: &State) -> Vec<&Task> {
    let mut tasks: Vec<_> = state.tasks.values().collect();
    tasks.sort_by(|a, b| {
        (&a.priority, &a.created_at, &a.id).cmp(&(&b.priority, &b.created_at, &b.id))
    });
    tasks
}
pub fn ready(state: &State) -> Vec<&Task> {
    ordered(state)
        .into_iter()
        .filter(|t| state.effective(t) == "ready")
        .collect()
}
fn compact(task: &Task) -> Value {
    json!({"id":task.id,"title":task.title,"type":task.kind,"priority":task.priority,"labels":task.labels,"parent":task.parent,"created_at":task.created_at,"updated_at":task.updated_at})
}
pub fn show(state: &State, task: &Task) -> Value {
    let blocked_by = state.blocked_by(task);
    let blocking_tasks:Vec<_>=blocked_by.iter().filter_map(|id|state.tasks.get(id)).map(|t|json!({"id":t.id,"title":t.title,"status":t.status,"resolution":t.resolution,"effective_state":state.effective(t),"bypassed":is_bypassed(t)})).collect();
    let children: Vec<_> = state
        .tasks
        .values()
        .filter(|t| t.parent.as_deref() == Some(task.id.as_str()))
        .map(|t| &t.id)
        .collect();
    let blocks: Vec<_> = state
        .tasks
        .values()
        .filter(|t| t.depends_on.contains(&task.id))
        .map(|t| &t.id)
        .collect();
    let discovered: Vec<_> = state
        .tasks
        .values()
        .filter(|t| t.discovered_from.as_deref() == Some(task.id.as_str()))
        .map(|t| &t.id)
        .collect();
    json!({"task":task,"computed":{"effective_state":state.effective(task),"bypassed":is_bypassed(task),"blocked_by":blocked_by,"blocking_tasks":blocking_tasks,"children":children,"blocks":blocks,"discovered":discovered}})
}
fn counts(state: &State) -> Value {
    let mut counts = json!({"total":state.tasks.len(),"ready":0,"blocked":0,"in_progress":0,"deferred":0,"closed_done":0,"closed_cancelled":0,"bypassed":0});
    for task in state.tasks.values() {
        if is_bypassed(task) {
            counts["bypassed"] = json!(counts["bypassed"].as_u64().unwrap() + 1);
        }
        let key = match state.effective(task) {
            "closed" if task.resolution.as_deref() == Some("done") => "closed_done",
            "closed" => "closed_cancelled",
            other => other,
        };
        counts[key] = json!(counts[key].as_u64().unwrap() + 1);
    }
    counts
}
pub fn build(state: &State, tasks_tree_oid: &str) -> Result<Files> {
    state.validate()?;
    let mut files = Files::new();
    let mut ready_bytes = Vec::new();
    let mut active_bytes = Vec::new();
    for task in ordered(state) {
        if state.effective(task) == "ready" {
            ready_bytes.extend(serde_json::to_vec(&compact(task))?);
            ready_bytes.push(b'\n');
        }
        if task.status != "closed" {
            let mut row = compact(task);
            let object = row.as_object_mut().unwrap();
            object.insert("status".into(), json!(task.status));
            object.insert("effective_state".into(), json!(state.effective(task)));
            object.insert("claim".into(), json!(task.claim));
            object.insert("manual_block".into(), json!(task.manual_block));
            object.insert("blocked_by".into(), json!(state.blocked_by(task)));
            active_bytes.extend(serde_json::to_vec(&row)?);
            active_bytes.push(b'\n');
        }
    }
    files.insert("views/ready.jsonl".into(), ready_bytes);
    files.insert("views/active.jsonl".into(), active_bytes);
    files.insert(
        "manifest.json".into(),
        store::json_bytes(
            &json!({"projection_version":1,"tasks_tree_oid":tasks_tree_oid,"counts":counts(state)}),
        )?,
    );
    files.insert("REPORT.md".into(), report(state).into_bytes());
    files.insert("FORMAT.md".into(), include_bytes!("FORMAT.md").to_vec());
    Ok(files)
}
pub fn fresh(snapshot: &Snapshot) -> bool {
    build(&snapshot.state, &snapshot.tasks_tree_oid).is_ok_and(|expected| {
        expected
            .iter()
            .all(|(path, bytes)| snapshot.files.get(path) == Some(bytes))
    })
}
fn escaped(text: &str) -> String {
    let mut result = String::new();
    for c in text.chars() {
        match c {
            '&' => result.push_str("&amp;"),
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '\n' | '\r' | '\t' => result.push(' '),
            '\\' | '`' | '*' | '_' | '{' | '}' | '[' | ']' | '(' | ')' | '#' | '+' | '-' | '.'
            | '!' | '|' => {
                result.push('\\');
                result.push(c);
            }
            c if c.is_control() => {}
            c => result.push(c),
        }
    }
    result
}
pub fn report(state: &State) -> String {
    let mut out = String::from("# Aye Task Report\n\n## Summary\n\n");
    let c = counts(state);
    for key in [
        "total",
        "ready",
        "blocked",
        "in_progress",
        "deferred",
        "closed_done",
        "closed_cancelled",
        "bypassed",
    ] {
        out.push_str(&format!("- {key}: {}\n", c[key]));
    }
    let all = ordered(state);
    let mut closed: Vec<_> = state
        .tasks
        .values()
        .filter(|t| t.status == "closed")
        .collect();
    closed.sort_by(|a, b| b.closed_at.cmp(&a.closed_at).then_with(|| a.id.cmp(&b.id)));
    closed.truncate(20);
    let mut bypassed: Vec<_> = state.tasks.values().filter(|t| is_bypassed(t)).collect();
    bypassed.sort_by(|a, b| b.closed_at.cmp(&a.closed_at).then_with(|| a.id.cmp(&b.id)));
    bypassed.truncate(20);
    let mut discovered: Vec<_> = state
        .tasks
        .values()
        .filter(|t| t.discovered_from.is_some())
        .collect();
    discovered.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| a.id.cmp(&b.id))
    });
    discovered.truncate(20);
    let mut sections: Vec<(&str, Vec<&Task>)> = [
        ("In Progress", "in_progress"),
        ("Ready", "ready"),
        ("Blocked", "blocked"),
        ("Deferred", "deferred"),
    ]
    .into_iter()
    .map(|(title, effective)| {
        (
            title,
            all.iter()
                .copied()
                .filter(|t| state.effective(t) == effective)
                .collect(),
        )
    })
    .collect();
    sections.push(("Last 20 Bypassed", bypassed));
    sections.push(("Last 20 Closed", closed));
    sections.push(("Last 20 Discovered", discovered));
    for (title, tasks) in sections {
        out.push_str(&format!("\n## {title}\n\n"));
        if tasks.is_empty() {
            out.push_str("None.\n");
        }
        for task in tasks {
            out.push_str(&format!(
                "- {} [{}] {}",
                task.id,
                task.priority,
                escaped(&task.title)
            ));
            if let Some(claim) = &task.claim {
                out.push_str(&format!(" — claimed by {}", escaped(&claim.actor)));
            }
            if let Some(block) = &task.manual_block {
                out.push_str(&format!(" — manual block: {}", escaped(&block.reason)));
            }
            let blockers = state.blocked_by(task);
            if !blockers.is_empty() {
                out.push_str(" — prerequisites: ");
                for (i, id) in blockers.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    out.push_str(id);
                    if let Some(t) = state.tasks.get(id) {
                        out.push_str(&format!(
                            " ({}, {})",
                            escaped(&t.title),
                            if is_bypassed(t) {
                                "bypassed"
                            } else {
                                t.resolution.as_deref().unwrap_or(&t.status)
                            }
                        ));
                    }
                }
            }
            if is_bypassed(task) {
                out.push_str(" — bypassed");
            } else if let Some(resolution) = &task.resolution {
                out.push_str(&format!(" — {resolution}"));
            }
            if let Some(origin) = &task.discovered_from {
                out.push_str(&format!(" — discovered from {origin}"));
            }
            out.push('\n');
        }
    }
    out
}
pub fn warnings(state: &State) -> Vec<Value> {
    let mut result = Vec::new();
    for task in state.tasks.values() {
        if serde_json::to_vec(task).is_ok_and(|bytes| bytes.len() > 256 * 1024) {
            result.push(
                json!({"code":"LARGE_TASK","message":"Task exceeds 256 KiB","task_id":task.id}),
            );
        }
        if task.status == "closed" {
            let children: Vec<_> = state
                .tasks
                .values()
                .filter(|child| {
                    child.status != "closed" && child.parent.as_deref() == Some(task.id.as_str())
                })
                .map(|child| &child.id)
                .collect();
            if !children.is_empty() {
                result.push(json!({"code":"CLOSED_PARENT_OPEN_CHILDREN","message":"Closed parent has non-closed children","task_id":task.id,"children":children}));
            }
        }
        let cancelled: Vec<_> = task
            .depends_on
            .iter()
            .filter(|id| {
                state.tasks.get(*id).is_some_and(|t| {
                    t.status == "closed" && t.resolution.as_deref() == Some("cancelled")
                })
            })
            .collect();
        if !cancelled.is_empty() {
            result.push(json!({"code":"CANCELLED_PREREQUISITE","message":"Cancelled prerequisites remain unsatisfied","task_id":task.id,"prerequisites":cancelled}));
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BYPASSED_LABEL;
    use crate::model::ManualBlock;
    const NOW: &str = "2026-09-17T03:10:00.000Z";
    fn task(n: usize) -> Task {
        let mut t = Task::new(format!("Task {n}"), NOW);
        t.id = format!("t-{n:020x}");
        t
    }
    #[test]
    fn readiness_order_counts_and_relations_are_canonical() {
        let mut s = State::empty();
        let mut a = task(1);
        a.priority = "P3".into();
        let mut b = task(2);
        b.priority = "P0".into();
        let mut c = task(3);
        c.depends_on = vec![a.id.clone()];
        c.parent = Some(b.id.clone());
        c.discovered_from = Some(b.id.clone());
        c.manual_block = Some(ManualBlock {
            actor: "a".into(),
            reason: "waiting".into(),
            blocked_at: NOW.into(),
        });
        for t in [a.clone(), b.clone(), c.clone()] {
            s.tasks.insert(t.id.clone(), t);
        }
        assert_eq!(
            ready(&s).iter().map(|t| &t.id).collect::<Vec<_>>(),
            vec![&b.id, &a.id]
        );
        let files = build(&s, "oid").unwrap();
        let manifest: Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
        assert_eq!(manifest["counts"]["ready"], 2);
        assert_eq!(manifest["counts"]["blocked"], 1);
        assert_eq!(manifest["counts"]["bypassed"], 0);
        assert_eq!(manifest["counts"]["total"], 3);
        let detail = show(&s, &c);
        assert_eq!(detail["computed"]["blocking_tasks"][0]["id"], a.id);
        assert_eq!(show(&s, &a)["computed"]["blocks"], json!([c.id]));
        assert_eq!(show(&s, &b)["computed"]["children"], json!([c.id]));
        assert_eq!(show(&s, &b)["computed"]["discovered"], json!([c.id]));
    }
    #[test]
    fn bypass_is_visible_without_changing_dependency_resolution() {
        let mut s = State::empty();
        let mut bypassed = task(1);
        bypassed.status = "closed".into();
        bypassed.resolution = Some("done".into());
        bypassed.closed_at = Some(NOW.into());
        bypassed.labels.push(BYPASSED_LABEL.into());
        s.tasks.insert(bypassed.id.clone(), bypassed.clone());
        let files = build(&s, "oid").unwrap();
        let manifest: Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
        assert_eq!(manifest["counts"]["closed_done"], 1);
        assert_eq!(manifest["counts"]["bypassed"], 1);
        assert_eq!(show(&s, &bypassed)["computed"]["bypassed"], true);
        let report = report(&s);
        assert!(report.contains("## Last 20 Bypassed"));
        assert!(report.contains("— bypassed"));
    }
    #[test]
    fn report_limits_orders_ties_and_escapes_user_content() {
        let mut s = State::empty();
        let origin = task(100);
        s.tasks.insert(origin.id.clone(), origin.clone());
        for n in 1..=23 {
            let mut t = task(n);
            t.status = "closed".into();
            t.resolution = Some("done".into());
            t.closed_at = Some(NOW.into());
            t.discovered_from = Some(origin.id.clone());
            if n == 1 {
                t.title = "<script>x</script>\n# injected | row".into();
            }
            s.tasks.insert(t.id.clone(), t);
        }
        let report = report(&s);
        let closed = report
            .split("## Last 20 Closed\n")
            .nth(1)
            .unwrap()
            .split("## Last 20 Discovered\n")
            .next()
            .unwrap();
        let discovered = report.split("## Last 20 Discovered\n").nth(1).unwrap();
        for section in [closed, discovered] {
            assert_eq!(section.lines().filter(|l| l.starts_with("- ")).count(), 20);
            assert!(section.contains(&task(20).id));
            assert!(!section.contains(&task(21).id));
            assert!(section.find(&task(1).id).unwrap() < section.find(&task(2).id).unwrap());
        }
        assert!(!report.contains("<script>"));
        assert!(!report.contains("\n# injected"));
        assert_eq!(build(&s, "oid").unwrap(), build(&s, "oid").unwrap());
    }
    #[test]
    fn freshness_rejects_wrong_version_oid_missing_and_corrupt_views() {
        let s = State::empty();
        let files = build(&s, "oid").unwrap();
        let snapshot = Snapshot {
            oid: "commit".into(),
            state: s,
            files,
            tasks_tree_oid: "oid".into(),
        };
        assert!(fresh(&snapshot));
        for path in [
            "manifest.json",
            "views/ready.jsonl",
            "views/active.jsonl",
            "REPORT.md",
            "FORMAT.md",
        ] {
            let mut bad = snapshot.clone();
            bad.files.remove(path);
            assert!(!fresh(&bad));
            let mut bad = snapshot.clone();
            bad.files.insert(path.into(), b"corrupt".to_vec());
            assert!(!fresh(&bad));
        }
        for (key, value) in [
            ("projection_version", json!(2)),
            ("tasks_tree_oid", json!("different")),
        ] {
            let mut bad = snapshot.clone();
            let mut m: Value = serde_json::from_slice(&bad.files["manifest.json"]).unwrap();
            m[key] = value;
            bad.files
                .insert("manifest.json".into(), serde_json::to_vec(&m).unwrap());
            assert!(!fresh(&bad));
        }
    }
    #[test]
    fn doctor_warns_for_each_defined_nonfatal_condition() {
        let mut s = State::empty();
        let mut a = task(1);
        a.status = "closed".into();
        a.resolution = Some("cancelled".into());
        a.closed_at = Some(NOW.into());
        let mut b = task(2);
        b.parent = Some(a.id.clone());
        b.depends_on = vec![a.id.clone()];
        b.description = "x".repeat(256 * 1024);
        for t in [a, b] {
            s.tasks.insert(t.id.clone(), t);
        }
        s.validate().unwrap();
        let w = warnings(&s);
        assert_eq!(w.len(), 3);
        for code in [
            "LARGE_TASK",
            "CLOSED_PARENT_OPEN_CHILDREN",
            "CANCELLED_PREREQUISITE",
        ] {
            assert!(w.iter().any(|v| v["code"] == code));
        }
        assert!(warnings(&State::empty()).is_empty());
    }
}
