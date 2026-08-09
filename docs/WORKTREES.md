# Worktrees, branches and work streams

The live index. **This file lives on `master` so it is always reachable** — an
earlier orientation doc was written onto a feature branch and became invisible
from everywhere else, which is the specific failure this file exists to avoid.

Update this file when a stream starts, lands or is abandoned.

| Scope | Authority |
| --- | --- |
| Current worktrees, branches, streams | **this file** |
| What happened during the 2026-08-09 cleanup | `docs/HANDOFF-2026-08-09.md` (on `feat/last-call`) — a dated forensic record, not live state |
| Last Call's own progress | `docs/superpowers/plans/2026-08-06-last-call-STATUS.md` (on `feat/last-call`) |

---

## 1 · Conventions

**Worktrees live outside the repo**, as siblings under
`~/projects/drawingportfolio.worktrees/<name>`.

Not in `.claude/worktrees/`. That directory is where the `EnterWorktree` tool
puts short-lived session worktrees, and the harness offers to delete them when
the session ends. A stream you will return to over weeks should not be one
keypress from removal. Short-lived task worktrees in `.claude/worktrees/` are
fine and expected — the rule is about longevity, not tooling.

**Branch names say what the work is, not how it was made.**

`feat/<stream>` for feature streams, `fix/<thing>` for single-fix branches.
Names like `master-3` (the third master-ish scratch branch) and
`worktree-artportfolio-visual-layer` (auto-named by the tool that created it)
were renamed on 2026-08-09 for this reason — neither said what it contained,
so `git branch` read as noise.

**The worktree directory is named after the branch**, minus the `feat/`
prefix, so `git worktree list` and `git branch` line up by eye.

---

## 2 · Streams

### A · Last Call — active

The drinking game's third mode, after Ring of Fire and 3 Man.

| | |
| --- | --- |
| Worktree | `~/projects/drawingportfolio.worktrees/last-call` |
| Branch | `feat/last-call` (tracks `origin/feat/last-call`) |
| Touches | `drinkinggame/` only |
| Status | Slice 1 is four plans, `A → A-vis → A2 → B`. **A, A-vis and A2 are done.** Playable up to the setup form; the beat state machine's transitions are stubbed by design. |
| Next | **Write Plan B.** It is not yet written. Read "Carried out of Plan A2" in the STATUS card first — `MAX_SEATS` enforcement and the missing `/lastcall/end` route are Plan B's to own. |

```bash
cd ~/projects/drawingportfolio.worktrees/last-call
./scripts/verify.sh
cargo run -p drinkinggame          # standalone on :3001
#   style guide, no login:  http://localhost:3001/lastcall/preview
#   the game:               http://localhost:3001/room/{CODE}
```

### B · Artportfolio visual layer — awaiting spec approval

A design-system-driven redesign of `/artportfolio`.

| | |
| --- | --- |
| Worktree | `~/projects/drawingportfolio.worktrees/artportfolio-visual-layer` |
| Branch | `feat/artportfolio-visual-layer` (tracks `origin/feat/artportfolio-visual-layer`) |
| Touches | `docs/` only so far |
| Status | **Docs only** — 2 commits, ~6800 lines, all under `docs/`. The design-system bundle in `docs/design/artportfolio-redesign/` plus `docs/superpowers/specs/2026-08-09-artportfolio-visual-layer-design.md` (marked *pending user approval*). No implementation started. |
| Next | Approve or revise the spec, then plan slice 1. |

Independent of Last Call — different crates, no overlap.

### C · Portfolio drawing tasks — dormant, being wound up

| | |
| --- | --- |
| Worktree | none |
| Branch | `fix/stale-image-id` → **PR #6, open** |
| Status | One rescued commit. `./scripts/verify.sh` green. |

`fix/stale-image-id` carries commit `82e0053` (2026-07-04), which guards
`insert_drawing_task` against a stale `image_id` and stops the S3 object being
deleted when the transaction fails. It was stranded for five weeks on
`claude/portfolio-drawing-tasks-9sazgl` and never reached `master`.

Once PR #6 merges, `claude/portfolio-drawing-tasks-9sazgl` has nothing unique
left and can go — see the cleanup queue.

---

## 3 · Cleanup queue

Deliberately manual: remote deletion is not recoverable from the reflog. Each
tip below is an ancestor of `master`, so any ref can be recreated locally with
`git branch <name> <sha>` — the SHAs are recorded for exactly that reason.

**Merged, verified 0 commits ahead of `master` with an empty file-level diff:**

