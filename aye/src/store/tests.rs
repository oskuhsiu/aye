use super::*;
use crate::domain::{self, Action};
use crate::reader::Reader;
use serde_json::json;

const NOW: &str = "2026-09-23T00:00:00.000Z";

fn fixture() -> Store {
    let cwd = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("test")
        .join(format!("receipt-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&cwd).unwrap();
    let store = Store {
        common: cwd.join(".git"),
        private: cwd.join(".git"),
        cwd,
    };
    store.git(&["init", "-q"], None).unwrap();
    store.initialize_offline().unwrap();
    store
}

#[test]
fn replay_is_whole_and_readers_observe_only_published_snapshots() {
    let store = fixture();
    let reader = Reader::open(&store.cwd).unwrap();
    let initial = store.head().unwrap();
    let first = Task::new("First atomic task".into(), NOW);
    let mut second = Task::new("Second atomic task".into(), NOW);
    second.depends_on.push(first.id.clone());
    let concurrent = Task::new("Unrelated concurrent write".into(), NOW);
    let mut attempts = 0;
    let receipt = store
        .transact(|_, state| {
            attempts += 1;
            for task in [&first, &second] {
                domain::apply(
                    state,
                    &Action::Create(Box::new(task.clone())),
                    Some("batch"),
                    NOW,
                )?;
            }
            domain::apply(
                state,
                &Action::Note {
                    id: first.id.clone(),
                    body: "Exactly once".into(),
                },
                Some("batch"),
                NOW,
            )?;
            // A competing publication between read and CAS deterministically loses
            // the first candidate. No test hook or timing-dependent sleep is needed.
            if attempts == 1 {
                store.mutate(|fresh| {
                    domain::apply(
                        fresh,
                        &Action::Create(Box::new(concurrent.clone())),
                        Some("other"),
                        NOW,
                    )
                })?;
                let visible = reader.load(&store.head()?.oid)?;
                assert_eq!(visible.state.tasks.len(), 1);
                assert!(visible.state.tasks.contains_key(&concurrent.id));
            }
            Ok(Mutation::Write(
                json!({"first":first.id,"second":second.id}),
            ))
        })
        .unwrap();
    assert_eq!(attempts, 2);
    assert_eq!(receipt.value["first"], first.id);
    let published = reader.load(&receipt.oid).unwrap();
    assert_eq!(published.state.tasks.len(), 3);
    assert_eq!(published.state.tasks[&first.id].notes.len(), 1);
    assert_eq!(
        published
            .state
            .effective(&published.state.tasks[&second.id]),
        "blocked"
    );
    assert!(reader.load(&initial.oid).unwrap().state.tasks.is_empty());
    store
        .mutate(|state| {
            domain::apply(
                state,
                &Action::Note {
                    id: concurrent.id.clone(),
                    body: "Later change".into(),
                },
                Some("other"),
                NOW,
            )
        })
        .unwrap();
    assert_ne!(receipt.oid, store.head().unwrap().oid);
    assert!(
        reader.load(&receipt.oid).unwrap().state.tasks[&concurrent.id]
            .notes
            .is_empty()
    );
}

#[test]
fn read_only_outcome_skips_projection_writes() {
    let store = fixture();
    let old = store.head().unwrap();
    let mut files = old.files.clone();
    files.insert("REPORT.md".into(), b"stale".to_vec());
    let stale = store
        .commit(&files, &[&old.oid], "fixture stale view")
        .unwrap();
    assert!(store.cas(Some(&old.oid), &stale).unwrap());
    let receipt = store
        .transact(|_, _| Ok(Mutation::ReadOnly("no assignment")))
        .unwrap();
    assert_eq!(receipt.oid, stale);
    assert_eq!(store.head().unwrap().files, files);
    assert_eq!(store.head().unwrap().oid, stale);
}
