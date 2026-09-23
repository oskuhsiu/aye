# Behavior-first acceptance cases

The current custom-ref transport is covered by [CUSTOM_REFS.md](CUSTOM_REFS.md).
Direct acquisition and atomic writes are specified in [DIRECT_OPERATIONS.md](DIRECT_OPERATIONS.md),
with release evidence in [DIRECT_OPERATIONS_RESULTS.md](DIRECT_OPERATIONS_RESULTS.md).
The bootstrap sequence below records the original development order.

Cases are specified before their implementation. Tests use the installed `aye`
binary (`cargo install --path . --force`) and disposable repositories under
`test/`; compilation and installation use Cargo's defaults.

## Bootstrap

1. Initialize offline, create task with acceptance, inspect canonical JSON and
   ready list, claim, append note, close done. Another linked worktree sees each
   result immediately. A missing actor fails without a ref update; a non-owner
   cannot append a note or close a claim. Closed tasks disappear from ready/list.
2. Start two claim processes in different linked worktrees together: exactly one
   succeeds, loser reports TASK_ALREADY_CLAIMED. Concurrent creates both survive.
3. Compare source HEAD, index bytes and status before/after mutations, including
   staged/unstaged/untracked content: all unchanged; state has a separate root.
4. Invalid input, nonexistent task, invalid transition and corrupt canonical
   state fail with JSON envelope and correct exit class; no partial ref update.

## Subsequent tasks (to be created using bootstrap aye)

5. Lifecycle/ownership: release versus forced release (reason and audit note),
   defer/resume and cancelled/done/reopen transitions. Wrong owner and invalid
   transitions fail; actor flag > environment > worktree file, identities differ
   between worktrees. Reopen prerequisite of in-progress dependent must fail.
6. Blocking/metadata: discovery then block releases claim atomically. Completion
   of prerequisite makes dependent ready without changing dependent task bytes.
   Cancelled prerequisite still blocks. Manual and task blockers clear separately.
   Reject satisfied/self/duplicate/cyclic/missing dependencies, parent cycles and
   invalid metadata. Parent and discovery alone never block. Labels, metadata,
   acceptance, and note-file roundtrip; provenance cannot be updated.
7. Read/projection: ready ordering/filtering and active defaults; rich show reverse
   relations; deterministic report with last 20 sections. Canonical manual edit
   produces VIEW_STALE, reads correct state without commit. Rebuild preserves raw
   canonical bytes and repeated rebuild makes no commit. Doctor detects malformed
   schemas, paths, relations and claims and reports focused warnings.
8. Sync: first publish, init adoption, equal/local-ahead/remote-ahead, disjoint merge
   with two parents. Both sides editing one task conflicts, writes freeze across
   linked worktrees, reads remain possible. Resolve local/remote/file then continue
   or abort. Cross-file graph cycle requires repair before publication. Foreign
   project, invalid remote JSON and deleted previously-seen branch are rejected.
   Valid external JSON with stale views is repaired. A remote advance before push
   triggers safe fetch/reconcile/retry; never force push or alter source push config.
9. Packaging/docs: installed binary works from nested worktree directories, help
   covers v1 commands, JSON failures are parseable, FORMAT permits tool-less reads.
   Full suite, cargo fmt/check/clippy and source diff review before completion.

## Deliberate bootstrap cuts

Remote sync, complete lifecycle/relations, rich filters and diagnostics are valid
v1 requirements but unnecessary to bootstrap task ownership. They become actual
tracked work after case 1–4 pass. No database, daemon, extra command families,
query language, or configurable remote branch is introduced.

## Architecture decision

Use Git CLI plumbing with explicit cwd and piped object content, never checkout
or use the source index. A pure task domain owns invariants. The Git store owns
object IO and expected-old ref updates. Projections are pure deterministic
functions. Sync owns canonical three-way reconciliation and common conflict
metadata. CLI owns parsing, actor lookup and JSON envelopes. All task writes
revalidate after each bounded CAS retry. Build/install through Cargo defaults.
