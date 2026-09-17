//! Explicit remote synchronization. Network operations never hold the local publication lock.
use crate::error::{Error, Result};
use crate::store::{Files, STATE_REF, Snapshot, Store, atomic_write, json_bytes};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;

const REMOTE_REF: &str = "refs/agent-tasks/state";
const RETRIES: usize = 8;

pub fn remote(store: &Store) -> Result<Option<String>> {
    let names = store.git(&["remote"], None)?;
    let names: Vec<_> = names.lines().collect();
    let configured = store.metadata().join("remote");
    if configured.exists() {
        let name = fs::read_to_string(configured)?.trim().to_string();
        if !names.contains(&name.as_str()) {
            return Err(Error::new(
                "REMOTE_UNAVAILABLE",
                "Configured task remote no longer exists",
            ));
        }
        return Ok(Some(name));
    }
    Ok(if names.contains(&"origin") {
        Some("origin".into())
    } else if names.len() == 1 {
        Some(names[0].into())
    } else {
        None
    })
}

pub fn configure_remote(store: &Store, value: Option<&str>) -> Result<Value> {
    if let Some(value) = value {
        let names = store.git(&["remote"], None)?;
        if value.starts_with('-') || !names.lines().any(|name| name == value) {
            return Err(Error::usage("Remote must name an existing Git remote"));
        }
        let _lock = store.lock()?;
        store.ensure_writable()?;
        atomic_write(&store.metadata().join("remote"), value.as_bytes())?;
    }
    Ok(json!({"remote": remote(store)?, "remote_ref": REMOTE_REF}))
}

fn observed(store: &Store) -> Result<BTreeMap<String, String>> {
    let path = store.metadata().join("sync-observed.json");
    if path.exists() {
        Ok(serde_json::from_slice(&fs::read(path)?)?)
    } else {
        Ok(BTreeMap::new())
    }
}
fn observation_key(name: &str) -> String {
    format!("{name}:{REMOTE_REF}")
}
fn remember(store: &Store, name: &str, oid: &str) -> Result<()> {
    let _lock = store.lock()?;
    let mut map = observed(store)?;
    map.insert(observation_key(name), oid.into());
    atomic_write(
        &store.metadata().join("sync-observed.json"),
        &json_bytes(&map)?,
    )
}
fn ancestor(store: &Store, a: &str, b: &str) -> Result<bool> {
    let output = store.raw(&["merge-base", "--is-ancestor", a, b], None)?;
    match output.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(Error::new(
            "GIT_ERROR",
            String::from_utf8_lossy(&output.stderr),
        )),
    }
}

pub fn status(store: &Store) -> Result<Value> {
    let name = remote(store)?;
    let known = observed(store)?;
    let fetched = name
        .as_ref()
        .and_then(|name| known.get(&observation_key(name)));
    let local = store.tip(STATE_REF)?;
    let relation = match (local.as_deref(), fetched) {
        (Some(l), Some(r)) if l == r => "equal",
        (Some(l), Some(r)) if ancestor(store, r, l)? => "ahead",
        (Some(l), Some(r)) if ancestor(store, l, r)? => "behind",
        (Some(_), Some(_)) => "diverged",
        _ => "unknown",
    };
    Ok(
        json!({"remote": name, "remote_ref": REMOTE_REF, "last_fetched_remote_oid": fetched,
        "sync_status": relation, "pending_conflict": store.conflict_path().exists()}),
    )
}

fn fetch(store: &Store, name: &str) -> Result<Option<Snapshot>> {
    let listing = store.raw(&["ls-remote", "--refs", name, REMOTE_REF], None)?;
    if !listing.status.success() {
        return Err(Error::new(
            "REMOTE_UNAVAILABLE",
            String::from_utf8_lossy(&listing.stderr),
        ));
    }
    if listing.stdout.is_empty() {
        if observed(store)?.contains_key(&observation_key(name)) {
            return Err(Error::new(
                "REMOTE_STATE_DELETED",
                "Previously observed remote task ref was deleted; refusing automatic recreation",
            ));
        }
        return Ok(None);
    }
    // A unique destination avoids another concurrent fetch changing the snapshot
    // between fetch completion and reading it. Never depend on FETCH_HEAD.
    let destination = format!("refs/agent-tasks/fetch/{}", uuid::Uuid::new_v4());
    let refspec = format!("{REMOTE_REF}:{destination}");
    let output = store.raw(
        &[
            "fetch",
            "--no-tags",
            "--no-write-fetch-head",
            name,
            &refspec,
        ],
        None,
    )?;
    if !output.status.success() {
        let _ = store.git(&["update-ref", "-d", &destination], None);
        return Err(Error::new(
            "REMOTE_UNAVAILABLE",
            String::from_utf8_lossy(&output.stderr),
        ));
    }
    let result = (|| {
        let oid = store
            .tip(&destination)?
            .ok_or_else(|| Error::new("REMOTE_UNAVAILABLE", "Fetch produced no task tip"))?;
        if store.git(&["cat-file", "-t", &oid], None)? != "commit" {
            return Err(Error::new(
                "STATE_CORRUPT",
                "Remote task ref must point directly to a commit",
            ));
        }
        let snapshot = store.load(&oid)?;
        // Keep fetched ancestry reachable without exposing it as a source branch.
        let tracking = format!("refs/agent-tasks/remotes/{name}/state");
        store.git(&["update-ref", "--create-reflog", &tracking, &oid], None)?;
        remember(store, name, &oid)?;
        Ok(Some(snapshot))
    })();
    store.git(&["update-ref", "-d", &destination], None)?;
    result
}

