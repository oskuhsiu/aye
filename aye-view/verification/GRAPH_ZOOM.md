# Graph density / zoom

## Behavior contract (before implementation)

Standard uses 28x4 cells per node with a six-row step; Compact uses 20x4 cells
with a five-row step. Track spacing, long-edge corridors, separate input/output
ports, dependency pairs, layers and source colors retain their semantics.

Graph Main alone accepts `-` for Compact and `+`, `=` or `0` for Standard.
Repeated requests at the current level are complete no-ops; reset does not
recenter. Help, query modals, List, History and Detail isolate these keys.
The graph border begins with the current density, including long Focus titles.

Changing density preserves selection, full details, detail scroll, Focus,
filters and temporary search reveal. A visible selected node anchors the change;
otherwise use the nearest viewport-center node, preferring intersecting nodes
and resolving ties by ID. Preserve its signed screen offset and clamp to graph
bounds. A previously fully visible selection remains visible when possible;
an offscreen selection must not override the user's pan on subsequent frames.

Verification cases: compact fits more complete chain nodes in the same terminal;
IDs/layers/pairs match; crossing and long-edge geometry/source colors hold at both
levels; visible and panned/offscreen anchors survive repeated frames and reset;
scopes and modal keys remain correct; Unicode, empty/tiny viewports, reload,
resize and world coordinates beyond u16 remain safe. No persistence or writes.

## Evidence

Developer checks on 2026-09-23, host `Dark-MBP-1063.local`, worktree
`/Users/apple/.herdr/worktrees/aye/feature-viewer-graph-zoom`, branch
`feature/viewer-graph-zoom`, base `7ff4382`:

- Before production changes, the new input/render test
  `zoom_compact_fits_more_complete_nodes_and_preserves_details_and_truth`
  failed on `app.graph.width < old_graph.width` after pressing `-`.
- `cargo test --manifest-path aye-view/Cargo.toml zoom_tests -- --nocapture`
  passed all six focused tests after implementation. They cover more complete
  chain nodes in the same viewport, exact IDs/layers/pairs and full Unicode
  detail preservation; visible/partially clipped/offscreen selected anchors;
  Shift-pan preservation, repeat frames, density-limit no-ops and reset;
  minimal expansion reveal and deterministic center ties; Focus/filter/search
  reveal/detail scroll; six non-graph surfaces; empty/tiny/resize/reload and a
  2,500-node chain beyond u16 world coordinates at both densities.
- `cargo test --manifest-path aye-view/Cargo.toml graph::tests -- --nocapture`
  passed all seven graph tests. Existing crossing, long-edge and both source-color
  tests now run at both densities, retaining their meaningful geometry and
  terminal-cell assertions, including neutral mixed-source arrowheads.
- Final `make -C aye-view check` passed formatting, strict Clippy and all 51 Rust
  tests. `git diff --check` passed. Cargo commands used
  `CARGO_TARGET_DIR=/Users/apple/Projects/codex-utils/aye/aye-view/target`.

Graph geometry now derives from the stored density; App includes density in its
cache key and uses one geometry transition that does not recompute scope or reset
selection/details. Graph Main handles the new keys after modal/help/history
handling. The density label precedes the graph title, including long Focus titles.
README controls and in-app Help describe the same keys. No dependencies, canonical
schema, settings, reader, storage or writer paths changed.

Limits: rendering was verified through actual TestBackend terminal cells, not a
terminal-theme screenshot matrix. Compact saves less space when routing tracks
dominate the graph. No developer installation, task mutation, source publication
or repository fingerprint probe was performed. Independent review, installed
PTY acceptance, source integration and worktree cleanup remain primary-owned.
No separate developer log files were created; command outputs are in the agent
transcript.


## Independent review and installed acceptance

On 2026-09-23 the primary verified candidate
`024a0b6d6dbdc3d8dd58c301fcacf16c28dcd50a`, the production source covered by
51 passing Rust tests above. Independent Standards and Spec static reviews each
reported zero findings. The installed `/Users/apple/.cargo/bin/aye-view` had
SHA-256 `6500ba2e7653c0610ca1be5fe186c8422b56306cfbaa423e5a09121b0be11391`.

- The dedicated installed zoom PTY captured 36 frames. In the same 50x18 terminal,
  Standard displayed one complete chain node and Compact displayed two. Actual
  key flows verified selected details, Focus and filters, offscreen selection
  with pan/zoom, limit no-ops and reset, List/History/Detail/Search key isolation,
  tiny-to-wide resize and Help. Repository fingerprints were unchanged, terminal
  state was restored and no Git subprocesses were invoked.
- Two additional colored/NO_COLOR PTYs captured four frames through Compact.
  Independent source strokes retained `00cdcd` and `cd00cd`, `╳` was neutral,
  and NO_COLOR had no route colors. Both sessions restored their terminals and
  preserved repository fingerprints without Git subprocesses.
- The complete installed suite passed five sessions / 37 frames, including
  read-only behavior, live refresh and error recovery. The 10,000-total /
  1,000-active / 100-ready fixture started in 0.2610s and reloaded in 0.5918s.
  No Git subprocesses or configured-remote connections were observed. These are
  single-run measurements with harness overhead, not dense-DAG guarantees.
- The primary visually inspected decoded Standard/Compact, panned-context and
  Help frames. No general terminal-theme contrast matrix was measured.

Evidence is retained in the primary checkout at
`aye-view/test/viewer-features-20260923/`: `zoom-review.json`,
`zoom-runtime-retry/result.json`, `compact-colors/result.json` and
`zoom-integration/result.json`, alongside full copied integration artifacts.
Probe `primary_source_head` / `source_head` fields identify the primary checkout
at probe time; `installed_source_commit` pins `024a0b6`. The full suite pins the
same source and binary hash.

The first `zoom-runtime` attempt failed a harness predicate expecting the full
word `filtered`, which the narrow header clipped to `filtere`. The corrected
predicate checks the visible one-task count; the failed evidence was retained.
An initial complete-node summary counted a routing-bend corner as a node. The
primary corrected the saved-frame metric to full node borders (one versus two)
and updated the probe accordingly. Neither correction required production code
changes.

This final evidence update is documentation-only; the full Rust suite was not
rerun. Source integration, task completion and developer-worktree cleanup were
pending when this record was written.
