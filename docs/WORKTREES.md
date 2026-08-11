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

### B · Artportfolio visual layer — active

A design-system-driven redesign of `/artportfolio`.

| | |
| --- | --- |
| Worktree | `~/projects/drawingportfolio.worktrees/artportfolio-visual-layer` |
| Branch | `feat/artportfolio-visual-layer` (tracks `origin/feat/artportfolio-visual-layer`) |
| Touches | `docs/`, `src/`, `static/`, `templates/`, `.sqlx/` |
| Status | **Slices 1 and 2 complete** — slice 1 on 2026-08-10 (`4c786a2`), slice 2 on 2026-08-11 (`cadabbc`). `./scripts/verify.sh` is green at 312 tests. Pushed; **not merged to `master`**. Slice 2's browser checkpoint has since been run and recorded (`1862428`); a permalink stretch bug it found is fixed (`b39f5c0`). **Slice 3 started 2026-08-12** — spec committed (`b32707e`, `docs/superpowers/specs/2026-08-12-artportfolio-collections-tags-design.md`); scope: collections + tags + full filter rail + per-card assignment editing; mobile filter sheet deferred; collection create+delete, no rename. |
| Next | Execute slice 3's plan(s) (`docs/superpowers/plans/2026-08-12-artportfolio-collections-tags*.md` on the branch). Slice 4 must code against the frozen `#art-head-label` OOB seam named in the spec. Any work here needs `export DATABASE_URL=sqlite:portfolio.db` — **there is no `.env` in this worktree**, and the sqlx macros need a live DB whenever the queries change. |

No ahead/behind counts live in this card on purpose — they are stale the moment
anything moves. Run `git rev-list --left-right --count master...HEAD` if you
need them.

Slice 1's split, so the next session does not re-derive it:

| Plan | Scope | State |
| --- | --- | --- |
| **A** — `docs/superpowers/plans/2026-08-10-artportfolio-visual-layer-plan-a.md` | Self-hosted fonts + icons · migration 012 (`image_width`/`image_height`) + sqlx regen · the `style.css` section under `body.art-page` · `art-page` derivation in **both** `base.html` and `admin.html` · the Askama templates that replace `post_card_html()` | done, checkpointed |
| **B** — `docs/superpowers/plans/2026-08-10-artportfolio-visual-layer-plan-b.md` | `get_posts_page(q)` + `count_posts` + LIKE escaping · `feed.rs` `q`/`last_month` + month grouping · `filter_rail.html` + `artfeed.js` · page-head counts | done, checkpointed |

B was **not** safely parallel with A — both rewrite the same `feed.rs`, the same
templates and the same `style.css` section. They ran in sequence.

**Debt slice 1 leaves, deliberately.** The page head goes stale by one after an
admin upload: the composer's response is a single `PostCardTemplate` swapped
`afterbegin` into `#feed`, so it carries no OOB label, and Plan B is what turned
the head into a number for that staleness to show in. It self-corrects on the
next page load or search. Slice 4 replaces the composer with the multi-upload
tray and owns the fix — return the OOB label alongside the new card, exactly as
`htmx_posts` does for page 0. Recorded in `3ace46d` and in Plan B's checkpoint.

Both plans' browser checkpoints record what they could **not** verify:
`resize_window` never changes `innerWidth` in this environment, so the 900px and
390px bands were verified through the CSSOM but **have not been seen rendered**;
key events were synthetic, not natively delivered; and Dark Reader repaints
colour, so only geometry and fonts were measured.

Two places execution contradicted the spec. Both are load-bearing:

- **`post_card_html()` has three callers, not the two the spec names.**
  `src/routes/admin.rs:211` returns it as the upload response when
  `source == "gallery"`. Converting only the two in `feed.rs` still compiles —
  the symptom is a legacy `.post-card` appearing in a feed of `hm-post` cards
  after a real upload, which no test and no build catches.
- **`feed.html`'s `{% if is_admin %}` admin composer must survive this slice.**
  The spec's template description omits it; the multi-upload tray that replaces
  it is slice 4, so removing it now strips upload from the page for two slices.

#### Slice 2 — the visibility model, complete 2026-08-11

