# aye-view delivery and acceptance plan

Source: `aye/plan/aye-view-spec-v0.2.md` (local product specification,
2026-09-17). Implement its full required terminal-viewer scope. Optional mouse,
custom recent duration, persistent preferences, and alternative layouts are not
required. This plan records observable cases before implementation.

## Product contract

The viewer helps a human understand current work, dependency order, blockers,
ownership, task details and closed history. It reads the shared local
`refs/agent-tasks/state` from any linked Git worktree. It never writes task data,
source files, refs, commits, configuration, projections or a second database,
and never fetches or synchronizes. Invalid canonical data must not be presented
as a healthy graph. Stale derived views must not prevent canonical reading.

Use the specification's compact terminal layout: current dependency graph on
the left, selected-task details on the right, status and keys at the edges.
At narrow sizes, show one readable pane with a keyboard route to details and
back. Retain status symbols without color. Keep full long text reachable in
details. Separate unrelated recently closed tasks from the current DAG.

## Delivery slices

Each numbered slice becomes an aye task with native dependency edges and
acceptance cases. Source implementation runs in an agent's own `hd new`
worktree. A different agent reviews the committed result before main merges it.
Only a reviewed, merged, verified task closes as done and unlocks dependents.

| Slice | Blocked by | Observable result |
| --- | --- | --- |
| 1. Read-only terminal foundation | None | Start from a repository, browse current tasks and full details, receive useful errors, quit safely. |
| 2. Current dependency graph | 1 | Stable prerequisite-to-dependent layered graph, correct closed context, isolated nodes and disconnected components. |
| 3. Search and filters | 1 | Find a task by title/ID/label, jump to it, combine the five simple filters and recover from zero results. |
| 4. Recent and closed history | 1 | Toggle recent closed separately and browse all closed tasks newest-first in incremental windows. |
| 5. Live shared-state refresh | 1 | Changes in another worktree appear locally, preserving useful selection and surviving reload errors. |
| 6. Focus and graph navigation | 2, 3 | Explore the selected task's ancestors and descendants, navigate neighboring nodes and pan independently. |
| 7. Integrated behavioral validation | 2, 3, 4, 5, 6 | Installed app passes realistic keyboard/worktree/read-only scenarios and target-scale measurements. |
| 8. User guide and final delivery | 7 | Reproducible Cargo install/check commands, complete controls and limitations, clean integrated source and worktrees. |

Task IDs, in that order: `t-862c48d87c375516b52e`,
`t-403432b5c43ba89fc00f`, `t-f9a7b8f5d1ef576753a5`,
`t-f9dbb5abece373ae4623`, `t-3a29c58219c9af41dd76`,
`t-b05477730503743c46d5`, `t-c13c61e2b64b46ebf63d`,
`t-273e1fb8fe8c125a9ee9`. The authoritative task state stays in aye's custom ref.

## Implementation decisions

The existing aye package gains a library entrypoint: existing CLI implementation
and write-capable modules remain private, with a public CLI runner for its binary.
A separate public read-only reader wraps the same canonical parsing/validation.
The viewer depends on that library by relative Cargo path, not on an installed
aye subprocess. It uses one pinned commit per snapshot, batch object reading,
and explicitly disables Git's implicit lazy fetching during reader operations.
No new workspace root or separate Git repository is needed.

