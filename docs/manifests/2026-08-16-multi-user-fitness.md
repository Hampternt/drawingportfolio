# Container — Multi-user fitness tracker

**Status:** 🟢 **complete** — all four packs landed, gate green
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

### Pack 2 — Nutrition data scoping 🟢 complete

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

### Pack 2 ledger — landed 2026-08-16

- [x] **2.1** Migration 017 — `user_id` on `meal_entries` and `recipes`, plus
      two indexes.
- [x] **2.2** Migrations 018 and 019 — `weights` re-keyed to
      `(user_id, date)`, `targets` to `user_id`, **one file and one
      `column_exists` guard each**.
- [x] **2.3** Migration 020 — `user_food_prefs`; the three per-user columns
      dropped from `food_items`.
- [x] **2.4** `UserId` newtype required on all 29 nutrition db functions.
- [x] **2.5** Ownership in the `WHERE` clause of every id-addressed read and
      mutation.
- [x] **2.6** 80 handler call sites thread `session.user()`; `week_for` takes
      the owner explicitly.
- [x] **2.7** Isolation tests, `.sqlx/` regen, gate.

**Pack gate:** `VERIFY OK`. Workspace tests **801 → 809**. Clippy **19**
distinct diagnostics, re-measured after a full `cargo clean` (down one more
from pack 1's 20 — `FoodItem`'s per-user fields are now read through the
join). `SQLX_OFFLINE=true cargo check` passes.

<details>
<summary><b>The rebuild guard, and why it is one guard per table</b></summary>

018 and 019 were originally a single file rebuilding both `weights` and
`targets` behind one `column_exists(weights, user_id)` check. That is subtly
wrong, and it would only ever have bitten in production:

DDL statements auto-commit individually — there is no enclosing transaction
around a `sqlx::query(include_str!(..))` batch. So a run that renamed
`weights` and then failed before renaming `targets` would leave the next boot
seeing `weights.user_id` present, skipping the whole file, and running against
a `targets` table still on its old `id` primary key — with the guard
cheerfully reporting "already done" and every targets query failing at runtime
against a baked `.sqlx` cache.

Split into one file and one guard each, checking exactly the table it
rebuilds. `targets` is an exact discriminator: the old schema keys on `id`,
the new one on `user_id`.

Also worth naming for the deploy: 020's `ALTER TABLE ... DROP COLUMN` needs
SQLite 3.35+ (2021) and runs under `.expect()` on the startup path, so an
older SQLite would be a boot panic rather than a degraded page. sqlx bundles a
far newer one, so this is a note, not a risk.

</details>

<details>
<summary><b>Verification evidence</b></summary>

**The upgrade path was exercised against a populated database**, not just a
fresh one: the dev DB was seeded with a food, a favourite, custom portions, a
weight, a non-default target and a meal entry *before* 017–019 ran. All of it
survived and was attributed to the owner; `food_items` lost its three personal
columns; `weights` and `targets` came back re-keyed; and two users could then
write the same date without collision.

**Live two-account walkthrough.** Owner (`Hampter`) and member (`Alex`) on the
same date, `2026-08-15`:

| | owner | member |
|---|---|---|
| calorie ring | 304 of 2500 · 12% | 0 of 2400 · 0% |
| protein rail | 10 / 165 g | 0 / 165 g |
| breakfast slot | Oats, 80.0g, 304.0 | empty |
| streak | 1 | 0 |
| weight | logged | no weight |
| Oats favourite | `fav-btn is-fav` | `fav-btn` |
| food library | 4 cards | 4 cards |

The last two rows are the pack's actual claim: the **catalog is shared** (both
see four foods) while the **opinion about it is not** (only the owner's card
carries `is-fav`). The member's 2400 is the default fallback — pack 2 seeds no
targets row per user.

**IDOR, end to end against the running server.** Alex, signed in, aims at the
owner's entry id 1:

- `GET /fitness/htmx/entries/1/edit` → **404**, the same answer an unknown id
  gets.
- `PUT /api/nutrition/entries/1` with `grams=9999` → 200, and the owner's
  entry is still `80.0g / 304.0`.
