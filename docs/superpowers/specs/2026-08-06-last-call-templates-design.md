# Last Call — slice 1: infrastructure and render primitives

**Date:** 2026-08-06
**Status:** design approved, ready for planning
**Slice:** 1 of 5. Templates and infrastructure only; the round loop is stubbed.
Executed as two plans, A and B — see §10.

## 1 · Context

Last Call is a third game mode for the `drinkinggame` crate, alongside Ring of
Fire and 3 Man. Players join a room on their phones, register what they are
actually drinking (which picks their deck), and spend *pulls* — sips of that
drink — to play cards. A round runs in six beats and a player is out at 0 HP.

The design bundle lives at `docs/design/last-call/`. Its precedence order, as
stated by the bundle itself:

| Question | Authority |
| --- | --- |
| Pixel values, layout, interaction detail | `Last Call - Game UI.dc.html` |
| Reusable module anatomy and exact values | `Last Call - Module Spec.dc.html` |
| Rules, object model, engine behaviour | `Last Call - Design Doc v2.dc.html` |
| Intent and rationale | `Last Call - Pitch.dc.html` |
| Illustration only — **non-normative** | `Last Call - A Round, Step by Step.dc.html` |
| Superseded, reference only | the two `v1 archive` files |

The walkthrough states outright that its damage numbers are invented. Where it
disagrees with Design Doc v2 (referred to below as **DDv2**), DDv2 wins.

`Last Call - Pitch.dc.html` and `Last Call - Design Doc v1 archive.dc.html` were
recovered from an earlier bundle; they were absent from the delivered zip even
though DDv2 §0 references the pitch as a live companion document. v1's §11
("Still undefined") and §12 ("Suggested order") have no v2 equivalent and remain
the most honest inventory of what the design has not decided.

### 1.1 What this slice is not

The user's direction was explicit: *"first round focus on placeholder templates
and getting infrastructure working before getting a full loop running. templates
first then plugging it together later."*

So this slice builds every rendering the game will need and the infrastructure
underneath them, and stops before the round loop does anything.

## 2 · Scope

### In

1. `last_call` as a third value of `games.kind`, startable from a room.
2. A plain, undesigned setup form: pick deck and container, which sets
   `pulls_max` per DDv2 §3.2, plus handicap. Any room member may set any
   player's handicap before the game starts, and it is public. There is no host
   or owner concept in this crate — `presets.rs` records the model as *"not
   owner-scoped — it's a friends app; anyone logged in may edit"* — and this
   also satisfies DDv1 §11's requirement that handicaps be set by the table
   rather than by the player, which exists to stop everyone declaring
   themselves a lightweight.
3. `last_call.rs` — the DDv2 §1 object model as pure serde types, with the beat
   state machine present but its transitions stubbed.
4. `lc_cards.rs` — a placeholder card catalog as static Rust data.
5. Persistence of `LastCallState` into `games.state_json`.
6. The signal-and-fetch SSE contract (§5).
7. Design tokens and all five card renderings: CardFace, CardPip, CardMini,
   CardBack, CardDot.
8. The F.1 phone shell — status row, phase banner, HAND / TABLE / LOG tabs,
   view, action bar.
9. The F.2 big screen — felt, seat ring, player plaques, hand strips, deck
   stacks, discard slot.
10. The F.3 mini table in the TABLE tab.

### Out — and which slice it belongs to

| Deferred | Why / when |
| --- | --- |
| HandWheel, ArmedColumn, CostRail | Module Spec: *"one widget in three parts and always ship together."* Slice 2. |
| Beat clock advancing; arm / lock / reveal / resolve; damage | Slice 3, the loop. |
| Card flights (E.1) | Motion driven by events that do not exist yet. |
| Events, tabs, pacts, ghosts, reactions | Content systems, one slice each, after the loop runs. |
| LOG tab content | Not designed. The tab renders as an empty pane (§7.3). |
| End-of-game screen, join/lobby, card art | Not designed. |
| Real card lists and the damage scale | Content, not code. See §9. |

The HAND tab renders a plain vertical list of `CardFace`s. That container markup
is deliberately throwaway — slice 2 replaces the container, not the card.

## 3 · Architecture decisions

### 3.1 Own phone shell, at its own route

**Decision:** Last Call gets a dedicated full-page shell, reusing the existing
SSE stream, auth and player identity, but not `room.html`.

