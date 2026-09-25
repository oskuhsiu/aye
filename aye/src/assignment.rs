//! A single transaction selects, owns, and describes one task.
use crate::domain::{self, Action};
use crate::error::{Error, Result};
use crate::is_bypassed;
use crate::model::{State, Task};
use crate::projection;
use crate::store::{Mutation, Store};
use clap::Args;
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;

const PACKET_BYTES: usize = 64 * 1024;
const BRIEF_LIMIT: usize = 20;

#[derive(Args, Default, Serialize)]
pub struct Filters {
    #[arg(long, value_parser = ["P0", "P1", "P2", "P3", "P4"])]
    priority: Option<String>,
    #[arg(long = "type", value_parser = ["task", "bug", "feature", "chore"])]
    #[serde(rename = "type")]
    kind: Option<String>,
    #[arg(long)]
    label: Option<String>,
}
impl Filters {
    pub fn matches(&self, task: &Task) -> bool {
        self.priority.as_ref().is_none_or(|v| v == &task.priority)
            && self.kind.as_ref().is_none_or(|v| v == &task.kind)
            && self.label.as_ref().is_none_or(|v| task.labels.contains(v))
    }
    pub fn is_empty(&self) -> bool {
        self.priority.is_none() && self.kind.is_none() && self.label.is_none()
    }
}

pub fn execute(
    store: &Store,
    explicit: Option<&str>,
    filters: &Filters,
    actor: Option<&str>,
    now: &str,
) -> Result<(Value, Vec<Value>)> {
    let actor = actor.filter(|a| !a.trim().is_empty()).ok_or_else(|| {
        Error::new(
            "ACTOR_REQUIRED",
            "Set --actor, AYE_ACTOR, or a worktree actor",
        )
    })?;
    let receipt = store.transact(|old, state| {
        let owned: Vec<_> = state.tasks.values()
            .filter(|t| t.claim.as_ref().is_some_and(|c| c.actor == actor))
            .map(|t| t.id.clone()).collect();
        let (outcome, id) = if let Some(input) = explicit {
            ("claimed", Some(state.resolve(input)?))
        } else if owned.len() == 1 && filters.matches(&state.tasks[&owned[0]]) {
            ("already_owned", Some(owned[0].clone()))
        } else if !owned.is_empty() {
            ("existing_assignments", None)
        } else {
            match projection::ready(state).into_iter().find(|t| filters.matches(t)) {
                Some(task) => ("claimed", Some(task.id.clone())),
                None => ("no_ready_task", None),
            }
        };
        let writes = outcome == "claimed";
        if writes {
            domain::apply(state, &Action::Claim(id.clone().expect("selected task")), Some(actor), now)?;
        }
        let warnings = if !writes && !projection::fresh(old) {
            vec![json!({"code":"VIEW_STALE","message":"Computed from canonical state; run aye rebuild to repair stored views"})]
        } else { vec![] };
        // The OID has the same serialized width before and after publication.
        // Build and bound everything before allowing the store to publish.
        let packet = packet(state, &old.oid, actor, filters, outcome, id.as_deref(), &warnings)?;
        let value = (packet, warnings);
        Ok(if writes { Mutation::Write(value) } else { Mutation::ReadOnly(value) })
    })?;
    let (mut packet, warnings) = receipt.value;
    packet["state_oid"] = json!(receipt.oid);
    Ok((packet, warnings))
}

fn brief(state: &State, task: &Task) -> Value {
    json!({"id":task.id,"title":task.title,"status":task.status,
        "resolution":task.resolution,"effective_state":state.effective(task),
        "bypassed":is_bypassed(task),"claim_owner":task.claim.as_ref().map(|c| &c.actor),
        "blocked_by":state.blocked_by(task),"manual_block":task.manual_block})
}
fn section(total: usize) -> Value {
    json!({"items":[],"total":total,"returned":0,"omitted":total})
}
fn envelope_size(packet: &Value, warnings: &[Value]) -> Result<usize> {
    Ok(serde_json::to_vec(&json!({"ok":true,"data":packet,"warnings":warnings}))?.len() + 1)
}
fn push_if_fits(packet: &mut Value, pointer: &str, item: Value, warnings: &[Value]) -> Result<()> {
    let section = packet.pointer_mut(pointer).expect("packet section");
    if section["returned"].as_u64().unwrap() >= BRIEF_LIMIT as u64 {
        return Ok(());
    }
    section["items"].as_array_mut().unwrap().push(item);
    section["returned"] = json!(section["returned"].as_u64().unwrap() + 1);
    section["omitted"] = json!(section["omitted"].as_u64().unwrap() - 1);
    if envelope_size(packet, warnings)? > PACKET_BYTES {
        let section = packet.pointer_mut(pointer).unwrap();
        section["items"].as_array_mut().unwrap().pop();
        section["returned"] = json!(section["returned"].as_u64().unwrap() - 1);
        section["omitted"] = json!(section["omitted"].as_u64().unwrap() + 1);
    }
    Ok(())
}