- `DELETE /api/nutrition/entries/1` → 200, and the owner's day still has its
  entry.

The 200s are deliberate: these are HTMX fragment endpoints and the response is
Alex's *own* unchanged day, so a probe learns nothing and the UI does not go
stale on a double click. Nothing was mutated.

**Mutation-checked**, as in pack 1 — four separate mutations, each restored
after:

| removed | test that caught it |
|---|---|
| `AND user_id = ?` from `update_meal_entry` + `delete_meal_entry` | `test_entry_addressed_by_id_is_not_reachable_by_another_user` |
| the `EXISTS` ownership gate from `delete_recipe` | `test_recipes_are_private_to_their_owner` — *"another user's delete stripped the recipe's items"* |
| the ownership `JOIN` from `log_recipe` | `test_recipes_are_private_to_their_owner` — *"another user's recipe must not be loggable"* |

The recipe test originally asserted only that the recipe still *existed* after
another user's delete attempt. That passes even with the `EXISTS` gone —
`delete_recipe` removes the child rows first, so a broken guard leaves the
recipe row standing with its items silently stripped. The `item_count`
assertion was added for exactly that, and is the assertion mutation 2 trips.
All four functions would otherwise compile, typecheck and pass every
single-user test with their ownership filter missing.

</details>

<details>
<summary><b>Fixed in passing: an empty day rendered "-0"</b></summary>

The member's first screen read `-0 of 2400 cal · -0%` and `-0 / 165 g`, with
the macro bars at `width:-0%`.

`f64`'s `Sum` identity is **negative** zero — `-0.0 + x == x` holds for every
`x` including `-0.0` itself, which `0.0` does not satisfy — so summing an
empty day yields `-0.0`, and `{:.0}` formats that as `-0`. `rail_pct`'s
`clamp(0.0, 100.0)` is powerless here: `-0.0 >= 0.0` is true, so the clamp
passes it straight through.

The bug predates multi-user — the owner sees it on any empty date — but pack 2
is what makes it matter, since an empty day went from "a date you scrolled
back to" to "the first screen every new member sees". Fixed by summing into
`+ 0.0`, covered by `test_empty_day_renders_zero_not_negative_zero`, and
confirmed live.

</details>

<details>
<summary><b>Deviations and debt</b></summary>

- **`delete_food_item` takes no `UserId`.** There is one shared catalog, so
  there is one delete, and no per-user variant to get wrong. It now also
  clears the orphaned `user_food_prefs` rows, which do not cascade (the pool
  runs with `foreign_keys` off) and would otherwise re-attach to whatever id
  `AUTOINCREMENT` issued next. That *any* signed-in user can delete a shared
  food is carried over from single-user and is the one place members can
  affect each other; restricting it is a pack 4 question.
- **Nutrition facts are shared, opinions are not.** `update_food_item` writes
  name/macros/category to the catalog row and favourite/portions to the
  editor's own prefs row. One person correcting a calorie count fixes it for
  everyone, which is the intent of a shared catalog.
- **The `is_admin` misnomer in `nutrition.rs` survives** — still hardcoded
  `true` by both page handlers, still meaning "logged in". Renaming is pack 4.
- **The owner is still named `admin`** unless renamed by hand; the local dev
  DB has them as `Hampter` plus a member `Alex`.

</details>

### Pack 3 — Accounts: name + PIN login, and the management page 🟢 complete

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

### Pack 3 ledger — landed 2026-08-16

- [x] `src/pin.rs` — Argon2 hash/verify through `spawn_blocking`, name and PIN
      validation, lockout constants. Ported from `drinkinggame/src/auth.rs`.
- [x] Migration 021 — `failed_pin_attempts` and `locked_until` on `users`.
- [x] `POST /api/auth/login/pin` + the login page's second path.
- [x] `RequireOwner` (deferred from pack 1 for want of a caller) and
      `/admin/users`: list, create, reset PIN, grant/revoke admin, delete.
- [x] `/fitness/account` — a member's own page.

<details>
<summary><b>What carries the security weight, and why</b></summary>

