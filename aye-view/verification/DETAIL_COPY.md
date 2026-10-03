# Detail selection and copy contract

Detail retains the viewer's mouse reporting and adds left-drag text selection.
Release copies automatically; Ctrl+C copies a nonempty selection again instead
of quitting. Copy-on-release works without needing Cmd+C delivery; if a terminal
does deliver Super+C, it also copies. An ordinary click activates Detail without
changing the clipboard. The selected text stays highlighted after release.

Rendering and pointer input use the same sanitized source ranges and terminal
grapheme widths. Forward/reverse selection includes whole endpoint graphemes,
including CJK continuation cells, emoji ZWJ sequences and combining marks.
Soft wrapping does not introduce copied newlines; hard/blank lines and source
spaces remain exact. Borders, padding and Main text are not clipboard content.
Release coordinates finish the selection even without a final motion event.
Captured motion/release outside Detail is clamped to its visible area, without
starting Graph pan or activating Back. There is no automatic edge scrolling.

Navigation, wheel, replacement press, resize, snapshot replacement, overlays and
reported focus loss clear selection and cancel capture. Redraw alone preserves
it. A wrong-button release does not copy. `q` quits; Ctrl+C without a nonempty
Detail selection keeps the existing quit behavior. Repository state is read-only.

## Clipboard output

Clipboard effects run in the terminal event loop, after the input reducer queues
the exact selected source slice. Local macOS uses `/usr/bin/pbcopy` with piped
UTF-8 stdin, fixed executable, no shell, checked exit status and suppressed helper
output. Success displays `Copied selection`.

SSH/non-macOS sessions, native-command failures and RTF/PostScript-like prefixes
(`{\rtf` or `%!`) use Crossterm's OSC52 command. The prefixes bypass pbcopy's
format detection so literal snippets remain unchanged. The status is `Copy sent
to terminal`, because OSC52 does not acknowledge clipboard acceptance. Terminals
and multiplexers must allow it. An output error displays `Copy failed`; the
selection remains available for retry. No clipboard data is read.

Primary references: [Crossterm clipboard command](https://docs.rs/crossterm/0.29.0/crossterm/clipboard/struct.CopyToClipboard.html),
[iTerm2 clipboard permission](https://iterm2.com/documentation-preferences-general.html),
and Apple's installed `pbcopy(1)` manual.

## Reproduce and evidence limits

Run `make -C aye-view check`, then the documented Cargo-default installation and
installed PTY suite in [README.md](README.md). Rust input/render tests cover both
pane layouts, exact source text and highlight, scroll mapping, cancellation,
wrong-button/outside release, empty geometry and invisible clipped graphemes.
Binary tests check exact UTF-8 subprocess input, failed exit status, OSC52 bytes,
format-prefix routing and output failures.

The installed suite's Detail session sends real SGR press/motion/release and
Ctrl+C input. It checks rendered reverse-video cells and decodes exact clipboard
requests for Unicode, hard/blank lines and soft wraps. Focus loss followed by
release emits no copy; unselected Ctrl+C restores the terminal. Its repository
byte/mode snapshots and Git/HTTP traps retain the existing read-only guarantees.
The session deliberately uses the SSH route to avoid replacing the host
clipboard; it does not establish native pasteboard service acceptance, physical
mouse/touchpad behavior, Cmd+C forwarding or terminal/multiplexer permission.
