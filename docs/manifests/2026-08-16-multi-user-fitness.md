# Container — Multi-user fitness tracker

**Status:** 🟢 pack 1 complete, gate green — pack 2 not started
**Stream:** `feat/multi-user-fitness`, worktree
`~/projects/drawingportfolio.worktrees/multi-user-fitness`, based on `dev`
**Opened:** 2026-08-16

## Goal

Turn `/fitness` from a single-person tracker into one that several people
use, each with their own entries, weights, targets, recipes and portion
preferences — over a **shared** food catalog. A second person logs in with
name + PIN; the owner keeps passkeys and remains the only art-portfolio
admin.

## Why this is a container, not a pack

There is no user concept anywhere in the portfolio crate today. `sessions`
has no owner column, `passkey_credentials` has no owner column, and every
nutrition table is implicitly single-user — `weights.date` is the primary
key, `targets` is a one-row table with `CHECK (id = 1)`. Adding a second
person touches the auth layer, the schema, ~25 db functions, ~30 handlers,
the sqlx offline cache and the test suite.

<details>
<summary><b>Decisions taken at container open</b></summary>

| Question | Decision | Notes |
|---|---|---|
| How does a second person log in? | **Name + PIN**, alongside passkeys | User's call, 2026-08-16. This part of the site is explicitly "not super critical to be super secure". Port the argon2 + `spawn_blocking` pattern from `drinkinggame/src/auth.rs` rather than inventing one. |
| Session persistence | 30-day cookie, same as today | `make_session_cookie` already sets `Max-Age=2592000`. No re-login per visit. |
| Food catalog | **Shared** | Barcode/OpenFoodFacts lookups compound across users. |
| Custom portions | **Per-user** | User's call. `custom_portions` (mig 006) and `default_portion_g` (mig 010) move off `food_items` into a per-user table. |
| Favourites | **Per-user** — *Claude's call, flag if wrong* | `is_favourite` (mig 010) is the same class of problem as portions: two users would fight over one flag. Folded into the same per-user table. |
| `category` | Stays shared | A property of the food, not of the person eating it. |
| Existing data | Backfills to user 1 = the owner | Non-destructive; your history stays yours. |
| Roles | `owner` \| `member` | Owner = art admin + fitness. Member = fitness only, never `/admin`. |
| Account creation | Owner-created, from the user-management page (pack 3) | Avoids a public write endpoint. Opening it to self-claim later is a small change. |
| Admin access | `is_owner` (one, immutable) + `is_admin` (grantable) | User's call, 2026-08-16. Art-portfolio admin is a **grantable permission**, not a synonym for "logged in". Effective admin = `is_owner \|\| is_admin`. |
| Who can grant admin? | **Owner only** — *Claude's call, flag if wrong* | An admin manages art; they cannot mint more admins. Keeps the privilege story acyclic and means a granted admin can never lock you out. |

</details>

<details>
<summary><b>Constraints found during survey</b></summary>

1. **`OptionalAuth(pub bool)` is the real hazard.** It means "has a valid
   session", and `feed.rs` converts that into `Viewer::Admin` while
   `tasks.rs` uses it to show management controls. The moment a
   fitness-only member holds a session, they would see unlisted and hidden
   art posts plus task-management UI. This leak is invisible from the
   nutrition code — it is why pack 1 exists and why it comes first.
2. **`login_finish` does not know who logged in.** `src/routes/auth.rs:204`
   is `Ok(_result) =>`, and `login_start` hands *every* passkey to the
   ceremony. The authenticated credential id has to be read off the result
   before `sessions.user_id` has anything to populate from.
3. **Two tables need rebuilds, not `ADD COLUMN`.** SQLite cannot alter a
   primary key. `weights` (`date TEXT PRIMARY KEY`) and `targets`
   (`CHECK (id = 1)` plus a seeded row) both need create-new / copy /
   drop / rename. CLAUDE.md's `IF NOT EXISTS` / `let _ =` idempotence
   idiom does not cover this — write those two carefully.
