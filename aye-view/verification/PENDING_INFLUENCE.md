# Stronger glyph emphasis (2026-10-05)

Affected unselected nodes use double borders, paths use heavy/mixed-weight glyphs
and affected arrowheads use ▶. Selection retains reverse video. Directional
weight preserves unrelated light arms at shared junctions; nonjoining ╳ and
status/source foregrounds remain unchanged. Geometry and influence rules remain
those documented below. BOLD is secondary; shape carries the distinction.

# Pending downstream influence

Selecting a Graph task keeps its existing bold reverse-video selection. Nonclosed
downstream dependency tasks and eligible connecting routes gain bold emphasis.
Status symbols, status colors and per-source path hues are unchanged. NO_COLOR
keeps the same glyphs and modifiers. Parent/discovery links and a dependent's
other prerequisites are not influence paths.

Every closed downstream target stops its branch before that node and its incoming
edge. This includes done, bypassed done and cancelled work. A selected done or
bypassed root has no pending influence. A selected cancelled root may influence
nonclosed dependents: cancellation still leaves its prerequisite unsatisfied.
Alternate eligible paths remain highlighted. This is pending work, not an
immediate-readiness promise or hypothetical reopening impact.

Influence uses the canonical snapshot's reverse dependency index, independently
of Graph visibility. Only existing in-scope nodes/edges are drawn. A hidden
nonclosed intermediary does not erase a visible descendant's influence. No
hidden task/edge is synthesized and scope, Focus, filters, selection, pan,
density and layout remain under existing controls.

The overlay is recomputed when Graph is rendered, so selection and live snapshot
changes cannot retain an old influence cache. Traversal is iterative; visited
targets bound work to reachable tasks and dependencies. Rendering retains the
terminal-sized stroke buffer and wide-coordinate clipping. A stroke cell is
bold when any eligible route passes it. Mixed-source cells remain neutral and
crossing/junction/arrow glyphs retain their existing meaning; unrelated segments
beyond the shared cell are ordinary.

## Verification

Durable Rust tests use production key/mouse selection, snapshot replacement and
Ratatui rendering. Cases cover transitive fan-in, ancestors/unrelated context,
manual blockers, deferred work and an in-progress root; closed/done/bypassed/
cancelled cutoffs and selected roots; alternate paths; filter/Focus with a hidden
intermediary; reopen/removal/missing selection; pan, both densities, resize and
empty/tiny frames. Canonical validation does not permit an in-progress task with
unresolved prerequisites, so the fixture uses a valid in-progress root and its
pending dependent rather than fabricating a blocked in-progress target.

The stroke test checks every route cell against the union of eligible paths,
including shared crossings, arrowheads and long edges. Colored/monochrome runs
assert identical glyphs, preserved foreground hues, neutral mixed crossings and
unchanged unrelated segments; reversed edge iteration produces the same frame.

On 2026-10-05, formatting, strict Clippy and 95 library plus five binary tests
passed. Independent Standards and Spec review found no substantive issues at
candidate `1f921625`; the Spec reviewer independently passed the six focused
influence tests. Cargo-default locked installation was built from `766a188`,
whose production source/manifests/locks are identical to that candidate.
Installed aye-view 0.2.0 SHA-256:
`756ecdb4cb6a995e066c02f5a0e5d370430da9db79842cc2845fbe6f36e286ec`.

The complete installed suite passed nine interactive sessions (150 frames) and
three startup-error sessions, including all existing browsing/copy/refresh
scenarios. Influence adds a 20-frame monochrome and seven-frame colored session.
Graph-local terminal attributes prove selected reverse video, affected bold,
ordinary other prerequisite/closed paths, preserved glyph/hue footprints, mouse
and key selection, both densities, resize, Focus/filter with a hidden intermediary
and live close/reopen/bypass updates. Canonically valid native atomic operations
stage intermediate closure without publishing temporary ready states. Repository
fingerprints, terminal restoration/capture cleanup, zero viewer Git subprocesses
and zero configured-remote contacts passed. Scale observations were startup
0.2736 s and reload 0.3416 s in the existing 100-chain/10,000-task fixture; these
single runs include harness overhead and do not characterize arbitrary dense DAGs.

Local evidence is archived in `aye-view/test/highlight-20261005/`, including
package/install logs, independent reports, PTY frames/raw bytes/cell attributes
and `installed-acceptance/result.json`. Initial harness compilation/invalid-fixture
failures are retained separately; `before-valid-fixture.log` is the actual
pre-implementation missing-highlight failure. Physical terminal themes/gestures
were not checked; PTY attributes do not prove universal font/theme contrast.

## Persistent manual fixture group

The user additionally requested semi-permanent test tickets. The working task
state contains 31 labelled `viewer-test-fixture-v1` records: guide
`t-139b6a3c585516415744` (`HL-00`) plus 30 scenarios. The guide is deferred and
contains the full ID/baseline manifest, expected influence sets and reset steps.
Ten records are deferred, 17 open/blocking and four closed (done/bypass/cancelled
plus an isolated closed History task), with zero ready fixtures and zero claims.
Bypass is explicit synthetic display data, not a production verification waiver.
These records remain in the canonical task ref; source publication and task
synchronization are separate operations.

A further installed read-only PTY probe on this actual project passed 35 frames,
12 selected-root cases, hidden-intermediary Focus/filter and the actual crossed
five-node fixture. It verified graph-local bold/reverse attributes and unchanged
task ref/source HEAD/worktree status. Baseline relationship/state checks matched
every fixture's expected influence set. Keep the group after successful tests;
temporary ready/in-progress exercises use the guide's C8 root and restore its
baseline. Empty/removal/race/large-scale cases belong to automated/disposable
tests rather than fabricated permanent records.
