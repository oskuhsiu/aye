from check import RepositoryCase


class BypassCases(RepositoryCase):
    def test_bypass_records_waiver_and_unlocks_dependents(self):
        implementation = self.aye(
            'create', 'Implement BLE reconnect', '--label', 'bluetooth'
        )['task']['id']
        dependent = self.create('Integrate reconnect screen')
        self.aye('block', dependent, '--by', implementation)
        self.aye('block', implementation, '--reason', 'Physical device unavailable')

        self.aye(
            'bypass', implementation,
            '--reason', 'Implementation and review are complete; no target device is available',
            '--missing', 'Physical-device smoke test',
            '--missing', 'Bluetooth reconnect test',
        )

        task = self.show(implementation)['task']
        self.assertEqual('closed', task['status'])
        self.assertEqual('done', task['resolution'])
        self.assertIn('aye:bypassed', task['labels'])
        self.assertIn('bluetooth', task['labels'])
        self.assertIsNone(task['manual_block'])
        self.assertIn('[aye:bypass]', task['notes'][-1]['body'])
        self.assertIn('Physical-device smoke test', task['notes'][-1]['body'])
        self.assertEqual('ready', self.show(dependent)['computed']['effective_state'])

    def test_bypass_resumes_deferred_work_atomically(self):
        task = self.create('Implementation complete')
        self.aye('defer', task)
        before = self.state()
        result = self.aye(
            'bypass', task,
            '--reason', 'External lab is unavailable',
            '--missing', 'Lab verification',
        )
        self.assertNotEqual(before, self.state())
        self.assertEqual(3, len(result['operations']))
        shown = self.show(task)['task']
        self.assertEqual('closed', shown['status'])
        self.assertIn('aye:bypassed', shown['labels'])

    def test_bypass_refuses_unresolved_task_dependencies(self):
        prerequisite = self.create('Required implementation')
        task = self.create('Dependent implementation')
        self.aye('block', task, '--by', prerequisite)
        before = self.state()
        error = self.aye(
            'bypass', task,
            '--reason', 'No physical device',
            '--missing', 'Device test',
            code='BYPASS_UNRESOLVED_DEPENDENCIES',
        )
        self.assertEqual([prerequisite], error['error']['details']['blocked_by'])
        self.assertEqual(before, self.state())

    def test_bypass_preserves_claim_ownership(self):
        task = self.create('Claimed implementation')
        self.aye('claim', task, actor='agent-a')
        before = self.state()
        self.aye(
            'bypass', task,
            '--reason', 'No physical device',
            '--missing', 'Device test',
            actor='agent-b',
            code='NOT_CLAIM_OWNER',
        )
        self.assertEqual(before, self.state())

    def test_bypass_requires_reason_and_missing_check(self):
        task = self.create('Implementation')
        before = self.state()
        self.aye('bypass', task, '--reason', '', '--missing', 'Device test',
                 code='INVALID_ARGUMENT')
        self.aye('bypass', task, '--reason', 'No device',
                 code='INVALID_ARGUMENT')
        self.assertEqual(before, self.state())