**The lockout is the control, not Argon2.** A 4-digit PIN is 10,000 guesses;
Argon2 only decides how long that takes. Five wrong PINs lock the account for
15 minutes, and the counter lives **in the database** — an in-memory one would
reset on the restart that every deploy performs, handing an attacker a fresh
budget for the price of waiting.

**The change-PIN form counts failures against the same lockout**, so it cannot
be used as an unmetered oracle for guessing a PIN the session holder does not
know — the case where someone picks up an unlocked phone. The consequence is
worth knowing before it turns up as a support request: a member who guesses
wrong five times on *this* form locks themselves out of logging in from
anywhere else, and the only way back is the owner resetting their PIN. That is
the right trade — the alternative is an unmetered guessing oracle behind a
borrowed session — but it is a trade, not a free win.

**A wrong name and a wrong PIN give the same answer.** Distinguishing them
hands over a list of valid account names. The lockout message is the one
deliberate exception: by the time it fires the attacker already knows the
account exists, and without it a member has no way to understand why a PIN
they know is correct has stopped working.

**An absent hash never verifies.** The owner authenticates with a passkey and
may have `pin_hash` NULL forever; `get_user_by_name` collapses that to `""`.
If a malformed hash returned `true`, an empty PIN would log in as the owner.
Asserted directly in `test_absent_or_malformed_hash_never_verifies`.

**`RequireOwner`, not `RequireAdmin`, on user management.** This is the
distinction the pack exists for and the one most likely to be assumed away:
an admin who could reach these routes could grant admin to anyone and delete
accounts. Mutation-checked — swapping the extractor fails
`test_admin_is_still_refused_user_management`.

**The owner invariants are SQL, not template logic.** `AND is_owner = 0` sits
on every destructive statement. The hidden buttons are a courtesy; the SQL is
what holds against a hand-made request. Renaming is deliberately *not* behind
that guard — it is not a privilege change, and migration 015 seeds the owner
as "admin", so they need it.

**`delete_user` clears eight tables by hand.** Nothing cascades (the pool runs
with `foreign_keys` off). A missed table would leave rows keyed to an id
`AUTOINCREMENT` eventually reissues, and the next member created would open
their tracker onto a stranger's food log — so the test asserts emptiness table
by table rather than trusting the delete.

</details>

<details>
<summary><b>Live walkthrough — the whole flow, on a running server</b></summary>

1. Owner creates an account: *"Created Alex. Give them that PIN — it is not
   shown again."*
2. Alex logs in from a **fresh cookie jar** with name + PIN → `{"ok":true}`,
   one session cookie issued.
3. Alex's `/fitness` renders `Signed in as Alex` and `0 of 2400 cal · 0%` —
   their own empty day, not the owner's.
4. `/admin` → 404, `/admin/users` → 404, `/fitness/account` → 200.
5. Five wrong PINs → five identical `Wrong name or PIN`; the sixth →
   `Too many wrong PINs`. The **correct** PIN is then refused too, so the
   lockout is real rather than cosmetic. An unknown account name gives the
   identical message throughout.
6. Owner resets the PIN → lockout cleared, login works again.
7. Owner grants admin → Alex's `/admin` becomes **200** while `/admin/users`
   stays **404**. Revoked → back to 404. This is the acyclic privilege graph,
   demonstrated rather than asserted.
8. Owner aims demote and delete at their own row → *"cannot be changed"*,
   *"cannot be deleted"*, and `/admin` still 200 for them.
9. Alex changes their own PIN: wrong current → refused; right current →
   changed; new PIN logs in; old PIN no longer does.

</details>

### Pack 4 — Multi-user polish 🟢 complete

Renaming (self-service and owner-side), a way to *reach* the account page, and
the `is_admin` misnomer.

**Observable:** a member manages their own account without the owner and
without the database; the owner stops being called "admin".

### Pack 4 ledger — landed 2026-08-16

- [x] `POST /api/account/name` (self) and `/api/admin/users/{id}/name`
      (owner, including their own row — migration 015 seeds them as "admin").
- [x] The signed-in-as chip is now a link to `/fitness/account`, which
      otherwise had no route into it from the UI.
