# Handoff — portfolio review, manifests and what is left (2026-09-27)

Picks up from a cloud session. Everything needed to continue locally is in
this file and its appendix,
[`2026-09-27-review-manifest-proposals.md`](2026-09-27-review-manifest-proposals.md).

## Where things stand

- **`master` @ `8f7ff44`, deployed.** PR #20 was **squash-merged**: Pack 1
  (shared dark shell), Pack 2 (dark hub) and Pack 6 (`/cv`) are live. The old
  `feat/hub-admin-cv-docs`, `claude/portfolio-review-updates` and the earlier
  `claude/cv-worktree-completion-lq2wps` were deleted — they carried the phone
  number in history. No remaining branch does (checked with `git log -S`).
- **This branch** (`claude/cv-worktree-completion-lq2wps`, recreated from
  `master`) adds this handoff and one fix: the CV's sorting entry never closed
  its `<ul>`.
- **Not started:** container Packs 3–5 (admin) and 7 (`/docs`).
- **`/cv`** shows `Jesper L.`, email + GitHub only, the employer kept, and the
  risk-scaled review wording. `cv.rs` asserts by *shape* that no phone-length
  number, `+47` or digit in the contact row reaches the page — never by value.

## Decisions taken (record these in the container manifest — proposals C1, C2, C12)

| # | Decision | Date |
| --- | --- | --- |
| Ruling 2 | `admin_page` takes `AuthSession` **alongside** `RequireAdmin` (admin gate first) for name/role/`is_owner`. No extractor changes. | 2026-09-27 |
| Ruling 3 | The how-it-works page lives at **`/docs`**. | 2026-09-27 |
| CV | Surname off the public page (`Jesper L.`); phone and postcode never; employer (Matvare-Expressen) stays; the CV does **not** link the website itself. | 2026-09-25/27 |
| History | Squash-merge to keep personal data out of `master`'s history; delete the branches that held it. | 2026-09-27 |

⚠ **Open conflict:** Pack 7's *Observable* expects "the CV rail's link card"
to land on `/docs`, but the CV was ruled to link nothing on the site, and a
`cv.rs` test pins "no `/docs` link". Decide before Pack 7 (proposal C2 drafts
this as ruling 4).

## The AI statement (final wording — supersedes any drafted AI text)

English, for `/docs` and the README:

> **How I use AI.** I design the solution first — the data model, the
> boundaries, the failure cases, what has to be tested — then hand Claude Code
> a precise brief to implement it. Because I know the technology and where it
> usually goes wrong, the brief names the right constraints up front, so the
> first pass lands close and there are few rounds of correction. That keeps
> both cycle time and token cost low. **Review is scaled to risk:** I weigh how
> critical a change is and how likely it is to go wrong. Routine, low-stakes
> work gets a quick check; anything that reaches live users, touches security
> or data, or is complex enough to hide errors gets a thorough review — often
> of the plan, before any code is written.

The Norwegian equivalent is live in `/cv` § Arbeidsmåte.

## The review, item by item

Review: an external assessment of the live site + public repo (artifact
"Plan: portfolio.dblo.net", 27 Sep). It was written against the **old**
`master`, so several items are already done. "→" names the proposal in the
appendix that would absorb the item.

