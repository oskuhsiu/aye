#!/usr/bin/env python3
"""Installed aye-view acceptance. Disposable repositories/evidence stay in test/."""
import argparse
import copy
from datetime import datetime, timedelta, timezone
import fcntl
import hashlib
import json
import os
from pathlib import Path
import platform
import pty
import select
import shutil
import signal
import socketserver
import struct
import subprocess
import sys
import tempfile
import termios
import threading
import time

import pyte
from wcwidth import wcwidth, wcswidth

PACKAGE = Path(__file__).resolve().parents[1]
GIT = shutil.which('git')
AYE = shutil.which('aye')
STATE = 'refs/agent-tasks/state'

MOUSE_ENABLE = (b'\x1b[?1000h', b'\x1b[?1002h', b'\x1b[?1003h',
                b'\x1b[?1015h', b'\x1b[?1006h')
MOUSE_DISABLE = (b'\x1b[?1006l', b'\x1b[?1015l', b'\x1b[?1003l',
                 b'\x1b[?1002l', b'\x1b[?1000l')


def run(cwd, *args, input=None, env=None):
    return subprocess.run(args, cwd=cwd, input=input, text=True, check=True,
                          capture_output=True, env=env).stdout.strip()


def git(repo, *args, **kwargs):
    return run(repo, GIT, *args, **kwargs)


def aye(repo, *args):
    result = json.loads(run(repo, AYE, '--json', '--actor', 'verification', *args))
    assert result['ok'], result
    return result['data']


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def snapshot(*roots):
    """Include objects, refs, config, index, dirty source and untracked files."""
    return {f'{root.name}/{p.relative_to(root)}': (p.stat().st_mode, digest(p))
            for root in roots for p in root.rglob('*') if p.is_file()}


def unchanged(before, *roots):
    after = snapshot(*roots)
    differences = sorted(k for k in before.keys() | after.keys()
                         if before.get(k) != after.get(k))
    assert not differences, ('Viewer wrote repository files', differences)


def stamp(value):
    return value.isoformat(timespec='milliseconds').replace('+00:00', 'Z')


def sgr_mouse(button, col, row, release=False):
    """Encode a 0-based terminal cell as one SGR mouse event."""
    assert button >= 0 and col >= 0 and row >= 0
    suffix = 'm' if release else 'M'
    return f'\x1b[<{button};{col + 1};{row + 1}{suffix}'.encode()


def verify_mouse_capture(raw):
    """Require cleanup only when the viewer reached mouse-capture setup."""
    enabled = [raw.find(sequence) for sequence in MOUSE_ENABLE]
    if all(position < 0 for position in enabled):
        # Startup failures can happen before terminal setup. They must still
        # restore termios, but cannot emit a disable sequence they never need.
        return False
    assert all(position >= 0 for position in enabled), raw
    disabled = [raw.find(sequence) for sequence in MOUSE_DISABLE]
    assert all(position >= 0 for position in disabled), raw
    assert max(enabled) < min(disabled), raw
    return True


def task_id(n):
    return f't-{n + 1:020x}'


def fixture(base, name, remote):
    root = base / name
    root.mkdir()
    repo = root / 'repo'
    repo.mkdir()
    git(repo, 'init', '-q')
    git(repo, 'config', 'user.name', 'Viewer verification')
    git(repo, 'config', 'user.email', 'viewer@example.invalid')
    (repo / 'nested').mkdir()
    source = repo / 'nested' / 'source.txt'
    source.write_text('base\n')
    git(repo, 'add', '.')
    git(repo, 'commit', '-qm', 'Fixture source')
    aye(repo, 'init', '--offline')
    git(repo, 'config', 'remote.origin.url', remote)
    git(repo, 'config', 'remote.origin.promisor', 'true')
    linked = root / 'linked'
    git(repo, 'worktree', 'add', '-qb', 'reader', str(linked))
    source.write_text('staged\n')
    git(repo, 'add', 'nested/source.txt')
    source.write_text('unstaged\n')
    (repo / 'untracked.txt').write_text('preserve me\n')
    return repo, linked


def seed_scale(repo):
    sample = aye(repo, 'create', 'Template')['task']
    parent = git(repo, 'rev-parse', STATE)
    message = 'Target scale fixture\n'
    parts = [f'commit {STATE}\ncommitter Verification <viewer@example.invalid> '
             f'1000000000 +0000\ndata {len(message)}\n{message}from {parent}\nD tasks\n']
    now = datetime.now(timezone.utc)
    for n in range(10_000):
        task = copy.deepcopy(sample)
        task.update(id=task_id(n), title=f'Scale {n:05}')
        if n >= 1_000:
            task.update(status='closed', resolution='done',
                        closed_at=stamp(now - timedelta(seconds=n - 1_000)))
        elif n >= 100:
            task['depends_on'] = [task_id(n - 100)]
        elif n < 10:
            task['depends_on'] = [task_id(1_000 + n)]
        if n == 0:
            task['description'] = '\n'.join(f'Detail line {i:02}: 中文界面' for i in range(60))
        body = json.dumps(task, ensure_ascii=False) + '\n'
        parts.append(f'M 100644 inline tasks/{task["id"][2:4]}/{task["id"]}.json\n'
                     f'data {len(body.encode())}\n{body}\n')
    parts.append('\ndone\n')
    git(repo, 'fast-import', '--quiet', input=''.join(parts))
    # Projections intentionally describe the old template: reading must not repair them.
    records = aye(repo, 'list', '--all')
    assert len(records) == 10_000
    assert sum(r['task']['status'] != 'closed' for r in records) == 1_000
    assert len(aye(repo, 'ready', '--all')) == 100