fn canonical(snapshot: &Snapshot) -> Files {
    snapshot
        .files
        .iter()
        .filter(|(path, _)| path.as_str() == "project.json" || path.starts_with("tasks/"))
        .map(|(path, bytes)| (path.clone(), bytes.clone()))
        .collect()
}
fn projected_oid(store: &Store, snapshot: &Snapshot) -> Result<String> {
    let files = store.project_files(&snapshot.state, canonical(snapshot))?;
    if files == snapshot.files {
        Ok(snapshot.oid.clone())
    } else {
        store.commit(
            &files,
            &[&snapshot.oid],
            "aye rebuild synchronized projections",
        )
    }
}

pub fn initialize(store: &Store, offline: bool) -> Result<Value> {
    if store.tip(STATE_REF)?.is_some() {
        let state = store.rebuild()?;
        return Ok(json!({"project_id": state.state.project.project_id, "state_oid": state.oid}));
    }
    if !offline
        && let Some(name) = remote(store)?
        && let Some(snapshot) = fetch(store, &name)?
    {
        let oid = projected_oid(store, &snapshot)?;
        if !store.cas(None, &oid)? {
            let winner = store.head()?;
            if winner.state.project.project_id != snapshot.state.project.project_id {
                return Err(Error::new(
                    "PROJECT_MISMATCH",
                    "Concurrent initialization selected another project",
                ));
            }
        }
        let state = store.head()?;
        return Ok(
            json!({"project_id": state.state.project.project_id, "state_oid": state.oid, "adopted": true, "remote_ref": REMOTE_REF}),
        );
    }
    let snapshot = store.initialize_offline()?;
    Ok(json!({"project_id": snapshot.state.project.project_id, "state_oid": snapshot.oid}))
}

#[derive(Serialize, Deserialize)]
struct Pending {
    remote_name: String,
    #[serde(default = "legacy_remote_ref")]
    remote_ref: String,
    base: String,
    local: String,
    remote: String,
    candidate: Files,
    conflicts: BTreeSet<String>,
    resolutions: BTreeMap<String, Option<Vec<u8>>>,
    validation_error: Option<String>,
}
fn legacy_remote_ref() -> String {
    "refs/heads/agent-tasks".into()
}
fn pin_conflict(store: &Store, pending: &Pending) -> Result<()> {
    let input = format!(
        "start\nupdate refs/agent-tasks/conflicts/base {}\nupdate refs/agent-tasks/conflicts/local {}\nupdate refs/agent-tasks/conflicts/remote {}\nprepare\ncommit\n",
        pending.base, pending.local, pending.remote
    );
    store.git(&["update-ref", "--stdin"], Some(input.as_bytes()))?;
    Ok(())
}
fn unpin_conflict(store: &Store) -> Result<()> {
    store.git(&["update-ref", "--stdin"], Some(b"start\ndelete refs/agent-tasks/conflicts/base\ndelete refs/agent-tasks/conflicts/local\ndelete refs/agent-tasks/conflicts/remote\nprepare\ncommit\n"))?;
    Ok(())
}
fn conflict_id(path: &str) -> &str {
    path.rsplit('/')
        .next()
        .unwrap_or(path)
        .strip_suffix(".json")
        .unwrap_or(path)
}
fn reconciliation(
    store: &Store,
    name: &str,
    local: &Snapshot,
    remote: &Snapshot,
) -> Result<std::result::Result<String, Pending>> {
    let base = store
        .git(&["merge-base", &local.oid, &remote.oid], None)
        .map_err(|_| Error::new("MISSING_HISTORY", "Task histories have no common ancestor; fetch complete task history before retrying"))?;
    let base_state = store.load(&base)?;
    let b = canonical(&base_state);
    let l = canonical(local);
    let r = canonical(remote);
    let paths: BTreeSet<_> = b.keys().chain(l.keys()).chain(r.keys()).cloned().collect();
    let mut candidate = Files::new();
    let mut conflicts = BTreeSet::new();
    let mut changed = BTreeSet::new();
    for path in paths {
        let (bv, lv, rv) = (b.get(&path), l.get(&path), r.get(&path));
        if lv != bv || rv != bv {
            changed.insert(path.clone());
        }
        let chosen = if lv == rv || rv == bv {
            lv
        } else if lv == bv {
            rv
        } else {
            conflicts.insert(path.clone());
            lv
        };
        if let Some(bytes) = chosen {
            candidate.insert(path, bytes.clone());
        }
    }
    let validation = Store::parse(&candidate);
    let validation_error = validation.as_ref().err().map(|e| e.message.clone());
    if validation_error.is_some() {
        // Graph failures can involve different files. Expose every changed task,
        // including changes automatically selected by the byte merge.
        conflicts.extend(changed);
    }
    if !conflicts.is_empty() {
        return Ok(Err(Pending {
            remote_name: name.into(),
            remote_ref: REMOTE_REF.into(),
            base,
            local: local.oid.clone(),
            remote: remote.oid.clone(),
            candidate,
            conflicts,
            resolutions: BTreeMap::new(),
            validation_error,
        }));
    }
    let state = validation?;
    let files = store.project_files(&state, candidate)?;
    Ok(Ok(store.commit(
        &files,
        &[&local.oid, &remote.oid],
        "aye reconcile task histories",
    )?))
}

