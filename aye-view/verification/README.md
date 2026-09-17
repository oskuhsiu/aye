# Viewer verification

Run `make -C aye-view check` for formatting, strict Clippy and the input/render
tests, including the 10,000-task scale case. Shared aye source changes also
require `make -C aye test`.

The installed-binary suite requires Git, installed aye, a Unix PTY (macOS or
Linux), Python 3 and the pinned terminal emulator dependencies. From repo root:

```sh
cargo install --path aye --force --locked
python3 -m venv aye-view/test/venv
aye-view/test/venv/bin/pip install -r aye-view/verification/requirements.txt
make -C aye-view integration PYTHON=test/venv/bin/python
```

The integration target runs package checks, installs aye-view through Cargo's
default location, and tests the installed binary. When that exact source is
already checked and installed, run the suite directly:

```sh
aye-view/test/venv/bin/python aye-view/verification/installed.py
```

Each invocation creates an isolated `aye-view/test/integrated-*` directory and
prints its location. `result.json`, decoded terminal frames, raw PTY bytes and
terminal-restoration records remain there, including on failure. Fixture source
is deliberately staged/unstaged/untracked; canonical test data is seeded with
Git plumbing and later changed by the explicit installed aye writer. No real
project tasks or remote are used. The suite does not delete prior runs.

Coverage includes Graph/List/Search/Filter/Focus/Recent/History/Detail/Help,
wide/narrow/tiny resizing, normal/Ctrl-c/error exits, live linked-worktree
create/claim/edit/close, packed refs, last-good errors and manual/automatic
recovery. The scale fixture has 100 chains of ten active nodes, ten completed
ancestors and 9,000 closed tasks: 10,000 total, 1,000 active, 100 ready, 1,010
current nodes and 910 dependency edges.

Repository byte/mode snapshots include source, index, config, refs and objects.
Snapshots are renewed after each explicit writer change. A PATH Git trap and
Git trace reject viewer Git subprocesses; a local HTTP trap records attempts to
contact the configured promisor remote. This is not a system-wide network syscall
trace. Native reader source uses local git2 calls with network features disabled.
Watcher unit tests establish unchanged-OID/failed-OID load counts; PTY timing
alone cannot prove that blobs were not reread. Scale input/render tests assert
complete node/edge sets and viewport-bounded History row materialization.

Timings are single-run observations, not hard performance thresholds. Startup
includes wrapper startup and terminal emulation. Key timings end when the
expected screen content is observed (10 ms sampling plus a 50 ms frame drain);
reload timings include
polling, parsing, rendering and snapshot audit overhead. Reports identify the
installed binary hash, platform and source base. Keep measured limitations and
manual frame-review findings with the result before claiming acceptance.