def seed_browsing(repo):
    """Create a compact graph with 40 active and 120 recently closed tasks."""
    sample = aye(repo, 'create', 'Browse template')['task']
    parent = git(repo, 'rev-parse', STATE)
    message = 'Mouse browsing fixture\n'
    parts = [f'commit {STATE}\ncommitter Verification <viewer@example.invalid> '
             f'1000000000 +0000\ndata {len(message)}\n{message}from {parent}\nD tasks\n']
    now = datetime.now(timezone.utc)
    for n in range(160):
        task = copy.deepcopy(sample)
        task.update(id=task_id(n), title=f'Browse {n:03}')
        if n < 40:
            task['depends_on'] = ([task_id(n - 1)]
                                  if n % 10 else [])
        else:
            task.update(status='closed',
                        resolution='cancelled' if n % 2 else 'done',
                        closed_at=stamp(now - timedelta(seconds=n - 40)))
            if n % 2 == 0:
                task['labels'] = ['even']
        if n == 0:
            task['description'] = '\n'.join(
                f'Detail line {i:02}: 中文內容' for i in range(60))
        body = json.dumps(task, ensure_ascii=False) + '\n'
        parts.append(f'M 100644 inline tasks/{task["id"][2:4]}/{task["id"]}.json\n'
                     f'data {len(body.encode())}\n{body}\n')
    parts.append('\ndone\n')
    git(repo, 'fast-import', '--quiet', input=''.join(parts))
    records = aye(repo, 'list', '--all')
    assert len(records) == 160
    assert sum(r['task']['status'] != 'closed' for r in records) == 40
    assert len(aye(repo, 'ready', '--all')) == 4


