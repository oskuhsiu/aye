#!/usr/bin/env python3
"""Reproducible target-scale evidence using ignored local Git fixtures."""
import hashlib
import json
import time
from check import RepositoryCase, git, run, AYE

case = RepositoryCase()
case.setUp()
seed = case.create('Scale sample')
sample = case.show(seed)['task']
parent = case.state()
chunks = [f'commit refs/agent-tasks/state\ncommitter Benchmark <benchmark@example.invalid> 1000000000 +0000\ndata 15\nScale fixture\n\nfrom {parent}\nD tasks\n']
for i in range(10000):
    task = dict(sample)
    task['id'] = 't-' + hashlib.sha256(str(i).encode()).hexdigest()[:20]
    task['title'] = f'Scale task {i}'
    if i >= 1000:
        task.update(status='closed', resolution='done', closed_at=sample['created_at'])
    elif i >= 100:
        task['status'] = 'deferred'
    content = json.dumps(task) + '\n'
    chunks.append(f'M 100644 inline tasks/{task["id"][2:4]}/{task["id"]}.json\ndata {len(content.encode())}\n{content}\n')
chunks.append('\ndone\n')
git(case.repo, 'fast-import', '--quiet', input=''.join(chunks))
results = {'fixture': str(case.repo), 'total': 10000, 'active': 1000, 'ready': 100}
for args in [('ready', '--all'), ('rebuild',), ('update', 't-' + hashlib.sha256(b'0').hexdigest()[:20], '--title', 'Updated scale task')]:
    start = time.monotonic()
    result = case.aye(*args)
    results[' '.join(args[:1])] = round(time.monotonic() - start, 3)
    print(args[0], results[args[0]], flush=True)
    if args[0] == 'ready':
        assert len(result) == 100
print(json.dumps(results, indent=2))
