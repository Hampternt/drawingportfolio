# Container — Hub, Admin, CV and How-it-works on the Hampter Design System

**Status:** ACTIVE — **Packs 1, 2 and 6 landed** (shared shell, Hub, CV), each
gated and walked; ledgers at the foot. They reached `master` as one
squash-merge, Hampternt/drawingportfolio#20, on 2026-09-27. **Pack 2b, the hub
follow-up, LANDED** via Hampternt/drawingportfolio#24 (2026-10-04). Packs 3, 4,
5 and 7 are not started. Ruling 1 was taken by the
user 2026-09-25 and Pack 6 shipped on it. Rulings 2 and 3 were taken by the user
2026-09-27: `admin_page` takes `AuthSession` alongside `RequireAdmin`, with
neither extractor changed, and the documentation page's route is `/docs`.
Ruling 4 was taken 2026-09-28: the CV's own content links nowhere on this site,
so its rail's link card to `/docs` is dropped for good. **No ruling blocks any
pack now** — Pack 3 can start, and Packs 4 and 5 still wait on it. An external
review of the live site was mapped onto this container on 2026-09-27;
it was written against `master` before #20 landed, so it did not see Packs 1, 2
and 6. The mapping and its proposed manifest changes are in
`docs/handoffs/2026-09-27-portfolio-review.md` and its appendix. Of those
proposals, C1, C2, C4, C5 and C12 are applied here, and C3 only as far as rows
2 and 2b of the pack table; the rest are not.

*(This line read "NOT STARTED" from 2026-09-08 until 2026-09-25: the Pack 1 and
Pack 2 ledger commits tried to update it with an unasserted string replacement
that silently did not match, so the ledgers below landed while the status above
them did not move. Assert on every replacement, including the cosmetic ones.)*
**Branch:** none of its own now. Packs 1, 2 and 6 were built on
`feat/hub-admin-cv-docs` (from `master` @ afbad7f) and squash-merged; that
branch was deleted from `origin` on 2026-09-27, because its history carries
personal data the squash keeps off `master`. The review follow-up runs on
`feat/portfolio-review` (from `master` @ 0c0e99d), and later packs branch from
`master` and merge back into it. `dev` is **not** the target: its only commit
`master` lacks is a `docs/WORKTREES.md` edit (e647d1c, 2026-08-29), whose
content `feat/portfolio-review` carries over by hand. Ancestry re-checked
2026-09-28: neither branch is an ancestor of the other. The commit hashes the
ledgers below cite resolve only through the **local** tag
`archive/hub-admin-cv-docs` (fdcebf7) — **never push it**: its tree still holds
the data the squash keeps off `master` (see the 2026-09-28 ledger entry).
**Spec:** `docs/design/hub-admin-cv-docs/` — the README is the spec; the
`_ds/…/tokens/*.css` and `_ds/…/components/components.css` files are
authoritative where they disagree with it. The `.dc.html` files are streaming
React prototypes and are **not** to be ported literally.
**Scope:** four screens — `GET /` and `GET /admin` redesigned, `GET /cv` and the
documentation page new. `/fitness`, `/artportfolio`, `/tasks` and `/drinks` keep
their current appearance and behaviour; Pack 1 changes their shared header and
their command palette, and that bleed is deliberate, gated and walked through.

---

## Goal

Move the site's front door and its admin surface onto the Hampter Design System
— the dark, violet-accented system `/fitness`, `/artportfolio` and `/sorting`
already partly wear — and add the two sections the handoff designed ahead of the
repo: a CV page and an employer-facing explainer of how the site is built.

The handoff's own fidelity bar, verbatim: *"High fidelity. Final colours, type,
spacing and interaction states. Recreate pixel-for-pixel."* Two substitutions are
flagged in the spec itself — Lucide v0.462.0 icons via CSS mask, and IBM Plex
Mono as the mono face — and both are decisions this container inherits rather
than makes.

## Why this is a container, not a pack

Four screens, two of them new sections, each carrying the repo's full
"adding a new section" checklist. More decisively: the substrate they share —
a fourth theme scope across 77 selector occurrences, four font faces, 25 icons,
a dozen unported `hm-*` primitives, the header, the nav and the command palette
— has a blast radius that reaches three pages this container declares out of
scope. That substrate is a pack on its own, and it has to land first.

One of the eight packs crosses a privilege boundary (Pack 4, the owner-only
Accounts pane). One was blocked on content the repo did not have (Pack 6,
until ruling 1). Neither is a thing to discover halfway through a pack.

## Pack sequence

Ordered by the hazard each pack retires, not by how visible its screen is.

