# Agent1 Meta-Level Health Checks

This document defines how Agent1 monitors its own operating health — not just task outcomes, but whether its own process, loops, and memory are functioning correctly.

---

## Why This Exists

The main self-improvement loop improves **tasks**. This loop checks whether the **agent's own operating process** is healthy. Without this, a degraded agent can keep "improving" while its foundations crumble.

---

## Health Indicators

### Process Health (Answer these at the end of every session)

1. **Did I leave artifacts in the right places?** If files are scattered, missing, or unreadable, the filesystem-first contract is broken.
2. **Did my queues stay balanced?** If `now` was empty for long stretches or `blocked` grew without bound, momentum broke.
3. **Did verification actually verify?** If tasks were marked complete without running checks, the verification-first rule is broken.
4. **Did memory get written and read?** If lessons were learned but not stored, or stored but not retrieved, the memory layer is broken.
5. **Did I convert failures into assets?** If the same failure happened 3+ times without a new eval or guardrail, the failure loop is broken.
6. **Did I answer with strategy alone?** If I produced essays without files, the "build not describe" rule is broken.
7. **Is my context window healthy?** If sessions grew without summarization or state writes, context rot is setting in.

### Threshold Checks

| Indicator | Healthy | Warning | Critical |
|---|---|---|---|
| Sessions ending with no next action defined | 0% | 1-10% | >10% |
| Repeated failures without new eval added | 0 | 1-2 | >2 |
| Tasks marked complete without verification | 0 | 1 | >1 |
| Queue imbalance (any queue empty >2 consecutive sessions) | none | 1 | >1 |
| Memory writes with no subsequent retrieval | <10% | 10-30% | >30% |
| Artifacts written to wrong or non-standard locations | 0 | 1-2 | >2 |

---

## Meta-Eval Triggers

Run a meta-eval when any threshold enters **Warning** or **Critical**.

### Meta-Eval Protocol

1. **Inspect** — read the last 5 session artifacts, queue states, memory writes, and eval results.
2. **Classify** — which health indicator(s) triggered this eval?
3. **Hypothesize** — what is the simplest possible fix? (Usually: a missing rule, a misconfigured permission, a forgotten habit.)
4. **Change** — make exactly one change.
5. **Test** — observe the next 2-3 sessions for improvement.
6. **Commit or revert** — if fixed, document the lesson. If not, escalate to human.

---

## Meta-Level Improvement Targets

These are improvements to the agent's **operating process**, not to task outcomes:

- adding a missing habit to the core loop (e.g., "always write to tasks.md before starting")
- fixing a broken queue maintenance rule
- adding a health-check step to session startup
- tightening a permission that caused a stall
- adding an eval for a failure that bypassed existing checks
- pruning stale memory that is no longer retrieved
- simplifying a workflow that accumulated unnecessary complexity

---

## The Meta-Improvement Rule

**Never improve the agent's process without also adding an eval that catches the same regression.**

If you fix a health indicator and do not add a check for it, the fix will eventually rot.