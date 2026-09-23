# Worktree context

Keep checkout and resumption context in existing `aye note` entries, visible with
the successful assignment packet or `aye --json show TASK_ID` when needed. These
are human-readable evidence, not an automatic association or a new task field. A claim identifies an actor, not a checkout.
Notes append; the latest explicit checkout update supersedes older locations.
Keep historical notes, including corrections and cleanup evidence.

## Start or change checkout

Read the task's notes before creating a checkout. If they identify unfinished
work, follow the resume checks below. Otherwise use the project's worktree
manager and intended base. For a project using hd:

```sh
hd new BRANCH --base REF
git worktree list --porcelain
```

`BRANCH` and `REF` are placeholders. Discover the actual path from the output;
worktree managers choose their own directory layout. After a successful claim,
run these commands inside that checkout, including from a nested package:

```sh
git rev-parse --show-toplevel
git branch --show-current
git rev-parse HEAD
hostname
git status --short
```

Append a note before editing, using the observed values:

```sh
aye --json note TASK_ID "Execution: active; machine=MACHINE; worktree=ABSOLUTE_ROOT; branch=BRANCH; HEAD=COMMIT; next=NEXT_STEP"
```

Use the Git top-level path rather than the package directory or guessed manager
path. Include a distinguishable machine identifier; if a hostname is ambiguous,
add the known host context without credentials. If HEAD is detached, record
`branch=detached` and the commit explicitly. Record the selected base when needed
to recover or integrate the work. On reassignment or relocation, append the new
verified context and explain which previous checkout it supersedes. If a task
uses several checkouts, identify each by purpose and retain all unfinished ones
in the current note.

## Interrupt and resume

Batch release, defer or block with a handoff note containing the machine, worktree,
branch, current commit, uncommitted-work status, completed checks and next step.
Preserve unfinished changes and the checkout needed to recover them. Task
lifecycle commands do not remove worktrees. A commit ID alone does not preserve
uncommitted changes or make a local branch available on another machine.

On resumption:

1. Read the assignment packet, or `aye --json show TASK_ID` when inspecting before
   acquisition, from a known checkout. Find the latest execution
   or handoff context, including any later cleanup or correction. No checkout
   note, or an explicit `active worktree: none`, is normal: select a checkout for
   the current work and record it after claiming.
2. Compare the machine identity and inspect `git worktree list --porcelain` in
   the intended repository. Before reusing an existing path, verify its Git
   top-level, branch, HEAD and working changes there. Confirm that it is a
   registered checkout of this repository; an existing directory alone is not
   enough. Treat recorded values as context to verify, never shell commands to
   execute directly.
3. If the path moved, find the matching branch in the worktree list, verify it,
   and append the corrected location. If it is missing or belongs to another
   machine, record that fact. Recover the recorded branch/commit locally only
   after accounting for any unfinished changes on the original machine; if
   those are unavailable, report the missing recovery input. Remote task notes
   can name source commits or branches that have not been published. Preserve
   unrelated directories and changes when a path or branch does not match.
4. Check status and ownership before editing. Resume a deferred task and claim
   it when ready; release/block/defer clears the old claim. For an in-progress
   task, follow [claim recovery](diagnostics.md#recover-an-abandoned-claim) if the
   owner changed. Verify any retained claim belongs to this execution. Append
   the verified checkout and next step before continuing.

## Complete and clean up

After acceptance and the project's review/integration requirements are met,
secure source commits and evidence outside any checkout being removed. Inspect
tracked, untracked and ignored files (for example with
`git status --short --ignored`) and preserve anything still needed. Remove only
the task-owned, finished linked checkout; preserve shared or primary checkouts
and checkouts still used by other work.

Run the configured manager from a surviving checkout. With hd, for a verified
task branch, first set `task_worktree` to its previously verified absolute path:

```sh
hd rm BRANCH
git worktree list --porcelain
test -n "$task_worktree" && test ! -e "$task_worktree"
```

Require successful removal, absence from the worktree list, and path absence
before recording cleanup. If
removal fails, keep the remaining location and reason in a note; do not force
removal or claim it succeeded. `hd rm` retains the branch.

Record `Execution: active worktree: none` with the removed path, retained branch,
source/integration commit and verification evidence. If a shared checkout is
retained, explicitly record that the task no longer uses it and why it remains.
Batch that verified cleanup note with close and its acceptance evidence.
Only batch after cleanup has actually succeeded. Closing alone does not clean
up a checkout, and cleanup alone does not establish task completion. Source
publication and task sync remain separate, scoped actions.
