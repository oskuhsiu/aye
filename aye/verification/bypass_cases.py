import json

from check import RepositoryCase


class BypassCases(RepositoryCase):
    def test_bypass_records_waiver_and_unlocks_dependents(self):
        implementation = self.aye(
            'create', 'Implement BLE reconnect', '--label', 'bluetooth'
        )['task']['id']
        dependent = self.create('Integrate reconnect screen')
        self.aye('block', dependent, '--by', implementation)
        self.aye('block', implementation, '--reason', 'Physical device unavailable')

        result = self.aye(
            'bypass', implementation,
            '--reason', 'Implementation and review are complete; no target device is available',
            '--missing', 'Physical-device smoke test',
            '--missing', 'Bluetooth reconnect test',
        )

        self.assertTrue(result['computed']['bypassed'])
        self.assertEqual([dependent], result['newly_ready'])
        self.assertEqual('Physical device unavailable', result['waived_manual_block'])
        task = self.show(implementation)['task']
        self.assertEqual('closed', task['status'])
        self.assertEqual('done', task['resolution'])
        self.assertIn('aye:bypassed', task['labels'])
        self.assertIn('bluetooth', task['labels'])
        self.assertIsNone(task['manual_block'])
        self.assertIn('[aye:bypass]', task['notes'][-1]['body'])
        self.assertIn('Physical-device smoke test', task['notes'][-1]['body'])
        self.assertEqual('ready', self.show(dependent)['computed']['effective_state'])
        listed = self.aye('list', '--state', 'bypassed')
        self.assertEqual([implementation], [item['task']['id'] for item in listed])
        self.assertTrue(listed[0]['computed']['bypassed'])

    def test_bypass_closes_deferred_work_in_one_native_action(self):
        task = self.create('Implementation complete')
        self.aye('defer', task)
        before = self.state()
        result = self.aye(
            'bypass', task,
            '--reason', 'External lab is unavailable',
            '--missing', 'Lab verification',
        )
        self.assertNotEqual(before, self.state())
        self.assertTrue(result['computed']['bypassed'])
        shown = self.show(task)
        self.assertEqual('closed', shown['task']['status'])
        self.assertIn('aye:bypassed', shown['task']['labels'])
        self.assertTrue(shown['computed']['bypassed'])

    def test_batch_can_apply_an_authorized_bypass(self):
        task = self.create('Implementation complete')
        request = self.repo / 'bypass.json'
        request.write_text(json.dumps({
            'version': 1,
            'operations': [{
                'op': 'bypass',
                'id': task,
                'reason': 'Target hardware unavailable',
                'missing': ['Physical-device verification'],
            }],
        }))
        result = self.aye('apply', '--file', str(request))
        self.assertEqual('bypass', result['operations'][0]['op'])
        self.assertTrue(result['tasks'][0]['computed']['bypassed'])
        self.assertEqual('closed', result['tasks'][0]['task']['status'])
        self.assertIn('[aye:bypass]', result['tasks'][0]['added_notes'][0]['body'])

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

    def test_reopen_clears_current_bypass_but_keeps_audit_history(self):
        task = self.create('Implementation')
        self.aye(
            'bypass', task,
            '--reason', 'No device',
            '--missing', 'Device test',
        )
        self.aye('reopen', task)
        shown = self.show(task)
        self.assertFalse(shown['computed']['bypassed'])
        self.assertNotIn('aye:bypassed', shown['task']['labels'])
        self.assertIn('[aye:bypass]', shown['task']['notes'][-1]['body'])
        self.assertEqual([], self.aye('list', '--state', 'bypassed'))
