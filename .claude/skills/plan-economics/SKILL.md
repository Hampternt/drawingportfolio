---
name: plan-economics
description: Use when writing a spec or plan for this repo, or when executing one with subagent-driven-development — sets plan sizing, who writes the plan, task risk classes and which of them earn an LLM review. Overrides parts of superpowers:writing-plans and superpowers:subagent-driven-development.
---

# Plan Economics

Measured on the 2026-08-01 fitness redesign (`docs/superpowers/plans/2026-08-01-fitness-redesign.md`):
one session executed its 11 tasks in **421 turns, context 41k → 517k, 139M
cache-read tokens**. Two things caused it, neither of which was the review loop:

- **65%** was one hour in which the controller implemented Tasks 4–11 itself,
  editing `nutrition.rs` / `db.rs` / `feed.html` at ~430k context per turn.
- **~20%** was the design corpus (17 DesignSync results) plus the plan text the
  controller had loaded before Task 1 and then re-read on all 350 later turns.

Reviews were 19% of wall clock. This skill is not about reviewing less; it is
about what the controller holds in context and who decides what.

## 1. One plan = one session = one deployable slice

4–6 tasks, ~800–1,200 lines. If the spec is bigger, write **several plans**, each
ending with software you could deploy. The fitness redesign should have been four:
auth gate + visual layer + targets ring · slots + week strip + copy-day · add
sheet + library · week view + desktop + docs.

End the session at the plan boundary. Start the next one from the SDD ledger —
resuming a 300k-context conversation costs more than starting fresh, because
every resume re-warms the whole context at cache-creation price.

## 2. The controller writes nothing

- **Design ingestion is delegated.** A subagent reads the mockups and writes
  `docs/design/<feature>/*.md`. The controller reads only the token table it
  needs to review the plan.
- **Plan writing is delegated.** Dispatch a plan-writer with the spec path and
  the file-structure decisions; it writes the plan file and returns the task
  list with a risk class per task. The controller never holds the plan text —
  the same file-handoff discipline SDD already uses for briefs and diffs.
- **Implementation is always delegated.** If you catch yourself running
  `python3 <<'PYEOF'` against a source file, you have become the implementer at
  ten times the context price. Dispatch instead.

## 3. Task risk classes

Every task in a plan carries a class. **The test for a class is mechanical: if
you can write the task's acceptance as a command whose output you can eyeball,
it is A or B. If you cannot, it is C.**

| class | what it is | gate |
|---|---|---|
| **A** — compiler/lint-gated | CSS sections, Askama template markup, route registration, static JS, docs | `./scripts/verify.sh` only. **No reviewer.** Askama compiles into the binary, so a broken template is a build error |
| **B** — logic whose tests are written in the plan | db functions, pure helpers (ring math, streak) with the plan naming the cases *and* expected values | `./scripts/verify.sh`. **No per-task reviewer** — the tests are the spec. Covered by the one review at plan end |
| **C** — logic tests cannot encode | locking and concurrency, auth/session gating, SSE broadcast ordering, migrations touching existing rows, anything with a cross-task invariant | `./scripts/verify.sh` **plus a task reviewer on a capable model**, every time |

Why the split is drawn there — what per-task LLM review actually did on this repo:

- `e99b723` nested `/*` in `game.css` silently dropped `.card-big` — reviewed, missed.
- `c72d614` nested palette entry broke `palette.js` syntax — reviewed, missed.
- `1e742d4` the room lock was not held across the mid-game join hook's broadcasts —
  the kind of bug only a reviewer or a human finds.

Both escapes are now caught in ~2 seconds by `tests/static_assets.rs` and
`node --check` inside `scripts/verify.sh`. **Anything a machine can decide is
never worth a model's attention; anything it cannot is worth a good model.**

Downgrading a task to A or B to skip a review is the one move that makes this
policy worse than the default. If the acceptance line does not exist, the task
is C.

## 4. Reviews under this policy

- Class C task → task reviewer immediately, as SDD describes (brief + report +
  review package, fix loop, scoped re-review).
- Class A/B tasks → no per-task review. They are covered by **one review of the
  whole plan's diff** at the end, on the most capable model — the final
  whole-branch review SDD already runs.
- Reviewers write their full report to a file and return the verdict plus
  one-line findings. Full reports pasted into the controller stay resident for
  the rest of the session.

## 5. What this overrides

Named explicitly so a future session reading both knows which governs:

- **superpowers:subagent-driven-development — "Never skip the task review."**
  Overridden for Class A and B tasks, for the reasons in §3. The class must be
  written in the plan; a task with no class defaults to C.
- **superpowers:writing-plans — "Bite-Sized Task Granularity" (2–5 minute steps,
  a separate step to run the test and watch it fail).** Replaced by §6. In Rust
  a missing function is a *compile* error, so a RED run before the function
  exists carries no information and pays a rebuild — the fitness plan had six.
  Write the test first; run it once, after the implementation.
- **superpowers:writing-plans — "No Placeholders" / full code in every step.**
  Kept for what is not derivable: SQL migrations, CSS token blocks, test cases
  with expected values, function signatures, route paths, magic numbers. Prose
  is enough for boilerplate that follows an existing pattern ("follow
  `post_card_html()`"). Verbatim code is what lets a task run on a cheap model —
  spend it where exactness matters, not on handler scaffolding.

superpowers:brainstorming is unchanged. It produces the spec; the only
difference is that each phase-plan is written fresh from the spec in its own
session rather than carried in context.

## 6. Task shape

Template: [plan-template.md](../../../docs/superpowers/plan-template.md).

Each task carries: a class, the files it touches, an Interfaces block (consumes
/ produces, exact signatures — this is what prevents the cross-task drift that
reviews exist to catch), its steps, and exactly one acceptance line:

```
**Acceptance:** `./scripts/verify.sh` — all green.
```

Browser checks do not belong in every task. Put them at two checkpoints per
plan: after the visual layer, and before the final review.

Run `cargo sqlx prepare` once per plan, before the final verification — not once
per migration task, unless that plan's commits need to build offline individually.
