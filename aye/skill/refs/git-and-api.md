# Git and API access

## Storage model

The ref `refs/agent-tasks/state` points to a commit in the repository object
database. Task and view paths are inside that commit's tree, not directories to
create in the source worktree or JSON files to write under `.git/refs/`.
Git may pack refs; linked worktrees may have a `.git` file. Resolve paths with Git
when needed (`git rev-parse --git-dir`, `--git-common-dir`, `--git-path`).

Canonical files are `project.json` and `tasks/<shard>/<full-id>.json`.
For `t-a31f82b47c6d9e1042af`, the path is
`tasks/a3/t-a31f82b47c6d9e1042af.json`. Managed `FORMAT.md` explains the schema.
`manifest.json`, `views/ready.jsonl`, `views/active.jsonl` and `REPORT.md` are derived.

## Read with Git

Pin one commit so concurrent updates do not mix snapshots:

```sh
aye_snapshot=$(git rev-parse refs/agent-tasks/state)
git show "${aye_snapshot}:project.json"
git show "${aye_snapshot}:FORMAT.md"
git show "${aye_snapshot}:views/ready.jsonl"
```

For remote-only inspection, explicitly fetch the custom ref into its tracking
namespace, then select that commit. This fetches without publishing local tasks:

```sh
git fetch origin refs/agent-tasks/state:refs/agent-tasks/remotes/origin/state
aye_snapshot=$(git rev-parse refs/agent-tasks/remotes/origin/state)
git cat-file -t "$aye_snapshot"
git show "${aye_snapshot}:FORMAT.md"
```

Substitute the intended remote name and matching tracking path for `origin`.
Require the selected object to be a commit. Direct Git fetching does not update
aye's own observed-remote bookkeeping; use aye sync for normal reconciliation.

Read the project format and stored `FORMAT.md` before interpreting blobs. Compare
manifest `tasks_tree_oid` with `git rev-parse "${aye_snapshot}:tasks"`. If views
are missing, corrupt or stale, read canonical tasks and compute readiness using
[Tasks and states](tasks-and-states.md), or use aye's read commands. A view is
not authority for a claim; task mutations still go through aye.

## Read through GitHub Git Database API

A repository URL ending in `.git` is a Git transport endpoint, not an HTTP
filesystem exposing `.git` contents. Use the repository's Git Database API:

1. `GET /repos/{owner}/{repo}/git/matching-refs/agent-tasks/state`.
   Select exactly `ref == "refs/agent-tasks/state"`; require object type `commit`.
   A similarly prefixed result does not establish that the exact ref exists.
2. Pin the selected commit SHA. Read `GET /repos/{owner}/{repo}/git/commits/{sha}`
   for its root tree SHA.
3. Traverse `GET /repos/{owner}/{repo}/git/trees/{sha}` by path. A recursive tree
   response may be truncated; fetch missing subtrees individually when necessary.
4. Read `GET /repos/{owner}/{repo}/git/blobs/{sha}` for each required file and decode
   its declared encoding. Read `project.json` and `FORMAT.md` before task/view data.

Compare manifest freshness with the selected root tree's `tasks` entry, and pin
every subsequent read to that commit's tree. Repository read permissions apply;
use the environment's established API authentication when needed. Custom refs are
not a secrecy mechanism. Branch pages and raw/Contents URLs are not this contract.

Official endpoint references: [refs](https://docs.github.com/en/rest/git/refs),
[commits](https://docs.github.com/en/rest/git/commits),
[trees](https://docs.github.com/en/rest/git/trees),
[blobs](https://docs.github.com/en/rest/git/blobs).

## Preserve state

Ordinary source clones and source-only bundles are incomplete task backups.
When backup is requested, explicitly include the task ref:

```sh
git bundle create BACKUP_PATH refs/agent-tasks/state
git bundle verify BACKUP_PATH
```

Choose a backup destination appropriate for the project. This preserves the
reachable task history, not worktree actor files or pending conflict metadata;
inspect pending state before planning a full recovery. Restore or migration needs
an explicit target and reconciliation plan.

Use aye for writes. External Git/API JSON edits bypass its normal ownership and
local coordination; valid edits may require projection rebuilding, while invalid
canonical data is rejected. Inspect through Git objects without checking out a
task branch, and never edit derived views as a way to change a task.