fn related(state: &State, task: &Task) -> Vec<Value> {
    // Category order is part of the contract; within a category sort full IDs.
    let mut relations: BTreeMap<&str, Vec<(usize, &str)>> = BTreeMap::new();
    let mut add = |id: &str, rank: usize, label: &'static str| {
        if id != task.id
            && let Some((id, _)) = state.tasks.get_key_value(id)
        {
            relations.entry(id).or_default().push((rank, label));
        }
    };
    for id in &task.depends_on {
        add(id, 0, "prerequisite");
    }
    for other in state.tasks.values() {
        if other.depends_on.contains(&task.id) {
            add(&other.id, 1, "dependent");
        }
    }
    if let Some(id) = &task.parent {
        add(id, 2, "parent");
    }
    for other in state.tasks.values() {
        if other.parent.as_ref() == Some(&task.id) {
            add(&other.id, 3, "child");
        }
    }
    if let Some(id) = &task.discovered_from {
        add(id, 4, "discovered_from");
    }
    for other in state.tasks.values() {
        if other.discovered_from.as_ref() == Some(&task.id) {
            add(&other.id, 5, "discovered");
        }
    }
    let mut entries: Vec<_> = relations.into_iter().collect();
    entries.sort_by_key(|(id, labels)| (labels[0].0, *id));
    entries
        .into_iter()
        .map(|(id, labels)| {
            let mut value = brief(state, &state.tasks[id]);
            value["relations"] = json!(
                labels
                    .into_iter()
                    .map(|(_, label)| label)
                    .collect::<Vec<_>>()
            );
            value
        })
        .collect()
}

fn packet(
    state: &State,
    oid: &str,
    actor: &str,
    filters: &Filters,
    outcome: &str,
    id: Option<&str>,
    warnings: &[Value],
) -> Result<Value> {
    let matching: Vec<_> = state
        .tasks
        .values()
        .filter(|t| filters.matches(t))
        .collect();
    let mut counts = json!({"total":matching.len(),"ready":0,"blocked":0,"in_progress":0,
        "deferred":0,"closed_done":0,"closed_cancelled":0,"bypassed":0});
    for task in &matching {
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
    let assigned = id.map(|id| &state.tasks[id]);
    let shown = assigned.map(|t| projection::show(state, t));
    let related = assigned.map(|t| related(state, t)).unwrap_or_default();
    let owned: Vec<_> = if outcome == "existing_assignments" {
        state
            .tasks
            .values()
            .filter(|t| t.claim.as_ref().is_some_and(|c| c.actor == actor))
            .map(|t| brief(state, t))
            .collect()
    } else {
        vec![]
    };
    let explanations: Vec<_> = if outcome == "no_ready_task" {
        matching
            .iter()
            .filter(|t| t.status != "closed")
            .map(|t| brief(state, t))
            .collect()
    } else {
        vec![]
    };
    let reason = match outcome {
        "no_ready_task" if state.tasks.is_empty() => Some("empty_project"),
        "no_ready_task" if matching.is_empty() => Some("no_matching_tasks"),
        "no_ready_task" => Some("no_ready_match"),
        "existing_assignments" if owned.len() > 1 => Some("multiple_owned_tasks"),
        "existing_assignments" => Some("owned_task_outside_scope"),
        _ => None,
    };
    let mut data = json!({"version":1,"outcome":outcome,"state_oid":oid,
        "project_id":state.project.project_id,"actor":actor,"filters":filters,
        "task":shown.as_ref().map(|v| &v["task"]),"computed":shown.as_ref().map(|v| &v["computed"]),
        "related":section(related.len()),"existing_assignments":section(owned.len()),
        "situation":{"project_total":state.tasks.len(),"matching":counts,"reason":reason,
            "explanations":section(explanations.len())},
        "limits":{"packet_bytes":PACKET_BYTES,"briefs_per_section":BRIEF_LIMIT}});
    if envelope_size(&data, warnings)? > PACKET_BYTES {
        return Err(Error::new("PACKET_TOO_LARGE",
            "Complete task/context exceeds 64 KiB; no new claim published. Use read-only show/list with sufficient output allowance; retain any existing claim.")
            .with_details(json!({"task_id":id,"actor":actor,"already_owned":outcome == "already_owned",
                "new_claim_published":false,"limit_bytes":PACKET_BYTES})));
    }
    for item in related {
        push_if_fits(&mut data, "/related", item, warnings)?;
    }
    for item in owned {
        push_if_fits(&mut data, "/existing_assignments", item, warnings)?;
    }
    for item in explanations {
        push_if_fits(&mut data, "/situation/explanations", item, warnings)?;
    }
    Ok(data)
}