DDv2 §14 says Last Call *"reuses the existing room shell"* and maps the hand to
a GAME tab. Module Spec F.1 specifies a different shell — status row, phase
banner, HAND / TABLE / LOG — with *"fixed vertical order, every screen, no
exceptions"* and *"the order never changes and the active tab is never
hoisted."* These cannot both hold. F.1 wins on the bundle's own precedence
rules, and DDv2 §14 is read as reusing the *transport* (one SSE connection, the
same auth), which this design does.

The existing `room.html` is GAME / STANDINGS / ROOM with a
`+1 DRINK / +1 SHOT / UNDO` thumb bar. Last Call needs that thumb zone for the
beat's decision, with the drinking option always amber (F.1). Ring of Fire and
3 Man are untouched by this slice.

### 3.2 Pure state machine, serialised into `games.state_json`

**Decision:** `last_call.rs` mirrors `three_man.rs` — no I/O, no SQL, no RNG
(random values are passed in by callers), round-tripping losslessly through
serde into the existing `games.state_json` column.

That shape is why `three_man.rs` carries 100 unit tests that need no database.
The DDv2 §1 object model is already specified, so defining the types now costs
little and prevents the renderers being rewritten when the loop lands.
Transitions are cheap to change later; the object model is not.

Relational tables per object were considered and rejected for this slice: they
buy queryability that only the LOG tab and replay-from-`rng_seed` will want, at
the cost of committing to a large schema while the content systems are still
hollow.

**This slice adds no migration.** `games.kind` is `TEXT NOT NULL DEFAULT
'ring_of_fire'` with no `CHECK` constraint, so `last_call` is a legal third
value as-is; `games.state_json TEXT` already exists, as do `start_game()` and
`set_game_state()` in `db.rs`. Anyone planning this slice should not write a
migration task.

### 3.4 The public renderer cannot see private state

**Decision:** public fragments are rendered from a projected `PublicView`, never
from `&LastCallState`.

`LastCallState` serialises every player's `hand[]` and `armed[]` into one blob.
If `lc_render.rs` builds the public fragment from `&LastCallState`, nothing
structural stops a renderer — now or in slice 3 — from reaching into
`players[i].hand` while building markup that is broadcast to the whole room.

So `LastCallState::public_view()` projects to a `PublicView` carrying only what
D.3 and F.2 legitimately display: seat, name, HP, status, vessel decks and pull
counts, hand *size*, deck and discard counts, round, beat, `seq`. Card identity
appears in it only for cards already revealed. Public renderers take
`&PublicView` and have no access to anything else.

This is the §6.1 pattern applied a second time: remove the input rather than
check it. It matters most here because slice 2 and slice 3 build every fragment
against whichever type this slice chooses, so retrofitting the projection later
is a refactor across all of them.

### 3.3 Hidden information is rendered per viewer, never broadcast

**Decision:** private state is fetched by the viewer, not pushed to the room.

`RoomHub` is a per-room broadcast: every subscriber — all phones *and* the
unauthenticated spectator screen — receives the identical rendered HTML. That is
safe for Ring of Fire and 3 Man, neither of which has hidden information. Last
Call is built on hidden information: hands are secret, armed cards are secret
until reveal (DDv2 §6.3, *"hold plays secret; show only a lock tick per seat"*),
tabs are private, pacts are secret by definition.

The existing `personalize()` contract cannot carry this. `data-show-player` sets
`el.hidden = true` — a presentation convenience. The markup stays in every
player's DOM and is readable from devtools. Using it for hands would publish the
whole table's hand to the whole table.

See §5 for the resulting SSE contract.

## 4 · Object model

`last_call.rs`, all `Serialize + Deserialize + Clone + Debug + PartialEq`,
mirroring DDv2 §1.

```
Deck        Beer | Cider | Wine | Liquor | Soft
Beat        Draw | Deal | Diplomacy | Lock | Reveal | Resolve
CardKind    Atk | Buff | Curse | Util | Reaction
Status      Alive | Eliminated

Vessel      { deck, pulls_max, pulls_left, container }
Card        { id, deck, kind, cost, targets, text, keywords[], duration }
LcPlayer    { seat, player_id, name, hp, handicap, vessels[], hand[],
              armed[], tabs[], status }
Play        { card, source_seat, target, paid_from, order_key }
Effect      { source_play, subject, op, magnitude, expires_round }
LastCallState { players[], round, beat, first_seat, rng_seed, plays[],
              effects[], discards, seq }
```

