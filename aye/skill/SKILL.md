---
name: aye
description: Use aye to track coding tasks, claim work across linked Git worktrees, manage blockers, and synchronize task state across clones. Use when the user requests aye or the repository uses aye for task coordination.
---

# aye

Coordinate work through the installed `aye` CLI. Run commands inside the target
repository or one of its linked worktrees. This guide targets aye 0.3.x and task
format 1. Check command compatibility once during setup, or after an actual
compatibility error. Executing agents call aye directly with their own actor;
no handler, runtime role or model-routing configuration is required.

## Read what the operation needs

- **Starting, pausing, deferring or completing task work:** read
  [Common workflow](refs/common.md) first. It distinguishes ordinary pauses from
  explicit shelving and covers acquisition, atomic batches and acceptance.
- **Waiving unavailable verification after implementation is complete:** read
  [Explicit verification bypass](refs/bypass.md). Never authorize a bypass without
  an explicit user instruction or a pre-existing project policy covering it.
- **Constructing batch requests beyond the common examples:** read
  [Atomic batch reference](refs/batches.md) for operation fields and clearing rules.
- **Starting, resuming or cleaning up a worktree:** read
  [Worktree context](refs/worktrees.md) to record and recover the task's checkout.
- **Metadata, filtering or lifecycle changes:** read
  [Tasks and states](refs/tasks-and-states.md).
- **Another clone, remote publication or a pending conflict:** read
  [Sync and conflicts](refs/sync-and-conflicts.md).
- **Errors, stale views or abandoned claims:** read
  [Diagnostics and recovery](refs/diagnostics.md).
- **Reading without aye, GitHub API access or storage inspection:** read
  [Git and API access](refs/git-and-api.md).

References are relative to this skill directory. Load only the relevant files.
Use `--json` for decisions; success is `ok: true`, failures use `error.code`.
Successful replies confirm the operation; do not add routine help/version/status
preflights, confirmation reads or task-accepted messages. Task mutations go
through aye, which coordinates ownership and validates state.
Keep source work, task completion and remote synchronization as distinct results
when reporting progress.
