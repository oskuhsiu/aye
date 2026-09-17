# Sync and conflicts

## Choose the coordination boundary

Linked worktrees share one local task ref and claims immediately, without network.
Independent clones share state only through explicit synchronization. A claim
coordinates one Git repository; it is not a distributed lock across clones.

```sh
aye --json config remote
aye --json status
aye --json sync
```

Remote selection is configured task remote, then `origin`, then the sole Git
remote, otherwise local-only. To change it within the intended project:

```sh
aye --json config remote REMOTE_NAME
```

The name must already be a Git remote. Inspect its URL with `git remote get-url
REMOTE_NAME` when the target is uncertain. Configuration is shared by linked
worktrees. `status` uses the last known remote state and makes no network request;
`equal` is not a fresh live-host check.

`sync` fetches, validates, reconciles and pushes task state. Run it when remote
publication is authorized, normally at session start and handoff for work across
clones. Source `git push` and task synchronization are separate operations.

## Transport and failures

Local and remote authority is the fixed custom ref `refs/agent-tasks/state`.
Fetched snapshots live at `refs/agent-tasks/remotes/<remote>/state`. Ordinary
clone/fetch normally omits them; `aye init` and `aye sync` fetch explicitly.
Normal branch lists remain source-only unless someone independently created a
task branch. Custom-ref history can still appear in `git log --all`.

Equal state is a no-op; a missing, never-observed remote ref permits first
publication. Ancestry permits fast-forward updates. Diverged changes to different
task files merge automatically if the combined graph is valid. Different edits
to the same task conflict, even when they touch different fields. Timestamps do
not choose a winner.

The CLI retries a remote advance before push by fetching and reconciling again.
Use normal sync; never force-push task state to bypass a rejection. After an
error, inspect status and the reported cause before retrying. Local state may
already include a valid reconciliation even if the final push failed.

| Error | Next action |
| --- | --- |
| `REMOTE_UNAVAILABLE` | Check configured remote, connectivity and credentials; local task work can continue unless a conflict is pending |
| `REMOTE_PUSH_REJECTED` | Inspect the rejection and host permissions; preserve local work and retry after the cause changes |
| `PROJECT_MISMATCH` | Check project IDs and the intended repository; preserve both stores rather than merge unrelated projects |
| `REMOTE_STATE_DELETED` | A previously observed ref disappeared; establish whether deletion was intentional and recover through an explicit restoration decision |
| `MISSING_HISTORY` | Obtain complete task history from the correct remote or backup, then retry; ordinary source history alone may be insufficient |
| `STATE_CORRUPT` / `FORMAT_VERSION_UNSUPPORTED` | Follow [Diagnostics](diagnostics.md); do not publish invalid or unsupported state |

Remote observations are scoped by remote name and fixed ref. Legacy
`refs/heads/agent-tasks` state is never automatically adopted or used as fallback.
An upgrade from branch-based storage needs a separately authorized migration:
preserve the legacy tip, reconcile its work, verify custom-ref publication, then
retire the old branch. Routine sync performs none of that destructive migration.

## Resolve a pending conflict

`SYNC_CONFLICT` leaves pending metadata shared by all linked worktrees. Task writes
are frozen; reads remain available. Inspect before selecting a resolution:

```sh
aye --json resolve
aye --json resolve TASK_ID
```

The detail reply contains `base`, `local`, `remote`, `remote_ref` and resolution
status. The summary also reports graph validation errors. Inspect every listed
conflict, including graph conflicts between individually valid task files.

Choose one action per conflicted item:

```sh
aye --json resolve TASK_ID --take local
# Or select remote, or supply the complete resolved canonical JSON object:
aye --json resolve TASK_ID --take remote
aye --json resolve TASK_ID --file resolved.json
```

These are alternatives, not a sequence to run together. A file is the raw task
object, not a `show` response envelope or a projection. Preserve its ID, required
fields, intended notes and graph invariants; inspect the stored `FORMAT.md` via
[Git access](git-and-api.md) for schema details. A `project.json` conflict uses
that identifier and a complete project object with unchanged project identity.

```sh
aye --json resolve --continue
```

Continue requires every conflict to have a selection, validates the entire graph,
rebuilds views, commits the merge locally and attempts sync. It can encounter new
remote changes; inspect the result instead of assuming publication succeeded.

If a resolution decision cannot be made from the assigned scope and evidence,
keep the conflict pending and report the exact choice needed. To deliberately
discard pending selections while retaining the local task state:

```sh
aye --json resolve --abort
```

Abort does not accept remote changes or resolve the underlying disagreement.
`SYNC_PROTOCOL_MISMATCH` indicates a pending conflict for a legacy/different ref;
inspect and preserve any useful decisions before aborting obsolete pending state
and starting fresh sync. Do not edit its target to force continuation.