| # | Pack | Ends in something you can see |
| --- | --- | --- |
| 1 | **The shared shell** | Every page's header reads `hampter.`, the active nav link lights, and Ctrl+K opens a dark palette instead of a white box |
| 2 | **The Hub** | `/` on the blueprint grid: hero, palette bar and section tiles (five at landing; Pack 6 added the CV's sixth) |
| 2b | **Hub follow-up** | The hero names `Jesper L.` in the user's words, every tile carries a status badge, the Drinks tile says a name and PIN come first, the footer drops the false `no build step`, and a signed-out visitor sees sign-in walls before hitting them and can get back from the login page |
| 3 | **Admin — shell + Posts pane** | `/admin` dark, with live counts, search, visibility chips and working row actions |
| 4 | **Admin — Accounts pane** | The owner sees the accounts grid inside `/admin`; a non-owner admin sees no trace of it in the page source |
| 5 | **Admin — New post pane** | Drop a file, caption it, tag it, choose a visibility, upload |
| 6 | **CV — `/cv`** | The CV page, its nav link, its hub tile and its palette command |
| 7 | **How it works** | The explainer at its ruled route, and every claim on it true of the site the packs before it shipped |

Packs 1 and 2 were planned to item level before they started, and Pack 6
shipped from its brief. Pack 2b is planned to item level below; the other packs
get their items when they start. All four numbered rulings below are taken, so
none blocks a pack. The non-blocking open rulings under trade-offs are
still answered before the pack that consumes each.

**Nav grows in the pack that creates the route.** The header stays at four links
through Pack 5, reaches five in Pack 6 and six in Pack 7. No pack ever leaves a
dead link in the header of a live site.

**No two packs run in parallel.** Every one of them writes into
`static/style.css`, and Packs 1, 2, 6 and 7 also write into
`templates/base.html`. Worktree isolation buys nothing here.

---

## Rulings — the user's to make

All four are taken. Rulings 1–3 each stopped a pack from starting; ruling 4
blocked Pack 7 until it was taken. The rest are recorded under trade-offs.

**1. Which CV is authoritative? (Blocked Pack 6. Taken 2026-09-25 — see the
Pack 6 ledger.)** The README cites
`uploads/cv-<owner>.html`; that file does not exist in this repo and
never has. Two other versions disagree structurally: `CV.dc.html` in the handoff
(four project articles, a Kabeltekniker/Get job, no Sorting) and an out-of-repo
file at `~/projects/arbeidssoking/cv/cv-<owner>.html` (Sorting-led,
`Matvare-Expressen` filled in). Bundled with it: what fills `[ARBEIDSGIVER]` and
`[ÅRSTALL]`, whether to ship with the amber placeholders visible, and whether a
public page carries the phone number and postcode. Shipping a literal amber
`[ARBEIDSGIVER]` in front of an employer is worse than shipping nothing.

**2. How does `admin_page` get identity? Taken by the user 2026-09-27: add
`AuthSession` alongside `RequireAdmin`, in `admin_page` only.** No extractor
changes shape: `RequireAdmin` stays the unit struct that carries nothing. It
goes first in the argument list, so a rejected request never pays for the second
`load_session`. In either order a signed-in non-admin gets the bare 404 and a
request with no session gets the redirect to `/admin/login`; an admitted request
pays two `load_session` calls, on this one full-page route. The `AuthSession` is
for display only. It fills the eyebrow's `signed in as <name> · <role>` and
decides whether the rail draws its *Owner only* entry. It never authorizes
anything. Every other handler keeps the extractor it has today: `RequireAdmin`
alone on `admin.rs`'s fragments and mutations, `RequireOwner` on the accounts
routes. Pack 4's pane is still fetched as its own `RequireOwner` fragment, never
rendered inline because `is_owner` was true. Not yet in the code (checked
2026-09-28): `src/routes/admin.rs:43` still reads
`admin_page(_: crate::middleware::RequireAdmin)`, and the doc comment on
`RequireAdmin` (`src/middleware.rs:94–97`) still predicts that Pack 3 will widen
it to carry the `AuthSession`. Pack 3 rewrites that comment to point here.

**3. Route name for the documentation page. Taken by the user 2026-09-27:
`/docs`.** These follow from it and are not part of the ruling: the module is
`src/routes/docs.rs` and the template is flat `templates/docs.html`, matching
`cv.rs` / `cv.html`. The palette entry is the README's `How this site works`,
with keywords `docs documentation architecture stack`; the real `palette.js`
has no id field, so the prototypes' `id: 'docs'` does not carry over. The nav
grows by exactly one link, to six.

**4. Does the CV link to `/docs`? Taken by the user 2026-09-28: no — the CV's
own content links nowhere on this site.** The 2026-09-25/27 decision that the
CV does not link the website itself covers `/docs` too. This overrides README
§3's "a link card out to the how-it-works page": the card is dropped for good,
not deferred, and Pack 7's *Observable* no longer expects it. The rule covers
everything inside the CV's `<main>`, which links only `mailto:` and
`github.com/Hampternt`. The shared header's nav on `/cv` is site chrome, not
CV, and keeps every nav link, `/docs` included once Pack 7 adds it; print hides
that header. One gap is outside the page's control: the browser's own print
header/footer can stamp the page URL onto a saved PDF, and suppressing it needs
`@page`, which Pack 6 ruled out. Links *towards* the CV — nav, hub tile,
palette — are unaffected. Pinned by `test_cv_content_links_nowhere_on_this_site`
(`src/routes/cv.rs`), which replaced `test_cv_links_nowhere_that_does_not_exist_yet`:
it checks the `<main>` slice only, so Pack 7's nav link cannot break it, and
rejects any root-relative `href="/` and any `portfolio.dblo.net` there.

---

## Packs

### Pack 1 — The shared shell: theme scope, self-hosted assets, chrome, palette

**Done when:** a fourth body class exists in the theme scope and in both
re-derivation scripts, the stylesheet has a section the rem guard cannot reach,
the four missing font faces and the full Lucide set are self-hosted, the header
carries the wordmark and a live active-nav state, the command palette is dark,
and the dead legacy card CSS is gone.

**Observable:** open `/artportfolio` — the header reads `hampter.` with a violet
full stop, the current section's nav link is bright while the rest are muted, and
Ctrl+K opens a dark palette panel instead of today's white box on a black page.
`/fitness`, `/sorting` and `/tasks` are unchanged apart from the same header, and
`/tasks`' nav renders as a spaced row rather than one run-on string.

**Agent brief:** read first — repo `CLAUDE.md` (Architecture Rules, Static asset
caching, the Hampter Design System note), this manifest, and
`docs/design/hub-admin-cv-docs/README.md` §Design tokens / §Assets /
§Interactions. Code to match: `static/style.css` lines 1711–1962 (the DS banner
and token block) and 3215–3249 (the specificity-escape block) — stripped
comments, scoped selectors, tokens never hex. Dependency edges: nothing
upstream; every other pack depends on this one. Not parallelizable with
anything.

**Rulings taken before writing any of it (2026-09-09, execution go given):**

- **Item 1.2 goes the "promote" way.** `.hm-post` and its six siblings move from
  `body.art-page` onto the shared `:is()` list. The handoff specifies the admin
  row as `.hm-post` and keeps its `hx-target="closest .hm-post"` contract, so
  forking would mean reproducing that contract rather than sharing it. Adding a
  name to an `:is()` list is cascade-neutral (`:is()` takes its most specific
  argument's weight) and no page carries `site-dark` yet, so this changes nothing
  visible today. Pack 3 may revisit if the shared row proves wrong.
- **Item 1.10 is a minimal dark override, not the full `.hm-cmdk` restyle.** It
  reaches Pack 1's observable — Ctrl+K opens a dark panel — at the smallest blast
  radius across the three shipped dark pages. The richer palette (groups,
  per-command icons, footer key hints, live count) is a Pack 2 candidate, where
  the Hub's palette bar makes it worth the churn.
- **Item 1.5's SVG files are gated on the user.** Lucide is on no local disk and
  the design bundle carries only CDN URLs, so the 25 files are a network
  download and need an explicit go. The item's CSS mask rules are written
  regardless; the files drop in behind them.

- [ ] **1.1 The fourth theme scope.** Add `body.site-dark` to all **77**
      occurrences of `:is(body.art-page, body.fitness-dark, body.sorting-dark)`
      in `static/style.css` (count verified), and add the matching
      `classList.toggle('site-dark', …)` line to the syncBodyTheme IIFE in
      **both** `templates/base.html` and `templates/admin.html`, which stay
      byte-identical. Never write the class literally on `<body>` —
      `classList.toggle` only removes what it toggles, so a hardcoded class on
      admin.html's `<body hx-boost="true">` would ride "View site" onto the hub.
      *Done: computed styles on `/artportfolio`, `/fitness` and `/sorting` are
      unchanged, no three-argument `:is(` list remains, and no page yet carries
      the marker.*
- [ ] **1.2 Rule on `.hm-post`.** Promote it and its six siblings from
      `body.art-page` onto the shared `:is()` list so Pack 3's admin row can keep
      the `hx-target="closest .hm-post" hx-swap="outerHTML"` contract, or fork a
      separate admin row class and leave the DS banner's deliberate
      artportfolio-only scoping intact.
      *Done: `/artportfolio`'s card renders with identical computed styles
      before and after, and the decision is recorded here for Pack 3.*
      ⚠ **Flagged for individual review** — cross-page blast radius.
- [ ] **1.3 The insertion anchor.** Open a
      `/* ── Site chrome: hub, admin, CV, how it works ─── */` section **above**
      line 2951's sorting banner — `tests/static_assets.rs` slices from that
      marker string to EOF and rejects a literal `px` in any rule body, and this
      handoff is written entirely in px, so placement is the fix rather than a
      px→rem conversion. The banner records the `body.site-dark .<prefix>-`
      specificity escape and the rename-on-change rule for `static/fonts/` and
      `static/icons/`.
      *Done: `cargo test --test static_assets` is green with a literal px value
      present in the new section, and the banner contains no nested comment.*
- [ ] **1.4 Four font faces and the token they unlock.** Copy `archivo-500`,
      `archivo-600`, `space-grotesk-600` and `space-grotesk-700` from the
      handoff's `_ds/…/assets/fonts/` into `static/fonts/` (md5-identical to
      `drinkinggame/assets/fonts/` — copy, never re-encode), add their four
      `@font-face` rules beside the existing seven, and declare `--type-h3`,
      rewriting the "Archivo 600 is deliberately absent" comment to say why it
      now exists.
      *Done: `document.fonts.check('600 20px Archivo')` is true and no weight is
      synthesised.*
- [ ] **1.5 Self-host the icon set.** `static/icons/` holds seven files today;
      add the 23 Lucide v0.462.0 names the handoff enumerates that are missing,
      plus `arrow-up-right` and `chevron-right`, each with its mask rule. Include
      the six reachable only through the command palette — `home`, `tag`, `eye`,
      `clipboard-list`, `book-open`, `settings` — or palette rows render
      iconless.
      *Done: every icon name in the handoff resolves to a local file painting
      from `currentColor`, and no `unpkg.com` URL exists in the repo.*
- [ ] **1.6 Port the missing `hm-*` primitives**, scoped: `.hm-badge` and its six
      tones, `.hm-tag`, `.hm-textarea`, `.hm-choice`, `.hm-hub`, `.hm-card`,
      `.hm-btn--danger`, the outline/accent icon-buttons, `.hm-input--mono`.
      Strip source comments — a nested `/* */` fails the asset test — and keep
      the repo's three deliberate deviations from the bundle.
      *Done: every ported class resolves each `var()` it names. Tabs, Tooltip,
      Toast, Switch, Select and CalorieRing are deliberately not ported.*
- [ ] **1.7 A prose baseline.** The scoped resets zero `p` and `h1–h4` margins,
      omit `h5`/`h6`, and carry no rule at all for lists or tables, so a prose
      page authored naively renders as one block over UA bullets. Add
      `.hm-prose`: stacked flow at 68ch, restored rhythm, and the design's bullet
      as a flex row with a 4px violet dot — **never `list-style`**, which the
      handoff forbids.
      *Done: a paragraph-and-bullets fixture renders with the design's spacing
      and no UA markers; CV and How-it-works inherit it.*
- [ ] **1.8 The header chrome, in `templates/base.html` only.** Replace
      `<a href="/" class="site-title">Portfolio</a>` with the wordmark, add the
      34px `Ctrl K` palette button and the `is_admin`-only settings icon-button,
      and lay `header nav` out to spec under **both** the scoped chrome and the
      legacy light chrome `/tasks` still uses. The link count stays at four.
      `admin.html` is deliberately untouched here — Pack 3 replaces its header
      wholesale, which is where CLAUDE.md's update-both rule is satisfied.
      *Done: `/artportfolio`, `/fitness` and `/sorting` read `hampter.`,
      `/tasks`' four links render as a spaced row, admin.html is unchanged.*
- [ ] **1.9 Active nav state.** Add `{% block nav_active %}` to `base.html` and
      fill it from the templates that extend it.
      *Done: `/fitness` renders Fitness at `--text-strong` and the rest muted,
      and a template that omits the block still compiles and marks nothing.*
- [ ] **1.10 The command palette goes dark.** `#palette-overlay`, `#palette-box`,
      `#palette-input`, `#palette-results` and `.palette-item` sit at ID
      specificity with no dark override anywhere, so Ctrl+K already opens a white
      box on a black page on three shipped dark pages — a pre-existing bug the
      redesign makes maximally visible. No COMMANDS entries are added here; `cv`
      and `docs` arrive with their routes.
      *Done: Ctrl+K on `/artportfolio`, `/fitness` and `/sorting` opens a dark
      panel.*
- [ ] **1.11 Delete the dead legacy card CSS** at `static/style.css:74–104` and
      correct CLAUDE.md's "Post cards" paragraph, which is wrong on both halves
      of its claim: `admin_post_card_html()` emits `class="admin-post"`
      (admin.rs:653), and a repo-wide grep finds no emitter of `post-card` at
      all. Doing it here decouples the deletion from the Admin redesign.
      *Done: `./scripts/verify.sh` green and CLAUDE.md no longer claims the rules
      must stay until `/admin` is migrated.*

**Item gate:** `./scripts/check.sh`, plus `cargo test --test static_assets`
after every CSS edit — that guard exists for exactly this.
**Pack gate:** `./scripts/verify.sh` + a real-browser walkthrough of
`/artportfolio`, `/fitness`, `/sorting` and `/tasks`. Navigate away and back to
exercise the boosted path: a missing `classList.toggle` line loses the theme on
the first boosted navigation but survives a hard reload, so a naive walkthrough
will not catch it.

**Risks.** Widest live blast radius in the container — the header change hits
nine URL families, three of which this container declares out of scope. The
77-occurrence sweep has no test behind it, so one missed occurrence is a
silently half-dark page. Files added under `static/fonts/` or `static/icons/`
never change `?v=` (`assets.rs` hashes only the top level of `static/`) and
nginx serves `/static/` immutable for a year, so **renaming is the only
cache-bust** and the filenames chosen here are chosen for good.

### Pack 2 — The Hub

**Done when:** `GET /` moves onto the shared dark scope — sticky glass header,
96/72px blueprint hero, `Portfolio.` display headline, a clickable Ctrl+K palette
bar, a "Sections" heading rule, the tile grid and a mono footer.
`src/routes/hub.rs` needs no change: `OptionalAdmin` already supplies the only
server value the design consumes, and every tile meta is a static string.

**Observable:** `/` renders dark on the 32px blueprint grid, the palette bar
opens the dark palette on click, five tiles lift and take an accent border on
hover, and the `/drinks` tile still does a full page load rather than a boosted
swap.

**Agent brief:** read first — `Hub.dc.html`, README §1, `templates/hub/hub.html`,
`src/routes/hub.rs` (22 lines, the whole model), Pack 1's stylesheet section.
Five tiles here; the CV tile arrives with the CV route in Pack 6. Depends on
Pack 1 only. Placed second because the Hub is the cheapest full exercise of every
Pack 1 decision — no data, no mutations, no permission boundary — so a missed
`:is()` occurrence surfaces on a page that costs nothing to fix.

**Rulings taken before writing any of it (2026-09-09):**

- **The hero's about line is omitted, not ported.** The design's
  "I draw most days, track what I eat…" is flagged by the README's own open
  items as placeholder copy the user owns. It is first-person copy about a real
  person on a public page, so it is not mine to invent or to ship as a
  stand-in. The hero renders eyebrow, headline and the real tagline (which is
  verbatim from today's `hub.html`) and stops there. **Owed: one line of copy
  from the user, or a decision to drop it permanently.**
- **The eyebrow keeps "five sections".** It is accurate today — artportfolio,
  tasks, fitness, sorting, drinks. It becomes wrong when `/cv` and the docs page
  land, and Pack 7 already owns reconciling that count with its own `sections`
  stat.
  *(Superseded 2026-09-25: Pack 6 changed the eyebrow to "six sections" when it
  added the CV tile (`templates/hub/hub.html:11`). The count is still not
  settled. The How-it-works design says "five sections" three times: in its hero
  line, its `sections` stat and its table heading. Once `/docs` lands, the nav
  (six links, no Drinks) and the hub (six tiles, no docs page) count different
  things. Pack 7 picks one definition of a section and makes the eyebrow and its
  own page agree. The stat is already on its list of wrong drafted claims.)*
- **The header's two disputed numbers stay as they are.** The mock specifies
  `padding: 0 32px` with `gap: 24px`; the live scoped rule has them the other way
  round, `padding: 0 var(--gutter)` = 24px with `gap: var(--space-9)` = 32px.
  Both are pre-existing and shared by every dark page, and style.css's own
  comment records that four links already overflow a 390px viewport. Swapping
  them is a container-level change that belongs with the mobile-nav ruling in
  Pack 6, not a hub pack's call.

- [ ] **2.1 The page marker.** Add the `main .site-page` marker the Pack 1
      sync script looks for, so `/` derives `body.site-dark`. Nothing else in
      this item.
      *Done: `/` carries `body.site-dark` on load, after a boosted navigation
      away and back, and after `htmx:historyRestore`.*
- [ ] **2.2 The hero.** 96/72px padding on the 32px blueprint grid, the mono
      eyebrow, the `Portfolio.` headline in Archivo 900 with a violet full stop,
      and the tagline verbatim from today's `hub.html`. No about line — see the
      ruling above.
      *Done: computed background-size is `32px 32px`, the headline resolves
      Archivo 900, and the full stop computes `#B48EF7`.*
- [ ] **2.3 The palette bar.** The 48px, 520px-max button under the hero with
      its search icon, "Search commands…" label, `Ctrl` `K` keycaps and the mono
      caption beneath. It opens the same palette the header button does — one
      handler, not two.
      *Done: clicking it opens the dark palette from Pack 1 item 1.10.*
- [ ] **2.4 The Sections heading row.** `h2` at Archivo 700/26px followed by a
      1px rule filling the remaining width.
      *Done: the rule reaches the container's right edge at 1440px and at
      390px.*
- [ ] **2.5 The five tiles.** `.hm-hub` cards in the auto-fit grid —
      Drawing Portfolio, Drawing Tasks, Fitness, Sorting, Drinks — descriptions
      and metas verbatim from the design (which took them from today's
      `hub.html`). **Five, not six: the CV tile arrives with its route in Pack
      6.** ⚠ `templates/hub/hub.html:19` carries `hx-boost="false"` on the
      /drinks card and the mock does not model it — it must survive.
      *Done: five tiles, each lifting 2px with an accent border on hover, and
      /drinks still does a full page load rather than a boosted swap.*
- [ ] **2.6 The footer.** The mono footer row. The "how this site works" link
      targets a route that does not exist yet, so it is **omitted** here and
      arrives in Pack 7 with its route — same no-dead-links rule the nav follows.
      *Done: no link on `/` 404s.*
- [ ] **2.7 Delete the light hub CSS** at `static/style.css:34-60`
      (`.hub-intro`, `.hub-projects`, `.hub-card` and friends) now that nothing
      renders it.
      *Done: `./scripts/verify.sh` green and no template references the deleted
      classes.*

**Item gate:** `./scripts/check.sh` plus `cargo test --test static_assets` after
every CSS edit.
**Pack gate:** `./scripts/verify.sh` + a browser walkthrough of `/` at 1440px
and 390px, the palette from both entry points, and one boosted round trip to
prove the marker survives.

**Risks.** `templates/hub/hub.html:19` carries `hx-boost="false"` on the
`/drinks` card and the mock does not model it; dropping it boosts a
`nest_service` mount whose templates do not extend `base.html`. The hero eyebrow
reads "one binary, five sections" while the design draws six tiles — resolve the
number before hardcoding the string. *(Resolved for the hub in Pack 6; see the
eyebrow ruling above.)* The about line is flagged placeholder copy the user
owns.

### Pack 2b — Hub follow-up: identity, status, and the copy that is wrong

**Rulings taken before execution (2026-09-28, execution go given):**

- **2b.1's line** is "Developer — internal digital tools and automation", an
  English rendering of the CV's role line, chosen by the user.
- **2b.2's badges are state only:** `live` (success) or `in progress`
  (warning). Access stays on the foot line.
- **2b.4's footer carries both contacts,** email and GitHub.
- **The boosted-login bug is fixed first, as 2b.0,** with a server-side
  `HX-Redirect` plus a template backstop. Found while planning 2b.6, it is
  pre-existing and live.
- **The boosted `/admin` links are fixed here as 2b.9.** Another session found
  them after #20 and proposed the fix; the user folded it into this pack so it
  ships once.

Planned from the appendix's C4, merging both verifiers' revisions, checked
against the tree at `25c88c2`, then corrected by a two-agent check pass.

**Why it exists.** The 2026-09-27 review was written against the live site
before #20 landed, so it saw the old light hub. Pack 2 already answers part of
it: the hub is rebuilt (R23), and the Fitness and Sorting tiles already say
`sign-in required` (`hub.html:63`, `:72`), the tile half of R9. Four of the
review's hub points still stand. The page names no one. No tile says whether its
section is live. The Drinks tile promises a room code (`hub.html:53-54`), but
`/drinks` first asks for a name and PIN (`drinkinggame/templates/landing.html:39`).
And nothing says how to reach the owner. One more comes from this manifest's own
findings: the footer's "no build step" (`hub.html:88`) is the claim Pack 7's
Risks call false for a Rust binary. The review mapping adds another:
`login.html` is standalone, so a visitor who follows a sign-in tile lands on a
page with no way back.

**Done when:** a signed-out boosted click into a gated route loads the login
page as a full page; the hero carries `Jesper L.` and the approved line; every
tile carries a status badge; the Drinks tile and the footer say only true
things, and the footer carries email and GitHub; the palette commands that need
sign-in say so; the login page links back to `/`; links to the two standalone
pages are never boosted; and `src/routes/hub.rs` has its first tests.
`hub_page` itself does not change.

**Observable:** logged out, in a private window, at 1440px and 390px: clicking
the Fitness tile lands on a *styled* `/admin/login` with no query string, which
links back to `/`; `Jesper L.` and the identity line sit under the tagline; each
of the six tiles shows one badge at its natural width and nothing clips at
390px; the footer reads "no front-end build step" and never "no build step";
Ctrl+K, on `/` and on `/artportfolio`, marks the four sign-in commands; and the
Drinks tile still does a full page load.

**Agent brief:** read first — Pack 2's rulings and ledger; Pack 6's ledger
(ruling 1 and the 2026-09-27 revision); `src/middleware.rs` (the three
no-session rejections); `templates/hub/hub.html`; `templates/cv.html` (`:16`
the name, `:17` the role line, `:31–33` the contacts, `:58` the front-end build
claim); `src/routes/cv.rs`'s tests, the pattern for 2b.8; `static/palette.js`
(rows render `cmd.label` through `textContent`, there is no hint field, item
1.10 deferred the richer palette, and the script loads on every page built on
`base.html` plus `admin.html`); `templates/login.html` (standalone like
`admin.html` but outside its recorded exception: its own `<style>` block, no
header, no palette); and `Hub.dc.html:54` for the about slot's styling. Depends
on Pack 6 for the six tiles. Independent of Packs 3–5. Runs before Pack 7, which
adds the footer's `/docs` link.

- [x] **2b.0 The login page always loads as a page.** Pre-existing and live. A
      boosted click on a sign-in tile or nav link sends an htmx request; the
      no-session rejection is a 302, which the XHR follows, so `login.html` is
      swapped into the current page. htmx drops the response's `<head>`, so the
      page renders unstyled. Worse, the swapped-in `#pin-form` is boosted: htmx
      2.0.4's boost listener never checks `defaultPrevented`, so a PIN submit
      also sends `GET /admin/login?name=…&pin=…`, pushing both values into the
      URL, the history and the access log. Fix, as ruled: the no-session branch
      of `AuthSession`, `RequireAdmin` and `RequireOwner` (`src/middleware.rs`)
      answers a request carrying `HX-Request` with `HX-Redirect: /admin/login`
      instead of a 302, so htmx performs a full navigation. And `login.html`'s
      `<main>` gets `hx-boost="false"` as a backstop. ⚠ **Auth — flagged for
      individual review.**
      *Done: tests pin both answers, a plain request getting a 302 to
      `/admin/login` and an `HX-Request` getting `HX-Redirect` with no
      `Location`. In the browser, a signed-out click on the Fitness tile lands
      on a styled login page, and a wrong PIN leaves the URL without a query
      string.*
- [x] **2b.1 The identity line.** Fills the about slot Pack 2 left empty
      (`Hub.dc.html:54`: a muted paragraph under the tagline, 62ch wide):
      **`Jesper L.`**, the form `/cv` has used since #21 and never the full
      surname, then the approved line "Developer — internal digital tools and
      automation".
      *Done: the hero shows the name and the line.*
- [x] **2b.2 Status badges and stack.** One `.hm-badge` per tile, from item
      1.6's tones (`static/style.css:2286–2291`), so no new primitive: `live`
      (success) on five tiles, and `in progress` (warning) on Sorting, as the
      CV badges it `Under utvikling` (`cv.html:92`). The badge sits in the top
      row between the title and the go arrow. `.hm-hub` is a column flexbox that
      stretches its children, so a badge placed as a direct child would become a
      full-width bar. If the top row clips at 390px, the badge moves into
      `.hm-hub__foot`. Stack words go on the foot line only where a section
      departs from the eyebrow's `rust · axum · sqlite · htmx`: Fitness
      (in-browser barcode scanning) and Sorting (a plain-JS board), each checked
      against CLAUDE.md. Drinks' foot belongs to 2b.3.
      *Done: six tiles, six badges, each at its natural width, and nothing
      clipped at 390px.*
- [x] **2b.3 The Drinks tile says what the page asks.** Description: "Four
      party games in one room — pick a name and PIN, then join with a room
      code." Foot: `name + PIN · room code · big screen`. The sections table
      Pack 7 ports makes the same mistake ("Anyone with the room code.",
      `How it works.dc.html:278`), so Pack 7 takes these words. `hx-boost="false"`
      stays.
      *Done: the tile names the PIN step and still does a full page load.*
- [x] **2b.4 The footer.** `server-rendered · no build step` becomes
      `server-rendered · no front-end build step`, the claim the CV already makes
      ("ingen byggesteg på frontend", `cv.html:58`). Add `mailto:jl@dblo.net`
      and `https://github.com/Hampternt`, both public on `/cv` under ruling 1.
      No `/docs` link, because Pack 7 adds it with its route. No dated "work in
      progress" line, because nothing real supplies the date. The footer text
      now departs from the handoff (`README.md:115`, `Hub.dc.html:85`); record
      that in this pack's ledger.
      *Done: the footer carries both contacts, and no "no build step" remains
      in `hub.html`.*
- [x] **2b.5 Sign-in is visible before the click.** `Go to Fitness Tracker`,
      `Go to Fitness Week`, `Go to Sorting` and `New sorting session`
      (`static/palette.js:60–75`) each carry the suffix ` · sign-in` in their
      label. The palette knows only `IS_ADMIN`, so signed-in members see the
      marker too. That is fine, because it describes the section, not the
      viewer. A hint field belongs to the deferred palette restyle, not to this
      item.
      *Done: the four labels carry the marker wherever the palette loads.*
- [x] **2b.6 A way back from the login page.** `templates/login.html` gains one
      link to `/`. ⚠ This is the auth page: template only, nothing in
      `webauthn.js` or `auth.rs`. **Flagged for individual review.**
      *Done: `/admin/login` links to `/`, and the page's contract with
      `webauthn.js` is unchanged. That contract is the ids it reads (`status`,
      `pin-btn`, `pin-name`, `pin-pin`) and the inline handlers `startLogin()`
      on `#login-btn` and `startPinLogin(event)` on `#pin-form`.*
- [x] **2b.7** The CV tile's Norwegian description gets `lang="nb"`, as `/cv`'s
      wrapper already has (`cv.html:10`).
      *Done: the attribute is on that tile's `.hm-hub__desc`.*
- [x] **2b.8 The hub's first tests.** A test module in `hub.rs` on `cv.rs`'s
      pattern: the `/drinks` tile and the footer's `/admin` link each keep
      `hx-boost="false"`; "no build step" is gone; there is no `/docs` link yet
      (Pack 7 flips this when it adds the footer link); `Jesper L.` is present;
      every tile carries a badge; and `<main>` carries no `+47` and no
      phone-length digit run. That last check is scoped exactly as `cv.rs:53`
      scopes it, because `base.html`'s `?v=` hashes are hex and must stay out
      of the scan. Checked by shape, never by value.
      *Done: the tests pass, and each one fails when its fault is planted.*
- [x] **2b.9 Links to standalone pages are never boosted.** `admin.html` is
      standalone, and its styles live in its own `<head>`, which a boosted swap
      drops. So the two links into `/admin` that #20 added — the header
      settings button (`base.html`) and the hub footer's `/admin` link — take
      `hx-boost="false"`, as the `/drinks` tile already does. Found by another
      session after #20 and folded in here at the user's call.
      *Done: both anchors carry the attribute, and 2b.8 pins the footer's.*

**Not in this pack:** screenshots on tiles — `.hm-hub` has no image slot
(`static/style.css:2505–2522`), and a Fitness or Sorting screenshot would
publish real data unless drawn from fictional data, which ties it to the demo
work. "Demo coming" copy, which ships only once a demo has its own manifest.
Page titles, favicon and meta tags, which are site-wide rather than the hub's.
The review's hire line (R12), which is the user's copy and lives on `/cv` unless
they supply a hub version.

**Item gate:** `./scripts/check.sh`, plus `cargo test --test static_assets`
after every CSS edit, plus `cargo test -p drawingportfolio hub::` once 2b.8
exists.
**Pack gate:** `./scripts/verify.sh`, plus the Pack 2 walk done logged out in a
private window, following every tile and every palette command to where it
lands. The review also asks for a *weekly* private-window check (R31). That is
the owner's habit, not an agent schedule.

**Risks.** 2b.1 and 2b.2 make statements about a real person and the state of
their tools on a public page, so nothing ships as a stand-in and the surname
stays off. Each badge is one more hand-maintained claim: `in progress` on
Sorting goes stale the day the tool is done. The `/drinks` tile's
`hx-boost="false"` must survive, and 2b.8 is the first test that pins it. The
footer departs from the handoff, so a later "port the mock faithfully" pass
must not restore "no build step". `palette.js` loads on every page built on
`base.html` and on `admin.html`, so its label change shows on all of them.
2b.0 changes what all three session extractors answer, which makes it the one
auth-boundary change in this pack.

### Pack 3 — Admin: the shell, the Posts pane, and the end of `admin_post_card_html()`

**Unblocked: ruling 2 was taken 2026-09-27. Items are written when the pack
starts.**

**Done when:** `/admin` renders the design's standalone dark shell — its own
header, a page head with four live counts, a 236px sticky rail and server-side
pane routing — with the Posts pane driven by a new filtered, paginated list
handler and rows rendered by an Askama template. `admin_post_card_html()` and
admin.html's inline `<style>` block both go, and `upload_post` grows the `tags`
multipart arm it has never had.

**Observable:** sign in and open `/admin` — dark shell, real counts from
`db::count_posts` and `db::list_users`, a search field and three visibility chips
filtering the list, each row showing its badge, tag pills, pixel dimensions, file
size and the amber `webp · avif pending` state, with edit / hide / unlist /
delete working in place and a Load more at the foot.

**Agent brief:** read first — `Admin.dc.html`, README §2, `src/routes/admin.rs`
(`upload_post`'s multipart arms, `htmx_admin_posts`, `patch_visibility`,
`patch_post`, `edit_post_fragment`, `admin_post_card_html`), `feed.rs`'s
`PageQuery::filter()` and `get_posts_page` as the pagination pattern to copy,
`templates/partials/post_card.html` for the swap contract, and
`src/middleware.rs`. Half the server side already exists and should be reused
unchanged. **Pack 3 owns `templates/admin.html`'s header** — Pack 1 deliberately
left it alone rather than hand-editing a header this pack replaces wholesale.

**Risks.** `patch_visibility` and `patch_post` both return the *feed* card;
wiring the admin row's buttons to them without a second response shape swaps a
feed card into the admin list — it compiles, it 200s, and `check.sh` cannot see
it. Keep `Viewer::Admin` on the list query: `Viewer::Visitor` still compiles and
simply stops listing the posts an admin most needs to see. Never reach for
`AuthSession` alone on a mutation — since multi-user a valid session only means
"you are somebody". Tags must go through `db::normalize_tags`. Tag pills are an
N+1 unless a batched query lands in `db.rs`, and SQL never leaves `db.rs`.

### Pack 4 — Admin: the owner-only Accounts pane

**Done when:** `/admin/users` folds into the shell as the rail's *Owner only*
group, with the rows and all five mutations behind `RequireOwner`, and the pane's
data never rendered into a `RequireAdmin` response.

**Observable:** as the owner, the rail shows Accounts with the user count and the
pane renders the grid — name, role badge, PIN state, added date — with Rename,
Grant/Revoke admin, Reset PIN and Delete working (the last three absent on the
owner's own row) plus the create row, each action swapping only its own row.
Signed in as a granted non-owner admin, the rail entry is absent and "view
source" on `/admin` contains no other account's name.

**Agent brief:** read first — `src/routes/users.rs` (all five `RequireOwner`
routes and every `render()` call site, each of which returns a whole page today),
`templates/users.html`, `src/models.rs` (`UserRow` covers every column the grid
draws), `src/middleware.rs`. Depends on Pack 3, which produces the shell, the
rail, the pane routing and ruling 2's `AuthSession` on `admin_page`. That
`AuthSession`'s `is_owner` decides only whether the rail *draws* the Accounts
entry. The pane's rows still come from their own `RequireOwner` fragment, so a
granted non-owner admin's `/admin` source holds no other account's row even if
the flag were wrong. **Every item in this pack is flagged for individual
review** — auth/session territory.
⚠ Placed before Pack 5 deliberately: risk before cosmetics.

**Risks.** **The read leak is the whole reason this is its own pack.**
`admin_page` is `RequireAdmin`; if the pane's rows render inline, a granted
non-owner admin receives every user's name, role, PIN state and creation date in
the HTML — with all five mutations still correctly `RequireOwner`. A client-side
pane toggle is worse: a hidden pane is still in the source. `db::list_users` has
no owner guard, and users.rs's `AND is_owner = 0` doctrine covers only the
destructive statements. Fetch the pane as a separate `RequireOwner` fragment.
`RequireOwner` rejects asymmetrically — 404 for a valid non-owner session, a
redirect when there is none — so an `hx-get` either gets an empty 404 swap or
HTMX follows the 302 and swaps a login page into the pane; both need a deliberate
client story. The rejection stays 404: never 403, which confirms the route
exists, and never a redirect, which bounces a signed-in member off a login they
are already past.

### Pack 5 — Admin: the New post pane

**Done when:** the rail's *New post* entry opens the design's upload card —
dashed blueprint dropzone, caption, tags, three visibility radios, primary upload
button — keeping the existing client-side canvas → WebP conversion before POST.

**Observable:** drop a file, type a caption and `ink, wash, figure`, choose
`unlisted`, upload: the post appears in the Posts pane carrying those tags and
that badge, with `webp · avif pending` in amber for a second or two before the
AVIF backfills.

**Agent brief:** read first — `Admin.dc.html` §New post pane, `upload_post` (the
`visibility` field already parses; the comment there even says "no upload control
sends this yet"), and admin.html's existing canvas→WebP script, currently bound
on `DOMContentLoaded` only. Any JS the dropzone adds must bind on **both**
`DOMContentLoaded` and `htmx:afterSwap` guarded on `e.target === document.body`.
Depends on Pack 3; on Pack 4 only for the rail's group ordering. Last of the
three admin packs because it is the only admin work that risks nothing.

**Risks.** The empty-`avif_url` branch is load-bearing rather than defensive — a
freshly uploaded post genuinely has no AVIF for a second or two, and the amber
state is the rendering of that fact, not an error state. The three-layer upload
limit is unchanged, and the card's "up to 35 MB" copy must keep matching all
three layers.

### Pack 6 — CV: `GET /cv`

**Ruling 1 taken 2026-09-25; done — see the ledger.**

**Done when:** a new public route and template carry the CV on the shared dark
scope, plus the four other artefacts the section checklist demands — the
`base.html` nav link, the hub's sixth tile, a `palette.js` COMMANDS entry, and a
`style.css` section. No migration: the page reads nothing, and `routes::hub` is
the standing precedent for a section without one. State the no-op explicitly
rather than leaving it looking skipped.

**Observable:** `/cv` renders the page head with its two action buttons and the
mono contact row, five sections of entries with violet-dot bullets and
right-aligned mono dates, and the 260px skills rail sticking beside them. The CV
link is live in every page's nav and lights on this page only, Ctrl+K → "cv"
navigates there, and the hub grid now has six tiles.

**Agent brief:** read first — `CV.dc.html` (the only in-repo copy of the
Norwegian text), README §3, `src/routes/hub.rs` as the route model, and
`templates/base.html` for the mandatory `is_admin: bool` field. Template path:
flat `templates/cv.html`, matching `account.html` / `login.html` / `users.html`.
Content is verbatim — do not rewrite, translate or merge sources on your own
judgement. Depends on Pack 1 for the scope, fonts and `.hm-prose`, and Pack 2 so
the hub tile has a dark grid to land in.

**Risks.** `@page` cannot be selector-scoped and templates extending `base.html`
may carry no `<style>` block, so a print rule lands in the shared stylesheet and
silently changes print output on every other page. Scope only what can be scoped
and omit `@page`, or serve a print-ready source file for the download button. A
public `/cv` publishes a phone number and postcode in plain text to scrapers —
the user's explicit call, not a side effect of porting the mock. A `CvTemplate`
without `is_admin: bool` is an Askama compile error pointing at base.html rather
than at the new struct.

### Pack 7 — How it works: the employer-facing explainer

**Done when:** the documentation page renders at its ruled route on the shared
dark scope, with its nav link, hub footer link, palette command and style
section. Last in the container because half its prose is a claim about the state
of the site that the earlier packs create.

**Observable:** the hero's four-stat mono row, the four request-path boxes, the
sections table scrolling horizontally rather than colliding, the upload timeline,
the visibility cards, the verification cards, the defended-decision rows and the
keyboard callout. The hub footer link lands here (the CV links nowhere on the
site — ruling 4), and every claim on the page is true of the site the packs
before it shipped.

**Agent brief:** read first — `How it works.dc.html`, README §4 (including its
rule: *"Every fact and number comes from the repo's own CLAUDE.md /
docs/design.md. If you change the code, change this page"*) and, for the numbers,
the actual output of `cargo test --workspace` and `./scripts/verify.sh` — never
another document. Depends on Pack 1 for the scope, Pack 2 for the hub footer
link, and Pack 6 for a complete nav. Depends on Pack 2b too: the sections
table's Drinks row takes 2b.3's words, adding the footer's `/docs` link flips
2b.8's no-`/docs` test in `hub.rs`, and the footer says "no front-end build
step", which this page's wording must match. This pack closes the
container: convert the 🚧 pointers in `docs/INVENTORY.md` into real entries in all
four places.

**Risks.** Stale by construction, on a public page aimed at employers, with no
test behind any of it. The board-check figure has **already** drifted inside
this repo once — CLAUDE.md said 458 while `scripts/verify.sh` and two other
copies said 432, until 2026-09-28 — so publishing another copy is the same bug
one level more visible. Four drafted
claims are wrong and must be fixed before publishing: "nothing to compile before
a change goes live" and the `build step: none` stat are false for a Rust binary
(the intended meaning is no front-end build step, the hub footer's wording since
Pack 2b); "four session extractors" undercounts, since `src/middleware.rs` has
five with `LocalhostOnly`; the `sections: 5` stat conflicts with a six-link nav
and a six-tile hub; and the sections table's Drinks row says "Anyone with the
room code." when `/drinks` asks for a name and PIN first — use 2b.3's words. This is also
the only pack with no server state and no mutations, which makes it the one an
implementer is most likely to skip the full gate on — and
`tests/static_assets.rs`, where a forgotten `?v=` surfaces, runs only under
`cargo test --workspace`, never bare `cargo test`.

---

## Accepted trade-offs and open rulings

Not blocking, but each needs an answer before the pack that consumes it.

- **The mobile header, once the nav reaches five and six links.** Today's fix is
  a fitness-only `@media (max-width:899px)` rule hiding the nav, and style.css's
  own comment records that silently removing `/artportfolio`'s mobile navigation
  "is not a fitness pack's call". Widen it to the whole scope list, ship a
  horizontally scrolling nav, or something else. Consumed in Pack 6.
- **The command palette: a minimal dark override, or the full restyle** with
  groups, per-command icons, footer key hints and a live count? There is no cheap
  per-page option — the existing rules sit at ID specificity with no dark
  override anywhere — so either way it changes three shipped pages in one commit.
  The README's "keep the real palette.js" settles the script, not the CSS.
  Consumed in Pack 1 item 1.10.
- **Does `/admin/users` survive as its own URL** after Pack 4 folds Accounts into
  the dark shell? The URL surviving is the cheap default; the real question is
  whether a light `users.html` behind a dark `/admin` is an acceptable seam.
- **Do the How-it-works numbers get a guard test** — the shape of
  `test_board_template_and_boot_agree_on_their_ids`, asserting the template's
  figures against their source — or is the drift accepted and recorded? The repo
  carried two different values for the board-check count until 2026-09-28
  (three copies said 432 against a measured 458), so "we will remember to
  update it" has a track record.
- **The PDF button on `/cv`.** A scoped `@media print` stylesheet plus
  `window.print()`, or serve a print-ready file as a static asset. The README
  recommends against generating server-side.
- **Language.** The CV is Norwegian, all site chrome is English, and the CV hub
  tile's description is Norwegian while its five siblings are English. The README
  raises the same mismatch for the docs page.
- **The hub's about line** is flagged placeholder copy the user owns. Consumed
  by Pack 2b item 2b.1.
  *(Until 2026-09-28 this bullet also said the eyebrow's "five sections"
  conflicted with the tile count, which stopped being true on 2026-09-25 when
  Pack 6 made both six. What remains is Pack 7's
  definition of a section; see Pack 2's eyebrow ruling.)*
- **IBM Plex Mono, weight 600 only.** The handoff flags the mono face as an
  unresolved substitution and its own `tokens/fonts.css` comment claims "no mono
  face is self-hosted in the repo yet" — **that claim is false, verified in this
  tree**: `static/fonts/` already ships `ibm-plex-mono-400.woff2` and
  `-500.woff2`, declared at `static/style.css:1968-1969`. The design's
  `@import` asks for 400, 500 and 600, so the only real question is whether
  anything in the four screens uses mono 600 and, if so, whether to ship that one
  face or snap it to 500. Consumed in Pack 1 item 1.4.

---

## Ledger

### Pack 2b — landed 2026-10-04 via #24 (built, walked and reviewed 2026-09-28)

Ten items, one commit each on `feat/portfolio-review`: `b1465c2` (2b.0 the
login redirect), `c78510d` (2b.1 identity line), `ed75f2b` (2b.2 badges),
`94ecf81` (2b.3 Drinks copy), `446b0ab` (2b.4 footer), `bfe3978` (2b.5
palette), `947806a` (2b.6 login back link), `128dcb3` (2b.7 `lang="nb"`),
`b3b1a29` (2b.8 hub tests), `92782e8` (2b.9 unboosted standalone links).

**Pack gate green,** run in the worktree: `VERIFY OK — fmt, clippy, tests, JS
syntax, board suites all clean.` Workspace **1114** tests (347 + 8 + 524 + 235;
+2 in `middleware.rs`, +4 in `hub.rs`). Board checks **458** (265 + 193).
Clippy raises nothing in any file this pack touched. That was checked against
the gate's own output; the 18-distinct baseline was not re-measured from a
clean build.

**Every new test was shown to fail on its fault.** Forcing `to_login` back to
the plain redirect fails the htmx test. Seven faults planted in the hub
template — the `/drinks` boost dropped, either `/admin` link boosted, "no build
step" restored, a `/docs` link added, a phone-shaped number in `<main>`, a
badge removed — fail seven assertions.

**Review, one pass for the pack plus the auth items individually** (two
reviewers, and a skeptic to refute each finding): four findings, one refuted.
The other three were two issues, both fixed in `dfcece2`:
- *Sign-out swapped an unstyled login page in* (medium, pre-existing): both
  sign-out forms sit in boosted pages and `logout` answered with a bare 303.
  It now answers an htmx request with 200 + `HX-Redirect`, the same shape as
  `to_login`.
- *A history-cache miss on a gated URL did nothing* (low, caused by 2b.0):
  htmx's miss fetch never reads `HX-Redirect`, so the 401 left the old page
  under the new URL. Both shells now set `refreshOnHistoryMiss`.
The refuted finding was that the badge test compares totals rather than
counting per tile; six tiles and six badges hold today. **Gate re-run on
`dfcece2`:** `VERIFY OK`, **1116** tests (+1 in `auth.rs`, +1 in
`middleware.rs`). Both new tests fail when their fault is planted. Neither
fix was walked signed in.

<details>
<summary><b>Walkthrough evidence</b> — logged out, fresh browser profile, dev
server on the worktree</summary>

*The hub.* `body.site-dark`; the about line reads "Jesper L. · Developer —
internal digital tools and automation" at 16px, `rgb(141, 135, 160)`, 10px
under the tagline, with the name at weight 600. Six tiles, six badges, five
`live` at 45px and one `in progress` at 99px. At 390×844, horizontal overflow
is **0** on the page, on every tile's top row and foot row, on the footer row
and on the about line. The footer reads "server-rendered · no front-end build
step · jl@dblo.net · github.com/Hampternt". Logged out, no `/admin` or
`/docs` link is present.

*2b.0, the bug and its fix, seen on the wire.* Clicking the Fitness tile sent
`GET /fitness → 401` (the htmx request), then a full document load of
`GET /admin/login → 200` with its stylesheet. The script context did not
survive, which a boosted swap would have kept. The login page came up styled
(`.login-box` `rgb(255, 255, 255)`, 360px) with its `<style>` in `<head>`, and
`<main hx-boost="false">`. A wrong PIN for a made-up name showed "Wrong name
or PIN"; the URL stayed `/admin/login` with no query, and the only request
carrying the PIN was `POST /api/auth/login/pin → 401`. The header's Fitness
nav link behaves the same.

*The rest.* "← Back to the hub" returns to `/`. Ctrl+K on `/`, and on
`/artportfolio` after a boosted click that stayed in the same document, lists
the four ` · sign-in` commands, and typing "fit" still matches both fitness
ones. The Drinks tile does a full page load.

</details>

<details>
<summary><b>Deviations and debt</b></summary>

- **The badge sits in the foot row, not the title row.** The plan named the
  title row first, with the foot as the fallback. The foot was chosen before
  walking, because a nowrap title, a nowrap badge and the go arrow leave too
  little width at 390px. The walk confirmed the foot row fits.
- **Drinks carries no stack word.** Its foot is 2b.3's, and "server-sent
  events" does not fit beside the name-and-PIN copy. Fitness gets "barcode
  scanning", Sorting "plain-JS board".
- **2b.9 took a third link.** `users.html` links `/admin/login` from a boosted
  page. It now carries `hx-boost="false"` as well, since every link into a
  standalone page follows the same rule.
- **The 401 shows in the console.** Browsers log any non-2xx load, so each
  signed-out boosted click logs one "Failed to load resource: 401". A 200 with
  `HX-Redirect` would be silent. The 401 was kept so a rejected request never
  reads as success in logs or tests.
- **`base.html`'s `<link rel="prefetch">` tags** fetch `/fitness` and `/sorting`
  on every page. They are plain requests, so they still get the 303 and land
  on the login page; the redirect is followed but its response is discarded.
  That behaviour predates this pack and was left alone.
- **Not walked:** the two `/admin` links and the owner-only paths, because
  they need a session and this session holds no credentials. `hub.rs` pins the
  attributes. A signed-in click through both links is owed at the next signed-in
  walk, along with Pack 1's `/fitness` and `/sorting` check.

</details>

### Pack 6 — done 2026-09-25

**Ruling 1, taken by the user 2026-09-25:** `CV.dc.html` is the source;
`[ARBEIDSGIVER]` is **Matvare-Expressen**; the public page carries **email and
GitHub only** — no phone number, no postcode (the city stays; every job entry
names it anyway). Scope was the CV page alone; Packs 3–5 and 7 stay open.

Shipped: `src/routes/cv.rs` + `templates/cv.html` (verbatim text), the
`base.html` nav link with its `data-active="cv"` rules, the hub's sixth tile
(eyebrow now "six sections"), the `Go to CV` palette command, and a CV block
in the site-chrome section of `style.css`. **No migration** — the page reads
nothing, `routes::hub` is the precedent.

Decisions made while porting, each flagged here for review:

- **Content revised by the user, 2026-09-25 (same day):** the CV no longer
  carries the handoff's text verbatim. Anything reading as criticism of the
  current workplace is gone; the site itself (`hampter.`) leads the projects,
  with claims checked against this repo; MVE Bread List and Breadify are one
  short, neutral entry; the driver job reads `2025–nå` (the user recalls
  starting around October 2025).
- **"Last ned som PDF" is `window.print()`** over an `@media print` block —
  every rule gated on `body:has(.cv-page)`, and no `@page`, which cannot be
  scoped. The print dialog's "Save as PDF" is the download; the PDF can never
  drift from the page.
- **The rail's how-it-works link card is omitted** until Pack 7's route
  exists, same rule as the hub footer. A test pins that no `/docs` link ships.
  *(Dropped for good by ruling 4, 2026-09-28; the test now checks `<main>`
  only.)*
- **Mobile nav, the trade-off this pack consumed:** below 900px the nav on
  `art-page`, `sorting-dark` and `site-dark` scrolls sideways (scrollbar
  hidden); fitness keeps hiding it. Verified: no page-level horizontal scroll
  at 390px on `/`, `/cv` and `/artportfolio`.
- **Narrow window:** the rail stacks above the entries and stops sticking, per
  the README's responsive note.
- `#3A3448` (contact separator) is off the ink scale; `--ink-600` stands in.

**Pack gate green:** `VERIFY OK`. Workspace **1108** tests (+3 in `cv.rs`:
marker/nav, nothing private or blank published, no dead links). Board checks
458, clippy 18 — unchanged. Walked in Chromium at 1440 and 390 wide, in print
emulation, and via a boosted click from the hub tile (`body.site-dark` held).

**Revised 2026-09-27** (Hampternt/drawingportfolio#21, merged into the feature
branch and carried to `master` inside #20's squash): the page's `<title>` and
heading show the shortened name `Jesper L.`; the working-method section opens
with design-first briefing, and its two review bullets became one — review
depth scales with how critical a change is and how likely it is to go wrong
(the English wording is in the handoff's §AI statement); the phone number and
postcode were scrubbed from the design handoff as well, and the privacy test
now checks by shape — no `+47`, no phone-length digit run in `<main>`, no digit
in the contact row — never by value. #22 then closed the sorting entry's
unclosed `<ul>`. Ruling 4 (2026-09-28) then settled that the CV links nowhere
on the site.

### Pack 2 — done 2026-09-09

Seven items, one commit each (`0f79bcd` marker, `22ef59a` hero, `3e034ba`
palette bar, `3128061` heading row, `20004ae` tiles, `bf19bd1` footer,
`7a86590` light-CSS deletion, plus `0145f6a` correcting an inverted comment).
Two files touched: `templates/hub/hub.html` and `static/style.css`.
`src/routes/hub.rs` is unchanged, as planned.

**Pack gate green,** re-run by the main session: `VERIFY OK — fmt, clippy,
tests, JS syntax, board suites all clean.` **1105** workspace tests and **458**
board checks, both unchanged — this pack adds no tests. Clippy holds at **18**.

<details>
<summary><b>Walkthrough evidence</b> — computed values, not impressions</summary>

*Cold load.* `/` computes `body.site-dark` with the `main .site-page` marker
present, page background `rgb(11, 9, 16)` = `#0B0910`, headline `Portfolio.` in
`Archivo / 900` with the full stop at `rgb(180, 142, 247)` = `#B48EF7`, and the
hero headline at its `clamp()` floor of 44px on a narrow viewport.

*Five tiles, not six.* Drawing Portfolio, Drawing Tasks, Drinks, Fitness
Tracker, Sorting. No CV tile. **The /drinks tile still carries
`hx-boost="false"`** — the one attribute the mock does not model and the pack's
biggest regression risk.

*No dead links.* Every `href` on the page fetched: `/`, `/artportfolio`,
`/tasks` and `/drinks` return 200; `/fitness` and `/sorting` return a redirect
to login, which is the correct answer for an anonymous visitor. **No `/admin`
link is rendered at all when logged out**, which is deviation 2 working.

*The theme survives every transition.* Real clicks, not reloads:
cold `/` → `site-dark`; boosted to `/artportfolio` → `art-page`; boosted back
via the wordmark → `site-dark` with all five tiles; then `history.back()` →
`art-page`. That last one exercises `htmx:historyRestore`, a separate listener
from `htmx:afterSwap`.

*Both palette entry points.* The hero bar computes 48px tall with
`max-width: 520px` and reads "Search commands… Ctrl K". It opens the palette;
Escape closes it; the header button reopens it. Nine commands, one handler.

*Hover.* `.hm-hub:hover` resolves the accent border, `translateY(-2px)` and
`--shadow-2`, with `body.site-dark .hm-hub:hover { text-decoration: none }`
cancelling the scoped `a:hover` underline that would otherwise strike through
every tile's title, description and meta.

</details>

<details>
<summary><b>Deviations and debt</b></summary>

- **Item 2.1 shipped both halves of the derivation, not just the marker.**
  `base.html`'s sync IIFE binds only `htmx:afterSwap` and `htmx:historyRestore`
  — it never runs at load — so the marker alone would have left `/` light on
  every cold load. The template also fills `{% block body_class %}`, which is
  what all nine other themed templates do. **Checked against the hazard:**
  base.html's own comment forbids a hardcoded class *instead of* the marker,
  because `classList.toggle` only removes what it toggles. Here `site-dark` IS
  toggled, and `admin.html` still has no literal class on its `<body>`, so the
  hazard is intact.
- **The footer's `/admin` link is gated on `is_admin`.** `RequireAdmin` answers a
  signed-in non-admin with 404, and item 2.6's done-condition is that no link
  404s. `OptionalAdmin`'s bool cannot tell an anonymous visitor from a member,
  and `hub.rs` was not to change.
- **One rule beyond the item text:** the tile hover underline fix described
  above. Without it every tile underlines on hover.
- **Three rules finish `HubCard`** (18px leading icon, 15px trailing
  `arrow-up-right`, `.hm-hub__go`) — the bundle carried these in JSX rather than
  in `components.css`, so Pack 1 had nothing to port.
- **Two off-scale values taken from the handoff:** the tagline at 19px (the
  scale steps 18 → 21) and `--white-a12` for the palette bar's `.10` border.
- **The about line is omitted**, per the ruling. ⚠ **Still owed: one line of
  copy from the user, or a decision to drop it permanently.**
- **Open fidelity question, not called:** at 1440px the hero's text column
  starts at 130px while "Sections" and the footer start at 162px, a 32px step
  that is faithful to the mock because it caps the hero and the Sections
  section differently. Flush would be
  `max-width: calc(var(--page-max) + 2 * var(--space-9))` on `.hub-sections`
  and `.hub-foot`.
- **Mobile nav is clipped, pre-existing.** At a narrow viewport the four nav
  links overflow and are cut off rather than scrolling — the nav-hiding media
  query is `body.fitness-dark`-scoped only. The light `/` did the same before
  this pack. Ruled to Pack 6's mobile-nav decision.

</details>

### Pack 1 — done 2026-09-09

All eleven items landed, one commit each (`279dbb0` scope sweep, `58c10e6`
`.hm-post`, `47419c0` section anchor, `7a3869e` fonts, `f4b42c8` icon masks,
`dae1f92` primitives, `4f0f9bd` + `38a5bd9` prose, `9f27e91` chrome, `601c713`
nav state, `44bd20b` palette, `2d971f7` legacy CSS).

**Pack gate green,** re-run by the main session rather than taken on report:
`./scripts/verify.sh` → `VERIFY OK — fmt, clippy, tests, JS syntax, board suites
all clean.` Workspace **1105 tests** (338 + 8 + 524 + 235), exactly the
documented baseline — this pack added no tests. Board checks **458** (265 + 193),
matching CLAUDE.md. Clippy holds at **18** distinct warnings from a clean build.

**The sweep was verified 1:1, not taken on trust.** Before `279dbb0` the file
carried 77 occurrences of the three-argument `:is()` list and no other shape;
immediately after, 77 of the four-argument list and zero three-argument. The
final tree's 202 occurrences trace to the four items that legitimately add
scoped rules: `.hm-post` +7, primitives +78, prose +26, palette +14.

<details>
<summary><b>Walkthrough evidence</b> — computed styles, not eyeballs</summary>

Driven at 1440×900 against `cargo run` on `:3000`. `/fitness`, `/sorting` and
`/admin` need a session and were **not** walked — see debt below.

*The chrome.* `/artportfolio` header computes `height: 56px`,
`background: rgba(14, 12, 20, 0.82)`, nav `gap: 20px`, `white-space: nowrap` —
the spec's values. Wordmark renders `hampter` + `<span class="site-title__dot">`.
Active link `rgb(242, 238, 248)` against `rgb(141, 135, 160)` for the other
three: `--text-strong` over `--text-muted`, as item 1.9 asks.

*The nav is still four links.* No CV or How-it-works entry anywhere, so no
header carries a dead link.

*`/tasks` keeps its light chrome.* Four links, `display: flex`, `gap: 20px` — a
spaced row, not the run-on string it was. Active `rgb(34, 34, 34)` against
`rgb(85, 85, 85)`, the light palette's own values.

*The boosted path, which is the one a hard reload hides.* Real clicks, not
reloads: load `/artportfolio` → `body.art-page`; boosted to `/tasks` → class
cleared; boosted back → `art-page` restored with the dark header intact. The
`classList.toggle` in both IIFEs survives hx-boost in both directions.

*The palette.* Opens on the 34px header button with 9 commands, first row
highlighted. Overlay `rgba(7, 6, 11, 0.72)`, box `rgb(23, 20, 31)`, input text
`rgb(242, 238, 248)`. Closed on load (`[hidden]`, `display: none`) — item 1.10's
specificity trap did not fire.

*Network.* Zero 404s, zero console errors, and **no `unpkg.com` or other CDN
request** on any page. Every `?v=` link resolves; fonts and icons are served
unversioned as documented.

</details>

<details>
<summary><b>Deviations and debt</b></summary>

- **Item 1.5 shipped rules without files, deliberately.** The 25 Lucide SVGs are
  a network download and are gated on the user. `static/icons/` still holds its
  original seven. Until the files land, an icon whose mask 404s renders as a
  solid block of `currentColor` rather than disappearing — that affects the
  admin-only settings button on the three dark pages and nothing a logged-out
  visitor sees.
- **Item 1.3 shipped banner-only.** Its done-condition wanted a literal `px` in
  the new section to prove `tests/static_assets.rs` cannot reach it; that was
  verified empirically with a throwaway rule (8/8 green) and then removed. The
  first real `px` lands in items 1.7 and 1.8.
- **Item 1.8 added an unscoped `header .site-admin` fallback** beyond the item
  text: `.hm-iconbtn` is scoped, so without it the admin settings button is a
  zero-size invisible link on `/tasks` and today's light hub.
- **Item 1.1 also corrected `CLAUDE.md:101`**, which quoted the three-argument
  `:is()` list literally and would have been false the moment the sweep landed.
- **The mono-600 trade-off is closed, not deferred.** No run at IBM Plex Mono 600
  exists in any of the four mocks (400 ×12, 500 ×1), so the two faces already
  shipping are enough and no fifth font file is needed.
- **`/fitness`, `/sorting` and `/admin` were not walked** — all three require a
  session and this session holds no credentials. Their chrome is the same shared
  rule verified on `/artportfolio`, but the fitness and sorting *active* nav
  fills and the sorting board's full-screen header hide are unverified. Owed
  before the container closes.

</details>

**Cross-pack contract Pack 2 inherits.** `site-dark` is re-derived from a
`main .site-page` marker that no template carries yet — the Hub must add it or it
renders light. And the shared dark header computes `padding: 0 var(--gutter)` =
**24px** while the handoff specifies **32px**; that rule and that token are both
pre-existing and unchanged by this pack, so widening the gutter is a Pack 2
decision that moves every dark page at once.


- 2026-09-28 — **A pushed tag re-published the scrubbed data; deleted the same
  day.** To keep the ledgers' cited hashes resolvable after the squash, another
  session tagged the pre-squash tip `archive/hub-admin-cv-docs` (fdcebf7) and
  pushed it. That tree still holds the phone number in `CV.dc.html` and the
  design README, and the repo is public. At the user's call the tag was
  deleted from `origin`; it survives locally only. GitHub may keep serving
  those commits by hash until it garbage-collects them. Lesson: a squash keeps
  data off `master` only while **no** pushed ref reaches the old history, tags
  included. Before pushing any archive ref, check
  `git merge-base --is-ancestor e703a03 <ref>`.
- 2026-10-04 — **Deleting refs was never enough: PR refs keep the data
  public.** GitHub keeps every PR's commits under `refs/pull/N/head`
  permanently. Those for #19, #20 and #21 still reach `e703a03`, whose design
  handoff carries the phone number in two files, visible in each PR's commit
  list. Only GitHub Support can purge PR refs; that request is the owner's.
- 2026-09-28 — Pack 2b given the go, after the user took its three rulings and
  chose to add 2b.0 (the boosted-login fix) and 2b.9 (the boosted `/admin`
  links). The plan was corrected from a two-agent check pass: `webauthn.js`'s
  real id contract, a digit scan scoped to `<main>`, badge placement, the
  Drinks foot's single owner, and Pack 7's hand-offs from 2b.
- 2026-09-28 — Pack 2b planned from the appendix's C4, merging both
  verifiers' revisions, together with C3's rows 2 and 2b and C5's five-sections
  annotations. Before execution the user decides 2b.1's copy, 2b.2's badge
  vocabulary and 2b.4's footer contacts.
- 2026-09-28 — ruling 4 taken by the user: the CV's link card to `/docs` is
  dropped for good. `cv.rs`'s link test narrowed to `<main>` and renamed to
  `test_cv_content_links_nowhere_on_this_site`.
- 2026-09-28 — rulings recorded from the portfolio-review handoff
  (`docs/handoffs/2026-09-27-portfolio-review.md`): appendix proposals C1 and
  C2 from the verifiers' revised texts, and C12 from its unchecked draft,
  checked against the code when applied. Rulings 2 and 3 marked taken,
  ruling 4 opened, the Status and Branch lines moved past #20's squash-merge,
  Pack 3 unblocked, and Pack 4's brief given ruling 2's consequence. The
  proposals' mentions of Pack 2b and Pack 8 were left out, since C3 is not
  applied.
- 2026-09-08 — handoff extracted from the delivered zip into
  `docs/design/hub-admin-cv-docs/`, stream registered in `docs/WORKTREES.md`,
  branch `feat/hub-admin-cv-docs` cut from `master` @ afbad7f and pushed
  (`e703a03`).
- 2026-09-08 — every count and line range in this manifest re-checked against
  the tree by the session that wrote it, not carried over from the survey:
  `:is()` occurrences **77**; the dead legacy block is `static/style.css:74-104`
  (the survey said 74-103, one short of the closing brace — corrected);
  line 2951 is the sorting banner; `admin.rs:653` emits `class="admin-post"` and
  the only surviving mention of `post-card` is an assertion at `feed.rs:1472`,
  so CLAUDE.md's "Post cards" paragraph is wrong as item 1.11 states;
  `static/icons/` holds 7 files and `static/fonts/` 7; `src/middleware.rs`
  defines 5 extractors; the 458-vs-432 board-check split is real
  (`CLAUDE.md:33` vs `scripts/verify.sh:17`); `uploads/cv-<owner>.html`
  does not exist.
- 2026-09-08 — container opened. Pack sequence planned one level deep from a
  14-agent survey of the handoff, the design system and the current tree: ten
  parallel readers, three independent pack decompositions scored against
  observability, risk ordering and repo fit, and a synthesis pass. Three blocking
  rulings recorded above; **no execution go given.**
