# Local aye session usage audit — 2026-10-14

The local evidence supports three small skill corrections: clarify actor transport
for separately launched command tools, surface direct command syntax, and explain
denied Git/filesystem access. It does not support changing CLI ownership, lifecycle,
batch, synchronization or canonical-format semantics.

## Evidence boundary

Luna inventoried 5,448 active and 10 archived Codex JSONL files, plus recognizable
Gemini/Antigravity and Claude/Codex application stores. The 793 exact-word Codex
candidates yielded 72 actual-use rollouts: 34 roots and 38 children, from
2026-09-17 through 2026-10-14. These contain 1,054 captured invocation attempts
across 937 unique structured execution records. The current audit and its
descendants are excluded at 2026-10-14T08:18:53.624Z UTC. The ten archived rollouts
and searched non-Codex session text yielded no additional actual-use sessions.

Quoted task notes, search patterns, injected instructions and copied approval
transcripts are excluded from invocation counts. Outcomes are counted once per
execution; a composite shell's exit code does not establish each subcommand's
outcome. There are 36 executions with captured exact aye errors. No overall error
rate is derived from mixed or unattributable executions.

Thirty-five of 918 historical JavaScript wrapper candidates have no exact linked
structured result: 12 direct shell candidates and 23 Python candidates. All belong
to rollouts already in the 72-session manifest, but their outcomes remain unknown.
Dynamic executable names, truncated records and binary application databases are
not fully covered. Thus the table below ranks **captured exact categories**, not
all possible failures across the computer. User-role records include parent-agent
dispatches and are not counted as independent human complaints. The rollouts'
`cli_version` metadata describes the Codex host, not the aye executable.

The complete local bundle, source-path/line citations and reproducible collector
are retained in ignored `aye/test/session-usage-audit-20261014/`. Its `sessions.jsonl`
is the collected actual-use manifest; `events.jsonl`, `wrappers.jsonl` and
`census.json` retain the counting and coverage evidence. Raw histories stay in
their original stores. This tracked report contains only the decision summary.

## Captured failures and current relevance

| Category | Executions | Rollouts | Assessment |
| --- | ---: | ---: | --- |
| `GIT_ERROR` | 10 | 8 | All captured messages deny temporary Git-object writes. Recurrent access friction; add a narrow diagnostics branch. |
| `ACTOR_REQUIRED` | 6 | 4 | Mutations omitted identity. Clarify persistent-shell exports versus per-call actor transport. |
| `INVALID_ARGUMENT` | 6 | 5 | Four nonexistent `task`-namespace calls, one note `--body`, one too-short ID. Surface valid syntax; existing full-ID guidance is sufficient. |
| `NOT_INITIALIZED` | 6 | 5 | Setup prerequisite already handled by current init guidance; no automatic initialization or recurring preflight. |
| `NOT_CLAIM_OWNER` | 2 | 2 | Correct protection of another actor's active work. |
| `INVALID_STATE_TRANSITION` | 2 | 2 | Correct lifecycle rejection, including release of open work. |
| `IO_ERROR` | 1 | 1 | Note `--file -` was mistaken for stdin and recovered immediately. Clarify the file contract. |
| `TASK_NOT_FOUND`, `REMOTE_UNAVAILABLE`, `BLOCKER_ALREADY_SATISFIED` | 1 each | 1 each | ID, remote-access and prerequisite questions; no current API change supported. |

Representative original records are indexed in the local reports:

- Session `01a0e7b7-327f-7651-ad1d-dcde8117f9c6`, lines 5212–5279: common
  workflow read, rejected `aye task list`, successful direct calls, then an omitted
  actor on a fresh-shell note and successful per-call-identity retry.
- Session `01a0cd02-0b53-7c43-95fe-fb8082b28bf7`, lines 6943/6950 and
  9644/9659: missing-actor notes followed by successful explicit-actor retries.
- Session `01a0e7b7-327f-7651-ad1d-dcde8117f9c6`, lines 19088/19095:
  rejected note `--body`, followed by a successful real-file note.
- Session `01a0d11c-bf88-71e1-9146-9ea240c2e210`, lines 3699/3706:
  note `--file -` fails as a missing filename; positional body succeeds.
- Session `01a1016b-24fe-7090-aea9-e2bd17c51649`, line 92: create fails
  because `.git/objects/pack/tmp_pack_XXXXXX` access is denied.

These records establish the commands and responses. They do not prove the cause
of every omission or predict that the edited text will prevent future mistakes.
The historical concerns about extra acquisition calls and ordinary pause handling
are already addressed by 0.3.x packets/atomic apply and the September23 guidance.
Counts alone do not show that every help/show call was redundant.

## Requested model judgments and resulting change

Host `turn_context` records verify the primary analysis as `gpt-6.1-sol/max` and
all three independent reviewers as `gpt-5.6-sol/xhigh`; exact log paths/lines are
in the local bundle's `model-evidence.json`.

| Question | Operator/UX | Contract/authority | Measurement/current version | Decision |
| --- | --- | --- | --- | --- |
| Per-call actor transport | Accept | Accept | Accept narrowly | Clarify shell lifetime and explicit actor transport. |
| Daily command syntax | Accept, compact | Revise, minimal | Accept, skill only | Add one common-workflow syntax block and strengthen its existing skill pointer. |
| Denied-write recovery | One concise row | Accept with bounded authorization | One cause-specific row | Verify target/path and use host-approved access for the same authorized operation, preserving scope and state. |
| Ownership/lifecycle/batch rules | Preserve | Preserve | Preserve | Keep current runtime and format. |
| Native Note stdin | Documentation only | Reject current API extension | Reject on sole recovered case | Explain Note file paths versus Apply stdin. |

Only `aye/skill/SKILL.md`, `refs/common.md` and `refs/diagnostics.md` change behavior
guidance. The syntax reference uses direct top-level commands, a positional create
title and note body, real Note file paths, and Apply's stdin marker. Existing
pause/defer, bypass authorization, ownership, atomic rollback, lost-output and
source-publication/task-sync boundaries remain authoritative. There are no new
aliases, actor defaults, lifecycle shortcuts, recurring preflights or handler
agents. CLI source and canonical format remain unchanged.

## Verification and limits

Six existing installed CLI checks passed in 8.881 seconds: actor precedence and
worktree-private configuration; metadata/file/error/nested-directory behavior;
batch aliases/schema; late failure with owner/cycle/missing-target rollback;
pause/defer/metadata clearing; and stale-state guard/stdin. The installed binary
was aye 0.3.1, SHA-256
`bdd4d6f8479312accecc59982507ddb993adf9c0468d8295b900305286f5c084`.

Final acceptance also checks skill frontmatter, relative links, concrete syntax
and preservation scenarios, the source diff, an independent candidate review,
installed-skill byte equality, source integration and worktree cleanup. Their
actual receipts are recorded in the local bundle and aye task
`t-2fe9563a10ecd9ad6ddf`; these are completion requirements, not results inferred
from the model judgments. No prospective session study measured the change's
effectiveness. Source publication and task sync are separate from this local audit.
