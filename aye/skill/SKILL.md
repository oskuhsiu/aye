---
name: aye
description: Use aye to track coding tasks, claim work across linked Git worktrees, manage blockers, and synchronize task state across clones. Use when the user requests aye or the repository uses aye for task coordination.
---

# aye

Coordinate work through the installed `aye` CLI. Run commands inside the target
repository or one of its linked worktrees. This guide targets aye 0.2.x and task
format 1. Check command compatibility once during setup, or after an actual
compatibility error. Executing agents call aye directly with their own actor;
no handler, runtime role or model-routing configuration is required.

## Read what the operation needs

- **Everyday task work:** read [Common workflow](refs/common.md) first. It covers
  direct task acquisition, atomic batches, acceptance, pauses and completion.
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
