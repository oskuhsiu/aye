# aye

A small Git-native task manager for coding agents. Linked worktrees share tasks
and claims immediately. Independent clones synchronize explicitly with `aye sync`.

## Install and verify

Requires Rust/Cargo and Git on PATH. From the repository root:

```sh
cd aye
cargo install --path . --force --locked
aye --help
make test
```

Installation uses Cargo's configured/default location (`$CARGO_HOME/bin`, normally
`~/.cargo/bin`). No install root or build output directory is hardcoded. `make test`
installs the current build before running black-box tests against `aye` on PATH.
Python 3 is needed only for the verification scripts. Disposable test repositories
live in ignored `test/`; executable checks are tracked in `verification/`.
These paths are relative to this `aye/` package directory. From the repository
root, `make -C aye test` runs the same checks. Deleting `test/` and `target/`
removes disposable fixtures and build output; tests and Cargo recreate them.

The [agent skill](skill/SKILL.md) and its [human installation guide](skill/README.md)
are distributed with this package.

## Agent workflow

Run inside any Git worktree, including nested directories:

```sh
aye init                  # adopts existing remote refs/agent-tasks/state
aye config actor agent-a  # this worktree only
aye sync                  # explicit remote fetch/reconcile/push
aye ready --json
aye create "Fix login race" --type bug --priority P1 \
  --acceptance "Concurrent callers share one refresh" \
  --acceptance "Failed refresh releases waiting callers"
aye claim <task-id>
aye note <task-id> "Reproduced the failure; regression test now fails"
# Implement and run the test cases.
aye close <task-id> --note "Regression and focused checks pass"
aye sync
```

For local-only setup, use `aye init --offline`. Actor precedence is `--actor`,
`AYE_ACTOR`, then the current worktree's actor file. Task mutations require an actor.
A claim belongs to one actor; ordinary edits while claimed require that owner.
Recovery is `aye release <id> --force --reason "Previous agent terminated"`.

Discover a prerequisite while working:

```sh
aye create "Repair dependency" --discovered-from <original-id>
aye block <original-id> --by <prerequisite-id>
```

Blocking releases the original claim atomically. Only `closed(done)` satisfies a
dependency; cancellation keeps dependents blocked. External waiting conditions use
`aye block <id> --reason "Waiting for credentials"`; `aye unblock <id>` clears only
that external blocker. `aye unblock <id> --by <prerequisite>` removes only that edge.

## Commands

| Purpose | Commands |
| --- | --- |
| Setup and sync | `init [--offline]`, `sync`, `config actor [value]`, `config remote [name]` |
| Discover work | `ready`, `list`, `show <id>`, `status`, `report` |
| Task metadata | `create`, `update`, `note <id> <text>` or `note <id> --file <path>` |
| Ownership | `claim`, `release [--force --reason <text>]` |
| Blocking | `block --by <id>` or `block --reason <text>`, `unblock [--by <id>]` |
| Lifecycle | `close [--cancelled] [--note <text>]`, `reopen`, `defer`, `resume` |
| Integrity | `doctor`, `rebuild`, `resolve` |

All commands support `--json`; inspect `error.code`. Exit codes are 0 success,
1 unexpected internal failure, 2 invalid usage, and 3 operational/domain failure.
`aye <command> --help` lists options. IDs may use a unique prefix of at least eight
payload hex characters. Ready defaults to 20 results; `--all` removes this limit.
List defaults to active tasks; `--state closed` or `--all` includes closed tasks.
Metadata updates replace repeated acceptance/label arrays; `--clear-acceptance`,
`--clear-labels`, and `--clear-parent` clear them explicitly.

## Storage and recovery

Canonical data lives in `refs/agent-tasks/state`, separate from source history:
`project.json` plus `tasks/<first-two-hex>/<full-id>.json`. Commands use Git objects
and expected-old ref updates; they never checkout a task branch or use the source
index. No database, daemon, heartbeat, TTL, or hidden network mutation is involved.
The ref stores an object ID; task/view paths exist in the referenced Git tree,
not as source folders. Git history viewers using `--all` can show that separate
history. Filter the source branch to view only source commits. Tests clone a
source branch and verify that initialization never checks out task metadata.

