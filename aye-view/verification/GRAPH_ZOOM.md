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
