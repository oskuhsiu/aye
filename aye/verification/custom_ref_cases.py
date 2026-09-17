"""Remote protocol v0.2 behavior contracts, written before implementation."""
import json
from check import RepositoryCase, git, run

REF = 'refs/agent-tasks/state'

class CustomRefCases(RepositoryCase):
    def remote_fixture(self):
        self.bare = self.home / 'remote.git'
        git(self.repo, 'init', '--bare', '-q', str(self.bare))
        git(self.repo, 'remote', 'add', 'origin', str(self.bare))
        git(self.repo, 'push', 'origin', 'HEAD:refs/heads/source')

    def test_publish_custom_ref_and_explicit_adoption_without_task_branch(self):
        task = self.create('Hidden-ref discovery')
        self.remote_fixture()
        self.aye('sync')
        self.assertEqual(self.state(), git(self.bare, 'rev-parse', '--verify', REF))
        branches = git(self.repo, 'ls-remote', '--heads', 'origin')
        self.assertNotIn('agent-tasks', branches)
        self.assertNotIn('agent-tasks', git(self.repo, 'branch', '-a'))
        other = self.home / 'clone'
        git(self.repo, 'clone', '-q', '--single-branch', '--branch', 'source', str(self.bare), str(other))
        self.assertEqual('', git(other, 'for-each-ref', '--format=%(refname)', 'refs/agent-tasks/'))
        self.aye('init', cwd=other)
        self.assertEqual(task, self.show(task, cwd=other)['task']['id'])
        self.assertNotIn('agent-tasks', git(other, 'branch', '-a'))
        self.assertFalse((other / 'tasks').exists())
        self.assertFalse((other / 'views').exists())

    def test_legacy_observation_does_not_count_as_deleted_custom_ref(self):
        self.remote_fixture()
        legacy_oid = self.state()
        git(self.repo, 'push', 'origin', f'{legacy_oid}:refs/heads/agent-tasks')
        self.create('Change belongs only to custom ref')
        metadata = self.repo / '.git' / 'agent-tasks'
        # v0.1 remembered the ordinary branch under the unqualified remote name.
        (metadata / 'sync-observed.json').write_text(json.dumps({'origin': legacy_oid}))
        self.aye('sync')
        self.assertEqual(self.state(), git(self.bare, 'rev-parse', '--verify', REF))
        self.assertEqual(legacy_oid, git(self.bare, 'rev-parse', 'refs/heads/agent-tasks'))
        git(self.bare, 'update-ref', '-d', REF)
        before = self.state()
        self.aye('sync', code='REMOTE_STATE_DELETED')
        self.assertEqual(before, self.state())
        self.assertEqual('', git(self.repo, 'ls-remote', '--refs', 'origin', REF))

    def test_remote_ref_must_point_directly_to_commit(self):
        self.remote_fixture()
        self.aye('sync')
        before = self.state()
        tree = git(self.repo, 'rev-parse', f'{REF}^{{tree}}')
        git(self.repo, 'tag', '-a', 'task-test-tag', before, '-m', 'unsupported tag indirection')
        tag = git(self.repo, 'rev-parse', 'refs/tags/task-test-tag')
        git(self.repo, 'push', 'origin', 'refs/tags/task-test-tag')
        blob = git(self.repo, 'rev-parse', f'{REF}:project.json')
        for invalid in (tree, tag, blob):
            git(self.bare, 'update-ref', REF, invalid)
            self.aye('sync', code='STATE_CORRUPT')
            self.assertEqual(before, self.state())
        git(self.bare, 'update-ref', REF, before)
        self.aye('sync')

    def test_legacy_pending_conflict_requires_abort_without_retargeting(self):
        task = self.create('Legacy pending resolution')
        self.remote_fixture()
        oid = self.state()
        task_path = f'tasks/{task[2:4]}/{task}.json'
        candidate = {name: list((git(self.repo, 'show', f'{REF}:{name}') + '\n').encode())
                     for name in ('project.json', task_path)}
        marker = self.repo / '.git' / 'agent-tasks' / 'conflicts' / 'pending.json'
        marker.parent.mkdir(parents=True, exist_ok=True)
        for include_ref in (False, True):
            with self.subTest(explicit_legacy_ref=include_ref):
                pending = {'remote_name': 'origin', 'base': oid, 'local': oid, 'remote': oid,
                           'candidate': candidate, 'conflicts': [task_path],
                           'resolutions': {task_path: candidate[task_path]}, 'validation_error': None}
                if include_ref:
                    pending['remote_ref'] = 'refs/heads/agent-tasks'
                marker.write_text(json.dumps(pending))
                before = marker.read_bytes()
                self.assertEqual(task, self.aye('resolve', task)['local']['id'])
                self.assertIn(task, json.dumps(self.aye('resolve')))
                self.aye('resolve', '--continue', code='SYNC_PROTOCOL_MISMATCH')
                self.assertEqual(oid, self.state())
                self.assertEqual(before, marker.read_bytes())
                self.assertEqual('', git(self.repo, 'ls-remote', '--refs', 'origin',
                                         REF, 'refs/heads/agent-tasks'))
                self.aye('resolve', '--abort')
                self.assertFalse(marker.exists())

    def test_custom_ref_rejects_normal_non_fast_forward_push(self):
        self.remote_fixture()
        self.aye('sync')
        old = self.state()
        self.create('Advance remote history')
        self.aye('sync')
        published = self.state()
        rejected = run(self.repo, ['git', 'push', 'origin', f'{old}:{REF}'], check=False)
        self.assertNotEqual(0, rejected.returncode)
        self.assertIn('non-fast-forward', rejected.stdout + rejected.stderr)
        self.assertEqual(published, git(self.bare, 'rev-parse', REF))
