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
visible. `B -> A` means A depends on B; only canonical `closed(done)` satisfies a
dependency. An explicitly authorized bypass is represented compatibly as
closed(done) plus an audit marker, so it satisfies dependents while remaining
visibly distinct from fully verified work. A cancelled prerequisite still blocks
its dependents. Parent and discovery relationships appear in details, not as graph
edges. A manual blocker adds no synthetic node. `╳` marks a crossing without a
join.

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
| `✓` | closed(done) | Completed and verified as required; satisfies dependents |
| `⚠` | closed(bypassed) | Missing named verification was explicitly accepted; satisfies dependents but is not fully verified |
| `×` | closed(cancelled) | Cancelled; does not satisfy dependents' prerequisites |

Symbols and text remain meaningful without color. Nodes may abbreviate titles;
Enter opens the full details, including IDs, acceptance, blockers, relationships,
labels and audit notes. A bypass note records the authorization reason and missing
checks; the viewer does not claim those checks passed. Wide terminals show both
panes; narrow terminals show the active pane. Up/Down or j/k and PgUp/PgDown scroll
details, including wrapped Unicode text. Esc returns to the main pane.

## Mouse browsing

In a terminal with mouse reporting enabled, single-click a visible Current
Graph node or List, Recent or History row to select it. Clicking a pane activates its keyboard
controls; clicking empty Main keeps the selected task. The compact `[Details]`
and `[Back]` controls work in wide and narrow layouts. Pane borders are inert
except for those controls. Search, Filter and Help isolate background clicks.
Mouse selection works at Standard and Compact graph density. Open Recent with
`c` or History with `h`; clicking their rows keeps that browsing context.

The wheel scrolls the surface under the pointer, retaining selection and keyboard
focus: List, Recent, History, Detail and Help scroll vertically; Graph pans.
Each event moves three rows or cells, clamped to content bounds. History exposes
more batches as you scroll, including its final matching task. List, Recent and
History keep independent viewports; deliberate keyboard selection reveals its
row again. Resizing keeps a previously visible selected row onscreen, while a
selection already hidden by manual scrolling keeps its independent viewport.
Reported horizontal wheel and Shift-wheel events pan Graph sideways;
horizontal input is ignored on vertical-only surfaces. Search and Filter keep
keyboard controls and consume pointer input.

Press the left button on Graph background (including dependency lines) and drag
to move the map in either axis. Task selection stays stable; a node press selects
instead of starting a pan. The gesture stays with Graph when crossing another
pane and ends on a reported release anywhere. A new press, keyboard navigation,
wheel input, resize, reload, density/scope/mode change or reported focus loss
cancels it. Terminals may lose releases outside their window; Escape cancels a
stale gesture. List dragging and scrollbars are deferred.

In Detail, drag with the left button to highlight text; releasing copies the
selection automatically. Ctrl+C copies that selection again without quitting.
A click without dragging never copies. Forward/reverse selection preserves whole
Unicode characters and original newlines; screen wrapping adds no copied newline.
Dragging outside Detail stays bounded to its visible text. To copy another part,
scroll there and start a new selection. Navigation, wheel input, a new press,
resize, reload or reported focus loss clears selection. Esc returns to Main;
`q` always quits. Copy-on-release also works when Cmd+C is handled by the terminal
rather than delivered to the app.

Local macOS uses the system clipboard and displays `Copied selection`. SSH and
other platforms send OSC52 and display `Copy sent to terminal`; the terminal and
any multiplexer must permit clipboard access. Native clipboard failure falls
back to this terminal route. RTF/PostScript-like snippets beginning with `{\rtf`
or `%!` also use that route to preserve literal text. Output failures show
`Copy failed` and keep the viewer usable. See the [copy contract](verification/DETAIL_COPY.md)
for reproducible checks and evidence limits.

For other panes, use your terminal's reporting override to select/copy text;
in iTerm2, hold Option to temporarily disable reporting. Check your terminal's
mouse-reporting settings if clicks do not work.
Keyboard controls remain available. This release is verified using xterm SGR
mouse input through Unix PTYs; this does not establish compatibility with every
terminal or physical mouse/touchpad. Mouse navigation is session-only and
read-only.

