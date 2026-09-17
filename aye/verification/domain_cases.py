from check import RepositoryCase, git


class DomainCases(RepositoryCase):
    def test_discovery_blocking_and_completion_across_worktrees(self):
        other = self.worktree()
        task = self.create('Original work')
        self.aye('claim', task)
        blocker = self.aye('create', 'Discovered prerequisite', '--discovered-from', task)['task']['id']
        out = self.aye('block', task, '--by', blocker)
        self.assertEqual('agent-a', out['released_claim'])
        self.assertIsNone(self.show(task, cwd=other)['task']['claim'])
        self.assertEqual('blocked', self.show(task, cwd=other)['computed']['effective_state'])
        self.aye('claim', task, code='TASK_NOT_READY')
        self.aye('close', task, code='INVALID_STATE_TRANSITION')
        path = f'refs/agent-tasks/state:tasks/{task[2:4]}/{task}.json'
        before = git(self.repo, 'show', path)
        self.aye('close', blocker, cwd=other)
        self.assertEqual(before, git(self.repo, 'show', path))
        self.assertEqual('ready', self.show(task)['computed']['effective_state'])
        self.aye('claim', task)
        self.aye('reopen', blocker, code='INVALID_STATE_TRANSITION')
        self.aye('release', task)
        self.aye('reopen', blocker)
        self.assertEqual('blocked', self.show(task)['computed']['effective_state'])

    def test_independent_blockers_cancelled_prerequisite_and_cycles(self):
        task, blocker = self.create('A'), self.create('B')
        self.aye('block', task, '--by', blocker)
        self.aye('block', task, '--reason', 'Waiting for credentials')
        self.aye('unblock', task)
        self.assertEqual('blocked', self.show(task)['computed']['effective_state'])
        before = self.state()
        self.aye('block', blocker, '--by', task, code='DEPENDENCY_CYCLE')
        self.assertEqual(before, self.state())
        self.aye('close', blocker, '--cancelled')
        self.assertEqual('blocked', self.show(task)['computed']['effective_state'])
        self.aye('block', task, '--reason', 'Still waiting')
        self.aye('unblock', task, '--by', blocker)
        self.assertEqual('blocked', self.show(task)['computed']['effective_state'])
        self.aye('unblock', task)
        self.assertEqual('ready', self.show(task)['computed']['effective_state'])
        self.aye('reopen', blocker)
        self.aye('close', blocker)
        self.aye('block', task, '--by', blocker, code='BLOCKER_ALREADY_SATISFIED')
        self.aye('update', task, '--parent', blocker)
        self.aye('update', blocker, '--parent', task, code='PARENT_CYCLE')
        self.aye('create', 'missing', '--parent', 't-00000000000000000000', code='RELATION_TARGET_NOT_FOUND')

    def test_recovery_defer_resume_and_owner_guard(self):
        task = self.create()
        self.aye('claim', task)
        for args in [('update', task, '--priority', 'P0'), ('defer', task),
                     ('block', task, '--reason', 'wrong'), ('unblock', task), ('release', task)]:
            self.aye(*args, actor='b', code='NOT_CLAIM_OWNER')
        self.aye('release', task, '--force', actor='b', code='INVALID_ARGUMENT')
        self.aye('release', task, '--force', '--reason', 'Crashed agent', actor='b')
        note = self.show(task)['task']['notes'][-1]
        self.assertEqual('b', note['actor'])
        self.assertIn('agent-a', note['body'])
        self.assertIn('Crashed agent', note['body'])
        self.aye('claim', task, actor='b')
        self.aye('defer', task, actor='b')
        self.assertIsNone(self.show(task)['task']['claim'])
        self.aye('block', task, '--reason', 'external')
        self.assertEqual('deferred', self.show(task)['computed']['effective_state'])
        self.aye('resume', task)
        self.assertEqual('blocked', self.show(task)['computed']['effective_state'])
        self.aye('defer', task)
        self.aye('close', task, '--cancelled')
        self.aye('reopen', task)
        self.assertEqual('blocked', self.show(task)['computed']['effective_state'])
