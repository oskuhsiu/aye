# Direct operation contract (version 1)

`aye --json --actor EXECUTOR claim --next [--priority Pn --type TYPE --label LABEL]`
selects and claims one scoped ready task in priority/creation/ID order. Explicit
`claim ID --packet` never substitutes a different task; old `claim ID` retains
its output and ownership rules. Filters require `--next`.

The JSON envelope remains `ok/data/warnings`. Packet data includes `version: 1`,
`outcome` (`claimed`, `already_owned`, `no_ready_task`, `existing_assignments`),
`state_oid`, `project_id`, `actor`, `filters`, full canonical `task` and `computed`
relations (null without assignment). `related`, `existing_assignments`, and
`situation.explanations` contain `items`, `total`, `returned`, and `omitted`.
Briefs include ID/title/status/resolution/effective_state/claim_owner and blockers;
related briefs also carry deduplicated relation labels. Situation contains
project total, matching state counts, and a reason explaining no allocation.

Only `--next` checks the actor's outstanding assignments: one matching claim
returns it unchanged; multiple or out-of-scope claims prevent new allocation.
No-assignment and already-owned observations must not update the ref or repair
stale projections. No global one-claim rule is introduced. Different actors
cannot claim the same task; concurrent same-actor requests allocate at most one
while it remains owned. Selection restarts on every lost CAS.

Packets fit 64 KiB of compact JSON including envelope/newline. Optional sections
have at most 20 briefs each. Keep complete task data before optional briefs;
omissions are counted. PACKET_TOO_LARGE includes candidate ID and ownership
context and prevents a new claim. The successful receipt identifies exactly the
published snapshot, not a subsequent head. Paths in notes remain unverified.

`apply --file PATH|-` accepts a strict version-1 ordered request capped at 100
operations and 1 MiB. It supports ordinary domain mutations, full task IDs and
backward local aliases, never administrative/source/forced operations. All steps
publish once or none. Optional expected_state_oid rejects stale decisions.
Create IDs and timestamps remain fixed across whole-batch CAS retries. Receipts
include final touched tasks, aliases, per-operation outcomes and the exact OID;
unchanged historical notes are omitted. Invalid request JSON is INVALID_ARGUMENT;
operation errors preserve the code with zero-based op_index/alias details.

Verification covers useful no-work reasons, stale projections, ordering/scope,
direct related statuses/limits, unchanged ownership observations, legacy explicit
claims, same/different actor races, pre-publication oversize, pinned receipts,
late batch rollback, alias/schema errors, dependency cycles, lifecycle batches,
CAS retry without duplicate notes/tasks, and read-only readers observing whole
states. Known lost responses require read-only recovery, not blind command
replay: after release/close the next allocation is different by design.
