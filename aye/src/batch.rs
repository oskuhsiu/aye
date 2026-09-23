//! Ordered, bounded task mutations published as one state transition.
use crate::domain::{self, Action, Update};
use crate::error::{Error, Result};
use crate::model::Task;
use crate::store::{Mutation, Store};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;

const MAX_BYTES: u64 = 1024 * 1024;
const MAX_OPERATIONS: usize = 100;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    version: u32,
    expected_state_oid: Option<String>,
    operations: Vec<Value>,
}
struct Prepared {
    action: Action,
    op: String,
    id: String,
    alias: Option<String>,
}
fn contextual(error: Error, index: usize, alias: Option<&str>) -> Error {
    let code = error.code;
    error.with_details(json!({"op_index": index, "alias": alias, "underlying_code": code}))
}
fn string(map: &Map<String, Value>, field: &str) -> Result<Option<String>> {
    map.get(field)
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .ok_or_else(|| Error::usage(format!("{field} must be a string")))
        })
        .transpose()
}
fn required(map: &Map<String, Value>, field: &str) -> Result<String> {
    string(map, field)?.ok_or_else(|| Error::usage(format!("Missing {field}")))
}
fn strings(map: &Map<String, Value>, field: &str) -> Result<Option<Vec<String>>> {
    map.get(field)
        .map(|v| {
            serde_json::from_value(v.clone())
                .map_err(|_| Error::usage(format!("{field} must be an array of strings")))
        })
        .transpose()
}
fn reference(value: &Value, aliases: &BTreeMap<String, String>) -> Result<String> {
    if let Some(id) = value.as_str() {
        if id.len() == 22
            && id.starts_with("t-")
            && id.as_bytes()[2..]
                .iter()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b))
        {
            return Ok(id.into());
        }
        return Err(Error::usage("Batch references require full task IDs"));
    }
    if let Some(map) = value.as_object()
        && map.len() == 1
        && let Some(alias) = map.get("local").and_then(Value::as_str)
    {
        return aliases
            .get(alias)
            .cloned()
            .ok_or_else(|| Error::usage(format!("Unknown or forward local alias: {alias}")));
    }
    Err(Error::usage(
        "Reference must be a full task ID or {\"local\":\"name\"}",
    ))
}
fn relation(
    map: &Map<String, Value>,
    field: &str,
    aliases: &BTreeMap<String, String>,
) -> Result<Option<String>> {
    map.get(field).map(|v| reference(v, aliases)).transpose()
}
fn prepare(value: &Value, aliases: &mut BTreeMap<String, String>, now: &str) -> Result<Prepared> {
    let map = value
        .as_object()
        .ok_or_else(|| Error::usage("Operation must be an object"))?;
    let op = required(map, "op")?;
    let allowed: &[&str] = match op.as_str() {
        "create" => &[
            "op",
            "as",
            "title",
            "type",
            "priority",
            "description",
            "acceptance",
            "labels",
            "parent",
            "discovered_from",
            "depends_on",
        ],
        "update" => &[
            "op",
            "id",
            "title",
            "type",
            "priority",
            "description",
            "acceptance",
            "labels",
            "parent",
        ],
        "note" => &["op", "id", "body"],
        "block" => &["op", "id", "by", "reason"],
        "unblock" => &["op", "id", "by"],
        "close" => &["op", "id", "cancelled", "note"],
        "cancel" => &["op", "id", "note"],
        "claim" | "release" | "defer" | "resume" | "reopen" => &["op", "id"],
        _ => return Err(Error::usage(format!("Unknown batch operation: {op}"))),
    };
    for field in map.keys() {
        if !allowed.contains(&field.as_str()) {
            return Err(Error::usage(format!("Unknown {op} field: {field}")));
        }
    }
    let alias = string(map, "as")?;
    if alias
        .as_ref()
        .is_some_and(|a| a.trim().is_empty() || aliases.contains_key(a))
    {
        return Err(Error::usage("Create aliases must be nonempty and unique"));
    }
    if op == "create" {
        let mut task = Task::new(required(map, "title")?, now);
        if let Some(v) = string(map, "type")? {
            task.kind = v;
        }
        if let Some(v) = string(map, "priority")? {
            task.priority = v;
        }
        if let Some(v) = string(map, "description")? {
            task.description = v;
        }
        task.acceptance = strings(map, "acceptance")?.unwrap_or_default();
        task.labels = strings(map, "labels")?.unwrap_or_default();
        task.parent = relation(map, "parent", aliases)?;
        task.discovered_from = relation(map, "discovered_from", aliases)?;
        if let Some(v) = map.get("depends_on") {
            task.depends_on = v
                .as_array()
                .ok_or_else(|| Error::usage("depends_on must be an array"))?
                .iter()
                .map(|v| reference(v, aliases))
                .collect::<Result<_>>()?;
        }
        let id = task.id.clone();
        if let Some(alias) = &alias {
            aliases.insert(alias.clone(), id.clone());
        }
        return Ok(Prepared {
            action: Action::Create(Box::new(task)),
            op,
            id,
            alias,
        });
    }
    let id = relation(map, "id", aliases)?.ok_or_else(|| Error::usage("Missing id"))?;
    let action = match op.as_str() {
        "update" => Action::Update {
            id: id.clone(),
            patch: Update {
                title: string(map, "title")?,
                kind: string(map, "type")?,
                priority: string(map, "priority")?,
                description: string(map, "description")?,
                acceptance: strings(map, "acceptance")?,
                labels: strings(map, "labels")?,
                parent: map
                    .get("parent")
                    .map(|v| {
                        if v.is_null() {
                            Ok(None)
                        } else {
                            reference(v, aliases).map(Some)
                        }
                    })
                    .transpose()?,
            },
        },
        "note" => Action::Note {
            id: id.clone(),
            body: required(map, "body")?,
        },
        "block" => Action::Block {
            id: id.clone(),
            by: relation(map, "by", aliases)?,
            reason: string(map, "reason")?,
        },
        "unblock" => Action::Unblock {
            id: id.clone(),
            by: relation(map, "by", aliases)?,
        },
        "close" | "cancel" => Action::Close {
            id: id.clone(),
            cancelled: op == "cancel"
                || match map.get("cancelled") {
                    None => false,
                    Some(v) => v
                        .as_bool()
                        .ok_or_else(|| Error::usage("cancelled must be a boolean"))?,
                },
            note: string(map, "note")?,
        },
        "claim" => Action::Claim(id.clone()),
        "release" => Action::Release {
            id: id.clone(),
            force: false,
            reason: None,
        },
        "defer" => Action::Defer(id.clone()),
        "resume" => Action::Resume(id.clone()),
        "reopen" => Action::Reopen(id.clone()),
        _ => unreachable!(),
    };
    let alias = map
        .get("id")
        .and_then(|v| v.get("local"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    Ok(Prepared {
        action,
        op,
        id,
        alias,
    })
}

pub fn execute(store: &Store, path: &str, actor: Option<&str>, now: &str) -> Result<Value> {
    let input: Box<dyn Read> = if path == "-" {
        Box::new(std::io::stdin())
    } else {
        Box::new(std::fs::File::open(path)?)
    };
    let mut bytes = Vec::new();
    input.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(Error::usage("Batch request exceeds 1 MiB"));
    }
    let request: Request = serde_json::from_slice(&bytes)
        .map_err(|e| Error::usage(format!("Invalid batch request: {e}")))?;
    if request.version != 1 {
        return Err(Error::usage("Only batch version 1 is supported"));
    }
    if request.operations.is_empty() || request.operations.len() > MAX_OPERATIONS {
        return Err(Error::usage("Batch requires between 1 and 100 operations"));
    }
    if request.expected_state_oid.as_ref().is_some_and(|oid| {
        oid.len() != 40
            || !oid
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }) {
        return Err(Error::usage(
            "expected_state_oid must be a full lowercase state OID",
        ));
    }
    let mut aliases = BTreeMap::new();
    let operations = request
        .operations
        .iter()
        .enumerate()
        .map(|(index, v)| {
            let alias = v.get("as").and_then(Value::as_str).or_else(|| {
                v.get("id")
                    .and_then(|v| v.get("local"))
                    .and_then(Value::as_str)
            });
            prepare(v, &mut aliases, now).map_err(|e| contextual(e, index, alias))
        })
        .collect::<Result<Vec<_>>>()?;
    let receipt = store.transact(|snapshot, state| {
        if request.expected_state_oid.as_ref().is_some_and(|oid| oid != &snapshot.oid) {
            return Err(Error::new("STALE_STATE", "Batch expected_state_oid does not match current state")
                .with_details(json!({"expected_state_oid": request.expected_state_oid, "observed_state_oid": snapshot.oid})));
        }
        let before = state.clone();
        let mut outcomes = Vec::new();
        let mut touched = BTreeSet::new();
        for (index, operation) in operations.iter().enumerate() {
            let result = domain::apply(state, &operation.action, actor, now)
                .map_err(|e| contextual(e, index, operation.alias.as_deref()))?;
            let mut outcome = json!({"op_index": index, "op": operation.op, "id": operation.id,
                "alias": operation.alias, "outcome": "applied"});
            if let Some(released) = result.get("released_claim") { outcome["released_claim"] = released.clone(); }
            outcomes.push(outcome);
            touched.insert(operation.id.clone());
        }
        let tasks = touched.iter().map(|id| {
            let task = &state.tasks[id];
            let original = before.tasks.get(id);
            let old = original.map(|t| serde_json::to_value(t).expect("Task serializes"));
            let mut final_task = serde_json::to_value(task).expect("Task serializes");
            final_task.as_object_mut().unwrap().remove("notes");
            let changes: Map<String, Value> = final_task.as_object().unwrap().iter()
                .filter(|(key, value)| old.as_ref().and_then(|v| v.get(*key)) != Some(*value))
                .map(|(key, value)| (key.clone(), value.clone())).collect();
            let prior_notes = original.map_or(0, |t| t.notes.len());
            json!({"task": final_task, "computed": {"effective_state": state.effective(task),
                "blocked_by": state.blocked_by(task)}, "changes": changes, "added_notes": &task.notes[prior_notes..]})
        }).collect::<Vec<_>>();
        Ok(Mutation::Write(json!({"version": 1, "aliases": aliases, "operations": outcomes, "tasks": tasks})))
    })?;
    let mut result = receipt.value;
    result["state_oid"] = json!(receipt.oid);
    Ok(result)
}
