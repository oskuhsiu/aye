#!/usr/bin/env python3
"""Read a known canonical task through GitHub Git Database API, no aye required.

Success: exact custom ref -> commit -> shard tree -> task blob bytes match Git.
Failure: missing/ambiguous ref, non-commit tip, truncated tree or different blob.
Only reads GitHub; call with OWNER/REPO and a full task ID after explicit sync.
"""
import argparse
import base64
import json
import subprocess
import urllib.request
from check import ROOT, git

parser = argparse.ArgumentParser()
parser.add_argument('repository', help='OWNER/REPO')
parser.add_argument('task_id')
args = parser.parse_args()
api = 'https://api.github.com/repos/' + args.repository + '/git/'

def get(path):
    request = urllib.request.Request(api + path, headers={
        'Accept': 'application/vnd.github+json', 'User-Agent': 'aye-ref-verification'})
    with urllib.request.urlopen(request) as response:
        return json.load(response)

matches = get('matching-refs/agent-tasks/')
refs = [item for item in matches if item['ref'] == 'refs/agent-tasks/state']
assert len(refs) == 1, refs
reference = refs[0]
assert reference['object']['type'] == 'commit'
oid = reference['object']['sha']
assert oid == git(ROOT, 'rev-parse', 'refs/agent-tasks/state'), 'API ref is not the current local synchronized state'
commit = get('commits/' + oid)
tree_oid = commit['tree']['sha']
path = f'tasks/{args.task_id[2:4]}/{args.task_id}.json'
for index, name in enumerate(path.split('/')):
    tree = get('trees/' + tree_oid)
    assert not tree.get('truncated', False), 'Tree response truncated; fetch objects with Git'
    entries = [item for item in tree['tree'] if item['path'] == name]
    assert len(entries) == 1, entries
    entry = entries[0]
    if index < 2:
        assert entry['type'] == 'tree'
        tree_oid = entry['sha']
    else:
        assert entry['type'] == 'blob'
        blob = get('blobs/' + entry['sha'])
        assert blob['encoding'] == 'base64'
        content = base64.b64decode(blob['content'])
        local = subprocess.run(['git','show',f'{oid}:{path}'],cwd=ROOT,
                                             capture_output=True,check=True).stdout
        assert content == local
        task = json.loads(content)
        assert task['id'] == args.task_id
print(json.dumps({'ok': True, 'ref': reference['ref'], 'commit': oid,
                  'task': task['id'], 'title': task['title'], 'canonical_bytes_match': True}, indent=2))
