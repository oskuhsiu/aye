# Common workflow

## Setup and identity

Run the installed aye inside the target repository or a linked worktree, including
nested directories. Check installation and command support once at setup, or on
an actual compatibility error; normal operations need no preliminary version,
help or status calls. If the executable is missing, report the prerequisite or
install it within the authorized scope. On `NOT_INITIALIZED`, choose setup:

```sh
aye --json init             # discover/adopt the configured remote task state
aye --json init --offline   # deliberately create local state without network
```

Online init joins an existing remote project. Offline init can create a separate
project and later cause `PROJECT_MISMATCH`. Setup, sync and reads need no actor.
Set a distinct process identity once for each independent agent:

```sh
export AYE_ACTOR=agent-a
```

Precedence is `--actor`, `AYE_ACTOR`, then the worktree-local actor file set by
`aye config actor agent-a`. Preserve a shared checkout's default; use process
identity or `--actor`. Keep the same identity throughout a claim. Executing agents
call aye directly; no handler or runtime role/model routing is needed.

Linked worktrees share changes immediately. For independent clones, use
`aye --json sync` at session start/handoff only when remote task publication is
in scope. It fetches and pushes; read [Sync and conflicts](sync-and-conflicts.md).

## Acquire assigned work directly

When authorized to start the next task within expressible scope:

```sh
aye --json claim --next
aye --json claim --next --priority P1 --type bug --label auth
```

Use only the appropriate one of these commands. Filters combine. Selection uses
priority P0–P4, then creation time and ID, and claims atomically. For a specified
task, use `aye --json claim TASK_ID --packet`; it never substitutes another task.
Explicit-ID claims retain their existing lifecycle rules.

Read the successful packet itself: it contains the complete assigned task,
acceptance, notes, claim, computed context, snapshot identity and bounded direct
related-task briefs with statuses. A new claim is already in progress; proceed
to required source/worktree checks without another aye acceptance or confirmation
call. The packet cannot verify recorded paths, source commits or test evidence.
Follow [Worktree context](worktrees.md) before editing.

Next-claim outcomes distinguish newly claimed work, already-owned work, no ready
task and existing assignments needing a decision. The command checks this actor's
existing claims before allocating: one matching claim is returned, while multiple
or out-of-scope claims prevent new allocation. This is a `--next` guard, not a
global one-task ownership limit. Preserve identity and resolve ambiguous scope.
For no ready work, use the returned matching counts and blocked/deferred/claimed
explanations; do not automatically resume deferred work or widen user scope.

Packets cap related briefs at 20 and serialized output at 64 KiB, reporting
omissions while retaining the full assigned task. Allow sufficient tool output.
If mandatory detail is too large, no new claim is made and no lower-priority task
is substituted; an already-owned task stays owned. Follow
[Diagnostics](diagnostics.md) for oversized or lost/truncated replies.

When the user wants comparison, or scope cannot be represented safely by filters,
inspect first and then explicitly claim the selected task:

```sh
aye --json ready
aye --json list --all --query "login"
aye --json show TASK_ID
```

Readiness is a snapshot, not ownership. `ready` defaults to 20 (`--all` removes the
limit); `list` defaults to non-closed tasks. Search before creating duplicate work.
Use full returned task IDs in automation; `TASK_ID` in shell examples is a
placeholder. Batch targets require full IDs.

## Record decided work atomically

Before implementing new tracked work, record observable success/failure acceptance
cases, including reproduction for a bug. Documentation can use executable examples
and scenario review. Use `aye --json apply --file request.json` (or `--file -` for
stdin) to record an already-decided plan in one transaction:

```json
{
  "version": 1,
  "operations": [
    {"op": "create", "as": "prerequisite", "title": "Repair refresh prerequisite", "acceptance": ["Failure releases waiting callers"]},
    {"op": "create", "as": "fix", "title": "Fix duplicate refresh", "type": "bug", "priority": "P1", "depends_on": [{"local": "prerequisite"}], "acceptance": ["Concurrent callers share one refresh"]},
    {"op": "create", "as": "verification", "title": "Verify refresh flow", "depends_on": [{"local": "fix"}], "acceptance": ["Concurrent success and failure scenarios pass"]}
  ]
}
```