The terminal uses Ratatui 0.30 with Crossterm 0.29, consistent with the
[official installation guide](https://ratatui.rs/installation/). A transient app
model feeds the same input/render path in production and tests. Keep graph,
query, history and local watcher concerns separate enough for bounded tasks;
do not create unused feature stubs merely for parallelism. Rendering clips to
the terminal viewport instead of allocating a buffer for the entire graph.

The source key proposal assigns `h` both to left navigation and History.
Preserve the explicit History shortcut: `h` opens History; Left or Ctrl-h moves
toward prerequisites. `j/k/l` and arrows retain their usual direction.
Shift-arrows pan without changing selection. Help must explain the resolved
bindings. The recent window is 24 hours; no optional duration flag is required.
Polling targets 500 ms; reload failures preserve a visibly marked last-good
snapshot and can recover on a later valid ref.

## Test boundaries

1. Real Git repositories and linked worktrees, driven by the installed `aye`
   and `aye-view` binaries. Fixtures live in ignored package `test/` directories,
   never in a temporary build/install prefix. Snapshot source HEAD, branch,
   index bytes, tracked/untracked contents, configuration and task ref before
   and after viewer-only sessions. Trace invoked Git commands to reject network
   or mutation operations. Changes deliberately made by the test writer are
   distinguished from viewer effects.
2. The real app input/update/render path with Ratatui's terminal test backend,
   plus an actual pseudo-terminal session for startup, resize and restoration.
   Assert visible content and navigation outcomes; use focused model checks
   only where coordinates, reachability or bounded row materialization cannot
   be inferred reliably from a cropped terminal frame.

## Cases defined before code

### A. Canonical reading, errors and isolation (slice 1)

Success: a mixed fixture opened at the repository root, a nested directory and
a linked worktree shows identical IDs, correct effective states, claims,
manual blockers and details. Completed prerequisites satisfy dependents;
cancelled prerequisites do not. Parent/discovery remain detail context only.
An empty initialized store gives the documented create hint.

Failure cases: outside Git, absent task ref, unsupported version and corrupt
canonical state each produce a useful distinct error and clean terminal exit.
Stale/missing projections still show current canonical content without repair.
A dirty source index/worktree and all canonical state remain byte-identical
after browsing, including errors. Long Unicode/control-containing input must
not escape the pane or issue terminal control sequences.

### B. Graph semantics and readability (slice 2)

Use a diamond dependency graph plus a completed ancestor, a cancelled
prerequisite, an unrelated old closed task, an isolated manual blocker and a
separate active component. Assert every edge points from prerequisite to
dependent, layers respect that order, closed prerequisite ancestry remains,
unrelated history is absent, and neither manual/parent/discovery adds an edge.
Repeated renders and shuffled input order preserve layout. Long titles truncate
only in nodes; the full title remains reachable in details. Dark/light/default
color and no-color rendering retain meaningful symbols and selection.

### C. Search and filters (slice 3)

Search title, ID and labels case-insensitively, select a result and reach its
details. Include a historical task hidden from Current Graph. No-match and
cancel flows preserve usable navigation. Each of effective state, priority,
type, label and claimant changes visible results; combined filters intersect.
Closed-state filtering can expose matching closed tasks. Clearing filters
restores current visibility and meaningful selection. Filtered-out nodes
must not create dangling visible edges.

### D. Recent and history (slice 4)

At a fixed clock, test closed times inside, exactly at and outside 24 hours,
plus done and cancelled. Recent is off by default; on adds only unrelated
recent entries to a secondary region and never duplicates graph context.
History includes all closed tasks ordered by closed time descending with a
stable tie-break. With more than 100 closed tasks, start at about 50 exposed
rows and extend near the end; select a late row and show the correct details.
Empty history and returning to Graph/List remain usable. No history files or
index are written.

### E. Local refresh and recovery (slice 5)

Keep the app open in linked worktree B while `aye` creates, claims, updates and
closes tasks in A. Observe changes within the polling interval plus bounded
reload time without network. An unchanged OID must not reload all task blobs.
Read a pinned commit consistently even if the ref changes during loading.
Keep the selected task when visible; choose a deterministic neighbor when it
becomes hidden. A corrupt/new-version replacement retains a clearly marked
last-good display (or useful error), and the next valid state recovers. Manual
refresh works even when the ref has not changed. Packed refs remain readable.

### F. Focus, keyboard and viewport (slice 6)

Focus a task in a branching graph: include every prerequisite ancestor and
dependent descendant, exclude unrelated branches, retain only dependency
edges. Direction keys choose prerequisites/dependents or same-layer neighbors.
Panning changes the viewport without changing selection. Graph/List toggles
preserve selection where possible. Empty selection makes Focus a harmless
action. All controls are reachable with the keyboard and help agrees with the
actual bindings. Narrow detail navigation and scrolling reveal full long text.

### G. Integrated terminal and scale acceptance (slice 7)

Use the installed executable in a pseudo-terminal, exercise search, filter,
recent, history, focus, list, help, refresh, resize and quit. Check normal and
error exits restore terminal state. Capture actual frames at wide, narrow and
tiny sizes and inspect clipping, symbols, full-text access and selection.

Seed 10,000 total tasks, 1,000 active and 100 ready. Measure initial load, unchanged
polling, changed-ref reload, graph/list/search/focus transitions and first/next
history windows. Report times and input-to-frame responsiveness; do not infer
speed from correctness tests. Require no random task omission, no whole-history
row rendering, no per-frame Git/blob reload, and no source/task/network writes.
Record machine and fixture shape alongside timings before claiming scale support.

### H. Delivery (slice 8)

From repo root, the documented install uses `cargo install --path aye-view
--force --locked`, honoring Cargo defaults. Package checks reproduce the above
evidence. The guide lists exact keys, local-only/read-only scope, state meanings,
errors and operational limits. Repository-wide Rust/fixture ignore patterns
cover this package while retaining Cargo.lock and durable verification sources.
The aye CLI's established behavior remains verified if its shared reader changes.
All task branches were independently reviewed before merge; all agent-created
worktrees are removed after their committed work is secured.
