# aye task format v1

Open the `agent-tasks` branch to read task state. Local CLI state lives at
`refs/agent-tasks/state`, shared by linked worktrees; the source branch and index
are unrelated. Synchronization publishes the task branch with ordinary Git
commits. The task history has its own root.

## Tool-less reading

1. Read this file and `project.json`.
2. Read `manifest.json`, then `views/ready.jsonl` for actionable work or
   `views/active.jsonl` for every non-closed task.
3. Select a full ID and read its canonical JSON. For example,
   `t-a1b20000000000000000` maps to `tasks/a1/t-a1b20000000000000000.json`.
4. If derived files are missing or stale, read all canonical task JSON files and
   compute the rules below. `REPORT.md` is only a human summary.

`project.json` contains `format` (`agent-tasks`), `format_version` (1), and a
UUIDv4 `project_id`. Different project IDs must never be merged. A newer format
version needs a compatible reader.

## Canonical task JSON

Every task is one UTF-8 JSON object at `tasks/<shard>/<id>.json`. The ID is `t-`
plus 20 lowercase hexadecimal characters representing 80 random bits; the shard
is the first two payload characters. IDs and paths agree and IDs are permanent.
All fields below are required; nullable fields must explicitly contain `null`.
Unknown fields are not part of v1.

| Field | Meaning |
| --- | --- |
| `schema` | 1 |
| `id`, `title` | Full ID and nonempty title |
| `type` | `task`, `bug`, `feature`, or `chore` |
| `priority` | `P0` (highest) through `P4` |
| `status` | `open`, `in_progress`, `deferred`, or `closed` |
| `resolution` | `done` or `cancelled` when closed; otherwise null |
| `description` | Free text |
| `acceptance` | Ordered list of acceptance strings |
| `labels` | Unique strings, serialized in sorted order |
| `claim` | Null or `{actor, claimed_at}` |
| `manual_block` | Null or `{reason, actor, blocked_at}` |
| `depends_on` | Sorted unique full IDs of prerequisites |
| `parent` | Null or one organizational parent ID |
| `discovered_from` | Null or immutable provenance task ID |
| `notes` | Ordered `{actor, created_at, body}` objects |
| `created_at`, `updated_at` | UTC timestamps with exactly three fractional digits |
| `closed_at` | Timestamp when closed, otherwise null |

Timestamp example: `2026-09-17T03:10:00.000Z`. Actors, note bodies, and manual
block reasons are nonempty. Notes are appended; their ordering and acceptance
ordering are significant.

## Status, readiness, and relations

A task is ready exactly when its canonical status is `open`, its manual blocker
is null, and every prerequisite is `closed` with resolution `done`. An open task
failing either blocker condition is effectively `blocked`. Other effective states
are their canonical statuses: `in_progress`, `deferred`, `closed`. Ready and
blocked are computed values, never canonical statuses.

A prerequisite cancelled with resolution `cancelled` **does not satisfy** a
dependency. `blocked_by` is the sorted list of unresolved prerequisite IDs; a
manual blocker is separate and clearing one kind never clears the other kind.
Only an unblocked open task can be claimed. Exactly in-progress tasks have a
claim; the claim owner is the task writer until release, defer, or closure.
In-progress tasks cannot retain unresolved blockers. Closing done requires
unblocked open/in-progress state; cancellation can abandon blocked/deferred work.
Reopening clears resolution and closed time, and cannot invalidate an active
dependent. Forced release requires an audit reason.

Relations reference existing tasks, never self. Dependencies and the parent
hierarchy must each be acyclic. Parent organizes work; it does not imply dependency,
readiness, or automatic closure. `discovered_from` records where work was found;
it also does not block. Children, reverse dependencies, and discoveries are
computed by scanning canonical tasks, never stored as additional canonical lists.

## Derived files and freshness

`views/ready.jsonl` contains one compact JSON object per ready task, ordered by
priority, then creation timestamp ascending, then full ID ascending.
`views/active.jsonl` contains every non-closed task with effective state, claim,
manual blocker and unresolved dependency IDs, in the same stable order.
`manifest.json` records `projection_version` (1), `tasks_tree_oid`, and counts for
total, ready, blocked, in_progress, deferred, closed_done, closed_cancelled.
The OID is the actual Git `tasks/` subtree OID, excluding derived files.

Compare the manifest OID to `git rev-parse agent-tasks:tasks` after fetching the
branch. A differing OID, unsupported projection version, missing file or corrupt
projection means views must not be trusted. Read canonical files instead. The
CLI warns `VIEW_STALE`, computes accurate results in memory, and does not commit
on reads. `aye rebuild` repairs derived files without changing canonical bytes;
repeating it with identical output creates no commit.

`REPORT.md` includes summary, in-progress, ready, blocked, deferred, last 20 closed
(closed time descending, ID ascending), and last 20 discovered (creation time
descending, ID ascending). No wall-clock metadata changes projection bytes.

## Tool-less write limitations

Reading needs no CLI. Editing via a web UI does not provide atomic claims,
validation, ownership enforcement, dependency-cycle checks, or concurrent-write
protection. Do not infer that a displayed ready task is still claimable; use
`aye claim` to obtain ownership. Prefer `aye` for writes. External editors must
preserve this entire schema and graph invariants and refresh views using the CLI.
Derived files are never canonical input, and editing a view does not edit a task.
Sync conflicts freeze writes across linked worktrees until resolved or aborted;
readers may still inspect the local canonical state. Never force-push task state
or merge different projects to bypass these protections.