pub fn sync(store: &Store) -> Result<Value> {
    store.ensure_writable()?;
    let name = match remote(store)? {
        Some(name) => name,
        None => {
            let state = store.rebuild()?;
            return Ok(
                json!({"remote": null, "remote_ref": REMOTE_REF, "status": "local_only", "state_oid": state.oid}),
            );
        }
    };
    let mut last_push_error = String::new();
    for _ in 0..RETRIES {
        store.ensure_writable()?;
        let remote = fetch(store, &name)?;
        let local = store.head()?;
        if let Some(remote) = &remote
            && local.state.project.project_id != remote.state.project.project_id
        {
            return Err(Error::new(
                "PROJECT_MISMATCH",
                "Local and remote project IDs differ",
            ));
        }
        let desired = match &remote {
            None => projected_oid(store, &local)?,
            Some(remote) if ancestor(store, &remote.oid, &local.oid)? => {
                projected_oid(store, &local)?
            }
            Some(remote) if ancestor(store, &local.oid, &remote.oid)? => {
                projected_oid(store, remote)?
            }
            Some(remote) => match reconciliation(store, &name, &local, remote)? {
                Ok(oid) => oid,
                Err(pending) => {
                    let _lock = store.lock()?;
                    store.ensure_writable()?;
                    if store.tip(STATE_REF)?.as_deref() != Some(&local.oid) {
                        continue;
                    }
                    pin_conflict(store, &pending)?;
                    atomic_write(&store.conflict_path(), &json_bytes(&pending)?)?;
                    return Err(Error::new(
                        "SYNC_CONFLICT",
                        "Task histories conflict; inspect aye resolve",
                    ));
                }
            },
        };
        if !store.cas(Some(&local.oid), &desired)? {
            continue;
        }
        if remote.as_ref().is_some_and(|remote| remote.oid == desired) {
            return Ok(
                json!({"remote": name, "remote_ref": REMOTE_REF, "status": "synchronized", "state_oid": desired}),
            );
        }
        // Push the validated snapshot OID, not a moving local ref. Ordinary push
        // enforces remote ancestry even when another clone wins after fetch.
        let refspec = format!("{desired}:{REMOTE_REF}");
        let output = store.raw(&["push", "--porcelain", &name, &refspec], None)?;
        if output.status.success() {
            remember(store, &name, &desired)?;
            return Ok(
                json!({"remote": name, "remote_ref": REMOTE_REF, "status": "synchronized", "state_oid": desired}),
            );
        }
        last_push_error = String::from_utf8_lossy(&output.stderr).into_owned();
        // Fetch/reconcile after rejection, including receive-hook rejection.
        // Bounded retries also report servers that prohibit direct task-ref pushes.
    }
    Err(Error::new(
        "REMOTE_PUSH_REJECTED",
        if last_push_error.is_empty() {
            "Local state changed repeatedly during synchronization".into()
        } else {
            last_push_error
        },
    ))
}

