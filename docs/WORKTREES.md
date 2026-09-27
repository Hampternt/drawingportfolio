# Worktrees, branches and work streams

The live index. **This file lives on `master` so it is always reachable.**
Update it when a stream starts, lands or is abandoned. Dated forensic records
(what happened during a cleanup or merge) go in `docs/HANDOFF-<date>.md`
files, not here — the detailed histories of the 2026-08 streams were trimmed
from this file on 2026-08-12; `git log docs/WORKTREES.md` recovers them.

## Conventions

- **Worktrees live outside the repo**, as siblings under
  `~/projects/drawingportfolio.worktrees/<name>`. Not in `.claude/worktrees/`
  — that is for short-lived session worktrees the harness offers to delete.
- **`dev` is the staging branch** (decision 2026-08-13): it tracks `master`
  and is where changes accumulate before merging into `master`. Merge
  `dev → master` locally to release; `dev` is never deleted after a merge.
- **Branch names say what the work is**: `feat/<stream>` for feature streams,
  `fix/<thing>` for single fixes.
- **The worktree directory is named after the branch** minus the `feat/`
  prefix, so `git worktree list` and `git branch` line up by eye.
- Before deleting a branch that looks stale: `git diff master...<branch>`,
  then check the files it names exist on `master`. Commit counts measure
  divergence; only content measures loss.

## Streams

- **`feat/portfolio-review`** (started 2026-09-28, from `master` @ 0c0e99d,
  in the main checkout — no separate worktree): works through the 2026-09-27
  portfolio review, `docs/handoffs/2026-09-27-portfolio-review.md`, in the order
  it suggests. Step 1 records rulings 2 and 3 in the Hub/Admin/CV container and
  fixes the status lines the review's mapping found stale.

- **`feat/hub-admin-cv-docs`** — **Packs 1, 2 and 6 LANDED 2026-09-27**,
  squash-merged to `master` as Hampternt/drawingportfolio#20 and deployed. The
  branch was deleted from `origin` with the merge, because its history carries
  personal data the squash keeps off `master`. The container stays open: the
  Claude Design handoff — the Hub and Admin onto the Hampter Design System,
  plus two new sections, a **CV page (`GET /cv`)** and an employer-facing
  "How it works" page at `/docs`. Fitness, artportfolio and drinks are
  deliberately untouched in content; they do inherit the shared header and
  command palette. Container manifest:
  `docs/manifests/2026-09-08-hub-admin-cv-docs.md` — seven packs, planned one
  level deep. Design handoff: `docs/design/hub-admin-cv-docs/` — **the CV
  design is `CV.dc.html`** (Norwegian, "Utvikler — interne digitale verktøy og
  automatisering"). Pack 6 first landed on the feature branch 2026-09-25 from
  `claude/cv-worktree-completion-lq2wps` via Hampternt/drawingportfolio#19,
  with ruling 1 taken by the user and the CV text revised by them. **Packs 3,
  4, 5 and 7 are not started.** Rulings 2 and 3 were taken 2026-09-27, so no
  ruling blocks Packs 3–5: Pack 3 can start, and 4 and 5 follow it. Ruling 4
  (does the CV link to `/docs`?) is open and blocks Pack 7.

- **`claude/secret-hitler-game-jg8xqs`** — **LANDED 2026-09-07**, merged to
  `master` via Hampternt/drawingportfolio#15 (with the CI fixes #16 and #17).
  **Backroom**, a fourth game for the `/drinks` room shell — a hidden-role
  game, mechanically Secret Hitler, re-themed, 5–10 players. Manifest:
  `docs/manifests/2026-09-07-backroom.md`. Drink-free in v1; the drinking
  variant ("Secret Sippler") is a later mode and is constrained by the
  leaderboard role-oracle finding recorded in the manifest and in
  `sh_theme.rs`. The remote branch is fully merged (ancestry checked
  2026-09-28) and can be deleted.

- **`claude/crate-counting-android-app-ewwc19`** — **LANDED 2026-09-07**,
  merged to `master` via Hampternt/drawingportfolio#14 (started 2026-08-25).
  The remote branch is fully merged (ancestry checked 2026-09-28) and can be
  deleted. The **Sorting & Loading Assistant**, a new `/sorting` section. A generated crate-sort/van-load
  plan is pasted in as JSON; what comes out is a board drawn from the van's own
  rear-right corner that the driver works the load on, plus a panel of sanity
  checks that re-derive the plan's arithmetic rather than trusting it. Built for
  an Android tablet on the warehouse floor: every move is optimistic, appends to
  a log, and queues in `localStorage` when there is no signal.

  The loading rules are one copy with three consumers —
  `static/sorting-{runtime,model,board}.js` and `templates/sorting/markup/` are
  the app's, and the demo in `docs/design/sorting-live/` and the design document
  in `docs/design/van-loading-board/` are built from them. Their 458 checks run
  the served files and `./scripts/verify.sh` runs the checks. Migrations 023 and
  024, `src/routes/sorting.rs`, `static/sorting-*.js`, `templates/sorting/`.
  Source spec: the user's `sortingwebsitespec.md` (companion to
  `delivery-loading-reference.md` and `van-loading-plan-generator.html`, neither
  of which is in this repo).
