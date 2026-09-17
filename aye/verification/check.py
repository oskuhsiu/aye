#!/usr/bin/env python3
"""Black-box contract tests against the Cargo-installed binary; no /tmp fixtures."""
import concurrent.futures
import json
import os
from pathlib import Path
import shutil
import subprocess
import threading
import unittest
import uuid

ROOT = Path(__file__).resolve().parents[1]
AYE = shutil.which('aye')
BASE = ROOT / 'test'


def run(cwd, args, *, env=None, check=True, input=None):
    proc = subprocess.run(args, cwd=cwd, env=env, input=input,
                          text=True, capture_output=True)
    if check and proc.returncode:
        raise AssertionError(f'{args}: {proc.returncode}\n{proc.stdout}\n{proc.stderr}')
    return proc


def git(cwd, *args, **kwargs):
    return run(cwd, ['git', *args], **kwargs).stdout.strip()


class RepositoryCase(unittest.TestCase):
    def setUp(self):
        if not AYE:
            self.fail('Installed aye missing: run cargo install --path . --force first')
        self.home = BASE / (self.id().split('.')[-1] + '-' + uuid.uuid4().hex[:8])
        self.repo = self.home / 'a'
        self.repo.mkdir(parents=True)
        git(self.repo, 'init', '-q')
        git(self.repo, 'config', 'user.name', 'Test User')
        git(self.repo, 'config', 'user.email', 'test@example.invalid')
        (self.repo / 'source').write_text('base\n')
        git(self.repo, 'add', 'source')
        git(self.repo, 'commit', '-qm', 'source fixture')
        self.aye('init', '--offline')

    def aye(self, *args, cwd=None, actor='agent-a', code=None):
        env = dict(os.environ)
        env.pop('AYE_ACTOR', None)
        if actor is not None:
            env['AYE_ACTOR'] = actor
        proc = run(cwd or self.repo, [AYE, '--json', *args], env=env, check=False)
        try:
            value = json.loads(proc.stdout)
        except Exception:
            self.fail(f'Non-JSON result {args}: {proc.returncode} {proc.stdout} {proc.stderr}')
        if code:
            self.assertFalse(value['ok'], value)
            self.assertEqual(code, value['error']['code'], value)
            self.assertIn(proc.returncode, (2, 3))
            return value
        self.assertEqual(0, proc.returncode, value)
        self.assertTrue(value['ok'], value)
        self.assertIn('warnings', value)
        return value['data']

    def create(self, title='Task', **kwargs):
        return self.aye('create', title, **kwargs)['task']['id']

    def show(self, task, **kwargs):
        return self.aye('show', task, **kwargs)

    def state(self):
        return git(self.repo, 'rev-parse', 'refs/agent-tasks/state')

    def worktree(self):
        path = self.home / 'b'
        git(self.repo, 'worktree', 'add', '-qb', 'agent-b', str(path))
        return path

    def edit_state(self, updates, cwd=None, reference='refs/agent-tasks/state'):
        cwd = cwd or self.repo
        env = {**os.environ, 'GIT_INDEX_FILE': str(self.home / ('index-' + uuid.uuid4().hex))}
        parent = git(cwd, 'rev-parse', reference)
        git(cwd, 'read-tree', parent, env=env)
        for path, value in updates.items():
            if value is None:
                git(cwd, 'update-index', '--force-remove', path, env=env)
            else:
                content = value if isinstance(value, str) else json.dumps(value, indent=4) + '\n'
                oid = git(cwd, 'hash-object', '-w', '--stdin', input=content)
                git(cwd, 'update-index', '--add', '--cacheinfo', f'100644,{oid},{path}', env=env)
        tree = git(cwd, 'write-tree', env=env)
        oid = git(cwd, 'commit-tree', tree, '-p', parent, input='External edit\n')
        git(cwd, 'update-ref', reference, oid, parent)
        return oid


