# Tasks and states

## Find and inspect

```sh
aye --json ready --all --priority P1 --type bug --label auth
aye --json list --state blocked
aye --json list --state in_progress --claimant agent-a
aye --json list --all --query "refresh"
aye --json show TASK_ID
aye --json report
```

Filters combine. Query is case-insensitive matching over title, description and
labels. `--state open` includes both ready and blocked open tasks; `--state closed`
includes closed tasks without needing `--all`. Ready ordering is priority P0–P4,
then creation time and ID ascending. `show` separates canonical `task` from
`computed`, including effective state, blocking details, children, tasks this one
blocks, and tasks discovered from it. `report` returns a human summary, with fixed
last-20 closed and discovered sections.

## Metadata and relationships

Task types are `task`, `bug`, `feature`, `chore`; priority defaults to `P2`.
Use `aye create --help` and `aye update --help` for the complete option surface.

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
`--clear-labels`, and `--clear-parent` explicitly clear them.

| Relationship | Purpose | Editing |
| --- | --- | --- |
| `depends_on` | Prerequisite must be closed(done) | `block --by` / `unblock --by` |
| `parent` | Organizational hierarchy only | `create --parent` / `update --parent` |
| `discovered_from` | Where the new work was discovered | `create --discovered-from`; immutable afterward |

Targets must exist. Dependencies and parent links cannot point to themselves or
form cycles. Parent and discovery relationships do not block or auto-close tasks.
Notes and acceptance are ordered; labels and dependencies are set-like. Status,
claims, blockers, timestamps and provenance are not generic `update` fields.

## State transitions

Canonical status is `open`, `in_progress`, `deferred`, or `closed`. Ready and
blocked are computed: an open task is ready when it has no manual block and all
dependencies are closed(done). Exactly in-progress tasks have a claim.

| Command | Eligible state | Result |
| --- | --- | --- |
| `claim` | Open and ready | In progress, actor owns claim |
| `release` | In progress, owned by actor | Open, claim cleared |
| `block --by` / `block --reason` | Open, in progress, deferred | Add blocker; release active claim; deferred stays deferred |
| `unblock [--by ID]` | Non-closed | Remove the selected blocker; recompute readiness |
| `close` | Ready open or owned in-progress task | Closed(done), claim cleared |
| `close --cancelled` | Any non-closed state | Closed(cancelled), claim cleared; blockers may remain |
| `reopen` | Closed | Open, resolution and closed time cleared |
| `defer` | Open or owned in-progress task | Deferred, claim cleared |
| `resume` | Deferred | Open; readiness depends on remaining blockers |

Ordinary mutations to an in-progress task require its claim owner, including
metadata, notes and blocker edits. Forced release recovery is described in
[Diagnostics](diagnostics.md).

Cancellation records abandoned work; it does not satisfy dependents. If a
prerequisite is still required, reopen and complete it. If the relationship is
obsolete, remove that edge deliberately. Reopening a prerequisite is rejected
while a dependent is in progress; arrange release by that dependent's owner first.
Adding a blocker that is already done returns `BLOCKER_ALREADY_SATISFIED`.

Deferred tasks have no automatic wake-up, deadline or scheduler. Resume explicitly
when work should be considered again. There is no task delete command.
