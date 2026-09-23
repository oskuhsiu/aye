# aye-view

A read-only terminal explorer for local aye tasks: dependency Graph, List,
selected-task details, Search, filters, Focus, Recent and closed History.
Use the [aye CLI](../aye/README.md) to change tasks or synchronize them.

## Install and start

With Rust/Cargo and a native build toolchain, run from the repository root:

```sh
cargo install --path aye --force --locked
cargo install --path aye-view --force --locked
```

From the `aye-view/` package directory, the equivalent viewer command is
`cargo install --path . --force --locked` (also `make install`). Cargo uses its
configured/default installation location, normally `~/.cargo/bin`; make sure
that directory is on PATH. Both packages track Cargo.lock.

Run inside a Git repository with initialized aye state, from a nested directory
or from any linked worktree:

```sh
aye-view
```

An interactive terminal on stdin and stdout is required. `aye-view --help`
(`-h`) and `aye-view --version` (`-V`) work without opening the UI; other arguments
are unsupported. For monochrome output, use `NO_COLOR=1 aye-view`.

If task state is absent, use `aye init` to discover/adopt configured remote
state, or deliberately choose `aye init --offline` for a new local project.
These are separate, state-changing CLI operations; the viewer never initializes
or syncs. See the CLI guide before choosing initialization for an existing project.

## Read the graph

Current Graph contains every non-closed task and its prerequisite ancestors,
including closed ancestors. Isolated tasks and disconnected components remain
visible. `B -> A` means A depends on B; only `closed(done)` satisfies a dependency.
A cancelled prerequisite still blocks its dependents. Parent and discovery
relationships appear in details, not as graph edges. A manual blocker adds no
synthetic node. `╳` marks a crossing without a join.

Dependency paths use a repeating cyan, magenta, yellow, blue, green, red palette.
Within each visible layer, full source IDs are sorted to assign colors; branches
from one source keep the same color. Mixed-source crossings, convergence strokes
and shared arrowheads are neutral. Colors identify paths, not task status; node
colors retain their status meaning. Scope changes may reassign path colors.

| Symbol | State | Meaning |
| --- | --- | --- |
| `●` | ready | Open, no manual blocker, and all prerequisites are done |
| `▶` | in_progress | Claimed by the actor shown in details |
| `!` | blocked | Open with an unmet prerequisite or manual blocker |
| `⏸` | deferred | Explicitly postponed; aye must resume it before it can be claimed |
| `✓` | closed(done) | Completed; satisfies dependents' prerequisites |
| `×` | closed(cancelled) | Cancelled; does not satisfy dependents' prerequisites |

Symbols and text remain meaningful without color. Nodes may abbreviate titles;
Enter opens the full details, including IDs, acceptance, blockers, relationships
and notes. Wide terminals show both panes; narrow terminals show the active pane.
Up/Down or j/k and PgUp/PgDown scroll details, including wrapped Unicode text.
Esc returns to the main pane.

## Keys by mode

Keys are case-sensitive. Search and Filter consume keys before main-view
shortcuts. Ctrl-c quits from every mode; `q` quits outside those two dialogs.

| Main-view key | Action |
| --- | --- |
| Left, Ctrl-h or Backspace | In Graph, select a visible prerequisite |
| Right or `l` | In Graph, select a visible dependent |
| Up / `k`, Down / `j` | In Graph, select a same-layer neighbor; in List, move one row |
| Shift-arrows | Pan Graph without changing the selected task |
| `-`, `+` / `=`, `0` | Graph Main only: Compact, Standard, reset to Standard |
| Tab | Toggle Graph/List and return to the main pane |
| Enter | Open selected-task details |
| Right / `l` in List or History | Open details |
| Esc, Left, Backspace or Ctrl-h in details | Return to the main pane |
| `/` | Search all canonical tasks, including hidden and closed tasks |
| `f` | Open the five-filter dialog |
| `F` | Focus the selected task's prerequisite ancestors and dependent descendants |
| `g` | Return to full Current Graph, clearing Focus and filters |
| Esc in focused main pane | Return to full Current Graph, clearing Focus and filters |
| `c` | Toggle unrelated tasks closed in the last 24 hours (off initially) |
| `]` | Cycle selection through that secondary Recent region |
| `h` | Open all-closed History (`h` is not left navigation) |
| `r` | Force a local refresh, even when the state ref has not changed |
| `?` | Open Help |
| `q` / Ctrl-c | Quit, restoring the terminal |

| Dialog or pane | Keys |
| --- | --- |
| Search | Type title/ID/label text (case-insensitive); Backspace deletes; Up/Down select a result; Enter reveals it; Esc cancels. Letters such as `j`, `q` and `f` enter text. |
| Filter | Up/Down or Tab/Shift-Tab choose state, priority, type, label or claimant; Left/Right or Space cycle values; `c` clears the draft; Enter applies; Esc cancels. |
| Details | Up/Down or `k`/`j` scroll one line; PgUp/PgDown scroll a page; Esc returns to main. |
| History main pane | Up/Down or `k`/`j` move rows; PgUp/PgDown move a page and expose more rows near the end; Enter opens details; Tab switches panes; Esc returns to the previous view. |
| Help | Up/Down or `k`/`j` scroll; PgUp/PgDown page; Esc or `?` closes Help. |

