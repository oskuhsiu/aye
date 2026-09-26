# Explicit verification bypass

Use `aye bypass` only when implementation is complete enough to unblock downstream
work, but one or more named verification conditions cannot currently be performed,
such as an unavailable physical device, lab, account, fixture or external service.

A bypass is a human risk-acceptance decision. An explicit user instruction such as
“bypass this task because the target device is unavailable” is sufficient
authorization; execute it without asking for a second confirmation. Never infer
authorization from schedule pressure, a failed test environment, the absence of
hardware or the agent's own confidence. A pre-existing project policy may also
authorize bypass when it clearly covers the exact missing checks.

## Direct command

Use the native aye command, not manual label edits followed by `close`:

```sh
aye --json bypass TASK_ID \
  --reason "Implementation and review complete; target hardware unavailable" \
  --missing "Physical-device smoke test" \
  --missing "Bluetooth reconnect test"
```

Use the reason and missing checks provided by the user or supported by task evidence.
Do not invent checks or say that a missing check passed. At least one nonempty
`--missing` value is required.

The command performs one native task mutation. It:

1. preserves claim ownership and refuses a different actor;
2. refuses unresolved task prerequisites with
   `BYPASS_UNRESOLVED_DEPENDENCIES`;
3. accepts open, owned in-progress or deferred work;
4. clears an external/manual blocker while retaining its reason in the audit note;
5. preserves ordinary labels and adds the reserved `aye:bypassed` marker;
6. appends an attributed note containing the reason and every missing check; and
7. closes the task as dependency-satisfying work.

The JSON result includes `computed.bypassed`, `newly_ready` and
`waived_manual_block`. Report which checks remain unverified and which downstream
tasks became ready. Do not describe bypassed work as fully verified.

## Atomic agent batches

When the user explicitly authorizes several bypasses, or the bypass must be grouped
with other already-decided task changes, use native batch operations:

```json
{
  "version": 1,
  "operations": [
    {
      "op": "bypass",
      "id": "t-0123456789abcdef0123",
      "reason": "Target hardware unavailable",
      "missing": [
        "Physical-device smoke test",
        "Bluetooth reconnect test"
      ]
    }
  ]
}
```

Run it with `aye --json apply --file request.json` or `--file -`. The entire batch
publishes or fails together. Each bypass operation still enforces authorization
semantics, ownership and unresolved-dependency checks.

## Inspecting and reopening

Use these read paths instead of interpreting the label yourself:

```sh
aye --json show TASK_ID
aye --json list --state bypassed
aye --json status
```

`show` reports `computed.bypassed`; status/report include bypass counts. The
canonical format remains version 1. Internally the task is `closed(done)` so older
aye versions preserve dependency behavior; current aye and aye-view present the
reserved marker as bypassed, with aye-view showing `⚠ closed(bypassed)`.

If later verification fails or implementation must change, use `aye reopen TASK_ID`.
Reopen automatically clears the current bypass marker while preserving the audit
note. If later verification succeeds without further work, append evidence with
`aye note`; the historical bypass remains visible in notes unless project policy
requires reopening and normal closure.

Do not add or remove `aye:bypassed` with create/update. It is controlled by
`bypass`, normal `close` and `reopen`.

Bypass is not appropriate when code is unfinished, a real task dependency remains
open, or only selected downstream tasks may proceed. In those cases keep the task
open, create a separate verification task, or restructure dependencies so only
authorized downstream work is released.