class Bootstrap(RepositoryCase):
    def test_workflow_shared_and_ownership(self):
        other = self.worktree()
        task = self.aye('create', 'Fix crash', '--type', 'bug', '--acceptance', 'No crash')['task']['id']
        self.assertRegex(task, r'^t-[0-9a-f]{20}$')
        shown = self.show(task, cwd=other)
        self.assertEqual(['No crash'], shown['task']['acceptance'])
        self.assertEqual('ready', shown['computed']['effective_state'])
        before = self.state()
        self.aye('claim', task, actor=None, code='ACTOR_REQUIRED')
        self.assertEqual(before, self.state())
        self.aye('claim', task)
        self.aye('note', task, 'wrong owner', actor='agent-b', code='NOT_CLAIM_OWNER')
        self.aye('close', task, actor='agent-b', code='NOT_CLAIM_OWNER')
        self.aye('note', task, 'Verified')
        self.aye('close', task)
        shown = self.show(task, cwd=other)
        self.assertEqual('done', shown['task']['resolution'])
        self.assertEqual('Verified', shown['task']['notes'][0]['body'])
        self.assertIsNone(shown['task']['claim'])
        self.assertEqual([], self.aye('ready'))
        self.assertEqual([], self.aye('list'))
        self.aye('claim', task, code='INVALID_STATE_TRANSITION')

    def test_claim_race_and_disjoint_writes(self):
        other = self.worktree()
        task = self.create()
        barrier = threading.Barrier(2)
        def claim(cwd, actor):
            barrier.wait()
            env = {**os.environ, 'AYE_ACTOR': actor}
            return run(cwd, [AYE, '--json', 'claim', task], env=env, check=False)
        with concurrent.futures.ThreadPoolExecutor(2) as pool:
            futures = [pool.submit(claim, self.repo, 'a'), pool.submit(claim, other, 'b')]
            results = [f.result() for f in futures]
        self.assertEqual([0, 3], sorted(p.returncode for p in results))
        loser = next(p for p in results if p.returncode)
        self.assertEqual('TASK_ALREADY_CLAIMED', json.loads(loser.stdout)['error']['code'])
        with concurrent.futures.ThreadPoolExecutor(2) as pool:
            futures = [pool.submit(self.create, f'Parallel {i}', cwd=cwd)
                       for i, cwd in enumerate([self.repo, other])]
            ids = [f.result() for f in futures]
        self.assertNotEqual(*ids)
        for task in ids:
            self.assertEqual(task, self.show(task)['task']['id'])

    def test_source_isolation_with_dirty_index(self):
        (self.repo / 'source').write_text('staged\n')
        git(self.repo, 'add', 'source')
        (self.repo / 'source').write_text('unstaged\n')
        (self.repo / 'untracked').write_text('preserve me\n')
        def snapshot():
            return (git(self.repo, 'rev-parse', 'HEAD'),
                    git(self.repo, 'symbolic-ref', 'HEAD'),
                    git(self.repo, 'status', '--porcelain=v1'),
                    (self.repo / '.git/index').read_bytes(),
                    (self.repo / 'source').read_bytes(),
                    git(self.repo, 'config', '--local', '--list'))
        before = snapshot()
        task = self.create()
        self.aye('claim', task)
        self.aye('close', task)
        self.assertEqual(before, snapshot())
        self.assertNotIn('tasks', git(self.repo, 'ls-tree', '--name-only', 'HEAD').splitlines())
        state_paths = git(self.repo, 'ls-tree', '-r', '--name-only', 'refs/agent-tasks/state').splitlines()
        self.assertIn(f'tasks/{task[2:4]}/{task}.json', state_paths)
        self.assertNotIn('source', state_paths)
        self.assertEqual(1, len(git(self.repo, 'rev-list', '--max-parents=0', 'refs/agent-tasks/state').splitlines()))

    def test_invalid_requests_are_atomic(self):
        before = self.state()
        self.aye('show', 't-00000000000000000000', code='TASK_NOT_FOUND')
        self.aye('create', '', code='INVALID_ARGUMENT')
        self.assertEqual(before, self.state())
        task = self.create()
        self.aye('close', task)
        before = self.state()
        self.aye('close', task, code='INVALID_STATE_TRANSITION')
        self.assertEqual(before, self.state())

    def test_corrupt_and_newer_state_refuse_writes(self):
        task = self.create()
        canonical = self.show(task)['task']
        canonical['status'] = 'in_progress'
        self.edit_state({f'tasks/{task[2:4]}/{task}.json': canonical})
        before = self.state()
        self.aye('create', 'Should fail', code='STATE_CORRUPT')
        self.assertEqual(before, self.state())
        canonical['status'] = 'open'
        project = json.loads(git(self.repo, 'show', 'refs/agent-tasks/state:project.json'))
        project['format_version'] = 2
        self.edit_state({f'tasks/{task[2:4]}/{task}.json': canonical, 'project.json': project})
        before = self.state()
        self.aye('create', 'Should fail', code='FORMAT_VERSION_UNSUPPORTED')
        self.assertEqual(before, self.state())


if __name__ == '__main__':
    unittest.main()
