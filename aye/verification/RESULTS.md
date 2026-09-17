# aye v0.1.0 verification — 2026-09-17

Historical release evidence. Remote transport was superseded in v0.2.0; see
[CUSTOM_REFS.md](CUSTOM_REFS.md) for current custom-ref verification.

Test cases preceded implementation. The local bootstrap passed five installed-CLI
cases and six unit tests before creating the remaining five development tasks.
Subsequent implementation used aye claims, notes, blocking, and completion.
Two discovered fixes were added as real tasks: target-scale object writes and
repairing generated files with altered Git modes.

## Final checks

| Check | Result |
| --- | --- |
| `cargo fmt --check` | Pass |
| `cargo clippy --all-targets -- -D warnings` | Pass |
| `cargo test` | 15 passed |
| Installed CLI bootstrap/worktree/isolation cases | 5 passed |
| Installed CLI identity/query/metadata cases | 3 passed |
| Installed lifecycle/blocking/ownership cases | 3 passed |
| Installed projection/repair cases | 3 passed |
| Installed independent-repository sync cases | 12 passed |
| Live GitHub independent-repository roundtrip | Pass |
| Independent Standards review | No blocking findings; residual product labels corrected |
| Independent Spec review | Derived-file mode issue reproduced and fixed; delta review clear |

Total: 41 automated unit/black-box cases, plus the explicit live-host roundtrip.
Rust 1.98.1 and Git 2.40.1 on the user's Mac. All black-box checks use the Cargo-
installed `aye` executable. Build and install use Cargo's default configuration.

The strengthened sync fixtures clone only a source branch, then explicitly adopt
the metadata ref. Every aye command checks unchanged source HEAD, branch, index
bytes, status and content, with no task/view files at the source root. One flow
also retains staged, unstaged and untracked source changes in both repositories.
Pending-conflict GC survival, graph-only conflicts, same-task resolution, remote
advance before push, deleted branches, project mismatch, invalid external JSON,
and stale projection repair are covered.

## Target-scale measurement

10,000 tasks, 1,000 active, 100 ready; final IDs spread across shards:

| Operation | Seconds |
| --- | ---: |
| ready (all 100) | 0.900 |
| rebuild | 2.399 |
| update one task | 2.191 |

Before batching, a rebuild exceeded 133 seconds and was stopped. Blob-only
fast-import and depth-batched mktree remove per-file/per-shard process overhead.
These are observed timings, not a performance SLA.

## Live GitHub evidence

Authorized repository: `https://github.com/oskuhsiu/aye`.

The original repository published its shared state, a second independent local
repository adopted the project, created and claimed a verification task, and
published it. The original observed that claim, refused a wrong-owner note without
advancing state, then observed completion after another sync. Source HEAD/index/
status/config remained unchanged. The retained closed task is
`t-7be64c494c05a612d6db`.

Task metadata remains in the specified shared ref `refs/agent-tasks/state`, with
objects in `.git/objects`. No independent bare task repository was introduced.
Generated task/view paths belong to that Git tree, not the source working tree.
Live remote publication is the ordinary `agent-tasks` branch, as specified.

The user subsequently authorized consolidation of the initial development
histories and force-publishing the result. This one-time repository maintenance
is separate from aye's normal sync protocol, which continues to use ordinary
fast-forward-safe pushes.
