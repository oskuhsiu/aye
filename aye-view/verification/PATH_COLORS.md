# Dependency path colors

## Behavior contract (recorded before implementation)

- Assign the six ANSI colors cyan, magenta, yellow, blue, green, red to sorted
  full source IDs within each visible layer; cycle after six sources. Only nodes
  with outgoing visible edges count. Disconnected components share layer ordering.
- Same-source branches retain exactly the same color, including shared strokes.
- Independent crossed paths differ on unshared strokes. Mixed-source crossings,
  convergence strokes and common arrowheads remain terminal-default neutral.
- Repeated frames, reordered visible IDs/dependencies and reordered edge drawing
  retain identical output. Color assignment can change when visible scope changes.
- Monochrome keeps identical glyphs and modifiers. Node semantic colors, selected
  emphasis, graph IDs, dependency pairs and route geometry remain unchanged.
- Rendering remains local and read-only; no storage or interaction changes.

## Verification

Developer verification on 2026-09-23, host `Dark-MBP-1063.local`, branch
`feature/viewer-path-colors`, based on `bad0eb3`:

- Before production changes, `cargo test --manifest-path aye-view/Cargo.toml
  source_colors -- --nocapture` ran both new tests: both failed as expected with
  `Reset` versus expected `Cyan` at an outgoing source stroke.
- After implementation, `cargo test --manifest-path aye-view/Cargo.toml
  graph::tests -- --nocapture` passed all seven graph tests. An intermediate
  crowded-fixture test exceeded its 120-row test buffer; the buffer was enlarged
  to 160 rows. This was a test viewport error, not a production routing change.
- `make -C aye-view check` passed formatting, strict Clippy and all 45 Rust tests.
  The first check found one collapsible nested conditional; it was corrected
  before the passing run. `git diff --check` also passed.
- Cargo runs used
  `CARGO_TARGET_DIR=/Users/apple/Projects/codex-utils/aye/aye-view/target`.

The new TestBackend tests inspect actual terminal cells for independent source
colors, per-layer restart, fanout, nine-source cycling, neutral crossing and
convergence strokes/arrowheads, repeat and reordered-input equality, and exact
monochrome glyph/modifier equality. Existing tests retain route-geometry,
dependency-pair, node-state symbol, selection/navigation and clipping checks.
The production change only styles existing edge cells; layout, model, storage
and input handling are unchanged.

Limits: TestBackend verifies color identities, not perceptual contrast on every
terminal theme. No screenshot, installed-binary run, storage fingerprint check,
installation, source publication or task mutation was performed by this developer.
Independent Standards/Spec review and installed runtime evidence are owned by the
primary agent and must precede final task acceptance.


## Independent review and installed runtime

The primary verified production candidate
`b16943abf574aa6abc620f4123d346bdc01f3918` on 2026-09-23:

- Independent Standards and Spec reviewers each reported zero findings. Both
  reviews were static; runtime checks below were performed by the primary.
- Cargo-default installation produced `/Users/apple/.cargo/bin/aye-view` with
  SHA-256 `0aab20a8fbf25befe7d063fdfc35550b52bca376e843d5b9ddb49a6b900433cd`.
- A crossed-pair installed PTY probe observed source colors `00cdcd` and `cd00cd`,
  with neutral `╳`. Its NO_COLOR counterpart retained the crossing and had no
  route colors. Both PTYs restored terminal state; repository fingerprints
  remained unchanged and neither session invoked Git subprocesses.
- The complete installed suite passed five sessions / 37 frames, including
  input/navigation, read-only checks, live refresh and error recovery. Its
  10,000-total / 1,000-active / 100-ready fixture started in 0.3031s and reloaded
  in 0.5704s. There were no Git subprocesses or configured-remote connections.
  These are single-run harness measurements, not arbitrary-DAG performance or
  a universal terminal-theme contrast claim.

Local evidence is retained in the primary checkout under
`aye-view/test/viewer-features-20260923/`: `colors-review.json`,
`candidate/result.json` and `colors-integration/result.json`, plus copied full
integration artifacts. In the candidate probe, `source_head` identifies the
primary checkout at probe time; `installed_source_commit` explicitly identifies
candidate `b16943a`. The full-suite report also pins that candidate and binary hash.
Developer commands were captured in the agent transcript; no separate developer
log files were created.

This documentation update changes no production source and does not rerun the
Rust suite. Source integration, task completion and worktree cleanup were still
pending when this evidence was recorded.
