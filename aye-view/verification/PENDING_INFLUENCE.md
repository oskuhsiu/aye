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

Installed PTY acceptance and final package/review evidence are recorded after
execution. Physical terminal theme/gesture checks are outside PTY evidence.