4. **The `Viewer` precedent applies directly.** CLAUDE.md already documents
   the answer for posts: a *required* parameter on every reading function,
   so omitting it is a compile error. Do the same with a `UserId` newtype
   rather than inventing a second pattern. It is what catches the easy-to-
   miss aggregates: `copy_day_entries`, `get_most_logged_between`,
   `get_protein_by_date_range`, `get_calories_by_date_range`,
   `get_logged_dates_desc`, `get_recent_foods`, `get_item_log_history`.
5. **Two mechanical costs, budgeted not discovered.** The sqlx offline-cache
   ritual (`DATABASE_URL=sqlite:portfolio.db cargo sqlx prepare`, commit
   `.sqlx/`) — ~30 changed queries will not compile without it — and the
   nutrition tests in `src/db.rs` / `src/routes/nutrition.rs`, whose
   signatures all break at once.
6. **`argon2` is a `drinkinggame` dependency only.** The root crate's
   `Cargo.toml` needs it added in pack 3.

</details>

## Packs

Planned one level deep: all four named, items drafted for pack 1 only.

### Pack 1 — Identity spine + admin permission 🟢 complete

Sessions stop meaning "the admin" and start meaning "this person, with these
permissions". No second user exists yet; the point is that the *shape* is
right and the `OptionalAuth` leak is closed **before** anyone can walk
through it.

The permission model:

```
users.is_owner   exactly one row, immutable — you. Cannot be demoted or deleted.
users.is_admin   grantable / revocable, owner only. Art-portfolio management.
effective admin  is_owner || is_admin   (nothing reads the raw column)
```

`/fitness` needs only a valid session. `/admin`, the art mutations and the
task mutations need effective admin. Granting admin is owner-only, so a
granted admin can never lock you out or mint more admins.

**Observable:** log in as today and `/fitness` shows a "signed in as …"
chip; everything else behaves exactly as before. A row inserted with
`is_admin = 0` gets `/fitness` but is refused `/admin`, the task management
controls, and hidden/unlisted art posts.

| # | Item | Done when |
|---|---|---|
| 1.1 | Migration 015: `users` (`id`, `name` UNIQUE COLLATE NOCASE, `pin_hash` NULL, `is_owner`, `is_admin`, `created_at`) + partial unique index enforcing one owner; seed owner as id 1 | Migration is idempotent on the dev DB; owner row exists; a second `is_owner = 1` insert fails |
| 1.2 | `sessions.user_id` + `passkey_credentials.user_id`, backfilled to 1. **`get_session` joins `users` and returns `user_id`/`is_owner`/`is_admin` in one query** | The existing session and passkey keep working across the migration; the extractor costs one round-trip, not two — it runs on every HTMX fragment swap on the fitness page |
| 1.3 | `login_start`/`login_finish` resolve the authenticated credential to its user | `login_finish` reads `AuthenticationResult::cred_id()` (today discarded as `Ok(_result)`, auth.rs:204) through the *same* serialize-to-string transform `register_finish` used to store it, and binds the session to that credential's user |
| 1.4 | `AuthSession` becomes `{ session_id, user_id, is_owner, is_admin }` with an `is_effective_admin()` helper; add `RequireAdmin` + `RequireOwner`. **Includes the mechanical sweep of ~30 `AuthSession(_): AuthSession` patterns in `nutrition.rs` to `_: AuthSession`** | `./scripts/check.sh` green — the struct change breaks every nutrition destructure in the same item, so the sweep is part of it, not deferred debt |
| 1.5 | **Risky — reviewed individually.** Rename `OptionalAuth` → `OptionalAdmin`; the impl loads the joined user and computes `is_owner \|\| is_admin` | The rename forces a visit to all 7 sites (`hub.rs:16`, `tasks.rs:342,364`, `feed.rs:710,773,855,889`) — but the *real* fix is in `middleware.rs`: assert the extractor no longer returns `db::get_session(..).is_some()`. A non-admin session sees a visitor's art feed |
| 1.6 | **Risky — reviewed individually.** `RequireAdmin` on all 12 `admin.rs` handlers and the 5 `tasks.rs` mutations. **Rejection is 404 for a valid non-admin session, redirect only when there is no session at all** | A non-admin session gets **404** on every one, asserted by test on the status code. Inheriting `AuthSession`'s `Redirect::to("/admin/login")` would bounce a signed-in member to a login they are already past, and confirm the route exists — the visibility model's own rule (CLAUDE.md: `hidden` 404s, never 403) applies here too |
| 1.7 | "Signed in as …" chip on `/fitness` + `/fitness/week` | Name renders from the session's user; makes the pack observable in a browser |
| 1.8 | Stream registered in `docs/WORKTREES.md` (on `master`) and 🚧 pointer pre-placed in `docs/INVENTORY.md` §Fitness | Both files name this manifest |

