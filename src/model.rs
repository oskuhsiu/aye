use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub format: String,
    pub format_version: u32,
    pub project_id: String,
}
#[derive(Clone, Debug)]
pub struct State {
    pub project: Project,
    pub tasks: BTreeMap<String, Task>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Claim {
    pub actor: String,
    pub claimed_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ManualBlock {
    pub reason: String,
    pub actor: String,
    pub blocked_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Note {
    pub actor: String,
    pub created_at: String,
    pub body: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Task {
    pub schema: u32,
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub priority: String,
    pub status: String,
    #[serde(deserialize_with = "required_nullable")]
    pub resolution: Option<String>,
    pub description: String,
    pub acceptance: Vec<String>,
    pub labels: Vec<String>,
    #[serde(deserialize_with = "required_nullable")]
    pub claim: Option<Claim>,
    #[serde(deserialize_with = "required_nullable")]
    pub manual_block: Option<ManualBlock>,
    pub depends_on: Vec<String>,
    #[serde(deserialize_with = "required_nullable")]
    pub parent: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    pub discovered_from: Option<String>,
    pub notes: Vec<Note>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(deserialize_with = "required_nullable")]
    pub closed_at: Option<String>,
}
impl Task {
    pub fn new(title: String, now: &str) -> Self {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        // The first six bytes avoid UUID version and variant bits.
        let payload: String = a.as_bytes()[..6]
            .iter()
            .chain(&b.as_bytes()[..4])
            .map(|v| format!("{v:02x}"))
            .collect();
        Self {
            schema: 1,
            id: format!("t-{payload}"),
            title,
            kind: "task".into(),
            priority: "P2".into(),
            status: "open".into(),
            resolution: None,
            description: String::new(),
            acceptance: vec![],
            labels: vec![],
            claim: None,
            manual_block: None,
            depends_on: vec![],
            parent: None,
            discovered_from: None,
            notes: vec![],
            created_at: now.into(),
            updated_at: now.into(),
            closed_at: None,
        }
    }
    pub fn path(&self) -> String {
        format!("tasks/{}/{}.json", &self.id[2..4], self.id)
    }
}
fn corrupt(message: impl Into<String>) -> Error {
    Error::new("STATE_CORRUPT", message)
}
fn nonempty(value: &str) -> bool {
    !value.trim().is_empty()
}
fn valid_id(value: &str) -> bool {
    value.len() == 22
        && value.starts_with("t-")
        && value.as_bytes()[2..]
            .iter()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b))
}
pub fn valid_timestamp(value: &str) -> bool {
    value.len() == 24
        && value.as_bytes()[10] == b'T'
        && value.as_bytes()[19] == b'.'
        && value.ends_with('Z')
        && chrono::DateTime::parse_from_rfc3339(value).is_ok()
}
impl State {
    pub fn empty() -> Self {
        Self {
            project: Project {
                format: "agent-tasks".into(),
                format_version: 1,
                project_id: Uuid::new_v4().to_string(),
            },
            tasks: BTreeMap::new(),
        }
    }
    pub fn resolve(&self, input: &str) -> Result<String> {
        if self.tasks.contains_key(input) {
            return Ok(input.into());
        }
        let prefix = input.strip_prefix("t-").unwrap_or(input);
        if prefix.len() < 8
            || prefix.len() > 20
            || !prefix
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(Error::usage(
                "Task IDs require at least eight lowercase payload hex characters",
            ));
        }
        let mut matches = self.tasks.keys().filter(|id| id[2..].starts_with(prefix));
        let first = matches
            .next()
            .ok_or_else(|| Error::new("TASK_NOT_FOUND", input))?;
        if matches.next().is_some() {
            return Err(Error::new("AMBIGUOUS_TASK_ID", input));
        }
        Ok(first.clone())
    }
    pub fn blocked_by(&self, task: &Task) -> Vec<String> {
        let mut ids: Vec<_> = task
            .depends_on
            .iter()
            .filter(|id| {
                !self.tasks.get(*id).is_some_and(|t| {
                    t.status == "closed" && t.resolution.as_deref() == Some("done")
                })
            })
            .cloned()
            .collect();
        ids.sort();
        ids
    }
    pub fn effective(&self, task: &Task) -> &'static str {
        match task.status.as_str() {
            "closed" => "closed",
            "deferred" => "deferred",
            "in_progress" => "in_progress",
            _ if task.manual_block.is_some() || !self.blocked_by(task).is_empty() => "blocked",
            _ => "ready",
        }
    }
    pub fn validate(&self) -> Result<()> {
        if self.project.format != "agent-tasks" {
            return Err(corrupt("Unsupported project format"));
        }
        if self.project.format_version != 1 {
            return Err(Error::new(
                "FORMAT_VERSION_UNSUPPORTED",
                "Only project format version 1 is supported",
            ));
        }
        if !Uuid::parse_str(&self.project.project_id)
            .is_ok_and(|u| u.get_version_num() == 4 && u.get_variant() == uuid::Variant::RFC4122)
        {
            return Err(corrupt("project_id must be UUIDv4"));
        }
        for (id, t) in &self.tasks {
            if !valid_id(id)
                || id != &t.id
                || t.schema != 1
                || !nonempty(&t.title)
                || !["task", "bug", "feature", "chore"].contains(&t.kind.as_str())
                || !["P0", "P1", "P2", "P3", "P4"].contains(&t.priority.as_str())
                || !["open", "in_progress", "deferred", "closed"].contains(&t.status.as_str())
            {
                return Err(corrupt(format!("Invalid task schema: {id}")));
            }
            if (t.status == "in_progress") != t.claim.is_some()
                || (t.status == "closed") != t.closed_at.is_some()
                || (t.status == "closed") != t.resolution.is_some()
                || t.resolution
                    .as_deref()
                    .is_some_and(|r| !["done", "cancelled"].contains(&r))
            {
                return Err(corrupt(format!("Invalid status invariants: {id}")));
            }
            let mut times = vec![t.created_at.as_str(), t.updated_at.as_str()];
            if let Some(v) = &t.closed_at {
                times.push(v);
            }
            if let Some(v) = &t.claim {
                if !nonempty(&v.actor) {
                    return Err(corrupt("Empty claim actor"));
                }
                times.push(&v.claimed_at);
            }
            if let Some(v) = &t.manual_block {
                if !nonempty(&v.actor) || !nonempty(&v.reason) {
                    return Err(corrupt("Empty manual block actor/reason"));
                }
                times.push(&v.blocked_at);
            }
            for n in &t.notes {
                if !nonempty(&n.actor) || !nonempty(&n.body) {
                    return Err(corrupt("Empty note actor/body"));
                }
                times.push(&n.created_at);
            }
            if times.into_iter().any(|v| !valid_timestamp(v)) {
                return Err(corrupt(format!("Invalid UTC millisecond timestamp: {id}")));
            }
            for values in [&t.labels, &t.depends_on] {
                if values.iter().collect::<BTreeSet<_>>().len() != values.len()
                    || values.iter().any(|v| !nonempty(v))
                {
                    return Err(corrupt(format!("Duplicate or empty set member: {id}")));
                }
            }
            for target in t
                .depends_on
                .iter()
                .chain(t.parent.iter())
                .chain(t.discovered_from.iter())
            {
                if target == id || !self.tasks.contains_key(target) {
                    return Err(corrupt(format!("Invalid relation {id} -> {target}")));
                }
            }
            if t.status == "in_progress"
                && (t.manual_block.is_some() || !self.blocked_by(t).is_empty())
            {
                return Err(corrupt(format!(
                    "In-progress task has unresolved blockers: {id}"
                )));
            }
        }
        // Iterative topological checks also handle long chains without stack growth.
        for parent in [false, true] {
            let mut degree: BTreeMap<&str, usize> = BTreeMap::new();
            let mut reverse: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
            for t in self.tasks.values() {
                let edges: Vec<&str> = if parent {
                    t.parent.iter().map(String::as_str).collect()
                } else {
                    t.depends_on.iter().map(String::as_str).collect()
                };
                degree.insert(&t.id, edges.len());
                for edge in edges {
                    reverse.entry(edge).or_default().push(&t.id);
                }
            }
            let mut ready: Vec<&str> = degree
                .iter()
                .filter_map(|(id, count)| (*count == 0).then_some(*id))
                .collect();
            let mut visited = 0;
            while let Some(id) = ready.pop() {
                visited += 1;
                if let Some(children) = reverse.get(id) {
                    for child in children {
                        let count = degree.get_mut(child).unwrap();
                        *count -= 1;
                        if *count == 0 {
                            ready.push(child);
                        }
                    }
                }
            }
            if visited != self.tasks.len() {
                return Err(corrupt(if parent {
                    "Parent cycle"
                } else {
                    "Dependency cycle"
                }));
            }
        }
        Ok(())
    }
}

