#!/usr/bin/env python3
"""Atomic batch contract tests against the installed aye binary."""
import concurrent.futures
import json
import os
import shutil
import subprocess
import sys
import threading
import time
import unittest
import uuid

from check import AYE, RepositoryCase, git, run


class Batches(RepositoryCase):
    def batch(self, operations, *, expected=None, actor='agent-a', code=None, cwd=None):
        request = {'version': 1, 'operations': operations}
        if expected is not None:
            request['expected_state_oid'] = expected
        path = self.home / (uuid.uuid4().hex + '.json')
        path.write_text(json.dumps(request))
        return self.aye('apply', '--file', str(path), actor=actor, code=code, cwd=cwd)

    def assert_rollback(self, operations, code='INVALID_ARGUMENT', **kwargs):
        before = self.state()
        tree = git(self.repo, 'rev-parse', before + '^{tree}')
        result = self.batch(operations, code=code, **kwargs)
        self.assertEqual(before, self.state())
        self.assertEqual(tree, git(self.repo, 'rev-parse', self.state() + '^{tree}'))
        return result['error']

    def test_graph_receipt_is_final_and_single_transition(self):
        before = self.state()
        result = self.batch([
            {'op': 'create', 'as': 'a', 'title': 'Contract'},
            {'op': 'create', 'as': 'b', 'title': 'Implementation',
             'depends_on': [{'local': 'a'}]},
            {'op': 'create', 'as': 'c', 'title': 'Verification'},
            {'op': 'block', 'id': {'local': 'c'}, 'by': {'local': 'b'}},
            {'op': 'update', 'id': {'local': 'a'}, 'acceptance': ['Review passed'],
             'description': 'Final metadata'},
        ])
        self.assertEqual(1, result['version'])
        self.assertEqual(self.state(), result['state_oid'])
        self.assertEqual(before, git(self.repo, 'rev-parse', self.state() + '^'))
        self.assertEqual(3, len(result['tasks']))
        tasks = {item['task']['id']: item for item in result['tasks']}
        for alias, effective in [('a', 'ready'), ('b', 'blocked'), ('c', 'blocked')]:
            item = tasks[result['aliases'][alias]]
            self.assertEqual(effective, item['computed']['effective_state'])
            self.assertNotIn('notes', item['task'])
        self.assertEqual('Final metadata', tasks[result['aliases']['a']]['changes']['description'])
        self.assertEqual([result['aliases']['b']], tasks[result['aliases']['c']]['task']['depends_on'])

    def test_late_failure_owner_cycle_and_missing_target_rollback(self):
        task = self.create()
        error = self.assert_rollback([
            {'op': 'note', 'id': task, 'body': 'Must roll back'},
            {'op': 'release', 'id': task},
        ], 'INVALID_STATE_TRANSITION')
        self.assertEqual(1, error['details']['op_index'])
        self.assertEqual('INVALID_STATE_TRANSITION', error['details']['underlying_code'])
        self.aye('claim', task)
        self.assert_rollback([
            {'op': 'create', 'title': 'Must roll back'},
            {'op': 'note', 'id': task, 'body': 'Wrong owner'},
        ], 'NOT_CLAIM_OWNER', actor='agent-b')
        self.assert_rollback([
            {'op': 'create', 'as': 'a', 'title': 'A'},
            {'op': 'create', 'as': 'b', 'title': 'B', 'depends_on': [{'local': 'a'}]},
            {'op': 'block', 'id': {'local': 'a'}, 'by': {'local': 'b'}},
        ], 'DEPENDENCY_CYCLE')
        self.assert_rollback([
            {'op': 'create', 'title': 'Must roll back'},
            {'op': 'note', 'id': 't-00000000000000000000', 'body': 'Missing'},
        ], 'TASK_NOT_FOUND')
        self.assertEqual([], self.show(task)['task']['notes'])

    def test_alias_and_schema_errors(self):
        cases = [
            [{'op': 'create', 'as': 'x', 'title': 'X'}, {'op': 'create', 'as': 'x', 'title': 'Y'}],
            [{'op': 'note', 'id': {'local': 'missing'}, 'body': 'Missing'}],
            [{'op': 'create', 'title': 'X', 'depends_on': [{'local': 'later'}]},
             {'op': 'create', 'as': 'later', 'title': 'Y'}],
            [{'op': 'create', 'title': 'X', 'status': 'closed'}],
            [{'op': 'sync'}],
            [{'op': 'release', 'id': 't-00000000000000000000', 'force': True}],
            [{'op': 'note', 'id': '00000000', 'body': 'Prefix'}],
            [],
            [{'op': 'create', 'title': 'X'}] * 101,
        ]
        for operations in cases:
            with self.subTest(operations=operations[:2]):
                self.assert_rollback(operations)
        before = self.state()
        for raw in ['{', '{"version":2,"operations":[]}',
                    '{"version":1,"operations":[],"extra":true}', ' ' * (1024 * 1024 + 1)]:
            path = self.home / 'invalid.json'
            path.write_text(raw)
            self.aye('apply', '--file', str(path), code='INVALID_ARGUMENT')
            self.assertEqual(before, self.state())

    def test_pause_defer_complete_and_metadata_clear(self):
        task = self.create()
        self.aye('note', task, 'Historical evidence')
        self.aye('claim', task)
        result = self.batch([
            {'op': 'note', 'id': task, 'body': 'Handoff'},
            {'op': 'release', 'id': task},
        ])
        item = result['tasks'][0]
        self.assertEqual('ready', item['computed']['effective_state'])
        self.assertEqual(['Handoff'], [n['body'] for n in item['added_notes']])
        self.assertNotIn('notes', item['changes'])
        result = self.batch([{'op': 'note', 'id': task, 'body': 'Shelved'}, {'op': 'defer', 'id': task}])
        self.assertEqual('deferred', result['tasks'][0]['task']['status'])
        self.assertEqual([], self.aye('ready'))
        result = self.batch([
            {'op': 'resume', 'id': task}, {'op': 'claim', 'id': task},
            {'op': 'update', 'id': task, 'acceptance': [], 'labels': [], 'parent': None},
            {'op': 'close', 'id': task, 'note': 'Accepted, reviewed and integrated in fixture'},
        ])
        item = result['tasks'][0]
        self.assertEqual('done', item['task']['resolution'])
        self.assertIsNone(item['task']['claim'])
        self.assertEqual(1, len(item['added_notes']))
        self.assertEqual(4, len(self.show(task)['task']['notes']))
        result = self.batch([{'op': 'reopen', 'id': task}, {'op': 'cancel', 'id': task}])
        self.assertEqual('cancelled', result['tasks'][0]['task']['resolution'])

    def test_manual_and_dependency_unblock_and_actor_requirement(self):
        task = self.create()
        self.assert_rollback([{'op': 'note', 'id': task, 'body': 'No actor'}],
                             'ACTOR_REQUIRED', actor=None)
        blocker = self.create('Prerequisite')
        result = self.batch([
            {'op': 'claim', 'id': task},
            {'op': 'block', 'id': task, 'reason': 'External wait'},
            {'op': 'block', 'id': task, 'by': blocker},
            {'op': 'unblock', 'id': task},
        ])
        self.assertEqual('agent-a', result['operations'][1]['released_claim'])
        self.assertEqual('blocked', result['tasks'][0]['computed']['effective_state'])
        self.assertIsNone(result['tasks'][0]['task']['manual_block'])
        result = self.batch([{'op': 'unblock', 'id': task, 'by': blocker},
                             {'op': 'claim', 'id': task}])
        self.assertEqual('in_progress', result['tasks'][0]['task']['status'])

    def test_stale_guard_and_stdin(self):
        stale = self.state()
        self.create()
        error = self.assert_rollback([{'op': 'create', 'title': 'Stale'}], 'STALE_STATE', expected=stale)
        self.assertEqual(self.state(), error['details']['observed_state_oid'])
        proc = run(self.repo, [AYE, '--json', '--actor', 'agent-a', 'apply', '--file', '-'],
                   input=json.dumps({'version': 1, 'operations': [{'op': 'create', 'title': 'stdin'}]}))
        self.assertTrue(json.loads(proc.stdout)['ok'])

    def force_publication_race(self, operations, template, *, expected=None):
        """Pause only this child's first CAS; a direct Git writer bypasses its lock."""
        control = self.home / ('cas-' + uuid.uuid4().hex)
        control.mkdir()
        wrapper = control / 'git'
        wrapper.write_text('#!' + sys.executable + '\n' + """
import os
from pathlib import Path
import sys
import time

control = Path(os.environ['AYE_TEST_CAS_CONTROL'])
args = sys.argv[1:]
ref = 'refs/agent-tasks/state'
if args and args[0] == 'update-ref' and ref in args:
    candidate = args[args.index(ref) + 1]
    with (control / 'attempts').open('a') as log:
        log.write(candidate + '\\n')
    paused = control / 'paused'
    if not paused.exists():
        paused.write_text(candidate)
        deadline = time.monotonic() + 20
        while not (control / 'release').exists():
            if time.monotonic() >= deadline:
                sys.exit('Timed out waiting for test CAS release')
            time.sleep(0.01)
os.execv(os.environ['AYE_TEST_REAL_GIT'], ['git', *args])
""")
        wrapper.chmod(0o755)
        request = {'version': 1, 'operations': operations}
        if expected is not None:
            request['expected_state_oid'] = expected
        request_path = control / 'request.json'
        request_path.write_text(json.dumps(request))
        env = {**os.environ, 'AYE_ACTOR': 'agent-a',
               'PATH': str(control) + os.pathsep + os.environ.get('PATH', ''),
               'AYE_TEST_REAL_GIT': shutil.which('git'),
               'AYE_TEST_CAS_CONTROL': str(control)}
        process = subprocess.Popen([AYE, '--json', 'apply', '--file', str(request_path)],
                                   cwd=self.repo, env=env, text=True,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        try:
            deadline = time.monotonic() + 15
            while not (control / 'paused').exists():
                self.assertIsNone(process.poll(), 'Batch exited before reaching CAS')
                self.assertLess(time.monotonic(), deadline, 'Batch never reached CAS')
                time.sleep(0.01)
            competitor = json.loads(json.dumps(template))
            competitor['id'] = 't-' + uuid.uuid4().hex[:20]
            competitor['title'] = 'Unrelated external writer'
            path = f"tasks/{competitor['id'][2:4]}/{competitor['id']}.json"
            competing_oid = self.edit_state({path: competitor})
        finally:
            (control / 'release').touch()
            try:
                stdout, stderr = process.communicate(timeout=30)
            except subprocess.TimeoutExpired:
                process.kill()
                process.communicate()
                raise
        attempts = (control / 'attempts').read_text().splitlines()
        self.assertTrue(stdout, stderr)
        return process.returncode, json.loads(stdout), attempts, competitor, competing_oid

    def test_forced_cas_replays_complete_batch_with_stable_ids(self):
        existing = self.create('Existing task')
        template = self.show(existing)['task']
        before = self.state()
        code, reply, attempts, competitor, competing_oid = self.force_publication_race([
            {'op': 'create', 'as': 'created', 'title': 'Stable created task'},
            {'op': 'note', 'id': {'local': 'created'}, 'body': 'New task evidence'},
            {'op': 'note', 'id': existing, 'body': 'Existing task evidence'},
        ], template)
        self.assertEqual(0, code, reply)
        self.assertTrue(reply['ok'], reply)
        result = reply['data']
        self.assertEqual(2, len(attempts))
        self.assertNotEqual(attempts[0], attempts[1])
        self.assertEqual(attempts[-1], result['state_oid'])
        self.assertEqual(self.state(), result['state_oid'])
        self.assertEqual(competing_oid, git(self.repo, 'rev-parse', self.state() + '^'))
        created = result['aliases']['created']
        created_path = f'tasks/{created[2:4]}/{created}.json'
        losing = json.loads(git(self.repo, 'show', attempts[0] + ':' + created_path))
        winning = json.loads(git(self.repo, 'show', result['state_oid'] + ':' + created_path))
        self.assertEqual(losing, winning, 'CAS replay changed preallocated ID, time, or notes')
        self.assertEqual(['New task evidence'], [note['body'] for note in winning['notes']])
        self.assertEqual(['Existing task evidence'],
                         [note['body'] for note in self.show(existing)['task']['notes']])
        self.assertEqual(competitor, self.show(competitor['id'])['task'])
        self.assertEqual(2, len(git(self.repo, 'rev-list', before + '..' + self.state()).splitlines()))
        self.assertEqual(3, len(self.aye('list', '--all')))

    def test_forced_cas_rechecks_expected_oid_and_rejects_whole_batch(self):
        existing = self.create('Existing task')
        template = self.show(existing)['task']
        before = self.state()
        code, reply, attempts, competitor, competing_oid = self.force_publication_race([
            {'op': 'create', 'as': 'discarded', 'title': 'Must never publish'},
            {'op': 'note', 'id': existing, 'body': 'Must never append'},
        ], template, expected=before)
        self.assertEqual(3, code, reply)
        self.assertFalse(reply['ok'], reply)
        self.assertEqual('STALE_STATE', reply['error']['code'])
        self.assertEqual(competing_oid, reply['error']['details']['observed_state_oid'])
        self.assertEqual(before, reply['error']['details']['expected_state_oid'])
        self.assertEqual(1, len(attempts), 'Guarded batch must not attempt a second publication')
        self.assertEqual(competing_oid, self.state())
        self.assertEqual(template, self.show(existing)['task'])
        self.assertEqual(competitor, self.show(competitor['id'])['task'])
        self.assertEqual(2, len(self.aye('list', '--all')))
        self.assertEqual([competing_oid], git(self.repo, 'rev-list', before + '..' + self.state()).splitlines())

    def test_concurrent_batches_preserve_whole_snapshots_and_receipts(self):
        before = self.state()
        other = self.worktree()
        barrier = threading.Barrier(2)
        def apply(cwd, label):
            barrier.wait()
            return self.batch([
                {'op': 'create', 'as': 'a', 'title': label},
                {'op': 'note', 'id': {'local': 'a'}, 'body': label},
                {'op': 'create', 'as': 'b', 'title': label + ' child', 'depends_on': [{'local': 'a'}]},
            ], cwd=cwd)
        with concurrent.futures.ThreadPoolExecutor(2) as pool:
            futures = [pool.submit(apply, cwd, label) for cwd, label in [(self.repo, 'A'), (other, 'B')]]
            results = [future.result() for future in futures]
        commits = git(self.repo, 'rev-list', before + '..' + self.state()).splitlines()
        self.assertEqual(2, len(commits))
        self.assertEqual(set(commits), {result['state_oid'] for result in results})
        ids = [task for result in results for task in result['aliases'].values()]
        self.assertEqual(4, len(set(ids)))
        for result in results:
            for item in result['tasks']:
                task = item['task']
                path = f"tasks/{task['id'][2:4]}/{task['id']}.json"
                persisted = json.loads(git(self.repo, 'show', result['state_oid'] + ':' + path))
                self.assertEqual(task, {k: v for k, v in persisted.items() if k != 'notes'})
            self.assertEqual(1, len(self.show(result['aliases']['a'])['task']['notes']))


if __name__ == '__main__':
    unittest.main()