Deck constants, from DDv2 §3.1–3.2 — a `Deck` method, not a database table:

| Deck | ABV band | Vessel | Pulls | Cost spread | Role |
| --- | --- | --- | --- | --- | --- |
| Beer | 0.5–7% | 50cl | 8 | 1–2 | Attrition |
| Cider | 0.5–7% | 50cl | 10 | 1–3 | Trickster |
| Wine | 7–20% | 15cl | 6 | 2–3 | Control |
| Liquor | 20%+ | 4cl | 4 | 2–3 | Burst |
| Soft | 0–0.5% | any | 6 | 1–2 | Support |

Pull count is a deck constant, not a volume: a Beer vessel is 8 pulls whether
the tin is 50cl or 25cl, and the app converts to a fraction of the container
(DDv2 §3.2). Starting HP is 15 for everyone (§2.4). Handicap multiplies card
cost in pulls and **rounds up**, and touches nothing else (§11).

### 4.1 Stubbed transitions

The state machine exposes the six-beat advance as functions with final
signatures and stub bodies. Slice 1 asserts only that state round-trips through
serde and that constructed states render. `LastCallState` fields that the loop
would set — `beat`, plaque status, deck counts, `armed[]` — are **settable but
never set** by this slice. Renderings are therefore complete and unit-testable
against constructed states; nothing is fixture-driven or faked.

## 5 · SSE contract

Reuses `/room/{code}/sse` — one connection, per DDv2 §14. Two new `RoomMessage`
variants:

- `LcPublic(String)` — rendered **public** fragment: felt, plaques, hand
  *sizes*, deck counts, phase banner, beat. Broadcast to everyone including the
  spectator screen. Rendered from `PublicView` (§3.4), so it cannot contain
  unrevealed card identity by construction.
- `LcTick(u64)` — the current `seq`. Carries no state.

On either message each phone issues `GET /room/{code}/lastcall/hand`. The server
renders that fragment for the authenticated viewer alone.

**Stale-drop rule.** SSE ticks and the phone's own fetch race: a slow fetch can
land after a newer tick and repaint an older hand. The client keeps the highest
`seq` it has seen and discards any fetch response carrying a lower one. The
fragment carries its `seq` in a `data-seq` attribute.

The spectator screen has no session identity, so it can never fetch a private
fragment. Privacy is structural rather than remembered.

## 6 · Routes

All under the crate's existing `base_path`. Every mutating `/room/...` route
takes that room's `RoomLocks` guard, as `tm_routes.rs` does, and every
`/room/...` route guards member → active game → `kind == "last_call"` →
`GameError::WrongGameKind`. The asset route is public and unguarded, like
`/assets/game.css`.

| Method | Path | Purpose |
| --- | --- | --- |
| GET | `/room/{code}/lastcall` | The F.1 phone shell |
| POST | `/room/{code}/lastcall/start` | Start a Last Call game in the room |
| POST | `/room/{code}/lastcall/vessel` | Register a drink (deck + container) |
| POST | `/room/{code}/lastcall/handicap` | Set a player's handicap |
| GET | `/room/{code}/lastcall/hand` | **Private.** The viewer's own hand |
| GET | `/room/{code}/lastcall/table` | Public mini table fragment |
| GET | `/assets/lastcall.css` | Stylesheet, `include_str!`-embedded |

**Entry point.** `GET /room/{code}` currently renders `room.html` — the
Ring of Fire / 3 Man shell — for any room. When the room's active game is
`last_call` it redirects to `/room/{code}/lastcall` instead, so a player who
opens the room link, scans the QR code or returns from the account page lands in
the right shell. With no active game the room page is unchanged, and starting a
Last Call game from it is what creates the redirect condition.

`/room/{code}/screen` stays a single URL — the room QR code already encodes it —
and branches on the active game's kind, rendering `lc_screen.html` for
`last_call` and the existing `ScreenTemplate` otherwise. It is *not* a third
branch inside `current_screen_panel`: that injects a panel into the existing
screen chrome, and F.2 specifies an 88px header with the felt filling everything
below it, with no leaderboard and no QR.

### 6.1 The private route's authorization is a constraint, not a check