<details>
<summary>Pack 1 gate</summary>

Item gate `./scripts/check.sh` after each item; pack gate `./scripts/verify.sh`
before review. Baseline to beat: **792 tests**, **21 clippy warnings**
(all in `drawingportfolio`; `drinkinggame` clean). Items 1.3–1.6 are
auth/session territory — flagged for individual review per the workflow.

</details>

### Pack 2 — Nutrition data scoping 🟡 in progress

A `UserId` newtype becomes a required parameter on all 29 nutrition db
functions, so a missed call site is a compile error rather than another
user's data — the same trick `Viewer` plays for posts.

**Observable:** two users logging the same day on the same server see only
their own entries, weights, targets and portion preferences — over the same
food catalog.

| # | Item | Done when |
|---|---|---|
| 2.1 | Migration 017: `user_id` on `meal_entries` and `recipes` (`NOT NULL DEFAULT 1` — the default *is* the backfill) | Existing rows belong to the owner; a fresh entry carries its logger |
| 2.2 | **Risky.** Migration 018: rebuild `weights` (PK → `(user_id, date)`) and `targets` (PK → `user_id`), each guarded by a `column_exists` check | Two users log the same date without collision. **The guard is the point**: a create/copy/drop/rename that re-runs on the next boot would copy every user's rows back to user 1 and drop the original — `let _ =` tolerance cannot express this, so the guard is explicit and tested by a double-boot |
| 2.3 | Migration 019: `user_food_prefs (user_id, food_item_id, is_favourite, default_portion_g, custom_portions)`; migrate the owner's values off `food_items` and drop the three columns | The catalog is shared; favourites and portions are not. Two users disagree about a food without overwriting each other |
| 2.4 | `UserId` newtype, required on all 29 nutrition db functions | Omitting it is a compile error. Catches the aggregates that read like they have no owner: `copy_day_entries`, `get_most_logged_between`, `get_protein_by_date_range`, `get_calories_by_date_range`, `get_logged_dates_desc`, `get_recent_foods`, `get_item_log_history` |
| 2.5 | **Risky — reviewed individually.** Ownership in the `WHERE` clause of every id-addressed read and mutation | `update_meal_entry`, `get_meal_entry`, `delete_meal_entry`, `delete_recipe`, `log_recipe` all take a bare id today. Accepting a `UserId` and not filtering on it is the failure mode: someone else's entry stays editable by guessing a sequential id. Asserted by a test that tries exactly that |
| 2.6 | Handlers thread `session.user_id` through (~30 sites) | `AuthSession` is bound rather than discarded; no handler invents a user id |
| 2.7 | Isolation tests, `.sqlx/` regen, pack gate | Two users, same day, zero bleed across entries, weights, targets, recipes and prefs |

### Pack 3 — Accounts: name + PIN login, and the management page ⚪ not started

Port `drinkinggame/src/auth.rs` (argon2, `spawn_blocking`, PIN verify, PIN
change) into the portfolio crate — `argon2 = "0.5"` moves into the root
`Cargo.toml`. Login page gains a name+PIN path beside the passkey button;
PIN attempts are rate-limited.

