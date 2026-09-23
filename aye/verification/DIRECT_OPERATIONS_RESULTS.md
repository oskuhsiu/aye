# Direct operations delivery evidence

Date: 2026-09-23. Release: aye 0.3.0, canonical format 1.
Implementation reviewed and tested at `5f8eee9b7dd50ef7f6ca1e4a3fc2465d7c78108b`,
against base `3e5026d39adbd84fb7aa0df992fde45e471235dd`. Later delivery-document
changes do not change the tested production source.

## Checks and independent review

- `make -C aye test` passed formatting, strict Clippy, 21 Rust tests, default
  Cargo installation, 5 installed Bootstrap cases and 42 installed CLI cases.
  The CLI cases include 7 assignment and 9 batch cases; two batch cases force
  a lost CAS with a fixture-only Git wrapper instead of relying on timing.
- `make -C aye-view check` passed formatting, strict Clippy and 43 Rust tests
  against the updated aye library. No viewer application source changed.
- Separate Standards and Spec reviewers inspected the complete candidate in
  isolated checkouts. Both reported no blocking/actionable findings. These were
  source/test/document reviews, not duplicate executions of the test suites.
- The skill validator passed; six JSON examples parsed, operation keys were
  checked and relative links resolved. The skill remains a portable directory;
  no handler/model configuration was introduced.
- A deterministic store test publishes a competing transaction between read and
  CAS. Whole-operation replay preserves the unrelated task, adds one note, and
  returns a pinned receipt. The native reader sees only published complete
  snapshots and can still load that receipt after later ref movement.

Installed aye SHA-256:
`79ea5f0185d44ba0150b0f95798fc0e8b602330b20eed9802e0f55d643667318`.
Local full logs, comparison data, reviewed-plan snapshot and runtime evidence
are retained in ignored `aye/test/direct-operations-20260923/` in the primary
checkout. Durable reproduction is the package Makefile suites and source tests;
fixture repositories are disposable, not a second task authority.

## Agent using only the portable skill

A separate runtime tester read the skill and relevant references without CLI
implementation/test source. In a disposable repository it acquired the authorized
label, received the full task and blocked/deferred related briefs, uppercased a
two-line file, verified its bytes, and paused with one atomic note plus release.
The primary independently verified the resulting file, notes and open/unclaimed
state. The fixture task was intentionally not closed.

The initial run had two unavailable-command attempts: test setup wording caused
the agent to treat the version as part of the executable name (`aye0.3.0`). Those
attempts did not execute aye. After using the actual executable `aye`, acquisition
succeeded; a required first-use checkout-context note was a separate write, then
pause used one apply. Both the mistake and original trace are preserved.

A coached follow-up corrected only the executable-name ambiguity and reused the
verified checkout. One aye invocation / one tool round trip returned the complete
assignment, including both prior notes and both related statuses. One invocation /
one tool round trip recorded the new handoff and released it. There were no extra
confirmation reads in that agent flow. Reply sizes were 3,075 and 2,011 bytes.
This demonstrates the direct workflow, not universal model reliability or a token
billing guarantee. Agent usage and exact context size were not exposed.

## Bounded CLI comparison

One scripted local observation used equivalent seeded states and excluded common
setup equally. No contention or retries were injected in these timing samples.

| Operation | Legacy CLI calls | Direct CLI calls | Legacy/direct output bytes | Legacy/direct seconds |
| --- | ---: | ---: | ---: | ---: |
| ready + show + claim / next claim | 3 | 1 | 1892 / 1749 | 0.333 / 0.198 |
| note + release / batch pause | 2 | 1 | 1361 / 1022 | 0.382 / 0.190 |
| create three tasks + two edges / batch graph | 5 | 1 | 2474 / 3036 | 1.001 / 0.202 |

The graph receipt is larger because it returns aliases, operation outcomes and
final task states together. These are CLI measurements controlled by a script;
CLI count is not model-call count. The separate agent trace above establishes
one-round acquisition/pause in that observed run. Cached/uncached input, reasoning
usage, billing and a near-200k-context comparison were unavailable and were not
measured or simulated. No savings percentage is claimed.

## Boundaries retained

The suites cover stale-view no-write outcomes, scope/order, existing claims,
related statuses/omissions, mandatory packet oversize before publication, explicit
claim compatibility, same/different actor races, late batch rollback, local alias
errors, ownership/graph guards, stable IDs/notes across CAS and stale guarded retry.
Lost stdout is deliberately discarded in a recovery fixture; the tests also show
that a later next-claim after close allocates different work. They do not establish
end-to-end transport delivery, historical exactly-once replay, or remote distributed
claim locking. The skill explains these exceptions and retains read-only recovery.

No real source push or task sync was part of this delivery. Local fixture sync
coverage in the existing regression suite does not publish this project's refs.


## Pause readiness clarification (2026-09-23)

The ordinary-pause versus explicit-shelving decision is now explicit in the
portable skill, including its worktree-resumption entry point. A session pause
for any reason uses a handoff note plus release (note only if already open).
Unblocked work remains eligible for the next session without resume or an extra
pause-specific confirmation. Explicit shelving uses defer until authorized
resumption. Genuine dependencies and manual blockers remain in force.

No Rust, canonical schema, viewer, or readiness algorithm changed. Before edits,
the installed aye 0.3.0 pause/defer test and a separate linked-worktree/fresh-actor
probe both passed; no runtime readiness defect was reproduced. The probe observed
ordinary pause followed by a fresh actor's successful next-claim, explicit deferral
remaining excluded, and explicit resume restoring eligibility. The change makes
the agent's intent classification precise and adds durable next-session coverage.

The expanded installed-binary test at
`batch_cases.Batches.test_pause_defer_complete_and_metadata_clear` checks both
open-task notes and owner release, linked-worktree ready visibility, fresh-actor
next-claim with preserved handoff notes, and deferred no-ready/no-write behavior.
It retains the existing explicit resume, completion and metadata assertions.
It passed alongside all three `domain_cases` tests, which cover ownership guards,
manual/dependency blockers and deferred resume retaining a real blocker:

```sh
cd aye/verification
python3 -m unittest batch_cases.Batches.test_pause_defer_complete_and_metadata_clear domain_cases -v
```

These four checks use the Cargo-installed executable. No binary reinstall or Rust
suite rerun was necessary because runtime source was unchanged. CLI checks prove
transitions and selection, not universal natural-language intent recognition.