Creation does not claim. `as` defines a unique request-local alias;
`{"local":"name"}` refers only to an earlier create. Existing tasks use full ID
strings. Operations run in supplied order, under one actor from normal identity
resolution, and obey normal ownership/lifecycle rules. Any invalid operation
rejects the whole batch. Unknown fields/operations, duplicate or forward aliases
are errors. Limits are 100 operations and 1 MiB; no automatic splitting occurs.

The reply confirms the committed snapshot, alias IDs, operation outcomes and final
states of touched tasks. Keep these facts for later operations; no routine reread
is needed. For read-dependent decisions, an optional top-level
`expected_state_oid` rejects changes against a stale snapshot. Independent creates
and append-only facts need no extra read merely to acquire this guard.

Batch create/update/note/block/unblock/claim/release/defer/resume/close/reopen as
needed. Forced release, sync, init, config, rebuild, conflict resolution and source
or filesystem operations remain separate. Different owners require separate
batches or an authorized handoff. Batch only facts already known: tests, review
and integration still have to happen before their success can be recorded.

For exact operation fields, metadata replacement/clearing and cancellation,
consult [Atomic batch reference](batches.md) when constructing those requests.

## Progress, pauses and completion

Record actual decisions, checks and remaining work. Notes append; correct earlier
facts with a new note. For one longer note, `aye --json note TASK_ID --file evidence.md`
remains available.

Choose the transition from the intended next-session availability:

| Intent | Action | Next authorized work session |
| --- | --- | --- |
| Ordinary pause, for any reason | Handoff note + owner `release`; note only if already open | Unblocked work remains in `ready` and eligible for `claim --next`; no `resume` or pause-specific confirmation |
| Explicit shelving pending confirmation | Handoff note + `defer` | Excluded from `ready` and `claim --next` until authorized `resume`; existing blockers still apply |

Treat an unqualified pause, "stop for now" or "continue next time" as an ordinary
pause. Session reasons such as time, compute/context limits or switching work
belong in the handoff note; they do not create a task blocker or imply shelving.
Use defer when the user explicitly wants work held out of normal selection until
confirmation. Record that intent and the condition for resumption. Existing
user authorization covering resumption is sufficient; do not ask again.

An ordinary pause preserves genuine prerequisites and manual blockers. An
already-blocked task remains blocked because of those prerequisites, not because
it was paused. Readiness does not authorize continuing after a request to stop.
For an ordinary pause of owned work, batch the handoff note and release:

```json
{
  "version": 1,
  "operations": [
    {"op": "note", "id": "t-0123456789abcdef0123", "body": "Handoff: machine=HOST; worktree=/actual/checkout; branch=fix-refresh; HEAD=COMMIT; changes=retained; checks=regression reproduced; next=implement correction"},
    {"op": "release", "id": "t-0123456789abcdef0123"}
  ]
}
```

Replace the illustrative ID and checkout facts with observed values. Both pause
paths preserve unfinished changes and checkouts; follow
[Worktree context](worktrees.md) when returning. Use the normal claim flow for
ordinary paused work; only deferred work needs the authorized resume transition.

For a discovered prerequisite, batch its create (with `discovered_from` set to the
original full ID) and a `block` of the original with `by` referencing its alias.
Provenance alone does not block. Blocking releases an active claim; only a
prerequisite closed as done satisfies the dependency. For external waits use
`block` with `reason`, then `unblock` when resolved. Removing `by` dependencies is
for obsolete relationships, not normal prerequisite completion.

Close as done only after acceptance, required independent review, source integration
and verified task-worktree cleanup. A commit alone is insufficient. Batch the
actual evidence note and `close` (using the same shape as note+release above), or
use `aye --json close TASK_ID --note "Acceptance, review, integration and cleanup evidence: ..."`.
Use the successful reply's final closed/done state as confirmation. Completion
neither commits/publishes source nor syncs task state; report these separately.

If output is lost, preserve the request and known actor/task IDs and inspect state
before deciding what happened. Local aliases are not durable replay keys. Never
blindly replay an uncertain create/note batch or allocate another task to replace
a lost reply; see [Diagnostics](diagnostics.md).