pub fn resolve(
    store: &Store,
    id: Option<&str>,
    take: Option<&str>,
    file: Option<&str>,
    continue_: bool,
    abort: bool,
) -> Result<Value> {
    if (continue_ && abort)
        || ((continue_ || abort) && (id.is_some() || take.is_some() || file.is_some()))
        || (take.is_some() && file.is_some())
        || (id.is_none() && (take.is_some() || file.is_some()))
    {
        return Err(Error::usage("Choose one resolution action"));
    }
    let lock = store.lock()?;
    if !store.conflict_path().exists() {
        return if id.is_none() && !continue_ && !abort {
            Ok(json!({"conflicts": []}))
        } else {
            Err(Error::new("SYNC_CONFLICT", "No pending sync conflict"))
        };
    }
    let mut pending: Pending = serde_json::from_slice(&fs::read(store.conflict_path())?)?;
    if abort {
        fs::remove_file(store.conflict_path())?;
        unpin_conflict(store)?;
        return Ok(json!({"aborted": true}));
    }
    if continue_ {
        if pending.remote_ref != REMOTE_REF {
            return Err(Error::new(
                "SYNC_PROTOCOL_MISMATCH",
                format!(
                    "Pending conflict targets {}; this version syncs {}. Inspect or abort the legacy conflict before syncing again",
                    pending.remote_ref, REMOTE_REF
                ),
            ));
        }
        if pending
            .conflicts
            .iter()
            .any(|path| !pending.resolutions.contains_key(path))
        {
            return Err(Error::new(
                "SYNC_CONFLICT",
                "Resolve every listed conflict before continuing",
            ));
        }
        for (path, value) in &pending.resolutions {
            if let Some(bytes) = value {
                pending.candidate.insert(path.clone(), bytes.clone());
            } else {
                pending.candidate.remove(path);
            }
        }
        let state = Store::parse(&pending.candidate)?;
        let original = store.load(&pending.local)?;
        if state.project.project_id != original.state.project.project_id {
            return Err(Error::new(
                "PROJECT_MISMATCH",
                "Resolution may not change project identity",
            ));
        }
        let files = store.project_files(&state, pending.candidate)?;
        let oid = store.commit(
            &files,
            &[&pending.local, &pending.remote],
            "aye resolve task conflicts",
        )?;
        if !store.cas_unlocked(Some(&pending.local), &oid)? {
            return Err(Error::new(
                "LOCAL_CONCURRENCY_RETRY_EXHAUSTED",
                "Task ref changed outside aye while conflict was pending",
            ));
        }
        fs::remove_file(store.conflict_path())?;
        unpin_conflict(store)?;
        drop(lock);
        return sync(store);
    }
    if let Some(id) = id {
        let prefix = id.strip_prefix("t-").unwrap_or(id);
        let paths: Vec<_> = pending
            .conflicts
            .iter()
            .filter(|path| {
                let full = conflict_id(path);
                full == id
                    || (prefix.len() >= 8
                        && full
                            .strip_prefix("t-")
                            .is_some_and(|p| p.starts_with(prefix)))
            })
            .cloned()
            .collect();
        let path = match paths.as_slice() {
            [path] => path.clone(),
            [] => {
                return Err(Error::new(
                    "TASK_NOT_FOUND",
                    "Task is not a pending conflict",
                ));
            }
            _ => return Err(Error::new("AMBIGUOUS_TASK_ID", id)),
        };
        if take.is_none() && file.is_none() {
            let local = store.load(&pending.local)?;
            let remote = store.load(&pending.remote)?;
            let base = store.load(&pending.base)?;
            let decode = |files: &Files| -> Result<Value> {
                Ok(files
                    .get(&path)
                    .map(|bytes| serde_json::from_slice(bytes))
                    .transpose()?
                    .unwrap_or(Value::Null))
            };
            return Ok(
                json!({"id": conflict_id(&path), "base": decode(&base.files)?, "local": decode(&local.files)?,
                "remote": decode(&remote.files)?, "remote_ref": pending.remote_ref, "resolved": pending.resolutions.contains_key(&path)}),
            );
        }
        let bytes = if let Some(file) = file {
            let bytes = fs::read(file)?;
            if path.starts_with("tasks/") {
                let task: crate::model::Task = serde_json::from_slice(&bytes)?;
                if task.id != conflict_id(&path) {
                    return Err(Error::new(
                        "STATE_CORRUPT",
                        "Resolution task ID does not match conflict",
                    ));
                }
            } else {
                let _: crate::model::Project = serde_json::from_slice(&bytes)?;
            }
            Some(bytes)
        } else {
            let oid = match take {
                Some("local") => &pending.local,
                Some("remote") => &pending.remote,
                _ => return Err(Error::usage("--take must be local or remote")),
            };
            store.load(oid)?.files.get(&path).cloned()
        };
        pending.resolutions.insert(path.clone(), bytes);
        atomic_write(&store.conflict_path(), &json_bytes(&pending)?)?;
        return Ok(json!({"id": conflict_id(&path), "resolved": true}));
    }
    Ok(
        json!({"conflicts": pending.conflicts.iter().map(|path| json!({"id": conflict_id(path),
        "resolved": pending.resolutions.contains_key(path)})).collect::<Vec<_>>(),
        "remote_ref": pending.remote_ref, "validation_error": pending.validation_error, "local_oid": pending.local, "remote_oid": pending.remote}),
    )
}
