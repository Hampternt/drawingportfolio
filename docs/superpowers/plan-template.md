# Plan template

Copy this shape when writing a plan for this repo. The rules behind it live in
`.claude/skills/plan-economics` — read that first; it overrides parts of
superpowers:writing-plans.

Size: 4–6 tasks, ~800–1,200 lines, ending in something deployable. A bigger spec
becomes several plans, not a bigger plan.

---

```markdown
# [Feature Name] Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILLS: `plan-economics` (this repo's
> task classes, review policy and plan sizing) then
> superpowers:subagent-driven-development to execute task-by-task.

**Goal:** [one sentence]

**Architecture:** [2-3 sentences]

**Slice:** [what works and is deployable when this plan is done, and what the
next plan in the series picks up]

## Global Constraints

[Project-wide requirements with exact values copied verbatim from the spec:
version floors, naming and copy rules, token names, platform requirements. Every
task's requirements implicitly include this section.]

**Verification for every task:** `./scripts/verify.sh` — all green, output quoted
in the report.

**Browser checkpoints:** after Task [N] (visual layer) and before the final
review. Not per task.

---

### Task N: [Component Name]

**Class:** A (compiler/lint-gated) | B (logic, tests specified below) | C (logic
tests cannot encode — reviewer required)

**Why this class:** [one line. For A/B, the acceptance command is the argument.
For C, name what a machine cannot decide: lock ordering, session gating,
broadcast ordering, a migration over existing rows, a cross-task invariant.]

**Files:**
- Create: `exact/path/to/file.rs`
- Modify: `exact/path/to/existing.rs:123-145`
- Test: `exact/path/to/test.rs`

**Interfaces:**
- Consumes: [exact signatures this task uses from earlier tasks]
- Produces: [exact function names, parameter and return types later tasks rely
  on. The implementer sees only this task — this block is how it learns the
  names its neighbours use.]

- [ ] **Step 1: [action]**

[Verbatim code ONLY where exactness is not derivable: SQL migrations, CSS token
blocks, test cases with their expected values, function signatures, route paths,
magic numbers. Prose for what follows an existing pattern — say which pattern.]

- [ ] **Step 2: [action]**

[No separate "run it and watch it fail" step when the function does not exist
yet — that failure is a compile error and carries no information. Write the test
first, run it once after the implementation.]

- [ ] **Step 3: Commit**

```bash
git add [paths]
git commit -m "feat(scope): [subject]"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---
```

## Before the plan is done

- Every task has a class, and every A/B task's acceptance is a real command.
- `cargo sqlx prepare` appears once, before the final verification — not once per
  migration task.
- Types, signatures and names used in later tasks match what earlier tasks
  produce.
- Every spec requirement maps to a task.