Spec `docs/superpowers/specs/2026-08-11-artportfolio-visibility-model-design.md`,
plan `docs/superpowers/plans/2026-08-11-artportfolio-visibility.md` (one plan,
8 tasks — it exceeds `plan-economics` §1 sizing at the user's direction).

`public` / `unlisted` / `hidden`, migration 013, enforced on all four
post-reading routes by a required `Viewer` parameter, plus a permalink
(`GET /artportfolio/{id}`), a `PATCH` route, per-card badges and controls, split
head counts and a `?visitor=1` preview.

**The bug it actually fixed was pre-existing:** `htmx_posts` and `api_posts`
never extracted `OptionalAuth`. Harmless while every post was public; the moment
visibility existed, page 0 would render filtered and the first *Load more* would
hand back everything.

Two trade-offs accepted in the spec, so slice 3 does not refile them as bugs:

- **Unlisted is enumerable.** Post ids are sequential, so unlisted means "not in
  the feed and not in the API", not "secret". The upgrade if that ever changes is
  additive: a `share_token` column and `/artportfolio/p/{token}`.
- **`/admin` shows no badge**, because the dashboard still renders through
  `admin_post_card_html()`, the legacy format string a later slice migrates.

**No browser checkpoint was run for slice 2** — the whole slice was verified by
312 passing tests, including sabotage checks proving the auth gate, the page-1
filter and the session helper each fail when broken. The visual work (three badge
tones, the hover cluster, the 900px always-visible fallback) is **unseen** and
needs human eyes.

Independent of Last Call — different crates, no overlap. Slices 3–5 (collections
+ tags, multi-upload tray, select mode + batch actions) remain scoped in slice
1's spec.

### C · Portfolio drawing tasks — **closed 2026-08-09**

Landed as **PR #6** (`be7cfdc`). Commit `82e0053` (2026-07-04) guards
`insert_drawing_task` against a stale `image_id` and stops the S3 object being
deleted when the transaction fails. It sat unmerged for five weeks on
`claude/portfolio-drawing-tasks-9sazgl` and existed only on one machine until
the day it landed. Both branches are gone; nothing is owed.

---

## 3 · Cleanup queue

**Open — `docs/index-refresh`, merged 2026-08-10.** Its one commit `e8610b3`
fast-forwarded into `master`, so the branch and its worktree have nothing
unique left:

```bash
git worktree remove .claude/worktrees/index-refresh
git branch -d docs/index-refresh
git push origin --delete docs/index-refresh
```

That worktree also sits in `.claude/worktrees/`, which §1 reserves for
short-lived session worktrees — correct for what it was, and the reason it is
safe to remove now rather than something to preserve.

Everything below was executed on 2026-08-09. Kept as a record of what was
checked, because the *method* is the reusable part.

Six remote branches deleted, each verified before removal:

| Branch | Tip | Why it was safe |
| --- | --- | --- |
| `claude/add-portion-sizes-SnZ3a` | `e0b806a` | 0 ahead, empty file diff |
| `claude/fitness-barcode-camera-reload-bj3i88` | `7b9cf9e` | 0 ahead, empty file diff |
| `claude/claude-md-docs-cfwpf6` | `f153acb` | 0 ahead, empty file diff |
| `claude/portfolio-drawing-tasks-9sazgl` | `66f8485` | see below |
| `fix/stale-image-id` | `53882c7` | merged, PR #6 |
| `docs/worktree-index` | `89ebdec` | merged, PR #7 |

> **Before deleting any branch that merely looks stale, run
> `git diff master...<branch>` — and then check the files it names actually
> exist on `master`.** Ahead/behind counts cannot tell *superseded* from
> *pending*.
>
> `9sazgl` is the worked example. It read as **4 commits ahead** with a
> non-empty diff naming ~3000 lines of plan and spec documents — which looks
> like unmerged work. Every one of those documents was already on `master`,
> having landed via PR #4 by a different path. The only object unique to the
> branch was a stray `160000` gitlink from someone `git add`-ing a worktree
> directory. Its one piece of real value, `82e0053`, had been rescued onto
> `fix/stale-image-id` first — the branch was only deleted after that merged.
>
> Commit counts measure divergence. Only content measures loss.

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

- **`cargo test` runs 53 of 230 tests.** The root `Cargo.toml` is both a package
  and the workspace root, and in that layout cargo defaults to the *current
  package* — so `drinkinggame`'s 177 tests are silently skipped. **
  `./scripts/verify.sh` is the gate precisely because it passes `--workspace`.**
  Never accept a bare `cargo test` as evidence.
- **Test counts, measured on `master` at `0eb05b0`:** 49 + 4 + 100 + 77 =
  **230**. PR #6 added one. On `feat/last-call` the numbers are **327 / 145 /
  130**, so whoever merges Last Call owns that update.

  `feat/artportfolio-visual-layer` moves them too, and **already carries its
  own CLAUDE.md correction** (`e67cf49`). Two branches now edit the same
  counts, so whichever merges second will conflict on that line — resolve it by
  re-measuring, not by taking either side.

  Counts in prose go stale on almost every merge. If you touch this line, get
  the number from `cargo test --workspace`, never from another document.
- ~~`…-last-call-STATUS.md` says "19 clippy warnings" where CLAUDE.md says 17
  distinct.~~ **Fixed 2026-08-09** (`d8b0a9a`). Both numbers were real and
  measured different things; the STATUS card now states **17 distinct** and
  explains that the raw 19 includes two per-target rollup summaries. Confirmed
  by measurement: `cargo clippy --workspace --all-targets` prints 19 lines.
- **No SDD ledger for Plan A-vis or Plan A2.** `.superpowers/sdd/` holds only
  `2026-08-06-last-call-plan-a/`. The working rules point a session at
  `<plan-basename>/progress.md` to learn which tasks are done; for two of the
  three finished plans that file does not exist.
- ~~Stale `master-3` references on `feat/last-call`.~~ **Fixed 2026-08-09**
  (`d8b0a9a`). The rename changed the worktree directory as well as the branch,
  so three were broken `cd` paths rather than stale names. The `HANDOFF` §1/§5
  mentions were deliberately kept — they record what happened on the day.
- **One manual browser check is owed** (Plan A-vis checkpoint 2, items 6–7):
  open `/lastcall/preview`, press every REPLAY and watch a flight actually
  travel, then repeat under devtools' *Emulate CSS `prefers-reduced-motion:
  reduce`*. Verified structurally, never watched. **Needs human eyes** — an
  automation tab stays backgrounded, which freezes animations. Five minutes.