## Keys by mode

Keys are case-sensitive. Search and Filter consume keys before main-view
shortcuts. Ctrl-c copies a nonempty Detail selection; otherwise it quits from
every mode. `q` quits outside those two dialogs.

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
| `f` | Open the five-filter dialog; state includes `closed(bypassed)` |
| `F` | Focus the selected task's prerequisite ancestors and dependent descendants |
| `g` | Return to full Current Graph, clearing Focus and filters |
| Esc in focused main pane | Return to full Current Graph, clearing Focus and filters |
| `c` | Toggle unrelated tasks closed in the last 24 hours (off initially) |
| `]` | Cycle selection through that secondary Recent region |
| `h` | Open all-closed History (`h` is not left navigation) |
| `r` | Force a local refresh, even when the state ref has not changed |
| `?` | Open Help |
| `q` / Ctrl-c without a Detail selection | Quit, restoring the terminal |

| Dialog or pane | Keys |
| --- | --- |
| Search | Type title/ID/label text (case-insensitive); Backspace deletes; Up/Down select a result; Enter reveals it; Esc cancels. Letters such as `j`, `q` and `f` enter text. |
| Filter | Up/Down or Tab/Shift-Tab choose state, priority, type, label or claimant; Left/Right or Space cycle values; `c` clears the draft; Enter applies; Esc cancels. `closed(bypassed)` selects only current bypasses. |
| Details | Left drag selects; release copies; Ctrl+C copies again. Up/Down or `k`/`j` scroll one line; PgUp/PgDown scroll a page; Esc returns to main. |
| History main pane | Up/Down or `k`/`j` move rows; PgUp/PgDown move a page and expose more rows near the end; Enter opens details; Tab switches panes; Esc returns to the previous view. |
| Help | Up/Down or `k`/`j` scroll; PgUp/PgDown page; Esc or `?` closes Help. |

Filters intersect. To recover from an empty filter result, press `f`, `c`, Enter,
or use `g` for full Current Graph. `g` retains the Recent toggle. A chosen Search
result can temporarily bypass filters; moving to another task or applying filters
ends that reveal. No-match Search stays open until you edit the query or cancel.

Graph selection uses reverse video; double-bordered nodes and heavy lines with
solid arrowheads show pending downstream influence. It follows dependency links through nonclosed work and stops before
every closed downstream task. Selecting done or bypassed work has no influence;
selecting a cancelled prerequisite may still affect pending dependents because
cancellation does not satisfy it. Highlighted tasks can remain blocked by other
prerequisites or manual blockers. Status colors and source-path hues stay intact,
and the emphasis works with NO_COLOR.

Influence is calculated from canonical tasks, even through a filtered-out pending
intermediary, then applied only to the nodes and edges already in Graph. Selection
does not enter Focus, reveal hidden tasks, clear filters or alter layout/pan.
Shared crossing cells can be bold where an affected route passes; the other
route stays ordinary beyond the shared cell. See the
[influence verification record](verification/PENDING_INFLUENCE.md).

Focus captures a fixed root even as selection moves. Entering it clears filters;
later filters narrow its scope. It excludes unrelated siblings and descendants'
other prerequisites. Searching for a result outside Focus exits Focus. Recent
is hidden during Focus. History temporarily overrides Focus, retains filters,
and includes done, bypassed and cancelled tasks, newest closure first. It exposes
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
rows, not canonical data. No visual theme matrix was measured; those installed
sessions used NO_COLOR. The [2026-09-23 path-color checks](verification/PATH_COLORS.md)
passed 45 Rust tests, an installed colored/monochrome crossed-pair probe and the
five-session integration suite. They verify palette identities and neutral
crossings, not contrast across every terminal theme. The subsequent
[zoom acceptance checks](verification/GRAPH_ZOOM.md) passed 51 Rust tests, a
36-frame installed zoom session, four colored/monochrome frames and the complete
five-session integration suite. Bypass adds focused coverage for status rendering,
state filtering and details. In a 50x18 chain view, Compact showed two complete
nodes versus one in Standard. The latest scale run started in 0.2610s and reloaded
in 0.5918s, subject to the same single-run limits.

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
