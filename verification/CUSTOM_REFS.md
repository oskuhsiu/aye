# Remote custom-ref change — behavior-first plan and evidence

Requested 2026-09-17: replace the remote ordinary task branch with a custom ref.
Agents can use Git CLI or GitHub Git Database API. Browser-only reading is no
longer required. Local task semantics and canonical schema stay unchanged.

## Decision

Both local authority and remote publication use `refs/agent-tasks/state`.
Fetched snapshots stay under `refs/agent-tasks/remotes/<remote>/state`, outside
branch listings. Normal push remains fast-forward-safe for commit objects;
remote tips must directly name commits. No force push or branch fallback is
introduced into aye sync. Existing branch-only observed tips are not evidence
that the new custom ref previously existed. Legacy pending conflicts may be
read/aborted but cannot silently continue against a different protocol.

## Cases specified before implementation

- Publish and explicitly adopt the custom ref; ordinary source clone does not
  fetch it automatically; neither remote nor local branch listings gain a task
  branch. Source files/index/history branch remain unchanged.
- Legacy branch observation permits first custom-ref publication; subsequent
  deletion of an observed custom ref is rejected without recreation.
- Tree/tag tips are rejected before adoption and without changing local state.
- Missing/legacy pending remote_ref refuses continue, while reads/abort remain
  available. New pending conflicts carry their remote ref explicitly.
- Direct divergent custom-ref commit pushes fail without force; normal sync
  reconciliation retains disjoint changes and reports same-task conflicts.
- GitHub CLI fetch and Git Database API exact ref -> commit -> tree -> blob
  recover byte-identical canonical task JSON.

## Test-first evidence

Old installed v0.1.0 failed custom-ref publication (missing remote custom ref).
Legacy pending-conflict test also failed before the guard was implemented.
Existing lifecycle/CAS/projection/source-isolation cases remain the regression
suite. Final results are appended after running the installed v0.2.0 binary.

## Migration boundary

The project keeps the same project_id and canonical tasks. Verify a recovery
bundle and the legacy branch tip. Reconcile any legacy work before retiring it.
Verify custom-ref publication by independent fetch and API read, then delete the
legacy branch only with its expected old tip. Remove only the matching legacy
tracking ref. Normal product sync never performs destructive migration or force.

GitHub does not expose a server-side `.git` directory as a browsable resource.
Use Git transport or Git Database API. Repository permissions still apply;
custom refs are not secrets, are not normal branch-rule targets, and can still
appear in local `git log --all`. Ordinary source clone alone is not a task backup.

## Completed verification — aye 0.2.0

- `cargo fmt --check`, all-target Clippy with warnings denied, and diff checks pass.
- `cargo test`: 15 passed.
- Installed CLI: bootstrap 5 passed; discovered `*_cases.py` suite 26 passed,
  including five new custom-ref cases and all 12 prior sync cases. Total 46.
- Independent review of the sync delta found no concrete defects.
- GitHub accepted ordinary push to `refs/agent-tasks/state`. Both exact ref GET
  and matching-ref lookup returned the expected commit. Commit/tree/blob API
  traversal recovered byte-identical canonical task JSON.
- A source-only clone initially had no task refs; explicit `aye init` adopted the
  complete state with unchanged source HEAD/status and no task/view directories.
- The legacy branch was an ancestor of the verified custom state and shared its
  project identity. A verified recovery bundle was retained locally before its
  expected-tip-guarded deletion. No source history was rewritten for this release.
- After branch retirement, the live GitHub roundtrip passed with no task branch
  or task remote-tracking branch. Retained evidence task:
  `t-84ad72731317413e7b58`.

The updated standalone specification is v0.3.2 in the local ignored `plan/`
directory. Published access instructions are in README and `src/FORMAT.md`.
