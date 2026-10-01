# Mouse v1 verification contract

The release adds read-only mouse browsing to Current Graph/List, Recent, History,
Detail and Help. Single left clicks select the displayed task or activate a pane;
Details/Back support narrow layouts. Search and Filter isolate pointer input.

Wheel input uses fixed three-row/cell steps on the pointed surface, retaining
selection and keyboard focus. Lists have independent offsets; explicit keyboard
selection reveals its row. History wheel exposure continues beyond the initial
50 rows to the last matching closed task. Horizontal or reported Shift-wheel
pans Graph; vertical-only surfaces ignore horizontal events.

Left background dragging moves Graph content with the pointer. Visible node
footprints, including clipped borders, select rather than drag. The accepted
press owns the gesture until release anywhere; crossing other panes does not
transfer it. Navigation, replacement press, resize/rendered geometry changes,
snapshot/scope/density/mode change and reported focus loss cancel capture.

## Reproduce

Use the viewer's documented package check and Cargo-default installation workflow:

```sh
make -C aye-view check
make -C aye-view integration PYTHON=test/venv/bin/python
```

Install the pinned Python requirements as described in the verification README.
The installed suite captures decoded frames, raw PTY output, terminal state,
repository fingerprints, binary hash and source commit in ignored `test/`.
Its browsing fixture contains 40 active tasks in four chains and 120 recently
closed tasks, with a 60-task label-filtered History. It also retains the existing
10,000-task scale fixture and tests passive mouse traffic there.

Deterministic input/render cases cover Unicode/wrapped details, all first-release
surfaces, both densities, partial clipping, independent scrolling, final History
rows, clicks after offsets/reload, overlay isolation, graph coordinates beyond
u16, captured release and cancellation, and empty/tiny/zero layouts. Existing
keyboard, zoom, Focus, reader and watcher tests remain required.

## Evidence limits

PTYs verify xterm SGR parsing and frames, not physical mouse or touchpad behavior
in every terminal. No physical-terminal smoke/copy check is claimed. iTerm2's
Option reporting override is documented in its terminal preferences; other
terminals require their own override/settings. Keyboard controls remain usable.

Raw enable/disable bytes and termios equality establish normal/Ctrl-c tracking
cleanup. Startup errors happen before capture. The shared cleanup-helper byte
test and source review establish routing of normal/error/panic restoration; a
forced post-setup panic was not exercised. Lost releases outside a terminal
window may not be reported; Escape cancels a remaining gesture.

Repository snapshots cover source, config, index, refs and objects. A Git PATH
trap/trace and configured HTTP remote trap detect those routes; they are not a
system-wide network trace. Poller tests/source establish unchanged-OID load
behavior; silent passive-motion PTY output alone does not prove blob read counts.
Performance timings are observations for the documented fixtures, not thresholds.
