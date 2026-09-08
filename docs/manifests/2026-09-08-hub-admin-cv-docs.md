# Container — Hub, Admin, CV and How-it-works on the Hampter Design System

**Status:** NOT STARTED — planned only. Pack 1 is drafted to item level and is
ready to run; **three rulings below block Packs 3, 6 and 7** and are the user's
to make. No execution go has been given.
**Branch:** `feat/hub-admin-cv-docs` (from `master` @ afbad7f, in the main
checkout — no separate worktree; merges → `master`). `dev` is **not** the
target: its only commit `master` lacks is a stale `docs/WORKTREES.md` edit
(e647d1c, 2026-08-29) and `master` has moved 33 commits past it, with the last
two streams merged by PR straight to `master`. Ancestry checked both ways:
`master` is not an ancestor of `dev`, `dev` is not an ancestor of `master`.
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

One of the seven packs crosses a privilege boundary (Pack 4, the owner-only
Accounts pane). One is blocked on content the repo does not have (Pack 6).
Neither is a thing to discover halfway through a pack.

## Pack sequence

Ordered by the hazard each pack retires, not by how visible its screen is.

| # | Pack | Ends in something you can see |
| --- | --- | --- |
| 1 | **The shared shell** | Every page's header reads `hampter.`, the active nav link lights, and Ctrl+K opens a dark palette instead of a white box |
| 2 | **The Hub** | `/` on the blueprint grid: hero, palette bar, five section tiles |
| 3 | **Admin — shell + Posts pane** | `/admin` dark, with live counts, search, visibility chips and working row actions |
| 4 | **Admin — Accounts pane** | The owner sees the accounts grid inside `/admin`; a non-owner admin sees no trace of it in the page source |
| 5 | **Admin — New post pane** | Drop a file, caption it, tag it, choose a visibility, upload |
| 6 | **CV — `/cv`** | The CV page, its nav link, its hub tile and its palette command |
| 7 | **How it works** | The explainer at its ruled route, and every claim on it true of the site the six packs before it shipped |

Only Pack 1 is planned to item level. Later packs get their items when they
start; several of them are blocked on rulings recorded below.

**Nav grows in the pack that creates the route.** The header stays at four links
through Pack 5, reaches five in Pack 6 and six in Pack 7. No pack ever leaves a
dead link in the header of a live site.

**No two packs run in parallel.** Every one of them writes into
`static/style.css`, and Packs 1, 2, 6 and 7 also write into
`templates/base.html`. Worktree isolation buys nothing here.

---

## Blocking rulings — the user's to make

These three stop a pack from starting. The rest are recorded under trade-offs.

**1. Which CV is authoritative? (blocks Pack 6.)** The README cites
`uploads/cv-jesper-lovland.html`; that file does not exist in this repo and
never has. Two other versions disagree structurally: `CV.dc.html` in the handoff
(four project articles, a Kabeltekniker/Get job, no Sorting) and an out-of-repo
file at `~/projects/arbeidssoking/cv/cv-jesper-lovland.html` (Sorting-led,
`Matvare-Expressen` filled in). Bundled with it: what fills `[ARBEIDSGIVER]` and
`[ÅRSTALL]`, whether to ship with the amber placeholders visible, and whether a
public page carries the phone number and postcode. Shipping a literal amber
`[ARBEIDSGIVER]` in front of an employer is worse than shipping nothing.

**2. How does `admin_page` get identity? (blocks Pack 3.)** The design's header
eyebrow reads `signed in as <name> · <role>`, and the rail's owner-only group
needs `is_owner`. Only `AuthSession` carries `user_name` and `is_owner`;
`RequireAdmin` is a unit struct that deliberately carries nothing. Either add
`AuthSession` alongside it (two `load_session` calls per request) or give
`RequireAdmin` the fields it currently refuses. Both change a security
extractor, so neither is an implementer's call.

**3. Route name for the documentation page.** `/docs` or `/how-it-works`? The
handoff writes `GET /docs` then adds "(name it what you like)", and the palette
command id is already `docs` in two prototypes. Needed before Pack 2 writes the
hub footer link and before Pack 7's module is named. Recommendation: `/docs`.

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
number before hardcoding the string. The about line is flagged placeholder copy
the user owns.

### Pack 3 — Admin: the shell, the Posts pane, and the end of `admin_post_card_html()`

**Blocked on ruling 2 before its items are written.**

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
rail, the pane routing and the extractor ruling. **Every item in this pack is
flagged for individual review** — auth/session territory.
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

**Blocked on ruling 1 before its items are written.**

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
keyboard callout. The hub footer link and the CV rail's link card both land here,
and every claim on the page is true of the site the previous six packs shipped.

**Agent brief:** read first — `How it works.dc.html`, README §4 (including its
rule: *"Every fact and number comes from the repo's own CLAUDE.md /
docs/design.md. If you change the code, change this page"*) and, for the numbers,
the actual output of `cargo test --workspace` and `./scripts/verify.sh` — never
another document. Depends on Pack 1 for the scope, Pack 2 for the hub footer
link, and Pack 6 for the CV link and a complete nav. This pack closes the
container: convert the 🚧 pointers in `docs/INVENTORY.md` into real entries in all
four places.

**Risks.** Stale by construction, on a public page aimed at employers, with no
test behind any of it. The board-check figure **already** disagrees inside this
repo — CLAUDE.md says 458 while `scripts/verify.sh` still says 432 — so
publishing a third copy is the same bug one level more visible. Three drafted
claims are wrong and must be fixed before publishing: "nothing to compile before
a change goes live" and the `build step: none` stat are false for a Rust binary
(the intended meaning is no front-end bundler); "four session extractors"
undercounts, since `src/middleware.rs` has five with `LocalhostOnly`; and the
`sections: 5` stat conflicts with a six-link nav and a six-tile hub. This is also
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
  already carries two different values for the board-check count, so "we will
  remember to update it" has a track record.
- **The PDF button on `/cv`.** A scoped `@media print` stylesheet plus
  `window.print()`, or serve a print-ready file as a static asset. The README
  recommends against generating server-side.
- **Language.** The CV is Norwegian, all site chrome is English, and the CV hub
  tile's description is Norwegian while its five siblings are English. The README
  raises the same mismatch for the docs page.
- **The hub's about line** is flagged placeholder copy the user owns, and the
  hero eyebrow's "five sections" conflicts with the tile count.
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
  (`CLAUDE.md:33` vs `scripts/verify.sh:17`); `uploads/cv-jesper-lovland.html`
  does not exist.
- 2026-09-08 — container opened. Pack sequence planned one level deep from a
  14-agent survey of the handoff, the design system and the current tree: ten
  parallel readers, three independent pack decompositions scored against
  observability, risk ordering and repo fit, and a synthesis pass. Three blocking
  rulings recorded above; **no execution go given.**