// Option fields are nullable, but remain required members of the v1 schema.
fn required_nullable<'de, D, T>(deserializer: D) -> std::result::Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

#[cfg(test)]
mod tests {
    use super::*;
    const NOW: &str = "2026-09-17T03:10:00.000Z";
    #[test]
    fn schema_status_and_timestamp_invariants() {
        let mut s = State::empty();
        let t = Task::new("test".into(), NOW);
        let id = t.id.clone();
        s.tasks.insert(id.clone(), t.clone());
        s.validate().unwrap();
        assert_eq!(s.resolve(&id[2..10]).unwrap(), id);
        for invalid in 0..6 {
            let mut bad = t.clone();
            match invalid {
                0 => bad.schema = 2,
                1 => bad.status = "ready".into(),
                2 => bad.status = "in_progress".into(),
                3 => bad.resolution = Some("done".into()),
                4 => bad.updated_at = "2026-09-17T03:10:00Z".into(),
                _ => {
                    bad.claim = Some(Claim {
                        actor: "a".into(),
                        claimed_at: NOW.into(),
                    })
                }
            }
            s.tasks.insert(id.clone(), bad);
            assert!(s.validate().is_err(), "invalid case {invalid}");
        }
    }
    #[test]
    fn future_format_and_noncanonical_timestamps_are_rejected() {
        let mut state = State::empty();
        state.project.format_version = 2;
        assert_eq!(
            state.validate().unwrap_err().code,
            "FORMAT_VERSION_UNSUPPORTED"
        );
        assert!(valid_timestamp(NOW));
        for timestamp in [
            "2026-09-17t03:10:00.000Z",
            "2026-09-17T03:10:00.000+00:00",
            "2026-09-17T03:10:00Z",
        ] {
            assert!(!valid_timestamp(timestamp));
        }
    }
    #[test]
    fn serialized_schema_requires_nullable_fields_and_rejects_unknown_fields() {
        let task = Task::new("schema".into(), NOW);
        let mut json = serde_json::to_value(&task).unwrap();
        json.as_object_mut().unwrap().remove("claim");
        assert!(serde_json::from_value::<Task>(json).is_err());
        let mut json = serde_json::to_value(&task).unwrap();
        json["unexpected"] = serde_json::json!(true);
        assert!(serde_json::from_value::<Task>(json).is_err());
    }
    #[test]
    fn graph_rejects_missing_cycles_and_active_unresolved_dependencies() {
        let mut s = State::empty();
        let a = Task::new("a".into(), NOW);
        let mut b = Task::new("b".into(), NOW);
        b.depends_on.push(a.id.clone());
        s.tasks.insert(b.id.clone(), b.clone());
        assert!(s.validate().is_err());
        s.tasks.insert(a.id.clone(), a.clone());
        s.validate().unwrap();
        s.tasks
            .get_mut(&a.id)
            .unwrap()
            .depends_on
            .push(b.id.clone());
        assert!(s.validate().is_err());
        s.tasks.get_mut(&a.id).unwrap().depends_on.clear();
        let active = s.tasks.get_mut(&b.id).unwrap();
        active.status = "in_progress".into();
        active.claim = Some(Claim {
            actor: "a".into(),
            claimed_at: NOW.into(),
        });
        assert!(s.validate().is_err());
    }
}
