"""Installed one-operation task acquisition, no-write and recovery contracts."""
import concurrent.futures
import json
import os
import threading

from check import AYE, RepositoryCase, git, run


class AssignmentCases(RepositoryCase):
    def packet(self, *args, actor='agent-a'):
        return self.aye('claim', '--next', *args, actor=actor)

    def test_single_claim_full_context_and_direct_statuses(self):
        parent = self.create('Parent')
        self.aye('defer', parent)
        prerequisite = self.create('Satisfied prerequisite')
        target = self.aye('create', 'Work', '--description', 'Complete requirement',
                          '--acceptance', 'Observable success', '--parent', parent,
                          '--discovered-from', parent, '--label', 'scope')['task']['id']
        self.aye('block', target, '--by', prerequisite)
        self.aye('close', prerequisite)
        child = self.aye('create', 'Child', '--parent', target)['task']['id']
        self.aye('block', child, '--by', target)
        cancelled = self.aye('create', 'Cancelled discovery', '--discovered-from', target)['task']['id']
        self.aye('close', cancelled, '--cancelled')
        self.aye('note', target, 'Retained evidence')
        source, index = git(self.repo, 'rev-parse', 'HEAD'), git(self.repo, 'write-tree')
        reply = self.packet('--label', 'scope')
        self.assertEqual('claimed', reply['outcome'])
        self.assertEqual(self.state(), reply['state_oid'])
        self.assertEqual(self.show(target)['task'], reply['task'])
        self.assertEqual('in_progress', reply['task']['status'])
        self.assertEqual('agent-a', reply['task']['claim']['actor'])
        self.assertEqual(['Observable success'], reply['task']['acceptance'])
        self.assertEqual('Retained evidence', reply['task']['notes'][0]['body'])
        related = {v['id']: v for v in reply['related']['items']}
        self.assertEqual([prerequisite, child, parent, cancelled], [v['id'] for v in reply['related']['items']])
        self.assertEqual(['dependent', 'child'], related[child]['relations'])
        self.assertEqual(['parent', 'discovered_from'], related[parent]['relations'])
        self.assertEqual('blocked', related[child]['effective_state'])
        self.assertEqual('open', related[child]['status'])
        self.assertEqual('done', related[prerequisite]['resolution'])
        self.assertEqual('cancelled', related[cancelled]['resolution'])
        self.assertEqual(0, reply['related']['omitted'])
        self.assertEqual(source, git(self.repo, 'rev-parse', 'HEAD'))
        self.assertEqual(index, git(self.repo, 'write-tree'))

    def test_no_ready_and_stale_projections_never_publish(self):
        empty = self.packet()
        self.assertEqual('empty_project', empty['situation']['reason'])
        blocked, deferred, active = [self.create(t) for t in ('blocked', 'deferred', 'active')]
        self.aye('block', blocked, '--reason', 'External wait')
        self.aye('defer', deferred)
        self.aye('claim', active, actor='other')
        self.edit_state({'REPORT.md': 'stale report'})
        before = self.state()
        reply = self.packet()
        self.assertEqual('no_ready_task', reply['outcome'])
        self.assertIsNone(reply['task'])
        self.assertEqual(before, reply['state_oid'])
        self.assertEqual(before, self.state())
        self.assertEqual({'blocked', 'deferred', 'in_progress'},
                         {v['effective_state'] for v in reply['situation']['explanations']['items']})
        restrictive = self.packet('--label', 'missing')
        self.assertEqual('no_matching_tasks', restrictive['situation']['reason'])
        self.assertEqual(3, restrictive['situation']['project_total'])
        self.assertEqual(0, restrictive['situation']['matching']['total'])
        self.assertEqual(before, self.state())
        self.aye('doctor', code='VIEW_STALE')

    def test_scope_order_existing_claim_guard_and_legacy(self):
        first = self.aye('create', 'First', '--priority', 'P1', '--type', 'bug', '--label', 'scope')['task']['id']
        second = self.aye('create', 'Second', '--priority', 'P1', '--type', 'bug', '--label', 'scope')['task']['id']
        outside = self.aye('create', 'Outside', '--priority', 'P0')['task']['id']
        # Force a creation-time tie to prove the ID tiebreaker.
        tasks = [self.show(id)['task'] for id in (first, second)]
        for task in tasks:
            task['created_at'] = tasks[0]['created_at']
        self.edit_state({f"tasks/{t['id'][2:4]}/{t['id']}.json": t for t in tasks})
        reply = self.packet('--priority', 'P1', '--type', 'bug', '--label', 'scope')
        target = min(first, second)
        self.assertEqual(target, reply['task']['id'])
        self.edit_state({'REPORT.md': 'stale again'})
        before, previous = self.state(), self.show(target)['task']
        owned = self.packet('--label', 'scope')
        self.assertEqual('already_owned', owned['outcome'])
        self.assertEqual(previous, owned['task'])
        self.assertEqual(before, self.state())
        ambiguity = self.packet('--label', 'outside')
        self.assertEqual('existing_assignments', ambiguity['outcome'])
        self.assertEqual('owned_task_outside_scope', ambiguity['situation']['reason'])
        self.assertEqual(before, self.state())
        self.aye('claim', target, '--packet', code='TASK_ALREADY_CLAIMED')
        # Legacy explicit claims permit this actor to own additional tasks.
        explicit = self.aye('claim', outside, '--packet')
        self.assertEqual(outside, explicit['task']['id'])
        before = self.state()
        ambiguity = self.packet()
        self.assertEqual('multiple_owned_tasks', ambiguity['situation']['reason'])
        self.assertEqual(2, ambiguity['existing_assignments']['total'])
        self.assertEqual(before, self.state())
        self.aye('claim', first, '--next', code='INVALID_ARGUMENT')
        self.aye('claim', first, '--label', 'scope', code='INVALID_ARGUMENT')
        self.aye('claim', '--next', actor=None, code='ACTOR_REQUIRED')

    def test_oversized_mandatory_task_prevents_claim_without_skipping(self):
        target = self.aye('create', 'Large', '--priority', 'P0')['task']['id']
        other = self.create('Small')
        note = self.home / 'large.md'
        note.write_text('requirements ' * 6000)
        self.aye('note', target, '--file', str(note))
        before = self.state()
        failure = self.aye('claim', '--next', code='PACKET_TOO_LARGE')
        self.assertEqual(target, failure['error']['details']['task_id'])
        self.assertFalse(failure['error']['details']['already_owned'])
        self.assertEqual(before, self.state())
        self.assertIsNone(self.show(other)['task']['claim'])
        self.aye('claim', target, '--packet', code='PACKET_TOO_LARGE')
        self.assertEqual(before, self.state())
        self.aye('claim', target)  # legacy intentionally still permits large tasks
        before = self.state()
        failure = self.aye('claim', '--next', code='PACKET_TOO_LARGE')
        self.assertTrue(failure['error']['details']['already_owned'])
        self.assertEqual(before, self.state())
        self.assertEqual('in_progress', self.show(target)['task']['status'])

    def test_related_brief_count_and_serialized_budget(self):
        target = self.create('Parent target')
        operations = [{'op': 'create', 'title': f'Child {i}', 'parent': target} for i in range(23)]
        request = self.home / 'children.json'
        request.write_text(json.dumps({'version': 1, 'operations': operations}))
        self.aye('apply', '--file', str(request))
        reply = self.aye('claim', target, '--packet')
        self.assertEqual((23, 20, 3), tuple(reply['related'][k] for k in ('total', 'returned', 'omitted')))
        self.assertEqual(23, len(reply['computed']['children']))
        # Optional long titles cannot displace or truncate full task data.
        self.aye('release', target)
        child = reply['related']['items'][0]['id']
        self.aye('update', child, '--title', 'x' * 65536)
        proc = run(self.repo, [AYE, '--json', '--actor', 'agent-a', 'claim', target, '--packet'])
        self.assertLessEqual(len(proc.stdout.encode()), 65536)
        packet = json.loads(proc.stdout)['data']
        self.assertEqual(target, packet['task']['id'])
        self.assertGreater(packet['related']['omitted'], 0)

    def test_concurrent_same_and_different_actors(self):
        other = self.worktree()
        for name in ('one', 'two', 'three'):
            self.create(name)
        barrier = threading.Barrier(2)
        def acquire(actor, cwd):
            barrier.wait()
            return self.aye('claim', '--next', actor=actor, cwd=cwd)
        with concurrent.futures.ThreadPoolExecutor(2) as pool:
            a = pool.submit(acquire, 'same', self.repo)
            b = pool.submit(acquire, 'same', other)
            results = [a.result(), b.result()]
        self.assertEqual(1, len({r['task']['id'] for r in results}))
        self.assertEqual({'claimed', 'already_owned'}, {r['outcome'] for r in results})
        barrier = threading.Barrier(2)
        with concurrent.futures.ThreadPoolExecutor(2) as pool:
            a = pool.submit(acquire, 'different-a', self.repo)
            b = pool.submit(acquire, 'different-b', other)
            results = [a.result(), b.result()]
        self.assertEqual(2, len({r['task']['id'] for r in results}))
        for result in results:
            task = result['task']
            raw = git(self.repo, 'show', f"{result['state_oid']}:tasks/{task['id'][2:4]}/{task['id']}.json")
            self.assertEqual(task, json.loads(raw))
            self.assertEqual('claimed', result['outcome'])

    def test_lost_reply_recovery_and_historical_guard_limit(self):
        first, second = self.create('First'), self.create('Second')
        # The caller deliberately discards stdout to model a lost successful reply.
        run(self.repo, [AYE, '--json', '--actor', 'lost', 'claim', '--next'])
        before = self.state()
        observed = self.aye('list', '--claimant', 'lost')
        self.assertEqual(1, len(observed))
        task = observed[0]['task']['id']
        self.assertEqual(before, self.state())  # recovery needs no new allocation
        same = self.packet(actor='lost')
        self.assertEqual(task, same['task']['id'])
        self.assertEqual('already_owned', same['outcome'])
        self.aye('close', task, actor='lost')
        # This is a new authorized start, NOT a valid lost-response replay.
        next_task = self.packet(actor='lost')['task']['id']
        self.assertEqual({first, second}, {task, next_task})