class NetworkTrap(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True


class TrapHandler(socketserver.BaseRequestHandler):
    def handle(self):
        self.server.attempts.append(self.client_address)


class Session:
    def __init__(self, cwd, output, binary, audit_bin, cols=190, rows=40):
        self.output = output
        output.mkdir()
        self.master, self.slave = pty.openpty()
        self.before = repr(termios.tcgetattr(self.slave))
        self.screen = pyte.Screen(cols, rows)
        self.stream = pyte.ByteStream(self.screen)
        self.raw = bytearray()
        self.cursor_tail = b''
        self.timings = {}
        self.frames = 0
        self.resize(cols, rows)
        self.audit = output / 'git-calls.txt'
        self.trace = output / 'git-trace.txt'
        self.restoration = output / 'terminal.json'

        def child_setup():
            os.setsid()
            fcntl.ioctl(self.slave, termios.TIOCSCTTY, 0)

        # Check termios before macOS closes the controlling PTY on wrapper exit.
        wrapper = '''import json, subprocess, sys, termios
from pathlib import Path
before = repr(termios.tcgetattr(0))
status = subprocess.call([sys.argv[1]])
after = repr(termios.tcgetattr(0))
Path(sys.argv[2]).write_text(json.dumps(dict(before=before, after=after)))
sys.exit(status)
'''
        self.started = time.monotonic()
        self.proc = subprocess.Popen(
            [sys.executable, '-c', wrapper, binary, str(self.restoration)],
            cwd=cwd, stdin=self.slave, stdout=self.slave, stderr=self.slave,
            preexec_fn=child_setup,
            env={**os.environ, 'TERM': 'xterm-256color', 'NO_COLOR': '1',
                 'PATH': str(audit_bin) + os.pathsep + os.environ['PATH'],
                 'VIEWER_GIT_AUDIT': str(self.audit), 'GIT_TRACE': str(self.trace)})

    def resize(self, cols, rows):
        self.screen.resize(lines=rows, columns=cols)
        fcntl.ioctl(self.slave, termios.TIOCSWINSZ,
                    struct.pack('HHHH', rows, cols, 0, 0))
        if hasattr(self, 'proc'):
            os.killpg(self.proc.pid, signal.SIGWINCH)

    def read(self, seconds):
        until = time.monotonic() + seconds
        while time.monotonic() < until:
            ready, _, _ = select.select([self.master], [], [], max(0, until - time.monotonic()))
            if not ready:
                break
            try:
                chunk = os.read(self.master, 65536)
            except OSError:
                break
            if not chunk:
                break
            self.raw.extend(chunk)
            self.stream.feed(chunk)
            query = self.cursor_tail + chunk
            for _ in range(query.count(b'\x1b[6n')):
                os.write(self.master, b'\x1b[1;1R')
            self.cursor_tail = query[-3:]

    def text(self):
        # Normalize orphan empty continuation cells left by pyte after wide glyphs.
        lines = []
        for y in range(self.screen.lines):
            row, x = [], 0
            while x < self.screen.columns:
                value = self.screen.buffer[y][x].data or ' '
                row.append(value)
                x += max(1, wcwidth(value[0]))
            line = ''.join(row)
            assert wcswidth(line) <= self.screen.columns, line
            lines.append(line)
        return '\n'.join(lines)

    def capture(self, name):
        self.frames += 1
        text = self.text()
        (self.output / f'{self.frames:02}-{name}.txt').write_text(text + '\n')
        return text

    def wait(self, name, predicate, started=None, limit=15):
        started = time.monotonic() if started is None else started
        while time.monotonic() - started < limit:
            self.read(.01)
            text = self.text()
            if predicate(text):
                # A matching header can arrive before the remaining frame bytes.
                # Drain the frame before saving it or reporting observed latency.
                self.read(.05)
                if not predicate(self.text()):
                    continue
                self.timings[name] = round(time.monotonic() - started, 4)
                return self.capture(name)
            if self.proc.poll() is not None:
                break
        self.capture('FAILED-' + name)
        raise AssertionError((name, self.text(), bytes(self.raw[-2000:])))

    def key(self, name, keys, predicate):
        start = time.monotonic()
        os.write(self.master, keys)
        return self.wait(name, predicate, start)

    def cells(self, value):
        """Find every occurrence of a displayed string in exact cells."""
        assert value
        matches = []
        for row, line in enumerate(self.screen.display):
            offset = 0
            while True:
                offset = line.find(value, offset)
                if offset < 0:
                    break
                col = wcswidth(line[:offset])
                width = wcswidth(value)
                if col >= 0 and width > 0 and col + width <= self.screen.columns:
                    matches.append((col, row))
                offset += max(1, len(value))
        return matches

    def cell(self, value, occurrence=0):
        """Find a displayed string and return its exact 0-based cell."""
        matches = self.cells(value)
        assert matches, (value, self.text())
        return matches[occurrence]

    def mouse_sequence(self, name, events, predicate):
        start = time.monotonic()
        os.write(self.master, b''.join(events))
        return self.wait(name, predicate, start)

    def mouse_click(self, name, col, row, predicate, button=0):
        """Send a press/release click using 0-based cells."""
        return self.mouse_sequence(
            name,
            [sgr_mouse(button, col, row), sgr_mouse(button, col, row, release=True)],
            predicate,
        )

    def mouse_text_click(self, name, value, predicate, occurrence=0, button=0):
        col, row = self.cell(value, occurrence)
        return self.mouse_click(name, col, row, predicate, button)

    def mouse_press(self, name, col, row, predicate):
        return self.mouse_sequence(name, [sgr_mouse(0, col, row)], predicate)

    def mouse_motion(self, name, col, row, predicate):
        return self.mouse_sequence(name, [sgr_mouse(32, col, row)], predicate)

    def mouse_move(self, name, col, row, predicate):
        """Send passive SGR Moved (button 35: motion without a button)."""
        return self.mouse_sequence(name, [sgr_mouse(35, col, row)], predicate)

    def mouse_release(self, name, col, row, predicate):
        return self.mouse_sequence(name, [sgr_mouse(0, col, row, release=True)], predicate)

    def mouse_drag(self, name, start, end, predicate, release=None):
        """Send left press, left drag motion and release in cell coordinates."""
        release = end if release is None else release
        return self.mouse_sequence(
            name,
            [sgr_mouse(0, *start), sgr_mouse(32, *end),
             sgr_mouse(0, *release, release=True)],
            predicate,
        )

    def mouse_wheel(self, name, col, row, direction, predicate, count=1, shift=False):
        """Send SGR wheel events, using Crossterm's four direction codes."""
        codes = {'up': 64, 'down': 65, 'left': 66, 'right': 67}
        assert direction in codes and count > 0
        button = codes[direction] + (4 if shift else 0)
        return self.mouse_sequence(
            name,
            [sgr_mouse(button, col, row) for _ in range(count)],
            predicate,
        )

    def finish(self, keys=b'q', expected=0):
        if self.proc.poll() is None and keys:
            os.write(self.master, keys)
        until = time.monotonic() + 10
        while self.proc.poll() is None and time.monotonic() < until:
            self.read(.02)
        assert self.proc.poll() == expected, (self.proc.poll(), self.text())
        self.read(.02)
        restoration = json.loads(self.restoration.read_text())
        assert self.before == restoration['before'] == restoration['after'], restoration
        mouse_capture = verify_mouse_capture(bytes(self.raw))
        if expected == 0:
            assert mouse_capture, 'Successful interactive session never enabled mouse capture'
        assert not self.audit.exists(), self.audit.read_text() if self.audit.exists() else ''
        assert not self.trace.exists() or not self.trace.read_text().strip()
        return {'timings_seconds': self.timings, 'frames': self.frames,
                'terminal_restored': True, 'mouse_capture_verified': mouse_capture,
                'git_subprocess_calls': 0}

    def __enter__(self):
        return self

    def __exit__(self, *unused):
        if self.proc.poll() is None:
            os.killpg(self.proc.pid, signal.SIGCONT)
            os.write(self.master, b'\x03')
            until = time.monotonic() + 2
            while self.proc.poll() is None and time.monotonic() < until:
                self.read(.02)
            if self.proc.poll() is None:
                os.killpg(self.proc.pid, signal.SIGKILL)
                self.proc.wait(timeout=5)
        (self.output / 'session.pty').write_bytes(self.raw)
        os.close(self.master)
        os.close(self.slave)


def contains(value):
    return lambda text: value in text


def pane_cell(session, value, x_min, x_max, y_min=0, y_max=None):
    """Locate text only when its cell lies in the expected pane rectangle."""
    y_max = session.screen.lines if y_max is None else y_max
    for col, row in session.cells(value):
        if x_min <= col < x_max and y_min <= row < y_max:
            return col, row
    raise AssertionError((value, (x_min, x_max, y_min, y_max), session.text()))


def scale_case(base, binary, audit, remote):
    repo, linked = fixture(base, 'scale', remote)
    seed_scale(repo)
    before = snapshot(repo, linked)
    with Session(linked / 'nested', base / 'scale-pty', binary, audit) as session:
        session.wait('startup', contains('1010 visible tasks'), session.started)
        passive_text = session.text()
        passive_raw = bytes(session.raw)
        passive_frames = session.frames
        session.mouse_move('passive-motion', 10, 10, lambda text: True)
        session.read(.5)
        assert session.text() == passive_text
        assert bytes(session.raw) == passive_raw
        assert session.frames == passive_frames
        session.key('list', b'\t', contains('aye-view · List'))
        session.key('search-open', b'/', contains('Search'))
        session.key('search-query', b'Scale 00999', contains('> Scale 00999'))
        session.key('search-select', b'\r', lambda t: 'Search · all tasks' not in t and task_id(999) in t)
        session.key('focus', b'F', contains('Focus Graph · 10 visible tasks'))
        session.key('current', b'g', contains('1010 visible tasks'))
        session.key('filter-open', b'f', contains('Filter · five'))
        session.key('filter-ready', b'\x1b[C\r', contains('100 visible tasks'))
        session.key('filter-clear', b'fc\r', contains('1010 visible tasks'))
        session.key('recent', b'c', contains('Recently closed'))
        session.key('recent-select', b']', contains(task_id(1010)))
        session.key('history-first', b'h', contains('50/9000'))
        session.key('history-next', b'\x1b[6~\x1b[6~', contains('100/9000'))
        session.key('detail', b'\r', contains('Detail'))
        session.resize(50, 18)
        session.wait('narrow', lambda t: 'Detail' in t and 'Scale 010' in t)
        session.key('back-history', b'\x1b', contains('History'))
        session.resize(20, 6)
        session.read(.4)
        session.capture('tiny')
        assert session.proc.poll() is None
        session.resize(190, 40)
        session.key('restore-current', b'g', contains('Current Graph'))
        session.key('help', b'?', contains('without joining'))
        session.key('help-close', b'\x1b', contains('Current Graph'))
        session.read(1.6)  # More than three unchanged watcher polls.
        session.capture('unchanged-polls')
        # Rendering must preserve stale projections, source, index, refs and objects.
        unchanged(before, repo, linked)
        session.key('reload-search', b'/Scale 00000', contains('> Scale 00000'))
        session.key('reload-select', b'\r', lambda t: 'Search · all tasks' not in t and task_id(0) in t)
        start = time.monotonic()
        aye(repo, 'update', task_id(0), '--title', 'ScaleChanged00000')
        writer_seconds = round(time.monotonic() - start, 4)
        published = time.monotonic()
        before = snapshot(repo, linked)
        session.wait('changed-ref-reload', contains('ScaleChanged00000'), published)
        unchanged(before, repo, linked)
        report = session.finish()
    unchanged(before, repo, linked)
    report.update(total=10_000, active=1_000, ready=100, current_with_ancestors=1_010,
                  edges=910, closed=9_000, readonly=True, stale_views_preserved=True,
                  explicit_writer_seconds=writer_seconds)
    return report


def mouse_case(base, binary, audit, remote):
    """Exercise installed SGR clicks against the last rendered hit map."""
    repo, linked = fixture(base, 'mouse', remote)
    titles = ['MouseGraphA', 'MouseB中文', 'MouseCompactC']
    titles.extend(f'MouseList{i:02}中' for i in range(36))
    task_ids = {}
    for title in titles:
        task_ids[title] = aye(repo, 'create', title)['task']['id']
    before = snapshot(repo, linked)
    selected = 'MouseB中文'
    selected_id = task_ids[selected]
    compact_title = 'MouseCompactC'
    compact_id = task_ids[compact_title]
    list_target = 'MouseList26中'
    list_target_id = task_ids[list_target]
    list_selected = 'MouseList27中'
    with Session(linked / 'nested', base / 'mouse-pty', binary, audit,
                 cols=130, rows=30) as session:
        session.wait('startup', contains('Current Graph'), session.started)
        session.mouse_text_click(
            'graph-unicode', selected,
            lambda text: text.count(selected) >= 2 and selected_id in text,
        )
        # The wide Detail pane is itself a pointer surface. Clicking its
        # title activates the pane; j must scroll Detail rather than move the
        # graph selection.
        detail_col, detail_row = session.cell(selected)
        assert detail_col > session.screen.columns * .65, 'Must click the Detail title'
        session.mouse_click('detail-activate', detail_col, detail_row,
                            lambda text: selected_id in text)
        session.key('detail-scroll', b'j',
                    lambda text: selected_id in text and text.count(selected) == 1)
        session.mouse_text_click(
            'wide-back', '[Back]',
            lambda text: 'Current Graph' in text and selected in text,
        )
        # After Back, keyboard navigation must be handled by Main. Move to
        # the next task and back so the following narrow cases retain B.
        session.key('main-down', b'\x1b[B',
                    lambda text: compact_id in text and compact_title in text)
        session.key('main-up', b'\x1b[A',
                    lambda text: selected_id in text and selected in text)

        session.resize(70, 24)
        session.wait('narrow-main',
                     lambda text: '[Details]' in text and selected in text)
        session.mouse_text_click(
            'narrow-details', '[Details]',
            lambda text: '[Back]' in text and selected_id in text,
        )
        session.mouse_text_click(
            'narrow-back', '[Back]',
            lambda text: '[Details]' in text and selected in text,
        )

        session.key('compact', b'-', contains('Compact'))
        session.mouse_text_click(
            'compact-unicode', compact_title,
            contains(compact_title),
        )
        session.key('standard', b'+', contains('Standard'))

        session.resize(130, 30)
        session.wait('wide-selection',
                     lambda text: compact_id in text and text.count(compact_title) >= 2)
        session.key('wide-list', b'\t', contains('aye-view · List'))
        session.key(
            'list-offset', b'\x1b[B' * 28,
            lambda text: list_selected in text,
        )
        session.mouse_text_click(
            'list-unicode-row', list_target,
            lambda text: list_target_id in text and text.count(list_target) >= 2,
        )

        # Search, Filter and Help are topmost modal surfaces. Clicking their
        # background must not select a row underneath them.
        session.key('search-overlay', b'/', contains('Search · all tasks'))
        session.mouse_click('search-background', 4, 4,
                            lambda text: 'Search · all tasks' in text)
        session.key('search-close', b'\x1b',
                    lambda text: 'List' in text and list_target_id in text)
        session.key('filter-overlay', b'f', contains('Filter · five'))
        session.mouse_click('filter-background', 4, 4,
                            lambda text: 'Filter · five' in text)
        session.key('filter-close', b'\x1b',
                    lambda text: 'List' in text and list_target_id in text)
        session.key('help-overlay', b'?', contains('Help · j/k'))
        session.mouse_click('help-background', 4, 4,
                            lambda text: 'Help · j/k' in text)
        session.key('help-close', b'\x1b',
                    lambda text: 'List' in text and list_target_id in text)

        session.resize(20, 6)
        session.read(.4)
        session.mouse_click('tiny-background', 0, 0,
                            lambda text: session.proc.poll() is None)
        assert session.proc.poll() is None
        session.resize(130, 30)
        session.wait('restore-wide', contains('List'))
        unchanged(before, repo, linked)
        report = session.finish()
        assert report['mouse_capture_verified'] is True
    unchanged(before, repo, linked)
    report.update(readonly=True, unicode_cell_click=True,
                  graph_densities=['Standard', 'Compact'],
                  list_offset_click=True, overlays_isolated=True,
                  tiny_click_safe=True)
    return report


def mouse_browsing_case(base, binary, audit, remote):
    """Exercise Recent/History browsing, wheel routing and graph gestures."""
    repo, linked = fixture(base, 'browsing', remote)
    seed_browsing(repo)
    before = snapshot(repo, linked)
    recent_initial_title = 'Browse 040'
    recent_title = 'Browse 043'
    recent_id = task_id(43)
    detail_title = 'Browse 000'
    detail_id = task_id(0)
    list_title = 'Browse 012'
    list_id = task_id(12)
    history_final_title = 'Browse 158'
    history_final_id = task_id(158)

    with Session(linked / 'nested', base / 'browsing-pty', binary, audit,
                 cols=130, rows=30) as session:
        session.wait('startup', contains('Current Graph'), session.started)
        session.key('recent', b'c', contains('Recently closed'))
        recent_col, recent_row = pane_cell(
            session, recent_initial_title, 0, 85, 24, 29)
        recent_before_wheel = session.text()
        session.mouse_wheel(
            'recent-wheel', recent_col, recent_row, 'down',
            lambda text: text != recent_before_wheel
            and recent_title in text and detail_id in text,
        )
        recent_col, recent_row = pane_cell(session, recent_title, 0, 85, 24, 29)
        session.mouse_click(
            'recent-click', recent_col, recent_row,
            lambda text: recent_id in text and recent_title in text,
        )
        detail_col, detail_row = pane_cell(session, recent_id, 85, 130, 2, 29)
        session.mouse_click(
            'recent-detail-activate', detail_col, detail_row,
            lambda text: recent_id in text,
        )
        session.mouse_text_click(
            'recent-wide-back', '[Back]',
            lambda text: 'Recently closed' in text and recent_id in text,
        )

        session.resize(70, 24)
        session.wait('recent-narrow',
                     lambda text: '[Details]' in text and recent_title in text)
        session.mouse_text_click(
            'recent-narrow-details', '[Details]',
            lambda text: '[Back]' in text and recent_id in text,
        )
        session.mouse_text_click(
            'recent-narrow-back', '[Back]',
            lambda text: 'Recently closed' in text and recent_title in text,
        )

        session.resize(130, 30)
        session.wait('recent-wide-again', contains('Recently closed'))
        session.key('list', b'\t', contains('aye-view · List'))
        assert 1 <= 4 < 60 and 3 <= 4 < 28
        selected_before_list_wheel = session.text()
        session.mouse_wheel(
            'list-wheel', 4, 4, 'down',
            lambda text: recent_id in text,
            count=3,
        )
        assert recent_id in session.text() and session.text() != selected_before_list_wheel
        list_col, list_row = pane_cell(session, list_title, 0, 60, 3, 28)
        session.mouse_click(
            'list-after-wheel', list_col, list_row,
            lambda text: list_id in text and list_title in text,
        )

        before_list_top = session.text()
        session.mouse_wheel(
            'list-to-top', 4, 4, 'up',
            lambda text: text != before_list_top and detail_title in text,
            count=4,
        )
        top_col, top_row = pane_cell(session, detail_title, 0, 60, 3, 28)
        session.mouse_click(
            'detail-task-row', top_col, top_row,
            lambda text: detail_id in text and detail_title in text,
        )
        detail_col, detail_row = pane_cell(session, detail_id, 55, 130, 2, 29)
        session.mouse_click(
            'detail-wheel-activate', detail_col, detail_row,
            lambda text: detail_id in text,
        )
        session.mouse_wheel(
            'detail-wheel-down', detail_col, detail_row, 'down',
            lambda text: detail_id in text and text.count(detail_title) == 1,
        )
        session.mouse_wheel(
            'detail-wheel-up', detail_col, detail_row, 'up',
            lambda text: detail_id in text and text.count(detail_title) >= 2,
        )
        session.mouse_text_click(
            'detail-back', '[Back]',
            lambda text: 'aye-view · List' in text and detail_id in text,
        )

        session.key('help', b'?', contains('Help · j/k'))
        session.mouse_wheel(
            'help-wheel', 4, 4, 'down',
            lambda text: 'Help · j/k' in text and 'Focus root stays fixed' in text,
            count=12,
        )
        session.key('help-close', b'\x1b',
                    lambda text: 'aye-view · List' in text and detail_id in text)

        session.key('history', b'h', contains('History · 50/120'))
        history_col, history_row = pane_cell(
            session, recent_initial_title, 0, 60, 3, 29)
        session.mouse_click(
            'history-first-click', history_col, history_row,
            lambda text: task_id(40) in text and recent_initial_title in text,
        )
        session.resize(70, 24)
        session.wait('history-narrow',
                     lambda text: 'History ·' in text and '[Details]' in text)
        session.mouse_text_click(
            'history-narrow-details', '[Details]',
            lambda text: '[Back]' in text and task_id(40) in text,
        )
        session.mouse_text_click(
            'history-narrow-back', '[Back]',
            lambda text: 'History ·' in text and recent_initial_title in text,
        )
        session.resize(130, 30)
        session.wait('history-wide', contains('History ·'))
        session.key(
            'history-filter-even', b'\x1b[B' * 3 + b'\x1b[C\r',
            lambda text: 'History · 50/60' in text,
        )
        assert 1 <= 4 < 60 and 3 <= 4 < 29
        session.mouse_wheel(
            'history-final-wheel', 4, 4, 'down',
            lambda text: history_final_title in text and task_id(40) in text,
            count=20,
        )
        final_col, final_row = pane_cell(session, history_final_title, 0, 60, 3, 29)
        session.mouse_click(
            'history-final-click', final_col, final_row,
            lambda text: history_final_id in text and history_final_title in text,
        )
        session.resize(70, 24)
        session.wait('history-final-narrow',
                     lambda text: 'History ·' in text and '[Details]' in text)
        session.mouse_text_click(
            'history-final-details', '[Details]',
            lambda text: '[Back]' in text and history_final_id in text,
        )
        session.mouse_text_click(
            'history-final-back', '[Back]',
            lambda text: 'History ·' in text and history_final_title in text,
        )
        unchanged(before, repo, linked)
        browse_report = session.finish()

    unchanged(before, repo, linked)

    with Session(linked / 'nested', base / 'graph-standard-pty', binary, audit,
                 cols=130, rows=30) as session:
        session.wait('graph-startup', contains('Current Graph'), session.started)
        graph_point = (75, 23)
        assert 1 <= graph_point[0] < 85 and 3 <= graph_point[1] < 28
        before_wheel = session.text()
        session.mouse_wheel(
            'graph-vertical-wheel', *graph_point, 'down',
            lambda text: text != before_wheel and detail_id in text,
        )
        before_horizontal = session.text()
        session.mouse_wheel(
            'graph-horizontal-wheel', *graph_point, 'right',
            lambda text: text != before_horizontal and detail_id in text,
        )
        before_shift = session.text()
        session.mouse_wheel(
            'graph-shift-wheel', *graph_point, 'down',
            lambda text: text != before_shift and detail_id in text,
            shift=True,
        )
        before_drag = session.text()
        session.mouse_drag(
            'graph-drag', graph_point, (70, 19),
            lambda text: text != before_drag and detail_id in text,
            release=(125, 29),
        )
        session.mouse_press('drag-cancel-press', *graph_point,
                            lambda text: True)
        session.mouse_motion('drag-cancel-motion', 70, 19,
                             lambda text: True)
        session.resize(100, 30)
        session.read(.2)
        resized = session.text()
        session.mouse_release('drag-cancel-release', 99, 29,
                              lambda text: True)
        session.read(.2)
        assert session.text() == resized
        assert session.proc.poll() is None
        unchanged(before, repo, linked)
        standard_report = session.finish()

    with Session(linked / 'nested', base / 'graph-compact-pty', binary, audit,
                 cols=130, rows=30) as session:
        session.wait('compact-startup', contains('Current Graph'), session.started)
        session.key('compact', b'-', contains('Compact'))
        graph_point = (80, 21)
        assert 1 <= graph_point[0] < 85 and 3 <= graph_point[1] < 28
        before_drag = session.text()
        session.mouse_drag(
            'compact-graph-drag', graph_point, (70, 18),
            lambda text: text != before_drag and detail_id in text,
            release=(125, 29),
        )
        unchanged(before, repo, linked)
        compact_report = session.finish()

    unchanged(before, repo, linked)
    assert browse_report['mouse_capture_verified'] is True
    assert standard_report['mouse_capture_verified'] is True
    assert compact_report['mouse_capture_verified'] is True
    return {
        'readonly': True,
        'recent_history_clicks': True,
        'wheel_surfaces': ['List', 'Detail', 'Help', 'History', 'Graph'],
        'history_final_visible': True,
        'graph_drag_densities': ['Standard', 'Compact'],
        'terminal_restored': True,
        'sessions': {
            'browsing': browse_report,
            'graph_standard': standard_report,
            'graph_compact': compact_report,
        },
    }


def replace_project(repo, base, version):
    parent = git(repo, 'rev-parse', STATE)
    project = json.loads(git(repo, 'show', parent + ':project.json'))
    project['format_version'] = version
    blob = git(repo, 'hash-object', '-w', '--stdin', input=json.dumps(project))
    env = {**os.environ, 'GIT_INDEX_FILE': str(base / 'canonical.index')}
    git(repo, 'read-tree', parent, env=env)
    git(repo, 'update-index', '--add', '--cacheinfo', f'100644,{blob},project.json', env=env)
    tree = git(repo, 'write-tree', env=env)
    bad = git(repo, 'commit-tree', tree, '-p', parent, input='Unsupported fixture\n')
    git(repo, 'update-ref', STATE, bad, parent)
    return parent, bad


def live_case(base, binary, audit, remote):
    repo, linked = fixture(base, 'live', remote)
    with Session(linked, base / 'live-pty', binary, audit) as session:
        session.wait('empty', contains('No current tasks'), session.started)

        def observe_writer(name, marker):
            before = snapshot(repo, linked)
            session.wait(name, contains(marker))
            unchanged(before, repo, linked)

        task = aye(repo, 'create', 'LiveCreated001')['task']['id']
        observe_writer('create', 'LiveCreated001')
        aye(repo, 'claim', task)
        observe_writer('claim', 'in_progress')
        aye(repo, 'update', task, '--title', 'ChangedTitle002')
        observe_writer('edit', 'ChangedTitle002')
        os.killpg(session.proc.pid, signal.SIGSTOP)
        aye(repo, 'update', task, '--title', 'ManualRecover003')
        pinned = git(repo, 'rev-parse', STATE)
        blob = git(repo, 'rev-parse', f'{pinned}:tasks/{task[2:4]}/{task}.json')
        original = repo / '.git' / 'objects' / blob[:2] / blob[2:]
        held = base / 'held-object'
        original.rename(held)
        os.killpg(session.proc.pid, signal.SIGCONT)
        observe_writer('missing-object', 'last good state')
        held.rename(original)
        before = snapshot(repo, linked)
        session.read(1.1)
        assert 'ManualRecover003' not in session.text()
        session.key('manual-refresh', b'r', contains('ManualRecover003'))
        unchanged(before, repo, linked)
        assert git(repo, 'rev-parse', STATE) == pinned
        good, bad = replace_project(repo, base, 999)
        observe_writer('unsupported-version', 'FORMAT_VERSION_UNSUPPORTED')
        git(repo, 'update-ref', STATE, good, bad)
        before = snapshot(repo, linked)
        session.wait('auto-recovery', lambda t: 'last good state' not in t and 'ManualRecover003' in t)
        unchanged(before, repo, linked)
        git(repo, 'pack-refs', '--all', '--prune')
        aye(repo, 'close', task)
        observe_writer('close-packed', 'No current tasks')
        report = session.finish(b'\x03')
    report.update(linked_worktree=True, readonly_between_writer_changes=True)
    return report


def error_cases(base, binary, audit, remote):
    repo, linked = fixture(base, 'errors', remote)
    cases = []
    # libgit2 discovery does not use Git CLI's GIT_CEILING_DIRECTORIES. Use an
    # existing directory outside the checkout; all evidence still stays in test/.
    outside = Path(tempfile.gettempdir()).resolve()
    discovery = subprocess.run([GIT, 'rev-parse', '--git-dir'], cwd=outside,
                               capture_output=True)
    assert discovery.returncode != 0, 'System temporary directory is inside Git'
    for name, cwd, marker in [('outside', outside, 'NOT_GIT_REPOSITORY'),
                              ('missing-ref', repo, 'NOT_INITIALIZED'),
                              ('unsupported', repo, 'FORMAT_VERSION_UNSUPPORTED')]:
        if name == 'missing-ref':
            saved = git(repo, 'rev-parse', STATE)
            git(repo, 'update-ref', '-d', STATE, saved)
        if name == 'unsupported':
            git(repo, 'update-ref', STATE, saved)
            replace_project(repo, base, 999)
        before = snapshot(repo, linked)
        with Session(cwd, base / ('error-' + name), binary, audit) as session:
            session.wait(name, contains(marker), session.started)
            session.finish(keys=None, expected=1)
        unchanged(before, repo, linked)
        cases.append(name)
    return cases


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', default=shutil.which('aye-view'))
    args = parser.parse_args()
    assert AYE and GIT and args.binary, 'Install aye and aye-view with Cargo defaults first'
    binary = str(Path(args.binary).resolve())
    (PACKAGE / 'test').mkdir(exist_ok=True)
    base = Path(tempfile.mkdtemp(prefix='integrated-', dir=PACKAGE / 'test'))
    print(f'Evidence: {base}', flush=True)
    audit = base / 'audit-bin'
    audit.mkdir()
    trap = audit / 'git'
    trap.write_text('#!/bin/sh\nprintf "%s\\n" "$*" >> "$VIEWER_GIT_AUDIT"\nexit 98\n')
    trap.chmod(0o755)
    report = {'machine': platform.platform(), 'python': platform.python_version(),
              'binary': binary, 'binary_sha256': digest(Path(binary)),
              'source_commit': git(PACKAGE, 'rev-parse', 'HEAD'),
              'aye_version': run(PACKAGE, AYE, '--version'),
              'viewer_version': run(PACKAGE, binary, '--version')}
    with NetworkTrap(('127.0.0.1', 0), TrapHandler) as server:
        server.attempts = []
        worker = threading.Thread(target=server.serve_forever, daemon=True)
        worker.start()
        remote = f'http://127.0.0.1:{server.server_address[1]}/unavailable.git'
        try:
            for name, case in [('scale', scale_case), ('mouse', mouse_case),
                               ('mouse-browsing', mouse_browsing_case),
                               ('live', live_case), ('errors', error_cases)]:
                print(f'Running {name}', flush=True)
                report[name] = case(base, binary, audit, remote)
                print(json.dumps({name: report[name]}, indent=2), flush=True)
            assert not server.attempts, server.attempts
            report['configured_remote_connections'] = 0
            report['result'] = 'PASS'
        except BaseException as error:
            report['result'] = 'FAIL'
            report['error'] = repr(error)
            raise
        finally:
            server.shutdown()
            (base / 'result.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'result': report['result'], 'evidence': str(base)}, indent=2))


if __name__ == '__main__':
    main()