- [x] `nutrition.rs`'s `is_admin` renamed to `can_edit` — it never meant admin,
      it meant "may edit the shared food library", which every signed-in user
      may.
- [x] **A real bug that rename uncovered:** both fitness page handlers passed
      `is_admin: true` to `base.html`, which sets the `IS_ADMIN` constant that
      gates the command palette's admin-only entries. Every fitness member's
      palette therefore offered admin commands. They 404 thanks to pack 1, so
      this was cosmetic rather than an access hole — but it is precisely the
      "logged in means admin" conflation this container set out to remove, and
      it survived three packs by hiding behind a name. Both now pass
      `session.is_effective_admin()`.
- [x] `INVENTORY.md`'s 🚧 placeholder converted to a real entry.

## Container closed — 2026-08-16

**Final gate:** `VERIFY OK`. Workspace tests **792 → 830** (+38). Clippy **19**
distinct, measured after `cargo clean` — *below* the 21 this container
started from, because `Session`, `FoodItem` and the user models gained read
fields along the way.

<details>
<summary><b>End-to-end, from an empty database</b></summary>

`portfolio.db` deleted, app booted: all 21 migrations ran through
`run_migrations` (no `_sqlx_migrations` table — that is the CLI's, so this was
the real deploy path), **zero panics**, owner seeded as `admin`, `weights`
keyed on `(user_id, date)`, `user_food_prefs` present.

`SQLX_OFFLINE=true cargo build --release` — the actual deploy command, not
`check` — succeeds **with no database file present at all**, so the `.sqlx`
cache covers every query this container added. Worth confirming rather than
assuming: `record_failed_pin` binds a runtime-built `'+15 minutes'` string
into `datetime('now', ?)`, and a mistyped parameter there would have compiled
locally against the live DB and failed only on the server.

Then, in order: the owner renamed themselves off "admin" to Hampter → created
an account for Sam → Sam logged in from a clean cookie jar with name and PIN →
Sam logged 100 g of Porridge and favourited it.

| | Sam | Hampter (owner) |
|---|---|---|
| chip | Sam | Hampter |
| 2026-08-16 | **380 of 2400 cal · 16%** | **0 of 2400 cal · 0%** |
| entry | Porridge | — |
| favourite | `fav-btn is-fav` | — |
| sees "Porridge" in the catalog | yes | **yes** |

The last row is the point of the whole design: the food is common property,
everything either person thinks or logs about it is not.

</details>

<details>
<summary><b>What is deliberately still true</b></summary>

- **No public sign-up.** Accounts exist because the owner made one. Opening it
  up later is a small change; the reason not to is that a public write
  endpoint on a personal server buys nothing here.
- **`POST /api/auth/login/pin` is the site's first public, unauthenticated
  write endpoint**, and it has to be — nginx denies `/api/auth/register/`
  externally precisely because passkey registration is localhost-only, whereas
  PIN login is the whole point of PIN login. The lockout it triggers is
  **per-account, not per-IP**, so someone who can guess account names can lock
  every account out at six requests each: denial of service against the
  household, not a breach, and self-healing after 15 minutes or one owner
  reset. `limit_req` on that path in nginx is the fix if it ever stops being
  theoretical.
- **Any signed-in user can delete a food from the shared catalog.** Carried
  over from single-user, and the one place members can affect each other.
  Restricting it needs a rule about who owns a catalog entry, which nothing so
  far has needed.
- **Post ids remain sequential** (the visibility model's own accepted
  trade-off, unchanged) and **`/admin` still shows no visibility badge**,
  because it renders through the legacy `admin_post_card_html` format string.
- **The owner's seeded name is `admin`** on a fresh database until they rename
  themselves — which is now a one-click operation rather than a database edit.

</details>

## Before this merges — the upgrade path

Everything above was verified against a **fresh** database. Production is not
fresh, and migration 020 is the only one in this container that *moves user
data*: it copies the owner's `is_favourite` / `default_portion_g` /
`custom_portions` into `user_food_prefs` and then drops those three columns.
On an empty `food_items` that `INSERT … SELECT` copies **zero rows**, so no
test run in this container — including the empty-database walkthrough — had
ever executed the copy on real input. It also destroys its own source.

