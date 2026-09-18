# Integrated acceptance evidence — 2026-09-17

Task: `t-c13c61e2b64b46ebf63d`. PASS for the cases below. Production viewer and
shared aye implementation were unchanged; this delivery adds durable checks and
their invocation/documentation. Source base was `1c7f09d`. The installed viewer
was built with `cargo install --path . --force --locked` through Cargo defaults.

- Platform: macOS 26.6.2, x86_64; Python 3.14.7; aye 0.2.0, aye-view 0.1.0.
- Installed viewer SHA-256:
  `969387a8a6e9585c915e81ce03e95611c506ae2564095a2eb33e23dae4e442bc`.
- `make check`: formatting, strict Clippy and all 43 Rust tests passed.
- Installed PTY suite: five sessions, 37 captured frames; normal, Ctrl-c and
  three startup-error exits all restored termios. Evidence run:
  `aye-view/test/integrated-h1xu5bd7/` (ignored, copied to primary before cleanup).
- Existing root README.md and .gitignore edits remained byte-identical.

## Target-scale observations

Fixture: 10,000 total, 1,000 open/active, 100 ready, 900 dependency-blocked and
9,000 closed(done). One hundred ten-node chains, with ten completed ancestors,
produce 1,010 current nodes and 910 edges. Derived projections are deliberately
stale initially. History starts at 50 of 9,000 entries and expands to 100.

| Operation | Observed seconds |
| --- | ---: |
| Installed startup and first frame | 0.320 |
| Graph to List | 0.076 |
| Search query (`Scale 00999`, all keystrokes) | 0.200 |
| Focus selected chain | 0.075 |
| Restore Current Graph | 0.106 |
| Apply ready filter | 0.075 |
| Enable Recent | 0.103 |
| Open first History window | 0.072 |
| Page into next History window | 0.076 |
| Changed-ref reload after explicit writer returns | 0.678 |

These are one-run measurements, not percentile estimates or performance limits.
They include Python/PTY emulation, 10 ms sampling and a 50 ms frame drain. The
explicit aye update took another 0.927 seconds, excluded from the reload value.
An unchanged 1.6-second polling observation preserved the repository snapshot;
load-count assertions come from the watcher tests, not inferred PTY timings.

The scale input/render test checks the full expected node and dependency sets,
search to the last active task, exact focused membership, all 9,000 closed IDs,
50-to-100 batching, one reused History index and at most 35 rendered rows at
190x40. Canonical tasks remain identical after all input/render operations.
This structured chain fixture does not establish latency for dense arbitrary DAGs.

## Runtime behavior and isolation

The installed binary was opened from a nested linked worktree and exercised
Graph, List, Search, Filter, Focus, Recent, History, Detail, Help and resize.
Live writes in the fixture's other checkout covered create, claim, title update
and close after packed refs. Missing canonical objects and unsupported project
versions retained the marked last-good frame. Restoring the missing object at
the same OID required manual refresh; restoring a valid ref recovered
automatically. Live observations ranged from 0.200 to 0.660 seconds, with
manual refresh at 0.320 seconds and automatic recovery at 0.438 seconds.

Source/index/config/refs/object bytes and file modes were compared around
viewer-only periods, including stale views and errors. Explicit writer mutations
established each new baseline. No viewer Git subprocess was recorded by the
PATH trap or Git trace; the configured local HTTP promisor-remote trap observed
zero connections. This is not a system-wide network syscall audit. The native
reader's local git2 calls and disabled network features were also inspected.

Startup errors outside Git, with a missing task ref, and with an unsupported
format produced their distinct error codes and clean terminal exits. The first
harness attempt incorrectly placed the outside-Git case inside the parent
checkout; the corrected passing run uses the system temporary directory only
as process cwd. All generated fixture/evidence files remain under package test/.

## Frame review and limits

Decoded actual terminal frames were reviewed at 190x40, 50x18 and 20x6. The wide
frame shows completed context, ready/blocked symbols, dependency arrows and
Chinese detail text within panes. Narrow mode exposes the selected historical
task ID and state in the detail pane with an Esc route back. The tiny frame stays
bounded and accepts resize/quit, but has no room for a task row and truncates its
status/footer; it is not a useful browsing size. Resizing back restores the UI.
Raw PTY output and termios reports accompany the decoded frames.

The installed run uses NO_COLOR. Existing Rust cases cover colored/monochrome
symbols, crossed/long edges, full detail scrolling, filters and focus semantics.
No visual theme matrix or arbitrary dense-graph benchmark was run. Shared aye
source did not change, so its full regression suite was not rerun. The user
requested a single agent: implementation and final self-review were performed
without an independent subagent review. Reproduction is in [README.md](README.md).

## Final delivery recheck — 2026-09-18

Task `t-273e1fb8fe8c125a9ee9` adds the root/package user guides without changing
production source or verification code. `make -C aye-view integration
PYTHON=test/venv/bin/python` passed: formatting, strict Clippy, 43 Rust tests,
default-location Cargo installation, and five installed PTY sessions / 37 frames.
The run used the primary checkout's Cargo target cache via CARGO_TARGET_DIR;
installation still used Cargo defaults. Python dependencies were installed into
the task checkout's ignored `aye-view/test/venv` from requirements.txt.

Platform and Python match the preceding run. The harness recorded production
source base `9614948e35f6349d34a640a780f1a1433b731841` before the documentation
commit. Installed viewer SHA-256:
`445e05b7de96625c42a5fdd7f7e147dd665b7203f2983e0211586a8c689a9365`.
Evidence is retained in primary `aye-view/test/integrated-ez46_of4/`:
result.json, decoded frames, raw PTYs and terminal-restoration records.

The same 10,000-task chain fixture started in 0.2841s and reloaded in 0.5238s;
first/next History windows took 0.0723s/0.0736s. The explicit writer took 0.8178s,
excluded from reload timing. Read-only snapshots, stale-view preservation,
linked-worktree updates, reload recovery and terminal restoration passed. Git
subprocess calls and configured-remote connections were both zero. The primary
reviewed decoded wide/narrow/tiny frames; the 20x6 no-task-row limitation remains.
All earlier measurement and coverage limits still apply. Shared aye source was
unchanged, so its full suite was not rerun.

An independent reviewer inspected the earlier slice 7 changes and compared its
committed report with preserved result.json, finding no actionable issues. This
is a later review of that evidence; the user-authorized omission of independent
review before the original slice 7 merge remains part of the historical record.
Current guide review and final integration/cleanup are recorded in the delivery
task's aye notes.
