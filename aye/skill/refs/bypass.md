# Explicit verification bypass

Use `aye bypass` only when implementation is complete enough to unblock downstream
work, but a named verification condition cannot currently be performed, such as an
unavailable physical device, lab, account, fixture or external service.

A bypass is a human risk-acceptance decision. An agent must not infer or authorize
one from schedule pressure, a missing environment or its own confidence. Require an
explicit user instruction or an already-recorded project policy that clearly covers
the exact missing checks.

```sh
aye --json bypass TASK_ID \
  --reason "Implementation and review complete; target hardware unavailable" \
  --missing "Physical-device smoke test" \
  --missing "Bluetooth reconnect test"
```

The command atomically:

1. refuses unresolved task prerequisites;
2. resumes a deferred task and clears only its external/manual blocker when needed;
3. preserves existing labels and adds the reserved `aye:bypassed` marker;
4. appends an attributed note containing the reason and every missing check; and
5. closes the task as dependency-satisfying work.

The canonical format remains version 1. Internally the task is `closed(done)` so
older aye versions preserve dependency behavior; current aye-view presents the
reserved marker as `closed(bypassed)` with `⚠`. Do not add or remove the reserved
label manually. Reopening a bypassed task currently requires removing the marker
explicitly if the work should no longer appear bypassed.

Bypass is not appropriate when code is unfinished, a real task dependency remains
open, or a downstream task must still wait for verification. In those cases keep
the task open, create a separate verification task, or restructure dependencies so
only authorized downstream work is released.
