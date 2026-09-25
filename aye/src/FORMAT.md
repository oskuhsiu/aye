# aye task format v1

Local and remote task state use the custom ref `refs/agent-tasks/state`, shared
locally by linked worktrees. The source branch and index are unrelated. Task
history has its own root. This transport is specified by v0.3.2 (aye 0.2.0);
canonical project format and task schema remain 1.

## Reading through Git or API

Ordinary clone/fetch normally omits custom refs. `aye init` and `aye sync` fetch
explicitly, tracking the selected remote at `refs/agent-tasks/remotes/<remote>/state`.
For direct Git inspection:

```sh
git fetch origin refs/agent-tasks/state:refs/agent-tasks/remotes/origin/state
git show refs/agent-tasks/remotes/origin/state:project.json
git show refs/agent-tasks/remotes/origin/state:views/ready.jsonl
```

A GitHub `.git` URL is a transport endpoint, not an HTTP filesystem. API readers:

1. Call `GET /repos/{owner}/{repo}/git/matching-refs/agent-tasks/state` and select
   the exact `refs/agent-tasks/state` result; require object type `commit`.
2. Read `/git/commits/{sha}` under that repository to obtain its root tree SHA.
3. Traverse `/git/trees/{sha}` entries to the desired path. If using a recursive
   tree response, check truncation and traverse omitted subtrees explicitly.
4. Read `/git/blobs/{sha}` and decode the declared content encoding.

Pin all reads to the selected commit. Read this file and `project.json`, then
`manifest.json` and `views/ready.jsonl` or `views/active.jsonl`. A full task ID such
as `t-a1b20000000000000000` maps to `tasks/a1/t-a1b20000000000000000.json`.
If views are absent or stale, traverse canonical task blobs and compute the rules
below. `REPORT.md` is only a derived human summary.

GitHub branch pages, branch protection, Contents URLs and raw branch URLs are not
the custom-ref access contract. Repository permissions apply; API readers need
applicable Contents read permission. Custom refs are not secret storage. Browser-only
Agents are outside the supported scope. Backups must include custom refs explicitly,
and `git log --all` can display locally reachable task history.

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

### Compatible bypass marker

Bypass is intentionally represented without a new status or resolution, so task
format 1 readers preserve dependency behavior. A currently bypassed task has all
of the following:

- canonical `status: "closed"` and `resolution: "done"`;
- reserved label `aye:bypassed`;
- an attributed audit note beginning `[aye:bypass]` that names the authorization
  reason and each check that remains unverified.

Current aye reports this as `computed.bypassed: true`; aye-view presents
`closed(bypassed)`. The marker means missing verification was explicitly accepted,
not that it passed. `aye bypass` is the writer for this state. Create/update cannot
add or remove the reserved marker; ordinary close and reopen clear the current
marker, while appended audit notes remain historical evidence. Older compatible
readers see an ordinary closed(done) task plus label and note.

## Status, readiness, and relations

A task is ready exactly when its canonical status is `open`, its manual blocker
is null, and every prerequisite is `closed` with resolution `done`. An open task
failing either blocker condition is effectively `blocked`. Other effective states
are their canonical statuses: `in_progress`, `deferred`, `closed`. Ready and
blocked are computed values, never canonical statuses.

A prerequisite cancelled with resolution `cancelled` **does not satisfy** a
dependency. A bypassed prerequisite does satisfy it because its canonical result is
closed(done), while current readers retain the accepted-risk distinction through
the reserved marker and audit note. `blocked_by` is the sorted list of unresolved
prerequisite IDs; a manual blocker is separate and clearing one kind never clears
the other kind.

Only an unblocked open task can be claimed. Exactly in-progress tasks have a
claim; the claim owner is the task writer until release, defer, bypass or closure.
In-progress tasks cannot retain unresolved blockers. Closing done requires
unblocked open/in-progress state; cancellation can abandon blocked/deferred work.
An explicitly authorized bypass accepts open, owned in-progress or deferred work,
refuses unresolved task dependencies, clears an external/manual blocker and claim,
and closes done with the compatibility marker. Reopening clears resolution,
closed time and the current marker, and cannot invalidate an active dependent.
Forced release requires an audit reason.

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
total, ready, blocked, in_progress, deferred, closed_done, closed_cancelled, and
bypassed. Bypassed is a subset of closed_done, not an additional canonical state.
The OID is the actual Git `tasks/` subtree OID, excluding derived files.

Compare the manifest OID to
`git rev-parse refs/agent-tasks/remotes/origin/state:tasks` after explicit fetch.
API readers compare it to the `tasks` tree entry in the selected commit. A differing OID, unsupported projection version, missing file or corrupt
projection means views must not be trusted. Read canonical files instead. The
CLI warns `VIEW_STALE`, computes accurate results in memory, and does not commit
on reads. `aye rebuild` repairs derived files without changing canonical bytes;
repeating it with identical output creates no commit.

`REPORT.md` includes summary, in-progress, ready, blocked, deferred, last 20
bypassed, last 20 closed (closed time descending, ID ascending), and last 20
discovered (creation time descending, ID ascending). No wall-clock metadata
changes projection bytes.

## External write limitations

Use `aye` for task writes: external Git/API editing does not automatically enforce
atomic claims, ownership, graph invariants, bypass authorization or concurrency
safety. A displayed ready task may already have been claimed; use `aye claim`.
Do not forge or remove the reserved bypass marker through external editing. External
editors must preserve the complete schema and graph and refresh views through the
CLI. Derived files are never canonical input, and editing a view does not edit a
task.

Sync conflicts freeze writes across linked worktrees until resolved or aborted;
reads remain available. Pending metadata identifies the remote and `remote_ref`;
legacy or mismatched targets must be rejected by `resolve --continue`. Observation
of remote state is scoped by both remote and ref. The exact remote state object
must be a commit. No legacy task branch is automatically adopted or used as fallback.
Never force-push task state or merge different projects to bypass these protections.
