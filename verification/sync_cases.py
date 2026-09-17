#!/usr/bin/env python3
"""Git synchronization contracts: local bare remotes, installed aye, real histories."""
import json
import os
from pathlib import Path
import unittest
from check import RepositoryCase, git, run, AYE

REF = 'refs/agent-tasks/state'
REMOTE = REF

def path(task):
    return f'tasks/{task[2:4]}/{task}.json'

class SyncCases(RepositoryCase):
    def source_snapshot(self, cwd):
        # Metadata must stay in the shared Git ref, even in single-branch clones.
        for name in ('tasks', 'views', 'project.json', 'manifest.json',
                     'REPORT.md', 'ready.jsonl', 'active.jsonl'):
            self.assertFalse((cwd / name).exists(), f'Metadata leaked into source: {cwd / name}')
        index = Path(git(cwd, 'rev-parse', '--path-format=absolute', '--git-path', 'index'))
        # Read-only status must not refresh the source index during this check.
        status = git(cwd, 'status', '--porcelain=v1', '--untracked-files=all',
                     env={**os.environ, 'GIT_OPTIONAL_LOCKS': '0'})
        return (git(cwd, 'rev-parse', 'HEAD'),
                git(cwd, 'symbolic-ref', 'HEAD'),
                index.read_bytes(), status, (cwd / 'source').read_bytes())

    def aye(self, *args, **kwargs):
        cwd = kwargs.get('cwd') or self.repo
        before = self.source_snapshot(cwd)
        value = super().aye(*args, **kwargs)
        self.assertEqual(before, self.source_snapshot(cwd),
                         f'aye {args} changed source HEAD, branch, index, status or content')
        return value

    def remote_fixture(self):
        self.bare = self.home / 'remote.git'
        git(self.repo, 'init', '--bare', '-q', str(self.bare))
        git(self.repo, 'remote', 'add', 'origin', str(self.bare))
        git(self.repo, 'push', 'origin', 'HEAD:refs/heads/source')
        self.aye('sync')
        self.other = self.home / 'clone'
        git(self.repo, 'clone', '-q', '--single-branch', '--branch', 'source', str(self.bare), str(self.other))
        self.assertEqual('+refs/heads/source:refs/remotes/origin/source',
                         git(self.other, 'config', '--get-all', 'remote.origin.fetch'))
        self.assertEqual('', git(self.other, 'for-each-ref', '--format=%(refname)',
                                 'refs/agent-tasks/remotes/origin/state'))
        git(self.other, 'config', 'user.name', 'Test User')
        git(self.other, 'config', 'user.email', 'test@example.invalid')
        self.aye('init', cwd=self.other)

    def remote_tip(self):
        return git(self.bare, 'rev-parse', REMOTE)

    def change(self, task, cwd=None, **fields):
        cwd = cwd or self.repo
        value = self.show(task, cwd=cwd)['task']
        value.update(fields)
        return self.edit_state({path(task): value}, cwd=cwd)

    def test_publish_adopt_equal_and_both_fast_forwards(self):
        task = self.create('initial')
        self.remote_fixture()
        self.assertEqual(self.state(), self.remote_tip())
        self.assertEqual('initial', self.show(task, cwd=self.other)['task']['title'])
        for cwd in (self.repo, self.other):
            (cwd / 'source').write_text('staged source change\n')
            git(cwd, 'add', 'source')
            (cwd / 'source').write_text('unstaged source change\n')
            (cwd / 'untracked').write_text('preserve this source file\n')
        before = self.state()
        self.aye('sync')
        self.assertEqual(before, self.state())
        local = self.create('local ahead')
        self.aye('sync')
        self.assertEqual(self.state(), self.remote_tip())
        self.aye('sync', cwd=self.other)
        self.assertEqual(local, self.show(local, cwd=self.other)['task']['id'])
        remote = self.create('remote ahead', cwd=self.other)
        self.aye('sync', cwd=self.other)
        self.aye('sync')
        self.assertEqual(remote, self.show(remote)['task']['id'])
        self.assertEqual(self.state(), self.remote_tip())

    def test_disjoint_merge_preserves_canonical_bytes_and_two_parents(self):
        a, b = self.create('A'), self.create('B')
        self.remote_fixture()
        self.change(a, title='local raw formatting')
        raw = git(self.repo, 'show', f'{REF}:{path(a)}')
        self.change(b, cwd=self.other, title='remote')
        self.aye('sync', cwd=self.other)
        self.aye('sync')
        self.assertEqual(raw, git(self.repo, 'show', f'{REF}:{path(a)}'))
        self.assertEqual('remote', self.show(b)['task']['title'])
        self.assertEqual(3, len(git(self.repo, 'rev-list', '--parents', '-n', '1', REF).split()))

    def conflict(self):
        task = self.create()
        self.remote_fixture()
        self.change(task, title='local')
        self.change(task, cwd=self.other, title='remote')
        self.aye('sync', cwd=self.other)
        local, remote = self.state(), self.remote_tip()
        self.aye('sync', code='SYNC_CONFLICT')
        self.assertEqual((local, remote), (self.state(), self.remote_tip()))
        return task

    def test_same_task_conflict_choices_block_shared_writes_and_abort(self):
        task = self.conflict()
        other = self.worktree()
        self.aye('create', 'blocked', cwd=other, code='SYNC_CONFLICT')
        self.assertEqual('local', self.show(task, cwd=other)['task']['title'])
        self.aye('resolve', '--continue', code='SYNC_CONFLICT')
        self.aye('resolve', task, '--take', 'remote', cwd=other)
        self.aye('resolve', '--abort')
        self.assertEqual('local', self.show(task)['task']['title'])
        self.aye('sync', code='SYNC_CONFLICT')
        self.aye('resolve', task, '--take', 'local')
        self.aye('resolve', '--continue')
        self.assertEqual('local', self.show(task)['task']['title'])
        self.assertEqual(self.state(), self.remote_tip())

    def test_identical_task_bytes_merge_and_remote_choice_can_publish(self):
        task = self.create('shared')
        self.remote_fixture()
        self.change(task, title='same bytes')
        self.change(task, cwd=self.other, title='same bytes')
        self.aye('sync', cwd=self.other)
        self.aye('sync')
        self.assertEqual('same bytes', self.show(task)['task']['title'])
        self.aye('sync', cwd=self.other)
        self.change(task, title='local second')
        self.change(task, cwd=self.other, title='remote second')
        self.aye('sync', cwd=self.other)
        self.aye('sync', code='SYNC_CONFLICT')
        self.aye('resolve', task, '--take', 'remote')
        self.aye('resolve', '--continue')
        self.assertEqual('remote second', self.show(task)['task']['title'])

    def test_pending_conflict_survives_gc_without_tracking_ref(self):
        task = self.conflict()
        # Ordinary fetch/prune can replace tracking refs while resolution waits.
        git(self.repo, 'update-ref', '-d', 'refs/agent-tasks/remotes/origin/state')
        git(self.repo, 'reflog', 'expire', '--expire=now', '--all')
        git(self.repo, 'gc', '--prune=now')
        detail = self.aye('resolve', task)
        self.assertEqual('remote', detail['remote']['title'])
        self.aye('resolve', task, '--take', 'remote')
        self.aye('resolve', '--continue')
        self.assertEqual('remote', self.show(task)['task']['title'])
        self.assertEqual('', git(self.repo, 'for-each-ref', '--format=%(refname)',
                                 'refs/agent-tasks/conflicts/'))

    def test_missing_common_history_refuses_without_pending_conflict(self):
        task = self.create('original')
        self.remote_fixture()
        tree = git(self.other, 'rev-parse', f'{REF}^{{tree}}')
        unrelated = git(self.other, 'commit-tree', tree, input='History truncated externally\n')
        git(self.other, 'update-ref', REF, unrelated)
        before = git(self.other, 'rev-parse', REF)
        self.aye('sync', cwd=self.other, code='MISSING_HISTORY')
        self.assertEqual(before, git(self.other, 'rev-parse', REF))
        self.assertEqual([], self.aye('resolve', cwd=self.other)['conflicts'])
        self.create('local work remains possible', cwd=self.other)

    def test_resolution_file_validates_identity_and_entire_graph(self):
        task = self.conflict()
        supplied = self.home / 'resolution.json'
        value = self.show(task)['task']
        value['depends_on'] = [task]
        supplied.write_text(json.dumps(value))
        # Selection may stage invalid graph, but continue must refuse publication.
        before_source = self.source_snapshot(self.repo)
        proc = run(self.repo, [AYE, '--json', 'resolve', task, '--file', str(supplied)], check=False)
        self.assertEqual(before_source, self.source_snapshot(self.repo))
        if proc.returncode == 0:
            self.aye('resolve', '--continue', code='STATE_CORRUPT')
        else:
            self.assertEqual('STATE_CORRUPT', json.loads(proc.stdout)['error']['code'])
        value['depends_on'] = []
        value['title'] = 'combined'
        supplied.write_text(json.dumps(value, indent=3) + '\n')
        self.aye('resolve', task, '--file', str(supplied))
        self.aye('resolve', '--continue')
        self.assertEqual(supplied.read_text(), git(self.repo, 'show', f'{REF}:{path(task)}') + '\n')

    def test_graph_only_conflict_exposes_participants_and_can_resolve(self):
        a, b = self.create('A'), self.create('B')
        self.remote_fixture()
        self.change(a, depends_on=[b])
        self.change(b, cwd=self.other, depends_on=[a])
        self.aye('sync', cwd=self.other)
        before = self.state()
        self.aye('sync', code='SYNC_CONFLICT')
        pending = json.dumps(self.aye('resolve'))
        self.assertIn(a, pending)
        self.assertIn(b, pending)
        self.assertEqual(before, self.state())
        self.aye('resolve', a, '--take', 'local')
        self.aye('resolve', b, '--take', 'local')
        self.aye('resolve', '--continue')
        self.assertEqual([], self.show(b)['task']['depends_on'])

    def test_project_mismatch_and_corrupt_remote_are_atomic(self):
        self.remote_fixture()
        git(self.other, 'update-ref', '-d', REF)
        self.aye('init', '--offline', cwd=self.other)
        self.aye('sync', cwd=self.other, code='PROJECT_MISMATCH')
        # Adopt original again, then publish an invalid external canonical edit.
        git(self.other, 'update-ref', REF, self.remote_tip())
        task = self.create('invalid external', cwd=self.other)
        self.change(task, cwd=self.other, status='in_progress')
        git(self.other, 'push', 'origin', f'{REF}:{REMOTE}')
        before = self.state()
        self.aye('sync', code='STATE_CORRUPT')
        self.assertEqual(before, self.state())

    def test_stale_remote_views_repaired_and_deletion_not_republished(self):
        task = self.create()
        self.remote_fixture()
        self.change(task, cwd=self.other, title='external valid')
        git(self.other, 'push', 'origin', f'{REF}:{REMOTE}')
        self.aye('sync')
        self.assertIn('external valid', git(self.repo, 'show', f'{REF}:REPORT.md'))
        self.assertEqual(self.state(), self.remote_tip())
        git(self.bare, 'update-ref', '-d', REMOTE)
        before = self.state()
        self.aye('sync', code='REMOTE_STATE_DELETED')
        self.assertEqual(before, self.state())
        self.assertEqual('', git(self.repo, 'ls-remote', 'origin', REMOTE))

    def test_remote_advance_before_push_is_reconciled(self):
        self.remote_fixture()
        a = self.create('local racing task')
        b = self.create('remote racing task', cwd=self.other)
        advanced = git(self.other, 'rev-parse', REF)
        # Transfer objects without publishing state. A fixture pre-receive hook
        # advances the remote once and rejects this push, exactly at the race.
        git(self.other, 'push', 'origin', f'{advanced}:refs/fixture/racing')
        hook = self.bare / 'hooks/pre-receive'
        hook.write_text('#!/bin/sh\n' +
            'if test ! -f "$GIT_DIR/raced"; then\n' +
            '  touch "$GIT_DIR/raced"\n' +
            f'  env -u GIT_QUARANTINE_PATH git update-ref {REMOTE} {advanced} || exit 1\n' +
            '  exit 1\nfi\nexit 0\n')
        hook.chmod(0o755)
        self.aye('sync')
        self.assertEqual(a, self.show(a)['task']['id'])
        self.assertEqual(b, self.show(b)['task']['id'])
        self.assertEqual(self.state(), self.remote_tip())

    def test_remote_selection_shared_and_unavailable_explicit(self):
        self.remote_fixture()
        git(self.repo, 'remote', 'rename', 'origin', 'only')
        self.aye('sync')
        git(self.repo, 'remote', 'add', 'second', str(self.bare))
        self.aye('config', 'remote', 'only')
        other = self.worktree()
        self.assertIn('only', json.dumps(self.aye('config', 'remote', cwd=other)))
        git(self.repo, 'remote', 'set-url', 'only', str(self.home / 'missing.git'))
        self.aye('sync', code='REMOTE_UNAVAILABLE')

if __name__ == '__main__':
    unittest.main()
