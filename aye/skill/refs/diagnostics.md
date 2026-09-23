# Diagnostics and recovery

## Read the failure

Use `aye --json ...`. Success has `ok`, `data`, `warnings`; failure has `ok: false`
and `error.code` / `error.message`. Exit codes are 0 success, 1 unexpected internal
failure, 2 usage failure, 3 expected operational/domain failure. Use the stable
error code to choose an action and the message for context.

```sh
aye --json status
aye --json doctor
aye --json show TASK_ID
```

Use these only for the relevant diagnostic question, not after every successful
claim or write. They inspect state without changing task history. `doctor` checks canonical
schemas, ID paths, relationships, claims and view freshness. A healthy store says
nothing about whether a task's software acceptance has been tested.

| Signal | Response |
| --- | --- |
| `NOT_INITIALIZED` | Follow the setup choice in [Common workflow](common.md) |
| `ACTOR_REQUIRED` | Supply your actor through `--actor`, `AYE_ACTOR`, or this worktree's actor config |
| `TASK_NOT_FOUND` / `AMBIGUOUS_TASK_ID` | Inspect `list --all`; use the returned full ID |
| `TASK_ALREADY_CLAIMED` / `NOT_CLAIM_OWNER` | Inspect the claim; choose other assigned work or coordinate ownership |
| `TASK_NOT_READY` / `INVALID_STATE_TRANSITION` | Inspect status, manual block and prerequisites; use the appropriate [lifecycle command](tasks-and-states.md) |
| `BLOCKER_ALREADY_SATISFIED` | The prerequisite is already done; inspect whether a different unresolved prerequisite was intended |
| `RELATION_TARGET_NOT_FOUND`, `DEPENDENCY_CYCLE`, `PARENT_CYCLE` | Correct the intended relationship from actual task IDs and graph |
| `LOCAL_CONCURRENCY_RETRY_EXHAUSTED` | Reload the task and pending-conflict status; retry only if the action is still valid after competing writes settle |
| `SYNC_CONFLICT` or another remote error | Follow [Sync and conflicts](sync-and-conflicts.md) |

For usage failures, check `aye COMMAND --help`. For IO/Git/internal failures,
inspect the reported cause and repository access before retrying; preserve the
state rather than repeatedly attempting a destructive reset.

## Assignment or batch uncertainty

A no-ready or existing-assignment packet is an unchanged observation, not a new
claim. Use its counts, filters and owner context to decide whether authorized
work remains. An oversized assignment error leaves a new candidate unclaimed;
an oversized already-owned task stays owned. Inspect its known ID deliberately
and arrange adequate output capacity; do not skip priority or invent missing detail.

After a lost/truncated reply, retain actor identity, the request, known task IDs
and any returned OID. Inspect the known task, or use
`aye --json list --state in_progress --claimant ACTOR` to locate a retained claim.
Read required details before editing. `--next`'s existing-claim guard is not
historical idempotency: after release or close, another call could allocate
different work. If the original result cannot be proved, report uncertainty
instead of blindly allocating again.

For uncertain apply results, inspect known tasks and recorded evidence before
replay. Local aliases are not durable request IDs; retrying creates or notes can
duplicate work. An ordinary rejected batch leaves no partial authoritative writes;
use its failed-operation details and underlying error to correct the request.
A `STALE_STATE` (`expected_state_oid` mismatch) requires reassessing the decision against the
observed state, not silently dropping the guard. Internal concurrency retries
replay the transaction; a caller rerun after output loss is a separate request.

## Stale projections versus corrupt canonical data

`VIEW_STALE` on reads means aye computed the result from canonical files without
committing a repair. `doctor` reports stale views as an error. With valid canonical
state and no pending conflict, repair using:

```sh
aye --json rebuild
aye --json doctor
```

Rebuild preserves canonical task bytes and recreates views/report; repeating it
without changes makes no commit. It is not a repair for malformed canonical JSON.

For `STATE_CORRUPT`, preserve the current ref and inspect the offending canonical
object using [Git access](git-and-api.md). Recover from a known valid state or
perform a deliberate validated repair; do not delete the store and reinitialize.
For `FORMAT_VERSION_UNSUPPORTED`, use a compatible aye version rather than change
the version field. Warnings about large tasks, cancelled dependencies or closed
parents are contextual findings, not instructions to rewrite task history.

## Recover an abandoned claim

There is no heartbeat, expiry or automatic recovery. A claim's age alone does not
prove abandonment. Establish that the owner stopped or that takeover is authorized.
Keep your own actor identity and record the reason:

```sh
aye --json release TASK_ID --force --reason "Owner terminated; reassigned to me"
aye --json claim TASK_ID
```

Forced release clears the claim, returns the task to open, and appends an audit
note. The subsequent claim is a separate operation and must succeed before work
resumes. Normal release by the current owner needs no force or reason.
