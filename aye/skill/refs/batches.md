# Atomic batch reference

Read this when constructing an `apply` request beyond the common examples.
Run `aye --json apply --file request.json`, or use `--file -` for UTF-8 JSON on
stdin. Supply one actor through `--actor`, `AYE_ACTOR` or worktree configuration;
there is no request-level or per-operation actor field.

## Request and references

The root object requires `version: 1` and an ordered `operations` array of 1–100
objects. Optional `expected_state_oid` is a full lowercase 40-hex state OID from
a prior observation. Omit it for writes that do not depend on a read; a mismatch
returns `STALE_STATE` with expected/observed OIDs and no write. Input is limited
to 1 MiB. Unknown fields and operations are rejected, not ignored.

A reference is either a full lowercase task ID string (`t-` plus 20 hex digits)
or exactly `{"local":"alias"}`. Only create accepts `as`, a nonempty unique
string defining an alias for later operations in this request. Forward references
and duplicate aliases are invalid. Aliases are not persisted or replay keys.

Each operation requires `op`. Every operation except create also requires `id`
as a reference. The table lists all other accepted fields; optional fields may
be omitted. Explicit null is supported only for clearing update's `parent`
(and the optional root snapshot guard); omit unused fields elsewhere.

| `op` | Other fields | Meaning |
| --- | --- | --- |
| `create` | Required string `title`; optional strings `as`, `type`, `priority`, `description`; string arrays `acceptance`, `labels`; references `parent`, `discovered_from`; reference array `depends_on` | Creates open work, without a claim. Defaults: type `task`, priority `P2`, empty description/arrays, absent parent/discovery. |
| `update` | Optional strings `title`, `type`, `priority`, `description`; string arrays `acceptance`, `labels`; reference or null `parent` | Omitted fields preserve their values. Arrays replace completely; `[]` clears. `parent: null` clears the parent; `description: ""` clears description. Reserved bypass metadata is preserved and cannot be created through update. |
| `note` | Required string `body` | Appends evidence; no `text` or file field. |
| `block` | Exactly one of reference `by` or nonempty string `reason` | Adds a prerequisite or manual blocker and releases an active claim. |
| `unblock` | Optional reference `by` | With `by`, removes that dependency; omitted, clears the manual blocker only. |
| `claim` | None | Claims the explicit ready task; no next-selection or packet fields. |
| `release` | None | Ordinary owner release; no force or reason fields. |
| `defer` | None | Shelves open or owned in-progress work, clearing its claim. |
| `resume` | None | Returns deferred work to open; blockers still govern readiness. |
| `close` | Optional boolean `cancelled` (default false), string `note` | Closes done, or cancelled when true; appends the supplied note. Normal close clears any current bypass marker. |
| `cancel` | Optional string `note` | Shorthand for close with `cancelled: true`. |
| `bypass` | Required nonempty string `reason`; required nonempty string array `missing` | With explicit human or project-policy authorization, records unavailable checks, clears a manual blocker, closes dependency-satisfying and returns newly ready dependents. It cannot skip unresolved task prerequisites. |
| `reopen` | None | Returns closed work to open, clears resolution and clears the current bypass marker while retaining audit notes. |

Types are `task`, `bug`, `feature`, `chore`; priorities are `P0`–`P4`.
Lifecycle/ownership and graph rules still apply at every step. Update cannot
replace dependencies, provenance, status, claims or timestamps; use the dedicated
operations. See [Tasks and states](tasks-and-states.md) for transition eligibility
and [Explicit verification bypass](bypass.md) for authorization rules.

## Examples

Replace illustrative existing IDs and evidence with actual values. This request
replaces labels, clears acceptance and parent, and records why; only clear actual
acceptance when that is the intended metadata decision:

```json
{
  "version": 1,
  "operations": [
    {"op": "update", "id": "t-0123456789abcdef0123", "labels": ["auth"], "acceptance": [], "parent": null},
    {"op": "note", "id": "t-0123456789abcdef0123", "body": "Scope was replaced; acceptance will be specified before implementation."}
  ]
}
```

Create a prerequisite, then block existing work using its returned local alias:

```json
{
  "version": 1,
  "operations": [
    {"op": "create", "as": "prerequisite", "title": "Restore staging access", "discovered_from": "t-0123456789abcdef0123", "acceptance": ["The staging check can authenticate"]},
    {"op": "block", "id": "t-0123456789abcdef0123", "by": {"local": "prerequisite"}}
  ]
}
```

Record an explicitly authorized verification bypass without manually editing labels
or composing resume/unblock/close steps:

```json
{
  "version": 1,
  "operations": [
    {
      "op": "bypass",
      "id": "t-0123456789abcdef0123",
      "reason": "Target hardware unavailable",
      "missing": ["Physical-device smoke test", "Reconnect test"]
    }
  ]
}
```

For an obsolete prerequisite, remove its edge and cancel that prerequisite only
when both actions are authorized. Normal done completion needs no edge removal:

```json
{
  "version": 1,
  "operations": [
    {"op": "unblock", "id": "t-0123456789abcdef0123", "by": "t-fedcba9876543210fedc"},
    {"op": "cancel", "id": "t-fedcba9876543210fedc", "note": "The staging dependency is no longer required."}
  ]
}
```

## Results and failures

All operations publish together or none do; there is no implicit chunking.
Readers see the old or final state. Results under `data` include `version`, the
exact committed `state_oid`, `aliases`, ordered `operations` and deduplicated
`tasks`. Operation entries include zero-based `op_index`, `op`, full `id`, `alias`,
`outcome` and, when applicable, `released_claim`, `newly_ready` or
`waived_manual_block`. Each task entry contains final task metadata excluding
historical notes, `computed` (including `bypassed`), `changes` and `added_notes`.
A task created then blocked is reported in its final blocked state.

Failed-operation details identify `op_index`, alias and `underlying_code`; inspect
`error.code` too. No receipt proves that a described test actually ran. Record
only completed checks and project-required review/integration/cleanup evidence.
A bypass receipt records accepted missing verification; it does not convert those
checks into passed evidence.
For lost output, retain the request and known IDs, then inspect before deciding
whether to replay; see [Diagnostics](diagnostics.md#assignment-or-batch-uncertainty).