Plus the **user-management page** (owner-only, `/admin/users`): list users,
create one with a name and an initial PIN, reset a PIN, grant or revoke
admin, delete. The owner row renders without demote or delete controls —
the invariant is enforced in `db.rs`, not just hidden in the template.

Creating an account and granting it admin is one page and one pack; that is
why management sits here rather than in pack 4. It is deliberately *after*
pack 2 — a user created before their data is scoped would open straight
into the owner's food log.

**Observable:** you create an account from `/admin/users`, hand over the
name and PIN, and that person logs in on their own phone to their own empty
`/fitness`. Grant them admin and `/admin` opens for them; revoke it and it
closes.

### Pack 4 — Multi-user polish ⚪ not started

Member-facing account page (change your own PIN, rename), sign-out that
returns to the right login screen, and whatever per-user surfacing packs
1–3 deferred.

**Observable:** a member manages their own account without the owner and
without the database.

## Ledger

### Pack 1 — landed 2026-08-16

- [x] **1.1** Migration 015: `users` + one-owner partial index + seeded owner.
- [x] **1.2** Migration 016: `sessions.user_id`, `passkey_credentials.user_id`.
      `get_session` joins `users`; `create_session`/`save_credential` take a
      user id.
- [x] **1.3** `login_finish` resolves `AuthenticationResult::cred_id()` to its
      user and binds the session to it.
- [x] **1.4** `AuthSession { user_id, user_name, is_owner, is_admin }` +
      `is_effective_admin()`; 28 `nutrition.rs` patterns swept.
- [x] **1.5** `OptionalAuth` → `OptionalAdmin`, impl now computes effective
      admin. All 7 sites re-expressed.
- [x] **1.6** `RequireAdmin` on 12 `admin.rs` handlers + 5 `tasks.rs`
      mutations, rejecting with 404.
- [x] **1.7** Signed-in-as chip on `/fitness`, name in the `/fitness/week`
      kicker.
- [x] **1.8** Stream registered in `WORKTREES.md`; 🚧 pointer in
      `INVENTORY.md` §Fitness.

**Pack gate:** `./scripts/verify.sh` → `VERIFY OK — fmt, clippy, tests, JS
syntax all clean.` Workspace tests **792 → 801** (+9). Clippy **20** distinct
diagnostics against a documented baseline of 21 — one *fewer*, because four
of `Session`'s fields are now read, collapsing two dead-code warnings into
one. Measured after a full `cargo clean`: clippy does not re-emit diagnostics
for crates it did not recompile, and incremental runs during this pack
reported 24, 26, 24 and 22 for the same tree. `SQLX_OFFLINE=true cargo check`
passes; `.sqlx/` regenerated (5 added, 3 removed).

CLAUDE.md was corrected in the same pack — it is loaded into every session,
and its stale lines ("all seven behind `AuthSession`", the middleware
description, migration count, test count, clippy baseline) would have told
the next contributor to reintroduce exactly this leak.

<details>
<summary><b>Deviations from the plan</b></summary>

- **`RequireOwner` was not added.** The plan named it alongside
  `RequireAdmin`, but pack 1 has no owner-only route, and an unused extractor
  is a dead-code warning no test exercises. It lands with the management page
  in pack 3, which is what needs it.
- **`RequireAdmin` is a unit struct**, not `RequireAdmin(AuthSession)`, for
  the same reason — no handler yet asks *which* admin. All 17 sites bind it
  as `_`, so widening it later touches only the definition.
- **`AuthSession` carries no `session_id`.** Nothing reads it (`logout` parses
  the cookie itself). Pack 4's "sign out everywhere" can add it back.
- **Fixed in passing:** `register_finish` derived its storage key with
  `.unwrap_or_else(|| Uuid::new_v4())`, so a serialisation failure would have
  stored a passkey under a key login could never re-derive — harmless while
  login only asked "is this anybody", load-bearing now that it asks "who".
  Both sides now share `cred_id_key()` and fail explicitly.