| Item | Status | → |
| --- | --- | --- |
| R1 About + AI statement | Partial: background and AI section on `/cv` (Norwegian). No English page. | C9 (Pack 7), C4 |
| R2 How it is built + diagram + numbers | Planned as Pack 7; drafted claims need fixing (see below) | C9 |
| R3 Loading-method explainer | Gap. Rules live in `static/sorting-model.js`; material in `docs/design/sorting-live/UI-SPEC.md` | C10 or N1 |
| R4 Engineering notes with excerpts | Partial (README §4 of the handoff) | C10 (Pack 8) |
| R5 Dated build log | Gap | C10 |
| R6 Demo account | Gap; needs a ruling | N1 |
| R7 Read-only `/sorting/demo` | Gap (a static demo exists in `docs/design/sorting-live/`) | N1 |
| R8 Drinks demo room | Gap (spectator screen `/room/{code}/screen` exists) | N1 |
| R9 Mark login-walled cards and palette entries | Partial: hub tiles say "sign-in required", palette entries don't, and the login page has no way back | C4 |
| R10 Per-project cards (status word, stack) | Partial: tiles exist | C4 |
| R11 "What I would change" | Partial ("decisions I'd defend" in the handoff README) | C9 |
| R12 Hire line, NO + EN | Partial (NO on `/cv`) | C11 |
| R13 Task prompt typos | **Your content** (database), not code | — |
| R14 Captions / alt text | Gap; the caption *is* the alt text | C6, C7 + your content |
| R15 Drinks tile copy vs name+PIN flow | Gap | C4 |
| R16 Tags rail with few tags | Gap | C6 / N2 |
| R17 Feed thumbnails | Gap; the perf manifest excludes the upload pipeline | W1, W5 |
| R18 `<source type=image/webp>` on legacy JPEG rows | Gap (old rows only) | W5 |
| R19 Titles with name | Partial | N2 |
| R20 Favicon, meta description, OG | Gap | N2 |
| R21 404 page with header | Gap (`main.rs` fallback is plain text) | N2 |
| R22 robots.txt | Gap | N2 |
| R23 Hub identity line / WIP / contact | Partial: hub rebuilt; the identity line is still owed by you | C4 |
| R24 nginx security headers | Gap; mind the `add_header` inheritance trap under `location /static/` | N3, W3 |
| R25 `style.css` weight (186 KB) | Partial (gzip live) | W7 |
| R26 README | Gap | N4 |
| R27 Commit email → noreply | Your git config | N4 |
| R28 Split giant modules | Gap | N4 |
| R29 Deploy user + pinned host key | Gap | N3 |
| R30 CI builds offline against `.sqlx` | Gap | N3 |
| R31 Private-window check | Process; the Pack 1 ledger still owes a signed-in walkthrough of `/fitness`, `/sorting`, `/admin` | — |

## Other findings from the mapping (not in the review)

- **The CV claims the sorting tool "lagres lokalt … ikke via nett".** The code
  POSTs every action to `/api/sorting/sessions/{id}/actions`, and localStorage
  is only the offline queue. This was your wording choice; it is flagged
  because it is checkable.
- **The hub footer says "no build step".** That is the same claim the manifest
  already flags as false for Pack 7's stat (it is a compiled Rust binary; the
  intended meaning is "no front-end bundler").
- **Pack 7's drafted claims that are wrong:** "all on the same server /
  nothing fetched from someone else's service" (images come from object
  storage); `sections: 5` and "the five sections" (now six); 1105 tests (now
  1108); "four session extractors" (five with `LocalhostOnly`).
- **Stale docs:**
  - The container manifest status and INVENTORY still say rulings 2 and 3 block.
  - The WORKTREES entries for Sorting (#14) and Backroom (#15) read unmerged.
  - `lc-challenge-cards` still says ACTIVE.
  - CLAUDE.md says `tasks.rs` uses `AuthSession`/`OptionalAuth`, but the code
    uses `RequireAdmin`/`OptionalAdmin`.
  - `sorting-live` README says 432 checks, but verify runs 458.
- **`login.html` is standalone.** A visitor who clicks a login-walled tile has
  no header and no way back.
- **One network-exposure finding** was left out of this public file on
  purpose; it was handed over privately.

## Suggested order locally

1. **Record the decisions:** C1, C2, C12. Fix the stale status lines listed
   above.
2. **Pack 2b — hub follow-up (C4).** Small and visible, but needs your
   identity line.
3. **Site polish (N2):** titles, meta and OG tags, favicon, robots.txt, 404
   page, and a way back from login.
4. **Pack 7 `/docs` (C9)** plus **the README (N4, pack 1)**, both carrying the
   AI statement above.
5. **Image weight (W5)**, then **stylesheet weight (W7)**.
6. **Public demos (N1)** and **Pack 8's long read (C10)**. These are the
   review's strongest evidence items (R3, R7).
7. **Admin Packs 3–5** and **infra hardening (N3)**. Load the `deploying`
   skill first.

## Using the appendix

- 27 proposals: C1–C12 (container), W1–W8 (perf), N1–N7 (new containers and
  index lines).
- **C1–C5 were checked** by two verifiers (accuracy, placement). All came back
  *revise*. Their problems and revised texts are included, so apply the revised
  text rather than the draft.
- **C6–C12, all W and all N are unchecked.** Verify every path, count and
  quote before applying.
- The drafts predate the squash-merge. Read any "feat branch" or "PR #20 open"
  wording as stale.
