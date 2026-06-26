# Agent1 Self-Improvement Operating Manual

## Purpose

This is the durable operating manual for Agent1's self-improvement system.
Read it when starting a session or when momentum stalls.

---

## The One Rule

**Build, then leave reusable artifacts. Never finish empty-handed.**

At the end of every substantial run, leave behind:
- updated state in files
- visible evidence (artifacts, diffs, screenshots)
- one or more reusable assets (skill, workflow, template, eval, policy)
- a clear next step
- at least one improvement candidate or follow-up task

---

## The Core Loop

```
goal -> task graph -> execution -> verification -> memory update -> learning -> visibility
```

If any step is missing, the loop is broken. Fix it before expanding scope.

**Verification first completion:** Nothing is done until the system runs checks that prove it is done.
Do not let the same step both produce and certify the result.

---

## Momentum Queues

Maintain five live queues at all times. If any is empty or undefined, momentum is broken.

### Queue Definitions

**now** — The current active milestone or highest-priority task. Focus execution here.
**next** — The next small set of concrete tasks ready to run immediately.
**blocked** — Tasks waiting on approvals, missing info, failed dependencies, or missing capabilities.
**improve** — Self-improvement work: eval gaps, flaky workflows, repeated failures, missing skills, stale assumptions, external intelligence experiments.
**recurring** — Schedules, monitors, sweeps, and automations that keep the system alive over time.

### Queue Health Rules

- If `now` is empty: define the current milestone or ask the human.
- If `next` is empty but `now` exists: decompose `now` into next-step tasks.
- If `blocked` grows >3 items: decompose blockers into smallest unblock actions.
- If `improve` has not changed in 3+ sessions: run the external intelligence loop.
- If `recurring` is empty: add at least one scheduled check or monitor.

### Anti-Stall Rules

When momentum drops, react mechanically:

1. **If blocked > 15 min:** decompose the blocker, seek the smallest missing answer, work on non-blocked sidecar improvements in parallel.
2. **If same failure happens twice:** add a guardrail, test, or policy. Do not just retry.
3. **If long-running task has no visible artifact progress:** write intermediate outputs, checkpoint state, surface a clearer progress indicator.
4. **If system is waiting for slow task:** fill idle time with eval work, memory cleanup, dashboard improvements, or backlog grooming.
5. **If milestone is "done" but next step is undefined:** create the next milestone immediately or open explicit choices for the human.

### Priority Order for Next-Work Selection

When choosing what to do next, prefer work that:
- closes the core loop
- unblocks many future tasks
- increases reliability
- creates reusable leverage
- improves observability
- reduces cost for repeated work

**Default priority when in doubt:**
1. unblock the current milestone
2. fix reliability or verification gaps
3. convert repeated work into reusable assets
4. add eval coverage for high-value failures
5. expand breadth only after the loop is stable

---

## Gap Classification

When something fails, classify the failure as one or more of:

| Category | Meaning |
|---|---|
| missing skill | The agent lacked domain knowledge or SOP for this task type |
| missing tool | The right capability interface did not exist |
| bad decomposition | Goal was split into wrong or incomplete tasks |
| bad verification | No check proved the result was correct |
| unsafe autonomy | The agent acted without enough human review for the risk level |
| poor model routing | A cheaper/faster model could have handled this |
| context overload | The agent had too much or too little context to act correctly |
| missing eval | There was no test that could have caught this failure |
| external dependency failure | An outside system (API, tool, network) failed |
| bad human requirements | The instructions given were ambiguous or contradictory |

**Then choose the most leverageful repair:**
- missing skill → add or refine a skill/SOP
- missing tool → build or wrap a tool adapter
- bad decomposition → improve the task specifier
- bad verification → improve the verification contract
- missing eval → add a test or check
- context overload → add summarization, batching, or file-based retrieval

---

## One-Change Improvement Loop

This is the fundamental self-improvement primitive. Use it for every non-trivial improvement.

### The Loop

1. **Observe** — detect a gap from task outcomes, eval results, or production failure
2. **Hypothesize** — propose one specific, bounded change that might close the gap
3. **Change** — make exactly one change (a new skill, a prompt tweak, a rule, an eval, a tool)
4. **Test** — run a representative eval slice (not the full suite, just one good sample)
5. **Compare** — keep if measurably better, revert if worse or same
6. **Log** — record what changed, what the result was, and what was learned

### Rules

- Make **one change at a time**. One change, one eval slice, one decision.
- **Never do giant prompt surgery** without eval protection.
- If a change keeps the **same score**, prefer the **simpler system** (complexity is a hidden tax).
- Run **full eval periodically**, delta eval in between.
- Background branches are safer than modifying production instructions blindly.

---

## Capability Acquisition Ladder

When learning a new domain, climb this ladder:

1. **Solve once** — complete the task at least once with human support if needed
2. **Make repeatable** — capture the successful trajectory in memory, files, or a runbook
3. **Turn into a skill** — distill the SOP, domain knowledge, and trigger conditions into a reusable skill
4. **Turn repeated high-value work into a workflow** — add explicit phases, typed inputs/outputs, state tracking, checkpoints
5. **Turn reliability-critical workflows into specialized harnesses** — add deterministic rails, validation gates, templates, structured artifacts
6. **Add eval coverage** — add offline tests, scenario tests, and production-derived checks
7. **Add automation** — turn the reliable process into a repeatable operating unit with triggers or schedules, validation, approvals, evidence capture, monitoring, escalation paths
8. **Add monitoring and interventions** — watch for drift, failures, stalled work, cost spikes, or stale assumptions
9. **Add trust-based autonomy** — let the system do more on its own only after success is measured in production-like conditions
10. **Package the gain** — convert the successful pattern into a reusable asset (skill, workflow, harness, template, dashboard, eval, policy)

**The system becomes "most capable" not when it can improvise one impressive run, but when it can repeatedly absorb new domains through this ladder.**

---

## Background Compounding Loops

Run these ongoing loops that compound capability over time:

### 1. Task-Completion Loop
After each task: verify, log, learn, create reusable assets.

### 2. Eval Loop
Continuously improve the quality and coverage of evaluations.

### 3. Failure Loop
Convert repeated mistakes into tests, policies, or harness constraints.

### 4. External Intelligence Loop
Watch the outside world for better patterns, tools, models, protocols, and benchmarks.
- Run on a schedule (weekly minimum)
- Produce: digest of important changes, ranked list of ideas worth testing, new eval candidates, new skills/workflows worth creating
- Source priority: open-source architecture-bearing repos, model provider blogs, protocol ecosystems, benchmark updates, relevant research
- De-prioritize: thin API wrappers, generic chat shells, UI-only products without public architecture, trend-driven demos without reliability design
- Do NOT adopt external claims into core system without local eval, shadow run, or replay-based validation
- Implementation: use `scripts/agent1-external-intel.ps1` for bounded proposal-only runs and `Agent1_Project_Docs/EXTERNAL_INTELLIGENCE_LOOP.md` as the runbook. Digests live in `Agent1_Project_Docs/external-intelligence/`.

### 5. Workflow Mining Loop
Detect repeated successful trajectories and convert them into workflows or skills.

### 6. Proactive Operations Loop
Inspect projects, workspaces, and environments for:
- blocked work
- stale plans
- KPI drift
- unattended incidents
- dirty repos
- too many in-progress tasks

Convert signals into proactive goals without waiting to be told.

### 7. Cost Loop
Identify expensive steps and replace them with cheaper models, narrower subagents, cached artifacts, or deterministic code where possible.

### 8. Trust Loop
Promote autonomy when outcomes justify it. Tighten controls when they do not.

---

## Reliability Engineering

### The Reliability Math

For serious workflows, reliability compounds across steps. Think in "nines":
- 90% step reliability × 5 steps = 59% overall reliability (not good enough)
- 99% step reliability × 5 steps = 95% overall reliability

Each additional nine of reliability usually requires substantial engineering effort.

### Rules for Reliability-Critical Work

1. **If something must happen every time, codify it in deterministic rails**, not in a prompt.
2. **Complex workflows → specialized harnesses** (state machines with explicit phases, entry/exit criteria, artifact recording, resumability).
3. **Fixed plans for repeatable workflows**, dynamic plans for open-ended ambiguous work.
4. **Keep the orchestrator lean** — use isolated subagents for narrow work packages with tightly scoped context.
5. **Parallelize only where dependencies allow** — independent work can run in parallel; dependent steps must remain sequenced and gated.
6. **Every phase should leave a file or artifact trail** — workspace is the scratchpad and evidence store.
7. **Use structured schemas at phase boundaries** — classification outputs, extracted data, findings, summaries, approvals each validate against a schema.
8. **Add validation loops, not just final summaries** — validate extracted data before analysis, analysis against playbooks, generated outputs before publishing.
9. **Programmatic outputs beat free-form when consistency matters** — if the final deliverable must follow a template, generate it programmatically from validated intermediate data.
10. **Every side-effecting action needs an idempotency key and replay policy** — retries are not enough when the workflow can send messages, create records, or trigger deploys.

---

## Filesystem-First Project State

Treat each project folder as a durable operating system for that project.

**The rule:** Any compatible agent should be able to enter the folder, inspect files, understand current state, continue work, and leave the folder in a better state.

**Canonical project file pack:**
- `project.md` / `charter.md` — mission, goals, constraints
- `plan.md` — current work plan, updated continuously
- `tasks.md` — task list, updated as work progresses
- `knowledge.md` — accumulated domain knowledge, decisions, conventions
- `decisions.md` — architectural and design decisions with rationale
- `status.md` — current status, blockers, health indicators
- `handoff.md` — what needs attention next, for the next session or operator
- `artifacts/` — generated files, evidence, outputs
- `evals/` — tests and evaluation results
- `runs/` or `logs/` — execution traces and session logs

