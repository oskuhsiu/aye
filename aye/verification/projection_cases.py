#!/usr/bin/env python3
"""Integrated read/repair contracts against the Cargo-installed aye binary."""
import json
import unittest

from check import AYE, RepositoryCase, git, run

STATE = 'refs/agent-tasks/state'


class Projections(RepositoryCase):
    def envelope(self, *args):
        proc = run(self.repo, [AYE, '--json', *args], check=False)
        self.assertEqual(0, proc.returncode, (proc.stdout, proc.stderr))
        reply = json.loads(proc.stdout)
        self.assertTrue(reply['ok'], reply)
        return reply

    def blob(self, path):
        # Preserve whitespace exactly; git() intentionally strips its output.
        return run(self.repo, ['git', 'show', f'{STATE}:{path}']).stdout

    def canonical_bytes(self):
        paths = git(self.repo, 'ls-tree', '-r', '--name-only', STATE).splitlines()
        return {p: self.blob(p) for p in paths
                if p == 'project.json' or p.startswith('tasks/')}

    def assert_fresh_manifest(self):
        manifest = json.loads(self.blob('manifest.json'))
        self.assertEqual(1, manifest['projection_version'])
        self.assertEqual(git(self.repo, 'rev-parse', f'{STATE}:tasks'),
                         manifest['tasks_tree_oid'])
        self.assertTrue(self.aye('doctor')['valid'])

    def test_external_edit_reads_current_state_without_commit_and_rebuild_preserves_bytes(self):
        task = self.create('Original title')
        self.aye('block', task, '--reason', 'External approval')
        self.assertEqual([], self.aye('ready'))
        canonical = self.show(task)['task']
        canonical['title'] = 'Externally corrected title'
        canonical['manual_block'] = None
        canonical['priority'] = 'P0'
        task_path = f'tasks/{task[2:4]}/{task}.json'
        raw = '\n' + json.dumps(canonical, indent=7) + '\n\n'
        self.edit_state({task_path: raw})
        before = self.state()
        preserved = self.canonical_bytes()
        for args in [('ready',), ('show', task), ('report',)]:
            reply = self.envelope(*args)
            self.assertIn('VIEW_STALE', [w['code'] for w in reply['warnings']])
            data = reply['data']
            if args[0] == 'ready':
                self.assertEqual([task], [v['task']['id'] for v in data])
            elif args[0] == 'show':
                self.assertEqual('ready', data['computed']['effective_state'])
                self.assertEqual(canonical['title'], data['task']['title'])
            else:
                self.assertIn(canonical['title'], data)
                self.assertNotIn('Original title', data)
            self.assertEqual(before, self.state())
        self.aye('doctor', code='VIEW_STALE')
        self.assertEqual(before, self.state())
        self.aye('rebuild')
        rebuilt = self.state()
        self.assertNotEqual(before, rebuilt)
        self.assertEqual(preserved, self.canonical_bytes())
        self.assertEqual(raw, self.blob(task_path))
        self.assert_fresh_manifest()
        self.assertEqual([], self.envelope('ready')['warnings'])
        self.aye('rebuild')
        self.assertEqual(rebuilt, self.state())

    def test_missing_derived_files_repair_and_active_membership(self):
        ready = self.create('Ready')
        active = self.create('Working')
        deferred = self.create('Later')
        closed = self.create('Finished')
        self.aye('claim', active)
        self.aye('defer', deferred)
        self.aye('close', closed)
        rows = [json.loads(line) for line in self.blob('views/active.jsonl').splitlines()]
        self.assertEqual({ready: 'ready', active: 'in_progress', deferred: 'deferred'},
                         {row['id']: row['effective_state'] for row in rows})
        self.assertEqual({ready, active, deferred},
                         {row['task']['id'] for row in self.aye('list')})
        self.assertEqual([ready], [row['task']['id'] for row in self.aye('ready')])
        expected = {p: self.blob(p) for p in
                    ('manifest.json', 'views/ready.jsonl', 'views/active.jsonl',
                     'REPORT.md', 'FORMAT.md')}
        preserved = self.canonical_bytes()
        self.edit_state({p: None for p in expected})
        before = self.state()
        reply = self.envelope('list')
        self.assertIn('VIEW_STALE', [w['code'] for w in reply['warnings']])
        self.assertEqual({ready, active, deferred},
                         {row['task']['id'] for row in reply['data']})
        self.aye('doctor', code='VIEW_STALE')
        self.assertEqual(before, self.state())
        self.aye('rebuild')
        self.assertEqual(preserved, self.canonical_bytes())
        self.assertEqual(expected, {p: self.blob(p) for p in expected})
        self.assert_fresh_manifest()
        rebuilt = self.state()
        self.aye('rebuild')
        self.assertEqual(rebuilt, self.state())


class ProjectionModeCases(RepositoryCase):
    def test_executable_derived_file_is_stale_and_repairable(self):
        import os
        task = self.create('Canonical task remains usable')
        parent = self.state()
        env = {**os.environ, 'GIT_INDEX_FILE': str(self.home / 'mode-index')}
        git(self.repo, 'read-tree', parent, env=env)
        blob = git(self.repo, 'rev-parse', 'refs/agent-tasks/state:REPORT.md')
        git(self.repo, 'update-index', '--cacheinfo', f'100755,{blob},REPORT.md', env=env)
        tree = git(self.repo, 'write-tree', env=env)
        changed = git(self.repo, 'commit-tree', tree, '-p', parent, input='External derived mode change\n')
        git(self.repo, 'update-ref', 'refs/agent-tasks/state', changed, parent)
        ready = self.aye('ready')
        self.assertEqual([task], [r['task']['id'] for r in ready])
        self.assertEqual(changed, self.state())
        self.aye('doctor', code='VIEW_STALE')
        self.aye('rebuild')
        self.assertTrue(git(self.repo, 'ls-tree', 'refs/agent-tasks/state', 'REPORT.md').startswith('100644 '))
        self.aye('doctor')


if __name__ == '__main__':
    unittest.main()