`FORMAT.md`, `manifest.json`, `views/ready.jsonl`, `views/active.jsonl`, and `REPORT.md`
are generated in task state. Both local and remote state use the custom ref
`refs/agent-tasks/state`; fetched remote snapshots use
`refs/agent-tasks/remotes/<remote>/state`. Ordinary clone/fetch normally omits custom
refs, so `aye init` and `aye sync` fetch the exact ref explicitly. To inspect files:

```sh
git fetch origin refs/agent-tasks/state:refs/agent-tasks/remotes/origin/state
git show refs/agent-tasks/remotes/origin/state:FORMAT.md
git show refs/agent-tasks/remotes/origin/state:views/ready.jsonl
```

GitHub does not expose `<repository>/.git/` as an HTTP filesystem. API clients
use Git database matching-refs with exact-name filtering, then commits, trees and
blobs; see [the Git/API reading instructions](src/FORMAT.md#reading-through-git-or-api).
Browser-only Agents are outside the supported scope. Valid external JSON edits can
make views stale:
reads warn and recompute without committing. `aye rebuild` repairs generated data
without changing canonical bytes. `aye doctor` checks canonical integrity and view
freshness, with warnings for large tasks, cancelled prerequisites, and closed
parents with open children.

A normal source `git push` keeps its existing behavior. `aye sync` explicitly fetches
and normally pushes only the fixed `refs/agent-tasks/state` custom ref. Remote selection is a
configured task remote, then `origin`, then the sole Git remote, otherwise local-only.
The host must permit access and normal pushes to that custom ref. GitHub branch
pages/protection are not the custom-ref interface; repository permissions still
apply. Private-repository API readers need Contents read permission; public reads can be anonymous. Custom refs are not
a secret store. Backups must explicitly include custom refs and their objects.
No legacy task branch is automatically adopted, created, or used as fallback.
The tool never force-pushes or changes source push settings/hooks.

When upgrading from 0.1, upgrade all writers before retiring the legacy branch.
Preserve and reconcile the old branch tip, verify the custom ref by explicit fetch
and API read, then remove the old branch with an expected-tip guard. `aye sync`
does not silently migrate or delete it. See [migration cases](verification/CUSTOM_REFS.md).
To back up local task history explicitly, use
`git bundle create <backup-path> refs/agent-tasks/state`; ordinary source clone alone
is not a complete task backup.

Different-task remote changes merge automatically. Different edits to the same
canonical task conflict, even when they affect different fields. Cross-file graph
conflicts also require resolution. During a pending conflict, local task writes
are blocked in every linked worktree; reads remain available:

```sh
aye resolve
aye resolve <id>
aye resolve <id> --take local   # or --take remote
aye resolve <id> --file resolved.json
aye resolve --continue         # validates the whole graph, merges, then syncs
# Or abandon only the pending resolution, retaining local task state:
aye resolve --abort
```

Choose a resolution for every listed task. The pending snapshots remain reachable
through Git refs even across garbage collection. Remote project mismatch, invalid
canonical data, and deletion of a previously observed custom task ref fail safely. Observations are
scoped by remote plus ref. Pending resolutions record the target `remote_ref`;
legacy or mismatched pending state cannot continue against the new target. Abort
obsolete pending resolution before starting a fresh sync.
`MISSING_HISTORY` means the task histories lack an available merge base; obtain
complete task history and retry. There is no distributed claim lock across clones.

## Development evidence

Release 0.2.0 follows specification v0.3.2; canonical project format and task schema remain version 1. Behavior cases and milestones
are in [verification/CASES.md](verification/CASES.md). A tested local bootstrap
created the remaining development tasks in its own shared state; subsequent work
used claims, notes, blocking, completion, and reopen where verification found bugs.
`aye list --all` and `aye report` inspect that repository-local development record.
It is published only if someone explicitly runs `aye sync` against a remote.

`make benchmark` creates 10,000 tasks (1,000 active, 100 ready), verifies ready
membership, and reports local ready/rebuild/update timings. Timings are evidence
for the current machine, not a performance guarantee.
