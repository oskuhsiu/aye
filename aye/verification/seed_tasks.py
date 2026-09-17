#!/usr/bin/env python3
"""Run once after bootstrap passes; task metadata lives only in the shared ref."""
import json
import subprocess

TASKS = [
    ('Complete lifecycle, metadata and blocking',
     'Implement v0.3.1 local domain commands after bootstrap. Cases: verification/CASES.md 5-6. Write executable behavior checks before code.',
     ['Owner-only mutations and forced release with attributed reason work; invalid transitions leave state unchanged.',
      'Block releases claim atomically; cancelled blockers stay unresolved; done unblocks without editing dependent canonical bytes.',
      'Dependency/parent cycles, self/missing/duplicate edges and unsafe prerequisite reopen are rejected.',
      'Metadata, labels, parent and immutable creation provenance roundtrip; defer/resume and cancellation behave as specified.']),
    ('Build deterministic projections and integrity diagnostics',
     'Implement v0.3.1 projections, FORMAT and doctor. Cases: verification/CASES.md 7. Write tests before implementation.',
     ['Ready/active projections and last-20 report are byte deterministic with stable ordering.',
      'Stale reads warn and recompute without commits; rebuild preserves canonical bytes and repeated rebuild makes no commit.',
      'Doctor rejects corrupt state and reports focused warnings; FORMAT documents tool-less reading.']),
    ('Implement explicit sync and conflict resolution',
     'Implement v0.3.1 remote init, fetch/reconcile/push and shared resolver. Cases: verification/CASES.md 8. No production remote writes; use local bare fixtures.',
     ['First publish, adoption, fast-forwards and disjoint two-parent merges preserve project identity and source state.',
      'Same-task and graph conflicts freeze mutations across worktrees; local/remote/file resolution and abort work.',
      'Invalid remote state, foreign project and deleted known branch fail without unsafe ref changes.',
      'Stale external canonical edits repair projections; remote advance before push safely retries without force.']),
    ('Complete CLI queries, actor configuration and integration',
     'Wire all domain/projection/sync features to documented v1 CLI and JSON contract. Cases: verification/CASES.md 5,7,9.',
     ['All v1 commands parse and return stable JSON envelopes and coarse exit codes.',
      'Actor flag overrides env and worktree-private identity; linked worktrees keep separate actors.',
      'List/ready filtering, reverse relations in show, report/status/rebuild/config match the specification.']),
    ('Verify v1 end-to-end and document installed workflow',
     'Review integrated implementation against v0.3.1 required matrix; add missing behavior cases before corrections. Package through Cargo defaults.',
     ['Required core, lifecycle, blocking, worktree, projection, sync and source-isolation cases pass against installed aye.',
      'cargo test, fmt and clippy pass; independent standards/spec reviews resolved.',
      'README explains Cargo installation, agent workflow, explicit sync, conflict recovery and limits; tasks contain actual verification evidence.']),
]

def aye(*args):
    proc = subprocess.run(['aye', '--json', '--actor', 'codex-main', *args], text=True, capture_output=True, check=True)
    return json.loads(proc.stdout)['data']

if __name__ == '__main__':
    print(json.dumps(aye('init', '--offline')))
    for title, description, acceptance in TASKS:
        args = ['create', title, '--type', 'feature', '--description', description]
        for case in acceptance:
            args += ['--acceptance', case]
        task = aye(*args)['task']
        print(task['id'], task['title'])
