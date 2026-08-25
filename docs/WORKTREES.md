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

- **`claude/crate-counting-android-app-ewwc19`** (started 2026-08-25, from
  `master`, in the main checkout — no separate worktree): the **Sorting &
  Loading Assistant**, a new `/sorting` section. A generated crate-sort/van-load
  plan is pasted in as JSON; the board that comes out is a pick checklist, a
  live van diagram and a panel of sanity checks that re-derive the plan's
  arithmetic rather than trusting it. Built for an Android tablet on the
  warehouse floor: ticks are optimistic and queue in `localStorage` when there
  is no signal. Migration 023, `src/routes/sorting.rs`, `static/sorting.js`,
  `templates/sorting/`. Source spec: the user's
  `sortingwebsitespec.md` (companion to `delivery-loading-reference.md` and
  `van-loading-plan-generator.html`, neither of which is in this repo).
  Open — not yet merged to `dev` or deployed.
- **`feat/last-call-refinement`** (started 2026-08-13, from `dev`): refining
  the recently released Last Call game — many small changes expected, worked
  one issue at a time, each an item committed on this branch; merges into
  `dev` when a coherent batch lands, then `dev → master`. First batch
  (clock removal, beat restructure, test play mode, screen declutter +
  installable app) merged to `master` 2026-08-13; the stream stays open for
  the Reveal/Resolve visual passes and the table-screen card-play design.
- **`feat/lc-challenge-cards`** (started 2026-08-14, from `dev`, in the
  main checkout — no separate worktree): challenge-card container — real-
  life party challenges as Last Call cards (vote-judged duels, solo dares,
  social penalties, challenge HUD). Three packs; manifest:
  `docs/manifests/2026-08-14-lc-challenge-cards.md`.
- **`feat/fitness-today-overhaul`** — **MERGED TO `dev` 2026-08-18**, not yet on
  `master` and not deployed. Rebuilt the `/fitness` Today screen from the design
  handoff in `docs/design/fitness-today-overhaul/`: quantity moved onto the
  logged row as one-tap fractions of the food's own basis, one-tap re-logging
  and batch meals, day-level macro composition, and a phone layout with a
  bottom action bar at a 900px breakpoint. All five packs complete; the
  manifest `docs/manifests/2026-08-17-fitness-today-overhaul.md` holds the
  ledgers, the per-pack walkthroughs and four recorded deviations from the
  design. The worktree
  `~/projects/drawingportfolio.worktrees/fitness-today-overhaul` can be removed.
  **Decoy warning, still live:** the planning ran in a harness session worktree,
  `.claude/worktrees/fitness-tracker-multi-user-54fb9b` on branch
  `claude/fitness-tracker-multi-user-54fb9b`, which still holds the same two docs
  commits under *different* SHAs (they were cherry-picked, so there is no ancestry
  link). That worktree is **abandoned** — the copies now on `dev` are canonical.
  Delete it and its branch.
- **`feat/multi-user-fitness`** — **LANDED 2026-08-17**, merged `dev → master`
  and deployed. Multi-user container: several people each with their own
  fitness log over a shared food catalog, logging in by name + PIN alongside
  the owner's passkeys; art-portfolio admin became a grantable permission
  rather than a synonym for "logged in". All four packs complete; the manifest
  `docs/manifests/2026-08-16-multi-user-fitness.md` holds the ledgers, the
  populated-database upgrade test and the no-rollback note. The worktree
  `~/projects/drawingportfolio.worktrees/multi-user-fitness` can be removed.
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