Filters intersect. To recover from an empty filter result, press `f`, `c`, Enter,
or use `g` for full Current Graph. `g` retains the Recent toggle. A chosen Search result can temporarily bypass
filters; moving to another task or applying filters ends that reveal. No-match
Search stays open until you edit the query or cancel.

Focus captures a fixed root even as selection moves. Entering it clears filters;
later filters narrow its scope. It excludes unrelated siblings and descendants'
other prerequisites. Searching for a result outside Focus exits Focus. Recent
is hidden during Focus. History temporarily overrides Focus, retains filters,
and includes both done and cancelled tasks, newest closure first. It exposes
about 50 rows initially, then adds batches as you move; only viewport rows are
rendered. If History appears empty, clear any active filters.

## Local refresh and recovery

The viewer reads canonical `refs/agent-tasks/state` through the local native
reader. Linked worktrees share that ref, so another worktree's aye changes become
visible without synchronization. The viewer polls every 500 ms; parsing and
rendering add latency. `r` forces a reload. An unchanged or previously failed OID
does not repeatedly load task blobs during automatic polling.

The viewer does not write task data, source/index files, refs, commits,
configuration, projections or preferences, and never fetches or syncs. Stale or
missing derived views do not block canonical reading. UI settings are temporary.
Separate clones need an explicit aye sync when remote exchange is wanted;
that operation belongs to the CLI and can both fetch and push.

| Error or symptom | Next step |
| --- | --- |
| `NOT_GIT_REPOSITORY` | Change into the intended repository or linked worktree. |
| `NOT_INITIALIZED` | Initialize/adopt task state through aye; an ordinary source clone may not contain the custom task ref. |
| `FORMAT_VERSION_UNSUPPORTED` | Use a viewer/aye version compatible with the stored format; the viewer cannot migrate state. |
| `STATE_CORRUPT` | Check the canonical state or missing local objects with the CLI's diagnostics/recovery workflow. The viewer will not repair or fetch missing objects. |
| `OBJECT_FORMAT_UNSUPPORTED` | This build supports SHA-1 Git repositories only; SHA-256 repositories are unsupported. |
| `GIT_ERROR` | Inspect the accompanying local repository/permission error. |
| Interactive-terminal error | Run directly in a terminal, with stdin/stdout attached to it. |
| Marked last-good display after reload failure | Treat the display as stale. After the underlying issue is fixed, a new valid ref reloads automatically; if objects were restored at the same OID, press `r`. |

Startup failures print an error and exit. Reload failures keep the last valid
snapshot visibly marked instead of presenting invalid data as current. An empty
initialized store is valid and shows a hint to create tasks. For CLI recovery,
see [diagnostics](../aye/skill/refs/diagnostics.md).

## Checks and measured limits

From the repository root:

```sh
make -C aye-view test
python3 -m venv aye-view/test/venv
aye-view/test/venv/bin/pip install -r aye-view/verification/requirements.txt
make -C aye-view integration PYTHON=test/venv/bin/python
```

`test` (also `check`) runs formatting, strict Clippy and Rust tests. `integration`
also installs the viewer using Cargo defaults and exercises the installed binary
in Unix PTYs; it requires installed aye, Git and Python 3. Shared aye source
changes additionally require `make -C aye test`. See the
[verification guide](verification/README.md) for reproducible scenarios and
evidence locations. Package `target/` and `test/` outputs are ignored; keep
Cargo.lock and tracked verification sources.

The [2026-09-17 results](verification/RESULTS.md) cover 43 Rust tests and five
installed PTY sessions with 37 frames. A 10,000-total/1,000-active/100-ready
fixture started in 0.320s and reloaded in 0.678s. These are single-run
measurements including harness overhead, from 100 chains rather than arbitrary
dense DAGs. All canonical tasks are loaded in memory; History batches visible
rows, not canonical data. No visual theme matrix was measured; the installed
suite uses NO_COLOR. The [2026-09-23 path-color checks](verification/PATH_COLORS.md)
passed 45 Rust tests, an installed colored/monochrome crossed-pair probe and the
five-session integration suite. They verify palette identities and neutral
crossings, not contrast across every terminal theme.

The checked 50x18 terminal supports narrow-pane navigation. At 20x6 there is no
room for a task row; resize to browse. Missing partial-clone objects fail locally.
Graph starts at Standard density (28x4-cell nodes); Compact uses 20x4-cell
nodes and tighter rows. `-` selects Compact; `+` or `=` selects Standard;
`0` resets to Standard without recentering. The graph border names the level.
These keys apply only in Graph Main, not List, History, Detail, Help or dialogs.

Zoom retains selected task, full details, Focus, filters and search reveal.
It keeps a visible selected node at its screen position where possible; after
panning away, a node near the viewport center anchors the change. Bounds and
keeping a previously fully visible selection onscreen may adjust the viewport.
Repeated requests at the current level do nothing. Resize retains the existing
selection-reveal behavior. Track-heavy graphs may gain less space than chains.
Density is session-only; there are no persistent layout settings. See the
[zoom verification record](verification/GRAPH_ZOOM.md) for checks and limits.