**Agent rules for this file pack:**
- read before acting
- update during execution, not only at the end
- write evidence and artifacts as produced
- record decisions when direction changes
- record failures when important attempts fail
- leave an explicit handoff with next actions, blockers, and open questions

---

## Success Metrics

Track these explicitly. Without measurement, improvement is guesswork.

### Core Metrics
- tasks completed
- tasks verified complete
- median time to completion
- cost per successful task
- intervention rate (how often a human had to step in)
- retry rate
- regression rate (old behavior breaking)

### Capability Metrics
- autonomy level by task type (supervised / guided / autonomous / trusted)
- eval pass rate (and pass under repeated runs)
- repeat-run stability
- memory reuse rate (how often past lessons actually apply)
- percentage of work completed proactively versus reactively
- percentage of work by domain: coding, browser, docs, operations, research, science, business

### Momentum Metrics (Leading Indicators)
- time from task completion to next queued task
- number of reusable assets created per milestone
- number of failures converted into evals or guardrails
- days since last eval improvement
- days since last new reusable skill or workflow
- number of proactive goals created
- percentage of runs that end with explicit next actions

---

## Anti-Patterns to Never Create

- a chat app that only pretends to be an operating system
- a single giant prompt that cannot evolve safely
- a fake multi-agent system with no real task boundaries
- a system that says tasks are complete without verification
- a system that forgets everything between sessions
- a system that cannot explain why it acted
- a system that cannot be paused, audited, or rolled back
- a system that optimizes demos over reliability
- a system that claims generality but only supports coding
- a system that depends on one proprietary runtime quirk
- a system that drifts into chat-only behavior instead of files, tasks, verification, and implementation

---

## Design Bets (Non-Negotiable Defaults)

When forced to choose a default, choose this:

1. **Start with a strong single-agent baseline.** Add more agents only when: work is embarrassingly parallel, a reviewer should be separate from author, task is long-running, or different machine environments are required.

2. **Separate open-ended reasoning from deterministic workflows.** Use workflows for routing, retries, approvals, timers, checkpoints, fan-out. Use open-ended agents for ambiguous reasoning, research, creative problem solving.

3. **Build a task graph, not a chat transcript with side effects.** Real system state = goals, tasks, events, artifacts, metrics, approvals, incidents, knowledge records. Chat is only one surface over that state.

4. **Per-project state file-first.** Markdown and repo-visible files as canonical per-project state. Databases for queueing, events, sessions, metrics, costs, approvals, operational indexing.

5. **Make verification a separate concern.** Do not let the same unverified step both produce and certify the result. Prefer: planner/executor → verifier → reviewer/approval.

6. **Make research mode and action mode distinct.** Research mode optimizes for breadth, citation quality, uncertainty tracking. Action mode optimizes for execution safety, approvals, state changes, rollback.

7. **Treat browser and desktop automation as real infrastructure**, not a gimmick. They need their own reliability, session persistence, replayability, and verification methods.

8. **Treat memory as a product surface, not an implementation detail.** Memory should be inspectable, editable, searchable, versioned.

9. **Favor typed interfaces and explicit schemas.** Tasks, tool calls, artifacts, decisions, and eval results all have structure. Free-text everywhere becomes impossible to debug.

10. **Prefer adapters over lock-in.** Wrap model providers, tools, browser backends, storage layers, and execution runtimes behind adapters so they can be swapped.

11. **Most gains come from better loops, not bigger prompts.** biggest improvements usually come from stronger task specs, better tools, cleaner verification, improved memory, clearer dashboards, tighter evals, better routing.

12. **Every repeated success should become a reusable asset.** Promote good trajectories into skills, playbooks, macros, workflows, or templates.

13. **Every repeated failure should become a test or guardrail.** If the system fails twice in a similar way, it should be much harder for that same failure to recur without detection.

14. **Optimize for the full loop before optimizing breadth.** Before expanding domains, make sure the system can reliably go from goal → task graph → execution → verification → memory update → learning. A wide but broken system is worse than a narrow but closed-loop one.

---

## Non-Negotiable Rules

- Prefer transparent files over hidden context
- Prefer task queues over vague collaboration stories
- Prefer measurable outcomes over self-reported success
- Prefer one-change eval loops over intuition-driven churn
- Prefer pull-based work claiming over brittle centralized control
- Prefer portable architectures over vendor lock-in
- Prefer durable memory over conversational memory
- Prefer bounded autonomy over blind autonomy
- Prefer graceful degradation over silent failure
- Prefer ongoing self-improvement over static scaffolds

---

## Self-Improvement Targets

The agent is allowed to improve:
- prompts
- skills and SOPs
- playbooks and rules
- tool adapters
- automations and workflows
- specialized harnesses
- dashboards and visibility
- task decomposition policy
- eval suites
- memory structure
- model routing and retry logic
- safety policies and guardrails

The agent must use stronger review before changing:
- approval policy
- security policy
- deployment paths
- destructive action rules
- trust thresholds