`GET /room/{code}/lastcall/hand` takes **no player identifier of any kind** — no
path segment, no query parameter, no form field. The viewer's identity comes
from the session cookie alone.

Written that way, "can player A fetch player B's hand?" is unanswerable rather
than merely guarded, and a reviewer can verify it from the handler signature.
This constraint is binding on every future private fragment.

## 7 · Rendering inventory

`lc_render.rs` builds fragments as formatted strings, matching the crate's
existing `render.rs` approach. Public builders take `&PublicView` (§3.4);
only the private hand builder takes the viewer's own cards.

### 7.1 Card primitives (Module Spec B)

One `Card` object, five renderings; the size decides what is dropped, never a
different data shape. A card the player owns is never shown as a back; a card
they do not own is never shown as a face.

| Rendering | Size | Content |
| --- | --- | --- |
| CardFace | fluid × 176, pad 16×18, r14 | Deck label + cost pip; title Archivo 900/30; body Space Grotesk 400/15 |
| CardPip | auto × ~24, pad 2×11, r5 | Deck fill ground, Archivo 900/17 numeral in text-on-fill |
| CardMini | 62 wide, pad 7×5, r6 | Cost above title, 1.5px deck border, hard lift |
| CardBack | 16×24 / 44×62 / 46×62 / 68×92 | `#1B1628`, deck-ink border, 9–10px grid at ~10% deck hue |
| CardDot | 8×8 circle | Deck fill plus an 8px glow |

### 7.2 Deck colour is the only taxonomy

No card-type palette, no per-player colour. Two ramps — *fill* for solid areas,
*ink* for anything on the dark ground — differing for Wine only, which is too
dark to read as text on near-black.

| Deck | Fill | Ink | Text on fill |
| --- | --- | --- | --- |
| Beer | `#FFB570` | `#FFB570` | `#14101D` |
| Cider | `#B48EF7` | `#B48EF7` | `#14101D` |
| Wine | `#8B2F4A` | `#D4657F` | `#F2EEF8` |
| Liquor | `#F7768E` | `#F7768E` | `#14101D` |
| Soft | `#6FB6FF` | `#6FB6FF` | `#0D1620` |

Beat hues, for the phase banner and beat timer: Draw amber, Diplomacy mint, Lock
violet, Reveal azure, Resolve rose.

### 7.3 Shells

**F.1 phone shell**, fixed vertical order, no screen may reorder it: status row
(time, room code) → phase banner → tab row → view → action bar. Tabs are always
HAND / TABLE / LOG in that order; the active tab is never hoisted, only its
colour and 2px underline change. Underlines are per-tab: HAND violet, TABLE
azure, LOG `#8D87A0`.

LOG renders as an empty pane and stays in the tab row. Omitting it would break
F.1's ordering rule, which is the spec this slice is built to; an empty pane
does not.

**F.2 big screen**, 1920×1080: 88px header (wordmark, room code, round, phase
banner right-aligned), then the felt filling everything below. A display, never
an input — no hover states, no focus rings, no controls. Nothing under 18px.

**F.3 mini table**, the TABLE tab: the same felt at 466px tall, seats as name/HP
chips, a centre column of draw pile and deck rows. Same state object, same
fields as the big screen.

### 7.4 Stylesheet

A new `assets/lastcall.css`, served at `/assets/lastcall.css`, linked only by
the Last Call templates. Not an extension of `game.css`: the shell is its own
page with its own `<link>`, `game.css` is already 832 lines, and the nested
`/* */` bug that once silently dropped `.card-big` is an argument for smaller
sheets rather than one larger one. The existing nested-comment guard is extended
to cover the new asset.

## 8 · Testing

`./scripts/verify.sh` is the acceptance gate for every task.

**`last_call.rs` — pure unit tests, no database.** Serde round-trip of
`LastCallState`. The §3.2 pull table per deck. Handicap rounding (§11 rounds
up). Seat-ring angle layout from seat 0 at bottom-centre going clockwise, up to
the 7-seat ceiling. And the projection: given a state where every player holds
distinctive cards, `public_view()` retains hand *sizes* and drops every
unrevealed card's identity.

