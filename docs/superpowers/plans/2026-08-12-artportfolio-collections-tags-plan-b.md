# Artportfolio Collections & Tags — Plan B (frontend)

> **For agentic workers:** REQUIRED SUB-SKILLS: `plan-economics` (this repo's task
> classes, review policy and plan sizing) then
> superpowers:subagent-driven-development to execute task-by-task.

**Goal:** The filter backend plan A shipped becomes visible and clickable: the full
filter rail (collections, tag pills, admin visibility group, inline collection
create/delete), an active-filter row with removable pills, a filter-aware empty
state, and the per-card pencil / folder-plus popovers.

**Architecture:** Every rail control is a pre-built HTMX GET to
`/artportfolio/htmx/posts` carrying the full current query string — URLs are
computed in Rust (on plan A's `append_filter_pairs`) and handed to templates as
`RailLink` values, so templates stay logic-free. Popover content is fetched from
plan A's two GET fragment routes and shown/hidden by small `artfeed.js` additions.

**Slice:** All of slice 3. When this plan is done, filters are drivable from the
page, shareable, reload-safe and Load-more-safe; admins organize posts without
leaving the feed. Slice 4 (multi-upload tray) picks up next, coding against the
frozen head-label seam. **This plan changes no SQL** — no migration, no
`cargo sqlx prepare` anywhere in it.

Spec: `docs/superpowers/specs/2026-08-12-artportfolio-collections-tags-design.md`.
Prerequisite: plan A (`2026-08-12-artportfolio-collections-tags-plan-a.md`) merged —
this plan consumes its interfaces and touches nothing below the route layer.

## Global Constraints

**The frozen seam.** The OOB head fragment keeps its exact id and shape:

```html
<div class="hm-eyebrow art-head__label" id="art-head-label" hx-swap-oob="true">…</div>
```

Task 2 edits `post_grid.html` around it; the fragment itself is untouchable, and
`head_label()` stays its single producer.

**The HTMX contract for every rail control** (spec, verbatim): an HTMX GET to
`/artportfolio/htmx/posts` with the full current query string,
`hx-target="#feed"`, `hx-swap="innerHTML"`, `hx-push-url="true"`. Note the
server's page-0 `HX-Push-Url` header (`src/routes/feed.rs`, `htmx_posts`)
overrides the attribute's fragment-endpoint URL with the real page URL — that
header, extended in plan A, is what lands `/artportfolio?tags=…` in the address
bar. Keep the attribute anyway: it is the specced contract and the header wins.

**hx-boost rule for all JS:** bind on `DOMContentLoaded` **and** `htmx:afterSwap`,
existence-guarded. `artfeed.js` already does this via `window.artfeedBound` —
extend inside `artfeedInit`, add no second init path.

**Rail staleness, accepted (spec):** editing a card's tags or memberships does
not OOB-update the rail counts; they self-correct on the next filter action or
page load. Single-user site; do not refile as a bug.

**Templates:** no `<style>` blocks; all styles in `static/style.css` under the
existing `body.art-page` section, in named subsection comments, **never nesting
`/* */`** (`tests/static_assets.rs` exists because that silently ate a rule
once). Every template gets pre-computed values — URL building happens in Rust.

**Mobile:** the full-screen filter sheet is deferred (non-goal, decision
2026-08-12). Below 900px the rail keeps its current stacked collapse; new rail
sections simply stack. Do not build a sheet.

**Interfaces consumed from plan A** (source of truth: its task Interfaces blocks):
`PostFilter`, `PageQuery::filter`, `append_filter_pairs`, `filter_desc`,
`list_collections_with_counts`, `list_tags_with_counts`, `CollectionWithCount`,
`TagWithCount`, the seven admin routes, and the three fragment partials with
their root ids (`#rail-collections`, `#art-checklist-{post_id}`).

**Verification for every task:** `./scripts/verify.sh` — all green, output quoted
in the report.

**Browser checkpoints:** two — after Task 2 (the filter UX) and in Task 5 (whole
slice, before the final review). Not per task. Environment limits recorded by
slices 1–2 apply: backgrounded tabs freeze animations, key events arrive
synthetic, `resize_window` never changes `innerWidth`, and Dark Reader repaints
colours — verify **geometry and structure only**; colour tones need human eyes.

---

### Task 1: The full filter rail

**Class:** B

**Why this class:** The markup is compiler-gated, but the toggle-URL builders are
pure helpers with expected values named below — and they are the part that can be
wrong while compiling (a pill that drops the active search, a checkbox that
resets the collection).

**Files:**
- Modify: `src/routes/feed.rs` — `RailLink`, `VisCheck`, three builder functions,
  `FeedTemplate`, `feed_page`, tests
- Modify: `templates/artportfolio/partials/filter_rail.html`
- Modify: `templates/artportfolio/partials/rail_collections.html` — active state
- Modify: `static/style.css` — rail sections

**Interfaces:**
- Consumes: `append_filter_pairs`, `PostFilter`, `list_collections_with_counts`,
  `list_tags_with_counts` (plan A); `RailCollectionsTemplate` in `admin.rs` (its
  construction gains one field here).
- Produces:
  ```rust
  pub struct RailLink { pub label: String, pub count: i64, pub url: String, pub active: bool }
  pub struct VisCheck { pub label: &'static str, pub url: String, pub checked: bool }
  fn filter_url(filter: &PostFilter, preview: bool) -> String; // /artportfolio/htmx/posts?…, no page pair
  fn collection_rail_links(collections: &[CollectionWithCount], filter: &PostFilter, preview: bool) -> Vec<RailLink>;
  fn tag_rail_links(tags: &[TagWithCount], filter: &PostFilter, preview: bool) -> Vec<RailLink>;
  fn vis_checks(filter: &PostFilter, preview: bool) -> Vec<VisCheck>; // always 3, admin only renders them
  // FeedTemplate gains:
  //   rail_collections: Vec<RailLink>, rail_tags: Vec<RailLink>,
  //   rail_vis: Vec<VisCheck>, active_collection: Option<String>
  ```
  Task 2 reuses `filter_url` for pill-removal URLs.

- [ ] **Step 1: The builders**

All three produce **toggle** semantics against a clone of the current filter:

- `collection_rail_links` — each collection's `url` is `filter_url` of the
  current filter with `collection` set to that slug, or cleared when it is
  already the active one (clicking the active row deselects). `active` when
  `filter.collection` matches. `label` is the name, `count` the viewer-aware
  count from plan A.
- `tag_rail_links` — each tag's `url` toggles that tag in/out of `filter.tags`
  (order preserved for the survivors); `active` when present.
- `vis_checks` — three fixed entries `public` / `unlisted` / `hidden`. `checked`:
  all three when `filter.vis` is `None`, else membership. Each `url` is the
  filter with that state toggled out of/into the subset; a toggle that would
  produce all three yields `vis: None` (drop the param — absent means all, and
  `vis=public,unlisted,hidden` in the bar would be noise); a toggle that would
  produce the empty set keeps the other two and drops the clicked one — i.e. it
  is a no-op URL rather than a zero-row feed. Pin that rule in a comment.

- [ ] **Step 2: `feed_page` fetches the rail data**

After the existing `count_posts` call: `list_collections_with_counts` and
`list_tags_with_counts` with the same effective `viewer`, then build the three
`Vec`s plus `active_collection: filter.collection.clone()` into `FeedTemplate`.
The rail only renders on the full page (HTMX swaps target `#feed`, which the rail
sits outside), so no fragment route needs this data.

- [ ] **Step 3: The rail template**

`filter_rail.html` keeps its search form and keyboard legend, and gains, between
them:

- **Collections** — a section header row: label "Collections" and, behind
  `{% if is_admin %}` (a new bool field the template already effectively has via
  `FeedTemplate.is_admin` — confirm it is in scope for the include), a **+**
  IconButton toggling (plain `hidden` attribute, one inline `onclick` is fine —
  precedent: `toggleComposer` in `feed.html`) an inline form: one text input
  `name="name"`, `hx-post="/api/admin/collections"`,
  `hx-target="#rail-collections"`, `hx-swap="outerHTML"`. Then
  `{% include "artportfolio/partials/rail_collections.html" %}`.
- `rail_collections.html` (plan A's fragment) upgrades in place: each row becomes
  the HTMX GET contract (Global Constraints) with its `RailLink.url`, an
  `is-active` class when `active`, count right-aligned mono; the admin delete
  control stays. **The include consumes `rail_collections` (the `Vec<RailLink>`)
  — update `RailCollectionsTemplate` in `admin.rs` to carry the same field
  names**, building its links with `collection_rail_links(&…, &PostFilter::default(), false)`
  (the create/delete responses render without filter context — accepted per the
  rail-staleness constraint; the highlight self-corrects on the next navigation).
  This template is included in two places now; its consumed field names are the
  contract — change them in lockstep or not at all.
- **Tags** — pill list from `rail_tags`: same HTMX GET contract, `is-active` =
  filled pill.
- **Visibility · admin** — behind `{% if is_admin %}`, three checkbox-styled
  controls from `rail_vis` (render as `<button>` styled as checkbox rows, not
  real inputs — the state lives in the URL, and a real checkbox's checked state
  would fight the swap).
- **The search input must carry the rest of the filter.** Today it submits only
  its own `q`. Add `hx-include="#art-rail-state"` to it, and render
  `<div id="art-rail-state" hidden>` containing `<input type="hidden">` fields
  for `tags` (comma-joined), `collection`, `vis` (comma-joined), `visitor` —
  each only when present, values from `FeedTemplate` fields. Without this,
  typing a search silently drops every other active filter.

- [ ] **Step 4: The styles**

Under `body.art-page` in `style.css`, new named subsections. From the design
README (`docs/design/artportfolio-redesign/README.md`, S1/S2): collection rows
6px/8px padding, radius 5px, label 14px, count 11px mono right-aligned, hover
`rgba(242,238,248,.04)`; tag pills 24px tall, pill radius, 6px gap, wrap,
lowercase, active = filled; the vis group and + input reuse existing `hm-*`
input/button primitives where they fit.

- [ ] **Step 5: Write the tests**

In `feed.rs`'s `mod tests` (pure functions — expected values):

| Test | Expected |
|---|---|
| `test_filter_url_round_trip` | `filter_url(&f, false)` with tags `[ink]`, collection `studies` == `"/artportfolio/htmx/posts?tags=ink&collection=studies"` |
| `test_collection_link_toggles_off` | active collection `studies` → its own link's `url` has no `collection=`; `active == true` |
| `test_collection_link_preserves_search` | filter q `cat`, link for `studies` → url contains `q=cat` **and** `collection=studies` |
| `test_tag_link_adds_and_removes` | tags `[ink]`: link for `perspective` → `tags=ink%2Cperspective`; link for `ink` → no `tags=` pair |
| `test_vis_checks_default_all_checked` | `vis: None` → 3 entries, all `checked` |
| `test_vis_check_toggle_off_drops_one` | `vis: None`, click `hidden` → its url contains `vis=public%2Cunlisted` |
| `test_vis_check_full_set_drops_param` | `vis: Some(["public","unlisted"])`, click `hidden` → url has **no** `vis=` pair |
| `test_rail_renders_on_feed_page` | route test: seeded collection + tag; GET `/artportfolio` → body contains `id="rail-collections"`, the collection name, the tag label |

- [ ] **Step 6: Commit**

```bash
git add src/routes/feed.rs src/routes/admin.rs templates/artportfolio/partials/filter_rail.html templates/artportfolio/partials/rail_collections.html static/style.css
git commit -m "feat(artportfolio): the rail grows collections, tags and the admin trio"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 2: The active-filter row and the filter-aware empty state

**Class:** B

**Why this class:** Markup plus one pure helper (pill removal URLs) whose cases
and expected values are below.

**Files:**
- Modify: `src/routes/feed.rs` — `FilterPill`, `active_pills`, `PostGridTemplate`,
  `render_grid`, tests
- Modify: `templates/artportfolio/partials/post_grid.html`
- Modify: `templates/artportfolio/partials/empty_state.html`
- Modify: `static/style.css`

**Interfaces:**
- Consumes: `filter_url` (Task 1), `filter_desc`, `append_filter_pairs`,
  `PostFilter` (plan A).
- Produces:
  ```rust
  pub struct FilterPill { pub label: String, pub remove_url: String }
  fn active_pills(filter: &PostFilter, preview: bool) -> Vec<FilterPill>;
  // PostGridTemplate gains:
  //   pills: Vec<FilterPill>, clear_url: String, filter_desc: String  // "" = no filter
  ```

- [ ] **Step 1: `active_pills`**

One pill per active filter part, in `filter_desc`'s order (tags, quoted q,
collection) so the row and the empty state read identically. Each `remove_url` is
`filter_url` of the filter minus that one part. The `vis` subset gets **no
pill** — it is the rail's admin plumbing, mirroring `filter_desc`. `clear_url` is
`filter_url(&PostFilter::default(), preview)` — preview survives clearing;
`filter_desc` comes from plan A's `filter_desc(filter).unwrap_or_default()`.

- [ ] **Step 2: `post_grid.html` renders the row**

`render_grid` fills the three new fields. In the template, immediately **after**
the OOB label block (whose bytes do not change — the frozen seam) and before the
empty-state/groups branch:

```
{% if is_first_page && !pills.is_empty() %}
```
— a row of removable pills (each pill: label + an `×`, the whole pill an HTMX GET
per the rail contract using `remove_url`) with a ghost **Clear filters** button
(`clear_url`, same contract) right-aligned. Rendered inside the grid payload so
both producers — the inlined first page and an `htmx/posts` page-0 swap — carry
it, and a Load more (page ≥ 1) never duplicates it.

- [ ] **Step 3: The empty state echoes the filter**

`empty_state.html` currently branches on `q` alone. Rebranch on `filter_desc`:

- `filter_desc` empty → `&gt; no drawings yet.` (unchanged)
- else → `&gt; no drawings match {{ filter_desc }}.` plus a **Reset filters**
  button (`clear_url`, rail contract) — spec S5.

The template consumes `filter_desc` and `clear_url` from the grid's scope; its
`q` field use disappears (keep the `PostGridTemplate.q` field itself — the rail's
search input elsewhere still needs it… verify: `q` is also consumed by
`FeedTemplate`; if nothing else reads the grid's `q`, drop the field and its
assignment in the same commit).

- [ ] **Step 4: The styles**

`style.css`: `.art-filter-row` (flex, wrap, 6px gap, margin under the head),
pills matching the rail's active-pill look with the `×` affordance, ghost button
reusing `hm-btn` secondary sizing; `.art-empty` unchanged.

- [ ] **Step 5: Write the tests**

| Test | Expected |
|---|---|
| `test_active_pills_order_and_labels` | tags `[ink, perspective]`, q `loomis`, collection `studies` → labels `["ink", "perspective", "\"loomis\"", "studies"]` |
| `test_pill_remove_url_drops_only_its_part` | pill `ink` → url contains `tags=perspective`, `q=loomis`, `collection=studies`, not `ink` |
| `test_q_pill_remove_keeps_tags` | q pill's url has no `q=` pair, keeps `tags=` |
| `test_vis_gets_no_pill` | `vis: Some(["hidden"])` only → `active_pills` empty |
| `test_clear_url_keeps_preview` | `active_pills`/`clear_url` with `preview = true` → `"/artportfolio/htmx/posts?visitor=1"` |
| `test_filter_row_renders_on_page_0` | route test: GET `/artportfolio/htmx/posts?page=0&tags=ink` → body contains the pill row and `Clear filters`; `?page=1&tags=ink` → does not |
| `test_empty_state_names_the_filter` | seed nothing; GET `…?page=0&tags=ink&q=loomis` → body contains `no drawings match ink + &quot;loomis&quot;.` (Askama escapes the quotes) and `Reset filters` |

- [ ] **Step 6: Commit**

```bash
git add src/routes/feed.rs templates/artportfolio/partials/post_grid.html templates/artportfolio/partials/empty_state.html static/style.css
git commit -m "feat(artportfolio): active filters you can see and remove"
```

**Acceptance:** `./scripts/verify.sh` — all green.

**Browser checkpoint 1 — the filter UX.** `cargo run`, seed a couple of
collections/tags via curl or the admin routes, then: click a collection → the
grid filters and the address bar reads `/artportfolio?collection=…`; add a tag →
both survive a reload and a Load more; the active row/pills render; remove a
pill → it leaves the URL; clear → back to `/artportfolio`; a search typed with a
tag active keeps the tag. Structure and geometry only (environment limits in
Global Constraints).

---

### Task 3: The card popovers — pencil and folder-plus

**Class:** A

**Why this class:** Askama markup and template-struct fields, all compiler-gated;
the behaviour they trigger was tested in plan A, and the show/hide logic is Task
4's.

**Files:**
- Modify: `templates/partials/post_card.html`
- Modify: `templates/artportfolio/partials/card_edit_popover.html`
- Modify: `templates/artportfolio/partials/collection_checklist.html`
- Modify: `static/style.css`

**Interfaces:**
- Consumes: `GET /api/admin/posts/{id}/edit`, `GET /api/admin/posts/{id}/collections`
  (plan A).
- Produces (Task 4's JS binds to these):
  - trigger buttons: `class="art-card-controls__btn" data-art-pop="edit"` /
    `data-art-pop="collections"`
  - container: `<div class="art-pop" id="art-pop-{{ post.id }}" hidden>`

- [ ] **Step 1: The card markup**

Inside the existing `{% if is_admin %}` block of `post_card.html`
(`templates/partials/post_card.html:27`), extend `.art-card-controls` with two
buttons after the visibility trio — same 28px IconButton shape:

- pencil (`✎`), `title="Edit caption & tags"`, `data-art-pop="edit"`,
  `hx-get="/api/admin/posts/{{ post.id }}/edit"`,
  `hx-target="#art-pop-{{ post.id }}"`, `hx-swap="innerHTML"`
- folder-plus (`⊞`), `title="Collections"`, `data-art-pop="collections"`,
  `hx-get="/api/admin/posts/{{ post.id }}/collections"`, same target/swap

and, after the controls `</div>`, the shared container
`<div class="art-pop" id="art-pop-{{ post.id }}" hidden></div>`. One container
per card, filled by whichever fragment was last fetched — that is also what makes
"one popover at a time" cheap for Task 4. A visitor's card must stay
byte-identical (everything sits inside the existing guard).

- [ ] **Step 2: Dress the two fragments**

`card_edit_popover.html` and `collection_checklist.html` keep plan A's ids,
fields and HTMX attributes exactly, gaining classes: `.art-pop__form`, labelled
textarea + input rows, a small primary Save; checklist rows as
label-wrapped checkboxes with counts optional. Since the PATCH response swaps
`closest .hm-post` outerHTML, the whole card re-renders and the popover naturally
disappears with it — no close plumbing needed on Save. A membership toggle swaps
only the checklist, so the popover stays open across toggles — correct, an admin
often files a post into two collections at once.

- [ ] **Step 3: The styles**

`style.css`, under `body.art-page`: `.art-pop` absolutely positioned within
`.hm-post__media` (which is already the positioning context for the badge and
controls), top-left inset under the control cluster, `#17141F`, 1px
`rgba(242,238,248,.10)`, radius 8, shadow, max-width ~260px, padding 12px;
`.art-pop[hidden]` stays `display: none` (the attribute must win over the
positioning rules — do not add a bare `display` that defeats it). Under 900px the
cluster is always visible (slice-2 rule) and the popover may span the card width.

- [ ] **Step 4: Commit**

```bash
git add templates/partials/post_card.html templates/artportfolio/partials/card_edit_popover.html templates/artportfolio/partials/collection_checklist.html static/style.css
git commit -m "feat(artportfolio): a pencil and a folder on every card"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 4: The popover behaviour and the palette entry

**Class:** A

**Why this class:** Static JS, syntax-gated by `node --check` in `verify.sh`; the
one historic failure mode here (a nested palette literal, `c72d614`) is exactly
what that gate catches.

**Files:**
- Modify: `static/artfeed.js`
- Modify: `static/palette.js`

**Interfaces:**
- Consumes: `data-art-pop` triggers and `#art-pop-{id}` containers (Task 3).

- [ ] **Step 1: Open/close in `artfeed.js`**

Inside `artfeedInit` (which is already existence-guarded by `window.artfeedBound`
and bound on both `DOMContentLoaded` and `htmx:afterSwap` —
`static/artfeed.js:43` — so this inherits the hx-boost rule for free), add one
delegated click listener:

- click on a `[data-art-pop]` button → close any open popover
  (`document.querySelector('.art-pop:not([hidden])')`), then un-`hidden` the
  clicked card's own container (`closest('.hm-post').querySelector('.art-pop')`).
  HTMX fills it via the button's own `hx-get` — the JS only manages visibility.
  Clicking the same button while its popover is open closes it (toggle).
- click anywhere outside an open popover and outside `[data-art-pop]` → close it.

And in the existing `keydown` listener, **before** the search-field Esc branch
(`static/artfeed.js:57`): if a popover is open, `Escape` closes it and returns —
one Esc closes the popover, the next leaves the search field, matching the
one-at-a-time model. The `artfeedIsTyping` guard stays after it, so Esc works
while focus is in the popover's textarea.

- [ ] **Step 2: The palette entry**

One **flat** object in the `COMMANDS` array (`static/palette.js:5`) — the nested
literal of `c72d614` is the failure to not repeat:

```js
  {
    label: 'Filter drawings by tag',
    keywords: ['filter', 'tags', 'tag', 'collection', 'rail', 'search'],
    action() {
      const rail = document.querySelector('.art-rail');
      if (rail) {
        const target = rail.querySelector('.art-rail__tags .art-pill') || document.getElementById('art-search');
        if (target) target.focus();
      } else {
        location.href = '/artportfolio';
      }
    },
  },
```

(Adjust the two selectors to the class names Task 1 actually shipped; the
fallback chain — first tag pill, else the search field, else navigate — is the
contract.) Not `adminOnly`: visitors filter too.

- [ ] **Step 3: Commit**

```bash
git add static/artfeed.js static/palette.js
git commit -m "feat(artportfolio): popovers that behave, and a palette way in"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 5: Docs, the slice-end checkpoint, and the final pass

**Class:** A

**Why this class:** Prose and a checklist; the checkpoint verifies, it does not
build.

**Files:**
- Modify: `CLAUDE.md` — feed route notes, test counts
- Modify: `docs/WORKTREES.md` — the artportfolio card

- [ ] **Step 1: Update the docs**

`CLAUDE.md`: the artportfolio route bullet gains the filter params and the rail;
**re-measure the test counts** (`cargo test --workspace 2>&1 | grep "test result"`).
`docs/WORKTREES.md`: Status → slice 3 complete; Next → slice 4 (multi-upload
tray), noting it codes against the frozen `#art-head-label` seam and that rail
counts going stale after card edits is accepted behaviour, not slice-4 debt.
Land the WORKTREES update on `master` too, per that file's own header.

- [ ] **Step 2: Commit**

```bash
git add CLAUDE.md docs/WORKTREES.md
git commit -m "docs: slice 3 lands — collections, tags, the full rail"
```

**Acceptance:** `./scripts/verify.sh` — all green.

**Browser checkpoint 2 — the whole slice**, logged in and out, with posts in all
three visibility states, two collections and a few tags:

1. Logged out: rail shows only collections/tags with public members; no `+`, no
   visibility group, no card cluster; `?vis=hidden` in the bar changes nothing.
2. Logged in: visibility checkboxes subset the feed and the head counts follow;
   the `+` input creates a collection (it appears in the rail with count 0);
   creating it again reports the 409 message.
3. Pencil: edit a caption and tags, Save — the card re-renders with the new
   caption, no reload; reopen the pencil — tags come back prefilled.
4. Folder-plus: check a collection, watch the checklist re-render checked; the
   rail count is allowed to be stale until the next navigation (accepted).
5. Combine: collection + tag + search → reload → Load more → everything still
   filtered; remove pills one at a time down to `/artportfolio`.
6. Esc closes an open popover; a second Esc leaves the search field; only one
   popover opens at a time.

Record what could not be verified under the environment limits (Global
Constraints) — colour tones and hover fades need human eyes.

---

## Before the plan is done

- Every task classed — no C tasks here; the whole plan's diff gets **one review
  on the most capable model**, its only review. Point it at:
  1. The rail/search composition — can any control's URL drop another active
     filter part? (`hx-include="#art-rail-state"` and the toggle builders are
     the two places.)
  2. The frozen seam — `post_grid.html` changed around the OOB label; did its
     bytes survive?
  3. A visitor's card and rail — byte-free of admin affordances?
  4. `rail_collections.html` — consumed field names identical from both its
     includers (`FeedTemplate` scope and `RailCollectionsTemplate`)?
- Plan A + plan B together close slice 3; the SDD ledger entry should say so and
  hand slice 4 the seam note.
