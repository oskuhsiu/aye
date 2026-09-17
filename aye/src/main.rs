mod domain;
mod error;
mod model;
mod projection;
mod store;
mod sync;

use clap::{Args, Parser, Subcommand};
use domain::{Action, Update};
use error::{Error, Result};
use model::Task;
use serde_json::{Value, json};
use store::{Snapshot, Store};

#[derive(Parser)]
#[command(
    name = "aye",
    version,
    about = "Git-native shared task coordination for agents"
)]
struct Cli {
    #[arg(long, global = true, help = "Return a stable JSON envelope")]
    json: bool,
    #[arg(
        long,
        global = true,
        help = "Actor identity; overrides AYE_ACTOR and worktree default"
    )]
    actor: Option<String>,
    #[command(subcommand)]
    command: Commands,
}
#[derive(Args, Default)]
struct Filters {
    #[arg(long, value_parser = ["P0", "P1", "P2", "P3", "P4"])]
    priority: Option<String>,
    #[arg(long = "type", value_parser = ["task", "bug", "feature", "chore"])]
    kind: Option<String>,
    #[arg(long)]
    label: Option<String>,
}
impl Filters {
    fn matches(&self, task: &Task) -> bool {
        self.priority.as_ref().is_none_or(|v| v == &task.priority)
            && self.kind.as_ref().is_none_or(|v| v == &task.kind)
            && self.label.as_ref().is_none_or(|v| task.labels.contains(v))
    }
}
#[derive(Subcommand)]
enum Commands {
    /// Initialize shared task state; adopt an existing remote store if available.
    Init {
        #[arg(long)]
        offline: bool,
    },
    /// Fetch, reconcile, and publish task state explicitly.
    Sync,
    /// Create a task. This does not claim it.
    Create {
        title: String,
        #[arg(long = "type", default_value = "task", value_parser = ["task", "bug", "feature", "chore"])]
        kind: String,
        #[arg(long, default_value = "P2", value_parser = ["P0", "P1", "P2", "P3", "P4"])]
        priority: String,
        #[arg(long, default_value = "")]
        description: String,
        #[arg(long)]
        acceptance: Vec<String>,
        #[arg(long)]
        label: Vec<String>,
        #[arg(long)]
        parent: Option<String>,
        #[arg(long)]
        discovered_from: Option<String>,
    },
    /// Edit metadata. Repeated acceptance/label options replace the entire list.
    Update {
        id: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long = "type", value_parser = ["task", "bug", "feature", "chore"])]
        kind: Option<String>,
        #[arg(long, value_parser = ["P0", "P1", "P2", "P3", "P4"])]
        priority: Option<String>,
        #[arg(long)]
        description: Option<String>,
        #[arg(long, conflicts_with = "clear_acceptance")]
        acceptance: Vec<String>,
        #[arg(long)]
        clear_acceptance: bool,
        #[arg(long, conflicts_with = "clear_labels")]
        label: Vec<String>,
        #[arg(long)]
        clear_labels: bool,
        #[arg(long, conflicts_with = "clear_parent")]
        parent: Option<String>,
        #[arg(long)]
        clear_parent: bool,
    },
    /// List active tasks, or include closed tasks with --all.
    List {
        #[arg(long)]
        all: bool,
        #[arg(long, value_parser = ["open", "ready", "blocked", "in_progress", "deferred", "closed"])]
        state: Option<String>,
        #[arg(long)]
        claimant: Option<String>,
        #[arg(long)]
        query: Option<String>,
        #[command(flatten)]
        filters: Filters,
    },
    /// List actionable tasks in priority order (default limit 20).
    Ready {
        #[arg(long, default_value_t = 20)]
        limit: usize,
        #[arg(long)]
        all: bool,
        #[command(flatten)]
        filters: Filters,
    },
    /// Show canonical task data and computed relations.
    Show {
        id: String,
    },
    /// Inspect local state and last known sync status without network access.
    Status,
    /// Display the deterministic human report without changing task history.
    Report,
    /// Atomically claim an open, ready task in this repository.
    Claim {
        id: String,
    },
    /// Release a claim; forced recovery requires an attributed reason.
    Release {
        id: String,
        #[arg(long)]
        force: bool,
        #[arg(long, requires = "force")]
        reason: Option<String>,
    },
    /// Add one unresolved task blocker or an external reason.
    Block {
        id: String,
        #[arg(long, required_unless_present = "reason", conflicts_with = "reason")]
        by: Option<String>,
        #[arg(long, required_unless_present = "by")]
        reason: Option<String>,
    },
    /// Remove a task blocker, or clear only the external blocker.
    Unblock {
        id: String,
        #[arg(long)]
        by: Option<String>,
    },
    /// Close as done, or cancel without satisfying downstream dependencies.
    Close {
        id: String,
        #[arg(long)]
        cancelled: bool,
        #[arg(long)]
        note: Option<String>,
    },
    Reopen {
        id: String,
    },
    Defer {
        id: String,
    },
    Resume {
        id: String,
    },
    /// Append an attributed note from a string or UTF-8 file.
    Note {
        id: String,
        #[arg(required_unless_present = "file", conflicts_with = "file")]
        body: Option<String>,
        #[arg(long)]
        file: Option<String>,
    },
    /// Read-only canonical integrity and projection checks.
    Doctor,
    /// Repair derived files, preserving canonical bytes; unchanged output is a no-op.
    Rebuild,
    /// Inspect and resolve a pending remote task conflict.
    Resolve {
        id: Option<String>,
        #[arg(long, value_parser = ["local", "remote"], requires = "id", conflicts_with = "file")]
        take: Option<String>,
        #[arg(long, requires = "id")]
        file: Option<String>,
        #[arg(long = "continue", conflicts_with_all = ["abort", "id", "take", "file"])]
        continue_: bool,
        #[arg(long, conflicts_with_all = ["id", "take", "file"])]
        abort: bool,
    },
    /// Read/set the repository remote or the current worktree's actor.
    Config {
        #[arg(value_parser = ["actor", "remote"])]
        key: String,
        value: Option<String>,
    },
}
struct Reply {
    data: Value,
    warnings: Vec<Value>,
}
impl Reply {
    fn plain(data: Value) -> Self {
        Self {
            data,
            warnings: vec![],
        }
    }
    fn read(data: Value, snapshot: &Snapshot) -> Self {
        let warnings = if projection::fresh(snapshot) {
            vec![]
        } else {
            vec![
                json!({"code":"VIEW_STALE","message":"Computed from canonical state; run aye rebuild to repair stored views"}),
            ]
        };
        Self { data, warnings }
    }
}
fn execute(cli: &Cli) -> Result<Reply> {
    let store = Store::discover()?;
    let actor = store.actor(cli.actor.as_deref())?;
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let action = match &cli.command {
        Commands::Init { offline } => return Ok(Reply::plain(sync::initialize(&store, *offline)?)),
        Commands::Sync => return Ok(Reply::plain(sync::sync(&store)?)),
        Commands::Resolve {
            id,
            take,
            file,
            continue_,
            abort,
        } => {
            return Ok(Reply::plain(sync::resolve(
                &store,
                id.as_deref(),
                take.as_deref(),
                file.as_deref(),
                *continue_,
                *abort,
            )?));
        }
        Commands::Config { key, value } => {
            if key == "remote" {
                return Ok(Reply::plain(sync::configure_remote(
                    &store,
                    value.as_deref(),
                )?));
            }
            let _lock = store.lock()?;
            if value.is_some() {
                store.ensure_writable()?;
            }
            let path = store.private.join("agent-tasks/actor");
            if let Some(value) = value {
                if value.trim().is_empty() {
                    return Err(Error::usage("Actor must not be empty"));
                }
                store::atomic_write(&path, format!("{}\n", value.trim()).as_bytes())?;
            }
            let configured = match std::fs::read_to_string(path) {
                Ok(value) => Some(value.trim().to_string()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => return Err(e.into()),
            };
            return Ok(Reply::plain(json!({"actor": configured})));
        }
        Commands::List {
            all,
            state,
            claimant,
            query,
            filters,
        } => {
            let snapshot = store.head()?;
            let mut tasks: Vec<_> = snapshot
                .state
                .tasks
                .values()
                .filter(|t| {
                    (*all || state.is_some() || t.status != "closed")
                        && state
                            .as_ref()
                            .is_none_or(|v| v == &t.status || v == snapshot.state.effective(t))
                        && claimant
                            .as_ref()
                            .is_none_or(|v| t.claim.as_ref().is_some_and(|c| &c.actor == v))
                        && filters.matches(t)
                        && query.as_ref().is_none_or(|q| {
                            let q = q.to_lowercase();
                            t.title.to_lowercase().contains(&q)
                                || t.description.to_lowercase().contains(&q)
                                || t.labels.iter().any(|l| l.to_lowercase().contains(&q))
                        })
                })
                .collect();
            tasks.sort_by_key(|t| (&t.priority, &t.created_at, &t.id));
            return Ok(Reply::read(
                json!(
                    tasks
                        .iter()
                        .map(|t| projection::show(&snapshot.state, t))
                        .collect::<Vec<_>>()
                ),
                &snapshot,
            ));
        }
        Commands::Ready {
            limit,
            all,
            filters,
        } => {
            let snapshot = store.head()?;
            let tasks: Vec<_> = projection::ready(&snapshot.state)
                .into_iter()
                .filter(|t| filters.matches(t))
                .take(if *all { usize::MAX } else { *limit })
                .map(|t| projection::show(&snapshot.state, t))
                .collect();
            return Ok(Reply::read(json!(tasks), &snapshot));
        }
        Commands::Show { id } => {
            let snapshot = store.head()?;
            let id = snapshot.state.resolve(id)?;
            return Ok(Reply::read(
                projection::show(&snapshot.state, &snapshot.state.tasks[&id]),
                &snapshot,
            ));
        }
        Commands::Report => {
            let snapshot = store.head()?;
            return Ok(Reply::read(
                json!(projection::report(&snapshot.state)),
                &snapshot,
            ));
        }
        Commands::Status => {
            let snapshot = store.head()?;
            let derived = projection::build(&snapshot.state, &snapshot.tasks_tree_oid)?;
            let manifest: Value = serde_json::from_slice(&derived["manifest.json"])?;
            return Ok(Reply::read(
                json!({
                    "project_id": snapshot.state.project.project_id,
                    "state_oid": snapshot.oid,
                    "views_fresh": projection::fresh(&snapshot),
                    "counts": manifest["counts"],
                    "sync": sync::status(&store)?,
                    "pending_conflict": store.conflict_path().exists()
                }),
                &snapshot,
            ));
        }
        Commands::Doctor => {
            let snapshot = store.head()?;
            if !projection::fresh(&snapshot) {
                return Err(Error::new(
                    "VIEW_STALE",
                    "Derived state is stale; run aye rebuild",
                ));
            }
            let mut warnings = projection::warnings(&snapshot.state);
            if store.conflict_path().exists() {
                warnings.push(json!({"code":"SYNC_CONFLICT","message":"Pending sync conflict"}));
            }
            return Ok(Reply {
                data: json!({"valid":true,"state_oid":snapshot.oid}),
                warnings,
            });
        }
        Commands::Rebuild => {
            let snapshot = store.rebuild()?;
            return Ok(Reply::plain(
                json!({"state_oid":snapshot.oid,"views_fresh":true}),
            ));
        }
        Commands::Create {
            title,
            kind,
            priority,
            description,
            acceptance,
            label,
            parent,
            discovered_from,
        } => {
            if title.trim().is_empty() {
                return Err(Error::usage("Title cannot be empty"));
            }
            let mut task = Task::new(title.clone(), &now);
            task.kind = kind.clone();
            task.priority = priority.clone();
            task.description = description.clone();
            task.acceptance = acceptance.clone();
            task.labels = label.clone();
            task.labels.sort();
            task.labels.dedup();
            task.parent = parent.clone();
            task.discovered_from = discovered_from.clone();
            Action::Create(Box::new(task))
        }
        Commands::Update {
            id,
            title,
            kind,
            priority,
            description,
            acceptance,
            clear_acceptance,
            label,
            clear_labels,
            parent,
            clear_parent,
        } => {
            if title.as_ref().is_some_and(|t| t.trim().is_empty()) {
                return Err(Error::usage("Title cannot be empty"));
            }
            Action::Update {
                id: id.clone(),
                patch: Update {
                    title: title.clone(),
                    kind: kind.clone(),
                    priority: priority.clone(),
                    description: description.clone(),
                    acceptance: if *clear_acceptance || !acceptance.is_empty() {
                        Some(acceptance.clone())
                    } else {
                        None
                    },
                    labels: if *clear_labels || !label.is_empty() {
                        Some(label.clone())
                    } else {
                        None
                    },
                    parent: if *clear_parent {
                        Some(None)
                    } else {
                        parent.clone().map(Some)
                    },
                },
            }
        }
        Commands::Claim { id } => Action::Claim(id.clone()),
        Commands::Release { id, force, reason } => Action::Release {
            id: id.clone(),
            force: *force,
            reason: reason.clone(),
        },
        Commands::Block { id, by, reason } => Action::Block {
            id: id.clone(),
            by: by.clone(),
            reason: reason.clone(),
        },
        Commands::Unblock { id, by } => Action::Unblock {
            id: id.clone(),
            by: by.clone(),
        },
        Commands::Close {
            id,
            cancelled,
            note,
        } => Action::Close {
            id: id.clone(),
            cancelled: *cancelled,
            note: note.clone(),
        },
        Commands::Reopen { id } => Action::Reopen(id.clone()),
        Commands::Defer { id } => Action::Defer(id.clone()),
        Commands::Resume { id } => Action::Resume(id.clone()),
        Commands::Note { id, body, file } => Action::Note {
            id: id.clone(),
            body: match file {
                Some(path) => std::fs::read_to_string(path)?,
                None => body
                    .clone()
                    .ok_or_else(|| Error::usage("Provide a note body or --file"))?,
            },
        },
    };
    Ok(Reply::plain(store.mutate(|state| {
        domain::apply(state, &action, actor.as_deref(), &now)
    })?))
}
fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    let json_mode = args.iter().any(|arg| arg == "--json");
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(e) => {
            if e.exit_code() == 0 {
                print!("{e}");
                return;
            }
            if json_mode {
                println!(
                    "{}",
                    json!({"ok":false,"error":{"code":"INVALID_ARGUMENT","message":e.to_string()}})
                );
            } else {
                eprint!("{e}");
            }
            std::process::exit(2);
        }
    };
    match execute(&cli) {
        Ok(reply) => {
            if cli.json {
                println!(
                    "{}",
                    json!({"ok":true,"data":reply.data,"warnings":reply.warnings})
                );
            } else {
                for warning in reply.warnings {
                    eprintln!("Warning: {warning}");
                }
                if let Value::String(text) = reply.data {
                    print!("{text}");
                } else {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&reply.data).expect("JSON output")
                    );
                }
            }
        }
        Err(e) => {
            if cli.json {
                println!(
                    "{}",
                    json!({"ok":false,"error":{"code":e.code,"message":e.message}})
                );
            } else {
                eprintln!("{e}");
            }
            std::process::exit(e.exit);
        }
    }
}
