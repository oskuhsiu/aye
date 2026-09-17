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
        assert not self.audit.exists(), self.audit.read_text() if self.audit.exists() else ''
        assert not self.trace.exists() or not self.trace.read_text().strip()
        return {'timings_seconds': self.timings, 'frames': self.frames,
                'terminal_restored': True, 'git_subprocess_calls': 0}

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


def scale_case(base, binary, audit, remote):
    repo, linked = fixture(base, 'scale', remote)
    seed_scale(repo)
    before = snapshot(repo, linked)
    with Session(linked / 'nested', base / 'scale-pty', binary, audit) as session:
        session.wait('startup', contains('1010 visible tasks'), session.started)
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
            for name, case in [('scale', scale_case), ('live', live_case), ('errors', error_cases)]:
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
