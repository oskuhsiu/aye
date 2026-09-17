from check import RepositoryCase, git, run, AYE
import json
import os


class CliCases(RepositoryCase):
    def test_actor_precedence_and_worktree_private_storage(self):
        other = self.worktree()
        self.aye('config', 'actor', 'local-a')
        self.aye('config', 'actor', 'local-b', cwd=other)
        first, second, third = [self.create(str(i)) for i in range(3)]
        self.aye('claim', first, actor=None)
        self.aye('claim', second, cwd=other, actor=None)
        self.aye('--actor', 'explicit', 'claim', third, actor='environment')
        self.assertEqual('local-a', self.show(first)['task']['claim']['actor'])
        self.assertEqual('local-b', self.show(second)['task']['claim']['actor'])
        self.assertEqual('explicit', self.show(third)['task']['claim']['actor'])
        self.assertNotIn('actor', git(self.repo, 'config', '--local', '--list'))

    def test_queries_and_rich_show(self):
        parent = self.create('Parent')
        bug = self.aye('create', 'Unicode 測試', '--type', 'bug', '--priority', 'P0',
                      '--label', 'Auth', '--description', 'Credential repair',
                      '--parent', parent, '--discovered-from', parent)['task']['id']
        self.aye('claim', bug)
        active = self.aye('list', '--state', 'in_progress', '--claimant', 'agent-a')
        self.assertEqual([bug], [r['task']['id'] for r in active])
        for args in [('list', '--query', 'CREDENTIAL'), ('list', '--type', 'bug'),
                     ('list', '--label', 'Auth'), ('list', '--priority', 'P0')]:
            self.assertEqual([bug], [r['task']['id'] for r in self.aye(*args)])
        relation = self.show(parent)['computed']
        self.assertIn(bug, relation['children'])
        self.assertIn(bug, relation['discovered'])
        self.aye('release', bug)
        self.assertEqual(bug, self.aye('ready', '--limit', '1')[0]['task']['id'])
        self.aye('close', bug)
        self.assertEqual(1, len(self.aye('list')))
        self.assertEqual(2, len(self.aye('list', '--all')))
        self.assertEqual(1, len(self.aye('list', '--state', 'closed')))

    def test_metadata_files_errors_and_nested_directory(self):
        task = self.create()
        note = self.home / 'note.md'
        note.write_text('Evidence from file\nsecond line\n')
        self.aye('note', task, '--file', str(note))
        self.assertEqual(note.read_text(), self.show(task)['task']['notes'][-1]['body'])
        self.aye('update', task, '--title', 'Changed', '--acceptance', 'A', '--acceptance', 'B',
                '--label', 'z', '--label', 'a')
        changed = self.show(task)['task']
        self.assertEqual('Changed', changed['title'])
        self.assertEqual(['A', 'B'], changed['acceptance'])
        self.assertEqual(['a', 'z'], changed['labels'])
        self.aye('update', task, '--clear-acceptance', '--clear-labels')
        self.assertEqual([], self.show(task)['task']['labels'])
        self.aye('update', task, '--status', 'closed', code='INVALID_ARGUMENT')
        self.aye('update', task, '--discovered-from', task, code='INVALID_ARGUMENT')
        self.aye('block', task, '--by', task, '--reason', 'both', code='INVALID_ARGUMENT')
        nested = self.repo / 'nested' / 'directory'
        nested.mkdir(parents=True)
        self.assertEqual(task, self.show(task, cwd=nested)['task']['id'])
        before = self.state()
        self.aye('status')
        self.assertEqual(before, self.state())
        self.aye('note', task, code='INVALID_ARGUMENT')
