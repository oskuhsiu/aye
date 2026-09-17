#!/usr/bin/env python3
"""Explicit live-host check; never included in automatic/local test discovery.

Success: an independent repo adopts project identity, publishes a claimed task,
the original sees ownership, then both see completion without source changes.
Failure: wrong owner cannot append a note or advance state; unexpected project,
missing task, altered source status/HEAD/index/config, or failed sync aborts.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import uuid
from check import ROOT, AYE, git, run

parser = argparse.ArgumentParser()
parser.add_argument('repository_url', help='Explicit authorized live Git remote URL')
args = parser.parse_args()
assert git(ROOT, 'remote', 'get-url', 'origin') == args.repository_url

def source_snapshot():
    return (git(ROOT, 'rev-parse', 'HEAD'), git(ROOT, 'symbolic-ref', 'HEAD'),
            git(ROOT, 'status', '--porcelain'), (ROOT / '.git/index').read_bytes(),
            git(ROOT, 'config', '--local', '--list'))

def aye(cwd, *args, actor='github-verifier', error=None):
    proc = run(cwd, [AYE, '--json', '--actor', actor, *args], check=False)
    value = json.loads(proc.stdout)
    if error:
        assert proc.returncode == 3 and value['error']['code'] == error, value
    else:
        assert proc.returncode == 0 and value['ok'], value
        return value['data']

before = source_snapshot()
aye(ROOT, 'sync')
clone = ROOT / 'test' / ('github-roundtrip-' + uuid.uuid4().hex[:8])
clone.mkdir(parents=True)
git(clone, 'init', '-q')
git(clone, 'remote', 'add', 'origin', args.repository_url)
adopted = aye(clone, 'init')
assert adopted['project_id'] == aye(ROOT, 'status')['project_id']
task = aye(clone, 'create', 'Verify live GitHub task synchronization', '--type', 'chore',
           '--label', 'verification',
           '--description', 'Live-host roundtrip authorized for oskuhsiu/aye; retain closed task as evidence.',
           '--acceptance', 'Original repository sees the independent repository claim after explicit sync.',
           '--acceptance', 'Wrong-owner mutation fails atomically; completion roundtrips without source changes.')['task']['id']
aye(clone, 'claim', task)
aye(clone, 'sync')
aye(ROOT, 'sync')
assert aye(ROOT, 'show', task)['task']['claim']['actor'] == 'github-verifier'
state = git(ROOT, 'rev-parse', 'refs/agent-tasks/state')
aye(ROOT, 'note', task, 'wrong owner', actor='other', error='NOT_CLAIM_OWNER')
assert state == git(ROOT, 'rev-parse', 'refs/agent-tasks/state')
aye(clone, 'note', task, 'GitHub publication/adoption and claim visibility verified; wrong-owner mutation refused without changing state.')
aye(clone, 'close', task)
aye(clone, 'sync')
aye(ROOT, 'sync')
assert aye(ROOT, 'show', task)['task']['resolution'] == 'done'
remote_tip = git(ROOT, 'ls-remote', '--refs', 'origin', 'refs/agent-tasks/state').split()[0]
assert remote_tip == git(ROOT, 'rev-parse', 'refs/agent-tasks/state')
assert 'refs/heads/agent-tasks' not in git(ROOT, 'ls-remote', '--heads', 'origin')
assert 'origin/agent-tasks' not in git(ROOT, 'branch', '-a')
assert not (clone / 'tasks').exists() and not (clone / 'views').exists()
assert git(ROOT, 'rev-parse', 'refs/agent-tasks/state') == git(clone, 'rev-parse', 'refs/agent-tasks/state')
aye(ROOT, 'doctor')
assert before == source_snapshot()
print(json.dumps({'ok': True, 'task': task, 'repository': args.repository_url,
                  'state_oid': git(ROOT, 'rev-parse', 'refs/agent-tasks/state'),
                  'independent_repository': str(clone)}, indent=2))