<details>
<summary><b>The populated-upgrade test</b></summary>

A database was built at the **pre-multi-user schema** (migrations 001–014
only) and seeded the way the server's actually is: three foods covering all
three shapes of the values being moved (favourite with a portion and custom
portions; favourite with a different portion; **not** favourite, `NULL`
portion, empty portions), a `targets` row, two `weights` rows, meal entries,
a recipe with items, a live session and a passkey credential. Then the real
binary was booted against it, so 015–021 ran through `run_migrations()` with
its guards — not through a CLI.

| | before | after |
|---|---|---|
| Porridge prefs | `1, 170.0, [{"label":"bowl",…}]` | `user_food_prefs(1, 1, 1, 170.0, [{"label":"bowl",…}])` |
| Chicken prefs | `1, 250.5, [{"label":"breast",…}]` | `user_food_prefs(1, 3, 1, 250.5, [{"label":"breast",…}])` |
| Eggs prefs | `0, NULL, ''` | no row — nothing to carry |
| `targets` | `(1, 2400, 180, 250, 70)` | `(user_id 1, 2400, 180, 250, 70)` |
| `weights` | `2026-08-10 → 82.4`, `-15 → 81.9` | both, under `user_id 1` |
| entries / recipes | 2 / 1 | 2 / 1, `user_id 1` |
| session + passkey | `live-session-abc`, `cred-xyz` | both `user_id 1` |

The Eggs row is the load-bearing case: a food with nothing personal attached
gets **no** prefs row, so it only survives if the readers `LEFT JOIN` with the
user predicate in the `ON` clause rather than the `WHERE`. They do (four
readers in `db.rs`), and serving that migrated database proves it end to end
— `/fitness?date=2026-08-15` returned 200 showing **925 of 2400 cal · 39%**
(Porridge 170 g + Chicken 200 g, arithmetically right), the catalog listed all
three foods **including Eggs**, and the favourites fragment listed exactly
Porridge and Chicken. `/fitness/week` showed 81.9 kg; `/admin` and
`/admin/users` both returned 200.

The session row is the operational half: **the owner does not get logged out
by this deploy**, and their existing passkey still resolves, because 016's
`DEFAULT 1` backfills both.

</details>

<details>
<summary><b>The CI path, which is not the boot path</b></summary>

`deploy.yml` builds its sqlx database with a python loop that applies every
migration raw, split on `;`, wrapped in `try/except: pass` — a different
application path from `run_migrations()`, and one that **swallows failures
silently**. Simulated exactly: **0 swallowed failures** across all 21 files,
and the resulting schema is correct. `cargo sqlx prepare --check` against that
CI-built database exits **0**, so the committed `.sqlx` cache matches the
queries under both the live-DATABASE_URL build CI does and the
`SQLX_OFFLINE=true` build the server does.

`.env.example` is unchanged versus `dev` — this deploy needs **no new
server-side environment variables**.

</details>

<details>
<summary><b>There is no rollback, and the deploy is not a separate step</b></summary>

Pushing `master` fires the workflow, which scps the binary and
`systemctl restart`s — migrations run on that restart. There is **no window**
between "binary lands" and "schema changes", so anything precautionary has to
happen *before the push*.

Reverting the binary alone does not work. `dev`'s `db.rs` selects
`is_favourite`, `default_portion_g` and `custom_portions` from `food_items`
in four places; 020 removes all three, so the old binary 500s on every food
read against the new schema. Recovery is **restore the database first, then
redeploy the old binary** — in that order.

The pipeline takes no backup (`deploy.yml` scps and restarts; the `deploying`
skill says nothing about the database either). So the one manual step this
deploy needs, run before pushing `master`:

```
ssh root@<server> "cp /opt/portfolio/portfolio.db /opt/portfolio/portfolio.db.pre-multiuser"
```

Adding that to `deploy.yml` as a standing step is the right fix for the gap —
it is a change to deploy behaviour, so it goes through the `deploying` skill
as its own item rather than riding along inside this container.

</details>

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
