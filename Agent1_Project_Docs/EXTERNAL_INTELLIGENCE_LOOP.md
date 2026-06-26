# Agent1 External Intelligence Loop

## Purpose

Keep Agent1 aware of current architectures, tools, and operating patterns without letting novelty directly rewrite the system.

This loop is proposal-only. It researches, writes a digest, and ranks bounded experiments. Code changes still require a separate implementation loop, verification, and human approval for risky areas.

## Schedule

Default cadence: daily at 03:00 local time.

Use weekly cadence if the generated digest is noisy or repetitive.

## Inputs

- `Agent1_Project_Docs/SELF_IMPROVE.md`
- Existing digests in `Agent1_Project_Docs/external-intelligence/`
- Persistent notes in `.agent1/external-intel-notes.md`
- Current public sources from GitHub, official docs, benchmark updates, and implementation writeups

## Source Priority

Prefer sources with inspectable architecture:

- agent orchestration repos with state machines, queues, evals, memory, tools, approvals, or replay
- MCP servers and clients with robust process, permission, and schema design
- local-first AI apps with durable project state and model-provider adapters
- workflow engines, task runners, and CI recovery loops
- benchmark and eval harnesses that can be adapted to Agent1

De-prioritize:

- thin API wrappers
- generic chat shells
- UI-only demos
- prompt-only systems with no evals or operational state
- claims without source code, docs, or reproducible evidence

## Digest Format

Create one dated Markdown file:

`Agent1_Project_Docs/external-intelligence/YYYY-MM-DD.md`

Use this shape:

```markdown
# External Intelligence Digest: YYYY-MM-DD

## Sources Reviewed

- name - link - why reviewed

## Patterns

- Pattern:
  Applicability:
  Evidence:
  Risk:

## Ranked Experiments

1. Experiment:
   Why:
   Smallest change:
   Verification:
   Rollback:

## Not Adopted

- Item:
  Reason:

## Next Focus

- ...

AGENT1_EXTERNAL_INTEL_COMPLETE
```

## Adoption Gate

An idea may become an Agent1 change only after:

1. It is reduced to one bounded experiment.
2. The expected improvement is stated.
3. A verification method is named.
4. The change avoids approval, security, dependency, and production-policy edits unless explicitly approved.
5. The implementation run records keep/revert evidence.

## Commands

Run once:

```powershell
.\scripts\agent1-external-intel.ps1
```

Register the Windows scheduled task:

```powershell
.\scripts\install-external-intel-task.ps1 -Schedule Daily -At 03:00
```

Inspect the task:

```powershell
Get-ScheduledTask -TaskName "Agent1 External Intelligence"
```

Disable the task:

```powershell
Disable-ScheduledTask -TaskName "Agent1 External Intelligence"
```
