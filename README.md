# aye tools

Two Rust tools share the root `.git/`, source history and task state: **aye**
manages tasks; **aye-view** explores them in a read-only terminal UI.

| Directory | Contents |
| --- | --- |
| [aye/](aye/README.md) | Git-native task manager CLI, Cargo package and development commands |
| [aye-view/](aye-view/README.md) | Live dependency graph, List, Focus, closed History and selected-task details |
| [aye/skill/](aye/skill/README.md) | Portable agent skill and human installation guide |
| [aye/verification/](aye/verification/CASES.md) | Behavior cases and executable verification scripts |

## Install

From this repository root, with Rust/Cargo and a native build toolchain:

```sh
cargo install --path aye --force --locked
cargo install --path aye-view --force --locked
```

Cargo uses its configured/default installation location, normally `~/.cargo/bin`.
Git is required for the aye CLI. Run either tool inside the target repository or
a linked worktree. Initialize task state with `aye init` when needed; see the
[CLI guide](aye/README.md) for initialization and explicit sync choices.

```sh
aye-view
```

## Explicit verification bypass

When implementation, review and available checks are complete but a named external
condition cannot be tested, an authorized user can release downstream work without
claiming that the missing checks passed:

```sh
aye bypass TASK_ID \
  --reason "Target hardware is unavailable" \
  --missing "Physical-device smoke test" \
  --missing "Bluetooth reconnect test"
```

The operation is atomic. It refuses unresolved task prerequisites, resumes deferred
work when necessary, clears only an external/manual blocker, preserves existing
labels, records the reason and missing checks in an attributed note, and adds the
reserved `aye:bypassed` marker before closing the task as dependency-satisfying.
Agents must not authorize this risk acceptance without explicit user approval or a
pre-existing project policy covering the exact missing checks.

The task format remains version 1: older aye versions see dependency-compatible
`closed(done)` data plus the label and note. Current aye-view renders the marker as
`⚠ closed(bypassed)`, so bypassed prerequisites stay visibly different from fully
verified `✓ closed(done)` work.

## View tasks

aye-view reads canonical local state from `refs/agent-tasks/state`. It observes
other linked worktrees' updates without network sync, task mutation or projection
repair. Reload errors retain a marked last-good display. The native reader
currently supports SHA-1 repositories; SHA-256 returns a compatibility error.

The default graph includes current work and closed prerequisite context. An
arrow `B -> A` means A depends on B; cancelled prerequisites still block. Parent
and discovery relations appear in details. `╳` marks lines crossing without a
join. Status symbols remain meaningful without color; `NO_COLOR` disables colors.

| Key | Action |
| --- | --- |
| Left / Ctrl-h, Right / l | Select a visible prerequisite or dependent in Graph |
| Up / k, Down / j | Same-layer graph navigation, list movement, or focused text scrolling |
| Shift-arrows | Pan Graph without changing selection |
| `-`, `+` / `=`, `0` | Graph Main: Compact, Standard, reset to Standard |
| Tab | Graph/List; in History, switch pane |
| Enter, Esc | Open details / go back |
| `/`, `f` | Search title/ID/labels, or filter state/priority/type/label/claimant |
| `F`, `g` | Focus selected dependency context / return to full Current Graph |
| `c`, `]` | Toggle recent closed (24 hours) / select a task in that secondary region |
| `h` | Closed History, newest first, exposed in batches |
| `r` | Force local refresh |
| `?` | Help; arrows or PgUp/PgDown scroll it |
| `q`, Ctrl-c | Quit |

Entering Focus clears filters; subsequent filters narrow its context. An outside
search result exits Focus. Details and Help scroll, and narrow terminals show a
single readable pane. The [viewer guide](aye-view/README.md) lists all
mode-specific controls, state symbols, errors and recovery steps. In Search,
letters (including `q`) enter text; Ctrl-c quits from every mode.

Integrated viewer validation passed with a 10,000-task fixture (1,000 active,
100 ready): the recorded run started in 0.320s and reloaded in 0.678s. These are
single-run observations for a chain-shaped graph, not performance guarantees
for arbitrary dense graphs. See [results and limits](aye-view/verification/RESULTS.md)
and the [reproduction guide](aye-view/verification/README.md).
Dependency paths use source colors within each layer. Graph offers Standard and
Compact density; zoom preserves selection and viewport context without changing
task scope. Density is session-only; `0` resets it without recentering.

## Development checks

From the repository root:

```sh
make -C aye test
make -C aye-view test
```

The aye target runs formatting, strict Clippy, Rust tests, installation and
installed CLI integration suites. The aye-view test target runs formatting,
strict Clippy and Rust model/input/render tests, including the 10,000-task case.
For the installed viewer's keyboard, refresh, read-only, terminal-restoration
and scale checks, install the Python PTY dependencies and run integration:

```sh
python3 -m venv aye-view/test/venv
aye-view/test/venv/bin/pip install -r aye-view/verification/requirements.txt
make -C aye-view integration PYTHON=test/venv/bin/python
```

The integration target checks and installs aye-view through Cargo defaults,
then runs the installed binary; it also requires installed aye and Git.
See the [verification guide](aye-view/verification/README.md) for evidence files
and coverage limits. Shared aye source changes also require `make -C aye test`.

Package `target/` outputs and `test/` fixtures are ignored and can be deleted;
builds/tests recreate them. Keep Cargo.lock files and durable verification
sources. Source commits and task synchronization remain separate operations.
