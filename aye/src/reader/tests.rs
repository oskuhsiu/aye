use super::*;
use crate::{model::Task, store::json_bytes};
use std::{fs, process::Command};

fn git(path: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(path)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().into()
}
fn fixture() -> (std::path::PathBuf, Store) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("test")
        .join(format!("reader-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&path).unwrap();
    git(&path, &["init", "-q"]);
    git(
        &path,
        &[
            "-c",
            "user.name=test",
            "-c",
            "user.email=test@example.com",
            "commit",
            "--allow-empty",
            "-qm",
            "source",
        ],
    );
    let store = Store {
        cwd: path.clone(),
        common: git(
            &path,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        )
        .into(),
        private: git(&path, &["rev-parse", "--absolute-git-dir"]).into(),
    };
    (path, store)
}
fn commit(store: &Store, state: &State, extra: Option<(&str, &[u8])>) -> String {
    let mut files = crate::store::Files::new();
    files.insert("project.json".into(), json_bytes(&state.project).unwrap());
    for task in state.tasks.values() {
        files.insert(task.path(), json_bytes(task).unwrap());
    }
    if let Some((path, bytes)) = extra {
        files.insert(path.into(), bytes.to_vec());
    }
    let oid = store.commit(&files, &[], "fixture").unwrap();
    git(&store.cwd, &["update-ref", STATE_REF, &oid]);
    oid
}
#[test]
fn canonical_root_nested_linked_pinned_and_readonly() {
    let (path, store) = fixture();
    let mut state = State::empty();
    let t = Task::new("Unicode 中文".into(), "2026-09-17T03:10:00.000Z");
    state.tasks.insert(t.id.clone(), t.clone());
    let now = "2026-09-17T03:10:00.000Z";
    let mut done = Task::new("done prerequisite".into(), now);
    done.status = "closed".into();
    done.resolution = Some("done".into());
    done.closed_at = Some(now.into());
    let mut cancelled = Task::new("cancelled prerequisite".into(), now);
    cancelled.status = "closed".into();
    cancelled.resolution = Some("cancelled".into());
    cancelled.closed_at = Some(now.into());
    let mut ready = Task::new("ready dependent".into(), now);
    ready.depends_on.push(done.id.clone());
    let mut blocked = Task::new("blocked dependent".into(), now);
    blocked.depends_on.push(cancelled.id.clone());
    let mut manual = Task::new("manual blocker".into(), now);
    manual.manual_block = Some(crate::model::ManualBlock {
        reason: "external wait".into(),
        actor: "test".into(),
        blocked_at: now.into(),
    });
    let mut active = Task::new("claimed task".into(), now);
    active.status = "in_progress".into();
    active.claim = Some(crate::model::Claim {
        actor: "test".into(),
        claimed_at: now.into(),
    });
    active.parent = Some(t.id.clone());
    active.discovered_from = Some(t.id.clone());
    for task in [&done, &cancelled, &ready, &blocked, &manual, &active] {
        state.tasks.insert(task.id.clone(), task.clone());
    }
    let oid = commit(
        &store,
        &state,
        Some(("views/ready.jsonl", b"broken projection")),
    );
    fs::create_dir(path.join("nested")).unwrap();
    fs::write(path.join("dirty"), "staged").unwrap();
    git(&path, &["add", "dirty"]);
    fs::write(path.join("dirty"), "unstaged").unwrap();
    let linked = path.with_extension("linked");
    git(
        &path,
        &["worktree", "add", "--detach", linked.to_str().unwrap()],
    );
    git(&path, &["pack-refs", "--all"]);
    let index = fs::read(store.private.join("index")).unwrap();
    let config = fs::read(store.common.join("config")).unwrap();
    let refs = git(&path, &["show-ref"]);
    for location in [&path, &path.join("nested"), &linked] {
        let reader = Reader::open(location).unwrap();
        assert_eq!(reader.current_oid().unwrap(), Some(oid.clone()));
        let loaded = reader.load(&oid).unwrap();
        assert_eq!(loaded.state.tasks, state.tasks);
        assert_eq!(loaded.state.effective(&ready), "ready");
        assert_eq!(loaded.state.effective(&blocked), "blocked");
        assert_eq!(loaded.state.effective(&manual), "blocked");
        assert_eq!(loaded.state.effective(&active), "in_progress");
        assert_eq!(loaded.oid, oid);
    }
    commit(&store, &State::empty(), None);
    assert_eq!(
        Reader::open(&path)
            .unwrap()
            .load(&oid)
            .unwrap()
            .state
            .tasks
            .len(),
        state.tasks.len()
    );
    git(&path, &["update-ref", STATE_REF, &oid]);
    assert_eq!(fs::read(store.private.join("index")).unwrap(), index);
    assert_eq!(fs::read(store.common.join("config")).unwrap(), config);
    assert_eq!(git(&path, &["show-ref"]), refs);
    assert_eq!(fs::read_to_string(path.join("dirty")).unwrap(), "unstaged");
    git(&path, &["worktree", "remove", linked.to_str().unwrap()]);
    fs::remove_dir_all(path).unwrap();
}
#[test]
fn distinct_errors_empty_and_noncommit() {
    let (path, store) = fixture();
    let reader = Reader::open(&path).unwrap();
    assert_eq!(reader.current_oid().unwrap(), None);
    let empty = commit(&store, &State::empty(), None);
    assert!(reader.load(&empty).unwrap().state.tasks.is_empty());
    for oid in [
        "HEAD".to_string(),
        "0".repeat(40),
        git(&path, &["rev-parse", "HEAD^{tree}"]),
    ] {
        assert_eq!(reader.load(&oid).unwrap_err().code, "STATE_CORRUPT");
    }
    let mut future = State::empty();
    future.project.format_version = 2;
    let oid = commit(&store, &future, None);
    assert_eq!(
        reader.load(&oid).unwrap_err().code,
        "FORMAT_VERSION_UNSUPPORTED"
    );
    let oid = commit(&store, &State::empty(), Some(("project.json", b"broken")));
    assert_eq!(reader.load(&oid).unwrap_err().code, "STATE_CORRUPT");
    assert_eq!(Reader::open("/").unwrap_err().code, "NOT_GIT_REPOSITORY");
    fs::remove_dir_all(path).unwrap();
}

#[test]
fn missing_promisor_object_cannot_invoke_transport_and_replacements_are_ignored() {
    for kind in ["commit", "tree", "blob"] {
        let (path, store) = fixture();
        let oid = commit(&store, &State::empty(), None);
        let bad = commit(&store, &State::empty(), Some(("project.json", b"broken")));
        git(&path, &["replace", &oid, &bad]);
        let reader = Reader::open(&path).unwrap();
        assert!(reader.load(&oid).unwrap().state.tasks.is_empty());
        git(&path, &["replace", "-d", &oid]);
        let missing = match kind {
            "commit" => oid.clone(),
            "tree" => git(&path, &["rev-parse", &format!("{oid}^{{tree}}")]),
            _ => git(&path, &["rev-parse", &format!("{oid}:project.json")]),
        };
        let marker = path.join("transport-called");
        let helper = path.join("transport.sh");
        fs::write(
            &helper,
            format!("#!/bin/sh\ntouch '{}'\nexit 1\n", marker.display()),
        )
        .unwrap();
        git(&path, &["config", "extensions.partialClone", "origin"]);
        git(&path, &["config", "remote.origin.promisor", "true"]);
        git(
            &path,
            &[
                "config",
                "remote.origin.url",
                &format!("ext::/bin/sh {}", helper.display()),
            ],
        );
        git(&path, &["config", "protocol.ext.allow", "always"]);
        assert!(
            reader.load(&oid).unwrap().state.tasks.is_empty(),
            "fully materialized partial clone must remain readable"
        );
        fs::remove_file(
            store
                .common
                .join("objects")
                .join(&missing[..2])
                .join(&missing[2..]),
        )
        .unwrap();
        let refs = fs::read(store.common.join(STATE_REF)).unwrap();
        let config = fs::read(store.common.join("config")).unwrap();
        let index = fs::read(store.private.join("index")).ok();
        assert_eq!(reader.load(&oid).unwrap_err().code, "STATE_CORRUPT");
        assert!(
            !marker.exists(),
            "reader must not launch a transport helper"
        );
        assert_eq!(fs::read(store.common.join(STATE_REF)).unwrap(), refs);
        assert_eq!(fs::read(store.common.join("config")).unwrap(), config);
        assert_eq!(fs::read(store.private.join("index")).ok(), index);
        // A control invocation proves this fixture would lazy-fetch without the reader guards.
        let control = Command::new("git")
            .current_dir(&path)
            .args(["cat-file", "-p", &missing])
            .env_remove("GIT_ALLOW_PROTOCOL")
            .env_remove("GIT_NO_LAZY_FETCH")
            .output()
            .unwrap();
        assert!(!control.status.success());
        assert!(
            marker.exists(),
            "unguarded Git must attempt the configured helper"
        );
        fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn unsupported_git_object_format_and_nonordinary_modes() {
    let (path, store) = fixture();
    let reader = Reader::open(&path).unwrap();
    let bytes = json_bytes(&State::empty().project).unwrap();
    let blob = store
        .git(&["hash-object", "-w", "--stdin"], Some(&bytes))
        .unwrap();
    let tree = store
        .git(
            &["mktree"],
            Some(format!("120000 blob {blob}\tproject.json\n").as_bytes()),
        )
        .unwrap();
    let oid = store
        .git(&["commit-tree", &tree], Some(b"invalid canonical mode\n"))
        .unwrap();
    assert_eq!(reader.load(&oid).unwrap_err().code, "STATE_CORRUPT");
    let sha256 = path.with_extension("sha256");
    fs::create_dir(&sha256).unwrap();
    git(&sha256, &["init", "-q", "--object-format=sha256"]);
    assert_eq!(
        Reader::open(&sha256).unwrap_err().code,
        "OBJECT_FORMAT_UNSUPPORTED"
    );
    fs::remove_dir_all(sha256).unwrap();
    fs::remove_dir_all(path).unwrap();
}
