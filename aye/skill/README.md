# aye skill for agents

This package teaches an agent how to use the aye task manager. It covers daily
task work, coordination across Git worktrees, explicit remote synchronization,
conflict handling and Git/API inspection. Agents invoke aye directly; no handler
agent, runtime role or model-routing setup is required. It targets aye 0.3.x with canonical
format 1. This README is for people; the agent starts at `SKILL.md`.

## Package layout

```text
skill/
|-- SKILL.md                     Brief agent entry point and reference routing
|-- README.md                    This human guide
`-- refs/
    |-- common.md                Everyday workflow, claims, blockers, completion
    |-- batches.md               Strict JSON request and operation fields
    |-- worktrees.md             Checkout notes, interruption/resume and cleanup
    |-- tasks-and-states.md      Metadata, queries, relationships and lifecycle
    |-- sync-and-conflicts.md    Remote synchronization and conflict resolution
    |-- diagnostics.md          Errors, stale views and abandoned claims
    `-- git-and-api.md           Storage inspection, API reads and backups
```

The agent reads the common workflow for ordinary task work and loads specialized
references only when needed. The package is self-contained: copying it does not
require copying aye's source tree or planning documents.

## Prerequisites and installation

Install Git and the aye executable on the agent's PATH. To build aye from its
source checkout, run from the repository root using Cargo's configured/default
installation location:

```sh
cargo install --path aye --force --locked
aye --version
```

Cargo normally installs into `~/.cargo/bin`; a configured Cargo installation may
use another location. The skill itself contains no executable and installs no
hooks, background services or API server.

Copy or link this entire directory into the skill location supported by your
agent, naming the installed directory `aye`. Preserve `SKILL.md` and `refs/`
together so their relative links work. `README.md` is optional for agent loading.
The frontmatter skill name is `aye`; the source directory is named `skill` for
distribution. Follow your agent application's normal skill discovery/reload step.

## Ask the agent to use it

Examples:

- “Use aye to find the next ready task in the login work and complete it.”
- “Create aye tasks for this plan, with success and failure acceptance cases.”
- “Record the prerequisite you discovered and block the original task on it.”
- “Synchronize aye state and explain any conflicts before choosing a resolution.”

Applications with explicit skill invocation can select the skill named `aye`.
Installation makes the guide available; task tracking still follows your requested
scope. Each independent agent should have a distinct actor identity.

## What to expect

Starting authorized next work normally uses one `claim --next`; a specified task
uses `claim TASK_ID --packet`. The reply supplies complete task details and bounded
related context, and confirms ownership without another acceptance call. Read-only
comparison remains available before choosing scope. Already-decided multi-task
plans, progress and lifecycle changes use an atomic `apply --file` request.
Compatibility checks belong at setup or an actual error, not every operation.

An ordinary pause for any session reason records a handoff note and releases the
claim (note only if already open). Unblocked work stays ready for the next session
without a separate resume or confirmation. Explicit shelving pending confirmation
uses defer and stays out of ready until authorized resumption; real blockers remain.
Lost output requires inspection before retrying, since replay is not exactly-once.
Source changes, task completion and remote task publication are separate results.
The agent should record actual verification evidence before closing work as done.
Local linked worktrees share claims immediately; independent clones need sync
and can conflict. A normal source push does not publish tasks.

For source work, the agent records the actual worktree root, branch and machine
in task notes before editing. Those notes retain unfinished-work context across
release, defer and blocking, so a later session can verify and reuse the right
checkout. Completed task worktrees are cleaned up through the project's manager
(for example hd), with verified cleanup appended to the task. This is agent
workflow guidance; aye does not automatically associate or remove worktrees.

Task data stays in Git objects reached through the custom `refs/agent-tasks/state`
ref locally and remotely. It does not add task folders to your source tree or
create a task branch. Git viewers using `--all` may still display task history.
GitHub access requires Git or Git Database API tools; browser-only task reading
is outside this version's supported workflow.

The skill does not grant permission to publish, override another agent's claim,
or rewrite repository history. Existing task authorization remains applicable;
the guide calls for a decision only when the intended action needs one.