**`lc_render.rs` — unit tests against constructed states.** The Module Spec's
own step-1 acceptance line: *one card renders at all five sizes, in all five
deck colours, from one object*. The hand-strip split (`n ≤ 8` → n backs;
`n > 8` → 7 backs plus `+{n−7}`). The multi-deck plaque rule (one dot per
vessel, 3px top rule split 50/50 between two deck fills).

A `#[cfg(test)]` fixture builder covers the cases a plain setup form cannot
reach by hand — seven seats, two-deck plaques, oversized hands. These are
asserted in tests, not demonstrated in a browser; they are out of the browser
acceptance criteria for this slice.

**`tests/http.rs` — integration.** Route guards: non-member is refused; wrong
game kind returns `WrongGameKind`. `/assets/lastcall.css` is served and free of
nested comment markers. And the one that matters: **a player's hand fragment
cannot be obtained by another player** — asserted by the absence of any input
that would name one (§6.1) as well as by a live two-session test.

## 9 · Recorded gaps, not resolved here

**No cards exist.** Five decks have bands, pull counts, cost spreads and roles,
but no card list, no card text and no damage numbers, anywhere in the bundle.
DDv1 §11: *"No card exists yet… Nothing else can be balanced until this
exists."* This slice ships a small placeholder catalog in `lc_cards.rs` — a few
cards per deck at costs 1–3, plain text, no keywords — sufficient to render
every primitive in every deck colour and nothing more. Real card lists and the
damage scale are content work, and are the true blocker for playability.

**Systems named but mechanically hollow**, each needing rules before it can be
built: events (§10.1 — no list, no effect vocabulary), tabs (§10.2 — no list, no
detection predicates), pacts (§10.3 — no win condition, though DDv2 calls it the
cheapest system with the biggest effect on beat 3), ghosts (§9.2 — no UI, no
timing window), reactions (§7.3 — no rule for which cards are reactions),
effects (§10.4 — no keyword vocabulary, no `op` enum), handicap (§11 — no
allowed set of multiplier values; who sets it is decided in §2, item 2).

**Doc conflicts, resolved in DDv2's favour** and recorded so they are not
re-litigated: the walkthrough shows next round's event already visible, against
§10.1's *"never two at once"*; it calls private objectives "quests" where the
object model has `tabs[]`; it adds a discard step at beat 1, where §5 has none
until beat 6 (§8.2); it says "four minimum for a real game" against §2.1's 2–8;
and it surfaces a "Level" concept that has no field in the object model — it is
`pulls_left`.

**Undecided values** carried as-is from DDv2 §13: starting HP 15 (TBD-1), soft
hand cap 12 (TBD-2), no healing ceiling (TBD-3), 5 cards per finished vessel
(TBD-4), finish-and-draw one vessel per round (TBD-5), diplomacy 60s (TBD-6),
reactions cannot be reacted to (TBD-7), effects do not stack (TBD-8). None block
this slice; all are wired as constants so a playtest can move them.

## 10 · Plans

Per the `plan-economics` skill, a plan is 4–6 tasks ending in something
deployable. This slice is ten tasks, so it is **two plans**, each its own
session:

**Plan A — phone and infrastructure.** Game-kind wiring, setup form, entry
redirect, `last_call.rs` types and `PublicView`, the placeholder card catalog,
CSS tokens, the five card renderings, the F.1 shell with HAND as a plain list,
the private hand route, the SSE contract. *Deployable:* a Last Call game can be
started and a player sees their own hand on their phone.

**Plan B — the felt surfaces.** `lc_screen.html`, seat ring, plaques, hand
strips, deck stacks, discard slot, and the F.3 mini table — which shares D.2
with the big screen at roughly 0.19 scale, so they are one piece of work.
*Deployable:* the spectator big screen and the TABLE tab.

Class C tasks (task reviewer required): the session-gated private hand route,
the SSE contract and its stale-drop rule, and the room entry redirect, which
branches on active-game kind and must not disturb Ring of Fire or 3 Man.
Everything else is A or B — Askama templates and CSS are compiler-gated, and the
render and pull-table tests are their own spec.

### Later slices

1. **The hand group** — HandWheel, ArmedColumn, CostRail, shipped together.
2. **The loop** — beat clock on the existing ticker, arm / lock / reveal /
   resolve, damage, elimination.
3. **Content systems** — pacts first, per DDv2 §10.3, then tabs, events,
   reactions, ghosts.
4. **Undesigned screens** — join/lobby, LOG, end-of-game.