- **`feat/last-call-refinement`** — **CLOSED 2026-08-29**, branch deleted
  (fully merged; first batch — clock removal, beat restructure, test play
  mode, screen declutter + installable app — hit `master` 2026-08-13).
  The Reveal/Resolve visual passes and the table-screen card-play design
  remain unstarted; recreate a branch from `dev` when they resume.
- **`feat/lc-challenge-cards`** — **Pack 1 LANDED 2026-08-14** (challenge
  engine, vote flow, bare UI), since on `master`; the container is idle, with
  Packs 2 and 3 not started. Real-life party challenges as Last Call cards
  (vote-judged duels, solo dares, social penalties, challenge HUD). Its cards
  ship at `copies: 0`, so live games draw none yet. Manifest:
  `docs/manifests/2026-08-14-lc-challenge-cards.md`.
- **`feat/fitness-today-overhaul`** — **LANDED** (merged to `dev` 2026-08-18,
  since released to `master`). Rebuilt the `/fitness` Today screen from the
  design handoff in `docs/design/fitness-today-overhaul/`: one-tap quantity
  fractions on the logged row, one-tap re-logging and batch meals, day-level
  macro composition, phone layout with a bottom action bar. Manifest with
  ledgers, walkthroughs and four recorded deviations:
  `docs/manifests/2026-08-17-fitness-today-overhaul.md`. Branch and worktree
  removed 2026-08-29; the decoy harness session worktree
  (`claude/fitness-tracker-multi-user-54fb9b`, cherry-picked older copies of
  the docs commits) was verified superseded and deleted with its branch.
- **`feat/multi-user-fitness`** — **LANDED 2026-08-17**, merged `dev → master`
  and deployed. Multi-user container: several people each with their own
  fitness log over a shared food catalog, logging in by name + PIN alongside
  the owner's passkeys; art-portfolio admin became a grantable permission
  rather than a synonym for "logged in". All four packs complete; the manifest
  `docs/manifests/2026-08-16-multi-user-fitness.md` holds the ledgers, the
  populated-database upgrade test and the no-rollback note. Branch and
  worktree removed 2026-08-29.
  Deploy-time discovery worth carrying forward: the server's nginx config was
  still the 2026-07-28 file and carried `listen [::]` lines the repo had
  dropped in April — read the `deploying` skill before copying `nginx.conf`.

## Reminders

- **Artportfolio slices 4–5 remain scoped** in the slice-1 spec
  (`docs/superpowers/specs/2026-08-09-artportfolio-visual-layer-design.md`):
  slice 4 is the multi-upload tray (must return the `#art-head-label` OOB
  fragment alongside each new card, fixing the stale-head debt), slice 5 is
  select mode + batch actions. A fresh worktree has no `.env` — sqlx macros
  need `export DATABASE_URL=sqlite:portfolio.db` whenever queries change.
- **Rail counts going stale after a card edit** is slice 3's accepted
  trade-off, not slice-4 debt. Residual slice-3 minors (parked with rulings):
  OOB swap resets a half-typed new-collection input; keyboard focus lost to
  `<body>` when a rail pill's own swap destroys it; collection create/delete
  responses render the rail with `PostFilter::default()`; `patch_post` runs
  caption+tags non-atomically; one tie-order-dependent test; no unknown-id
  404 test on the two GET fragment routes.
- ~~Two orphaned stashes need a human decision.~~ **Resolved 2026-08-12** —
  both inspected and dropped. `stash@{1}` was a 7-line pre-dependency
  `Cargo.lock`. `stash@{0}`'s 1109 insertions were an uncommitted `cargo fmt`
  pass (verified by reproducing it: base commit + `cargo fmt` matched 10 of
  11 files exactly) plus one rejected experiment — making the nutrition
  routes public via `OptionalAuth`, the opposite of the 2026-08-01
  session-gating decision that shipped.
- **`.superpowers/sdd/` destroys its own ledgers**: its `.gitignore` is `*`,
  so SDD progress files are worktree-local and die with worktree cleanup —
  no ledger survives for any completed plan. Commit ledgers or store them
  outside the repo before the next SDD run.
- **One manual browser check owed** (Last Call, plan A-vis checkpoint 2):
  open `/lastcall/preview`, press every REPLAY, watch a flight travel, then
  repeat under emulated `prefers-reduced-motion: reduce`. Needs human eyes —
  automation tabs background and freeze animations. Five minutes.
- **Never accept a bare `cargo test`** — it runs 219 of 744 tests (root
  `Cargo.toml` is both package and workspace root). `./scripts/verify.sh`
  is the gate; details in CLAUDE.md.
