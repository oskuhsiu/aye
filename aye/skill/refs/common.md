# Common workflow

## Enter the project

Run from the target repository or a linked worktree, including a nested directory:

```sh
aye --version
aye --json status
```

If the executable is missing, report that prerequisite or install it within the
authorized scope. If status returns `NOT_INITIALIZED`, choose the intended setup:

```sh
aye --json init             # discover/adopt the configured remote task state
aye --json init --offline   # deliberately create local state without network
```

Use online init when joining an existing remote project. Starting offline creates
a separate project if no local state exists; later sync can return
`PROJECT_MISMATCH`. Setup, sync and read commands do not require an actor.

Use a distinct actor for each independent agent. Set a process identity once:

```sh
export AYE_ACTOR=agent-a
```

Lookup order is `--actor`, `AYE_ACTOR`, then the worktree-local actor file set by
`aye config actor agent-a`. Agents sharing one worktree should use separate
process identities or `--actor`, rather than overwrite that worktree's default.
Use the same identity throughout a claim.

Linked worktrees see task changes immediately. For work involving independent
clones, run `aye --json sync` at session start and handoff when remote task
publication is in scope. This both fetches and pushes; read
[Sync and conflicts](sync-and-conflicts.md) for remote behavior or failures.

## Find or define work

```sh
aye --json ready
aye --json list --all --query "login"
aye --json show TASK_ID
```

`ready` defaults to 20 tasks; `--all` removes that limit. `list` defaults to
non-closed tasks. Both return `data` arrays containing `task` and `computed`;
`show` returns one such object. Check acceptance, notes, blockers and ownership
before choosing work. Readiness identifies candidates within the assigned scope.
Search existing tasks before creating the same work again.

Before implementing new tracked work, record observable success and failure cases
in acceptance. Choose a few meaningful checks from the intended behavior; for a
bug, include its reproduction. Documentation can use executable examples and
scenario review instead of tests that merely match wording.

```sh
aye --json create "Fix duplicate refresh" --type bug --priority P1 \
  --acceptance "Concurrent callers share one refresh request" \
  --acceptance "A failed refresh releases all waiting callers"
```

Creation does not claim. Read the new full ID from `data.task.id`. In these guides,
`TASK_ID`, `PREREQUISITE_ID` and similar uppercase arguments are placeholders for
IDs returned by aye. Prefer full IDs in automation; unique prefixes need at least
eight payload hex characters.

## Claim, work and finish

For source work, inspect existing checkout notes before choosing or creating a
worktree. Follow [Worktree context](worktrees.md) to verify the checkout and,
after claiming, record its actual location before editing.

```sh
aye --json claim TASK_ID
aye --json note TASK_ID "Reproduced the failure; regression case fails as expected"
# Implement the assigned work and run its checks.
# Integrate and clean up task worktrees as required by the project; record evidence.
aye --json close TASK_ID --note "Regression and focused checks pass; evidence: ..."
```

Start work after the claim succeeds. A ready listing is a snapshot, not ownership.
`TASK_ALREADY_CLAIMED` means another actor holds the task: inspect it and choose
other assigned work or coordinate the handoff. `NOT_CLAIM_OWNER` means the current
actor cannot make that edit; preserve the claim. Other failures are mapped in
[Diagnostics](diagnostics.md).

Record decisions, actual check results and remaining work in notes. Notes append;
append a correction when needed. For longer evidence:

```sh
aye --json note TASK_ID --file evidence.md
```

`close` defaults to `done`. Use it only when the task's acceptance is met, and
include evidence or its location. A source commit by itself is not acceptance.
Verify `data.task.status == "closed"` and `data.task.resolution == "done"` in the
reply. Completion does not commit source changes or publish task state remotely.

## Blocked or handing off

Before releasing, deferring or blocking unfinished work, append progress,
remaining checks and its retained [worktree context](worktrees.md).

If a prerequisite is discovered while working:

```sh
aye --json create "Repair refresh prerequisite" --discovered-from TASK_ID \
  --acceptance "The prerequisite behavior is verified"
aye --json block TASK_ID --by PREREQUISITE_ID
```

Use the newly returned ID as `PREREQUISITE_ID`. `discovered_from` only records
provenance; the `block` command adds the dependency. Blocking an in-progress task
atomically releases its claim and changes it to open/blocked. After another actor
closes the prerequisite as done, the dependent becomes ready if nothing else
blocks it. Claim the dependent again before resuming.

For a wait that is not another task:

```sh
aye --json block TASK_ID --reason "Waiting for staging credentials"
aye --json unblock TASK_ID
```

Run `unblock` after the external wait is resolved. It clears only the manual
blocker. `unblock TASK_ID --by PREREQUISITE_ID` removes only that dependency;
use it when the prerequisite relationship is no longer required. Normal
prerequisite completion needs no manual edge removal. A cancelled prerequisite
continues to block its dependents.

For an unblocked handoff, use `aye --json release TASK_ID`. Use `defer` for
deliberately postponed work; see [Tasks and states](tasks-and-states.md). Synchronize when
remote handoff is intended and report any sync failure separately from local
progress.