</details>

<details>
<summary><b>Verification evidence</b></summary>

Nine tests added. The four that carry the security claims:

- `test_member_session_sees_the_visitor_feed` — a signed-in member sees
  neither hidden nor unlisted posts, on the page **and** the htmx fragment.
- `test_member_session_gets_the_visitor_api_and_permalink` — the other two
  `OptionalAdmin` handlers, which carry contracts of their own: the JSON API
  excludes unlisted entirely, the permalink 404s hidden but serves unlisted.
  All four handlers share the extractor, so this was very likely already
  right — asserted anyway, because a visibility leak is not something to
  infer from a sibling passing.
- `test_member_session_is_refused_every_admin_route` — all 11 admin routes
  return exactly 404, and neither a collection nor a visibility change
  survives the attempt.
- `test_granted_admin_reaches_admin_routes` — the same member gets 404, then
  `UPDATE users SET is_admin = 1` alone opens `/admin` with a 200.

Plus `test_single_owner_invariant` (a second `is_owner = 1` insert is
rejected by the database), `test_session_carries_user_flags`,
`test_orphaned_session_reads_as_logged_out` (deleting a user logs its
sessions out, via the INNER JOIN), `test_credential_resolves_to_its_user`,
and `test_migrations_are_idempotent_across_boots` — 015 `.expect()`s rather
than shrugging, and `run_migrations` runs on every boot, so a non-idempotent
re-run would show up as the site failing to start after a deploy restart.
Every other test builds its pool once and never exercises that path.

**Live server, two real accounts** (owner `Hampter` id 1, member `Alex`
id 2, sessions inserted directly into the dev DB):

| Path | as member | as owner |
|---|---|---|
| `/fitness` | 200 | 200 |
| `/admin` | **404** | 200 |
| `/htmx/admin/posts` | **404** | 200 |
| `/fitness` (no session) | 303 → `/admin/login` | — |

Each account's `/fitness` renders its *own* name in the chip — **and nothing
else about it is its own.** Alex's `/fitness` shows the owner's entries,
targets and weights, because no nutrition query filters by user yet. That is
correct for pack 1 and it is exactly what pack 2 fixes; it is called out here
so this table is not misread as data isolation.

**Browser check done** (the workflow's rule for user-visible changes):
`/fitness` in a real browser gives `.fitness-whoami` present,
`display: flex`, text "SIGNED IN AS Hampter", name colour
`rgb(233, 233, 237)` (`--noc-text`), `body.fitness-dark` applied. Screenshots
are unavailable in this environment — the pane does not composite frames when
backgrounded — so this was read off the live DOM and computed styles rather
than by eye.

**Mutation-checked.** The leak test was verified to actually catch the bug:
reverting `OptionalAdmin`'s impl to the old `load_session(..).is_some()` and
re-running gave `test_member_session_sees_the_visitor_feed ... FAILED` at
`feed.rs:1596` (221 passed, 1 failed), then the correct impl was restored and
the suite went green again. The rename alone does **not** fix the leak — the
semantics inside `middleware.rs` do, and this is the proof.

</details>

<details>
<summary><b>Debt handed to later packs</b></summary>

- **`nutrition.rs`'s `is_admin` flag is a misnomer** — both page handlers pass
  a hardcoded `true`, and it gates food-library edit/delete controls. It means
  "you are logged in", which is now correct for every user but reads as the
  exact conflation this pack removed. Not a leak (every nutrition route is
  session-gated); rename it in pack 4.
- **The owner's seeded name is `admin`.** Renaming arrives with the account
  page in pack 4; until then the chip reads "Signed in as admin".
- **Local dev DB carries two seeded accounts** — `Hampter` (owner) and `Alex`
  (member), plus the sessions `devsession` / `membersession`. `portfolio.db`
  is gitignored, so this is worktree-local; useful for pack 2, and worth
  knowing about before reading anything into a two-user query result.

</details>
