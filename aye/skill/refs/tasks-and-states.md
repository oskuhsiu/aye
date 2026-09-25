# Tasks and states

## Find and inspect

```sh
aye --json ready --all --priority P1 --type bug --label auth
aye --json list --state blocked
aye --json list --state in_progress --claimant agent-a
aye --json list --state bypassed
aye --json list --all --query "refresh"
aye --json show TASK_ID
aye --json report
```

Filters combine. Query is case-insensitive matching over title, description and
labels. `--state open` includes both ready and blocked open tasks; `--state closed`
includes all closed tasks without needing `--all`; `--state bypassed` selects only
currently bypassed work. Ready ordering is priority P0–P4, then creation time and
ID ascending. `show` separates canonical `task` from `computed`, including
effective state, `bypassed`, blocking details, children, tasks this one blocks,
and tasks discovered from it. `status` and `report` include bypass counts, and the
report has a fixed last-20 bypassed section in addition to closed and discovered.

## Metadata and relationships

Task types are `task`, `bug`, `feature`, `chore`; priority defaults to `P2`.
Consult `aye create --help` or `aye update --help` when an option is needed,
not as a recurring preflight. [Common workflow](common.md) covers atomic batches
for already-decided metadata and relationship changes.

```sh
aye --json update TASK_ID --title "Prevent duplicate refresh" --priority P1 \
  --description "Observed behavior and intended outcome" \
  --acceptance "Concurrent success is verified" \
  --acceptance "Failure leaves no waiting caller stranded" \
  --label auth --label concurrency
aye --json update CHILD_ID --parent PARENT_ID
```

Supplied acceptance and label options replace their entire arrays. Include the
entries to retain. Omitted fields remain unchanged; `--clear-acceptance`,
`--clear-labels`, and `--clear-parent` explicitly clear them. The reserved
`aye:bypassed` marker is controlled by lifecycle operations: do not add or remove
it through create/update.

| Relationship | Purpose | Editing |
| --- | --- | --- |
| `depends_on` | Prerequisite must be dependency-satisfying closed(done), including an explicitly authorized bypass | `block --by` / `unblock --by` |
| `parent` | Organizational hierarchy only | `create --parent` / `update --parent` |
| `discovered_from` | Where the new work was discovered | `create --discovered-from`; immutable afterward |

Targets must exist. Dependencies and parent links cannot point to themselves or
form cycles. Parent and discovery relationships do not block or auto-close tasks.
Notes and acceptance are ordered; labels and dependencies are set-like. Status,
claims, blockers, timestamps and provenance are not generic `update` fields.

## State transitions

Canonical status is `open`, `in_progress`, `deferred`, or `closed`. Ready and
blocked are computed: an open task is ready when it has no manual block and all
dependencies are closed(done). Exactly in-progress tasks have a claim. Bypassed is
a controlled closed(done) presentation state with an audit note and reserved
marker; it is not a fifth canonical status.

| Command | Eligible state | Result |
| --- | --- | --- |
| `claim` | Open and ready | In progress, actor owns claim |
| `release` | In progress, owned by actor | Open, claim cleared |
| `block --by` / `block --reason` | Open, in progress, deferred | Add blocker; release active claim; deferred stays deferred |
| `unblock [--by ID]` | Non-closed | Remove the selected blocker; recompute readiness |
| `close` | Ready open or owned in-progress task | Closed(done), claim cleared; any current bypass marker is cleared |
| `close --cancelled` | Any non-closed state | Closed(cancelled), claim cleared; blockers may remain |
| `bypass --reason ... --missing ...` | Open, owned in-progress or deferred; no unresolved task prerequisites | Closed(done) with bypass audit and marker; manual blocker and claim cleared; dependents may become ready |
| `reopen` | Closed | Open, resolution, closed time and current bypass marker cleared; audit notes remain |
| `defer` | Open or owned in-progress task | Deferred, claim cleared |
| `resume` | Deferred | Open; readiness depends on remaining blockers |

Ordinary mutations to an in-progress task require its claim owner, including
metadata, notes, blocker edits and bypass. Forced release recovery is described in
[Diagnostics](diagnostics.md). Bypass additionally requires explicit user or
project-policy authorization; see [Explicit verification bypass](bypass.md).

Cancellation records abandoned work; it does not satisfy dependents. An authorized
bypass does satisfy dependents while preserving the named missing checks as risk,
not passed evidence. If a prerequisite is still required and later verification
fails, reopen it and address the work. If the relationship is obsolete, remove
that edge deliberately. Reopening a prerequisite is rejected while a dependent is
in progress; arrange release by that dependent's owner first. Adding a blocker
that is already done returns `BLOCKER_ALREADY_SATISFIED`.

Pause intent maps to existing transitions; there is no canonical `paused` status.
Follow the [pause decision table](common.md#progress-pauses-and-completion): an
ordinary pause uses release (note only if already open), so unblocked work stays
ready for the next session. Defer means explicitly shelved pending confirmation.
Deferred tasks have no automatic wake-up, deadline or scheduler. Resume within
user-authorized scope when work should be considered again; existing blockers
still apply. Do not migrate old deferred records merely to make them ready.
There is no task delete command.