```bash
git push origin --delete claude/add-portion-sizes-SnZ3a               # e0b806a
git push origin --delete claude/fitness-barcode-camera-reload-bj3i88  # 7b9cf9e
git push origin --delete claude/claude-md-docs-cfwpf6                 # f153acb
```

**After PR #6 merges** — and not before, it is the only copy of `82e0053`
outside the PR:

```bash
git branch -D claude/portfolio-drawing-tasks-9sazgl                   # 66f8485
git push origin --delete claude/portfolio-drawing-tasks-9sazgl
```

> **Before deleting any branch that merely looks stale, run
> `git diff master...<branch>`.** Ahead/behind counts cannot tell *superseded*
> from *pending*. `9sazgl` read as 4 commits unmerged and looked live, but
> three were content-identical to work already on `master` by another path and
> only one was real. A file-level diff is the only check that distinguishes
> them.

### Two orphaned stashes

Not touched — they need a human decision, and one is large enough to matter.

```
stash@{0}  2026-08-01  WIP on master-1: 5eed2bb docs: fitness redesign —
                       imported Claude Design mockups + implementation plan
                       11 files, +1109 −374  (nutrition.rs, tasks.rs, feed.rs,
                       auth.rs, storage.rs, hub.rs, …)
stash@{1}  2026-03-25  Auto stash before merge of master and
                       feature/drawing-portfolio — Cargo.lock, +7
```

`stash@{1}` is a 7-line `Cargo.lock` fragment from a merge a year ago; drop it.

`stash@{0}` is the one to look at. Its base branch `master-1` was deleted on
2026-08-01, so the stash ref survives but nothing points at its context any
more. It is dated the same day as the fitness-redesign plan import and predates
that redesign landing, so it is **probably a superseded first attempt at work
that has since shipped** — but 1109 insertions across the nutrition and tasks
routes is too much to drop on "probably".

```bash
git stash show -p 'stash@{0}' | less    # inspect before deciding
```

---

## 4 · Known debts

- **`cargo test` runs 52 of 229 tests.** The root `Cargo.toml` is both a package
  and the workspace root, and in that layout cargo defaults to the *current
  package* — so `drinkinggame`'s 177 tests are silently skipped. CLAUDE.md's
  "`cargo build` / `cargo test` at the root cover both" is wrong for `test`.
  **`./scripts/verify.sh` is the gate precisely because it passes
  `--workspace`.** Never accept a bare `cargo test` as evidence.
- **CLAUDE.md's test counts are correct for `master` and will break on merge.**
  It says 229 workspace / 100 drinkinggame / 77 http; `master` measures exactly
  48 + 4 + 100 + 77 = 229. On `feat/last-call` the numbers are **327 / 145 /
  130**, so whoever merges Last Call owns that update.
- **`…-last-call-STATUS.md:180` says "19 clippy warnings"** where CLAUDE.md says
  17 distinct. Both numbers are real and measure different things — 19 raw lines
  = 17 distinct + 2 rollup summaries — but as written it points the next session
  at the wrong baseline.
- **No SDD ledger for Plan A-vis or Plan A2.** `.superpowers/sdd/` holds only
  `2026-08-06-last-call-plan-a/`. The working rules point a session at
  `<plan-basename>/progress.md` to learn which tasks are done; for two of the
  three finished plans that file does not exist.
- **Stale `master-3` references on `feat/last-call`.** The 2026-08-09 rename
  changed both the branch *and* the worktree directory, so three of these are
  broken paths, not just stale names. Fix in one pass on that branch:

  | File | Lines | |
  | --- | --- | --- |
  | `docs/HANDOFF-2026-08-09.md` | 49, 170 | **broken paths** — `worktrees/master-3` → `worktrees/last-call` |
  | `docs/HANDOFF-2026-08-09.md` | 65, 87 | stale branch name |
  | `docs/HANDOFF-2026-08-09.md` | 53 | the §2 worktree-location paragraph now states the **opposite** of the policy — rewrite, don't patch |
  | `docs/superpowers/plans/2026-08-06-last-call-STATUS.md` | 191 | **broken path** |
  | `docs/superpowers/plans/2026-08-06-last-call-STATUS.md` | 10 | stale branch name |

  **Leave `HANDOFF` lines 14, 16 and 110 saying `master-3`** — they describe
  what happened on the day and rewriting them would make the doc lie about the
  past.

- **One manual browser check is owed** (Plan A-vis checkpoint 2, items 6–7):
  open `/lastcall/preview`, press every REPLAY and watch a flight actually
  travel, then repeat under devtools' *Emulate CSS `prefers-reduced-motion:
  reduce`*. Verified structurally, never watched. **Needs human eyes** — an
  automation tab stays backgrounded, which freezes animations. Five minutes.
