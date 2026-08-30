# Last Call — engine status

Developer counterpart to `RULES.md` (which is written for players). What the
engine does today, and what the next wave has to add. The four rules that
must not be broken while doing any of it are in `CLAUDE.md` §Last Call's
engine — read those first.

---

## Already built

These are done. Listed because three of them were on the "would be needed"
list and it's worth knowing you already have them.

| Capability | Where | Notes |
|---|---|---|
| **Player hands** | `LcPlayer::hand`, `LcPlayer::armed` | Cards move in only via `deal()`, out only via `discard()`. `hand_by_deck` is public (deck composition), card identity is private. |
| **Decks on the table** | `lc_deck.rs` — `LcTable`/`Shoe` | Real `Vec<Card>` draw + discard piles per deck. Auto-reshuffles when dry. Cards are conserved. |
| **Staged / readied cards** | `ArmedCard { card, target }`, `locked`, `ready` | Staging is private (`public_view()` never reads `armed`, only `locked`). |
| **Targeting a specific player** | `set_target()`, `Card::targets` | `"one"` requires a live seat and permits self-targeting; `"other"` is the same but refuses your own seat (for cards needing two different people); `"self"`/`"all"`/`"right"` require `None`. Rejects with `BadTarget`. Classes that name a seat go through `targets_a_seat()` — one definition, because six sites used to spell it out separately. |
| **Per-seat phase** | `lc_phase.rs` — `seat_phase()`, `blocking_seats()` | Derived server-side, so no client re-derives "is the table waiting on me". |
| **A log** | `LogEntry` (20 variants), rendered by `lc_render::lc_log` | There is already a LOG tab in `lc_room.html`. It renders newest-first off `PublicView::log`. |
| **Challenge win/lose selection** | `ChallengeState`, `challenge_vote()`, `settle_challenge()` | Duel (vs. the seat on your right) or Solo (perform, table votes pass/fail). Electorate frozen at activation; `key` stops a stale screen voting on the next challenge. Penalties are catalog-side: `Penalty::{Damage,Drain,Drink,Rule}`. |
| **Health / shields / drains** | `damage`/`heal`/`shield`/`drain` | You said these are fine as-is. They are, and everything routes through them. |
| **Revealing a hand** | `lc_cards::RevealDef`, `LastCallState::reveals`, `reveals_for()` | Snapshot at resolve, readable through the following round. Two scopes: table (rides `PublicView`) and caster-only (served per viewer, never projected publicly). Three cards at `copies: 0`. |

So the log and the challenge verdict flow **exist** — what's missing on both
is described below, and it's narrower than "build the thing".

---

## Built but inert — the trigger queue

`lc_triggers.rs` is complete: `TriggerWhen::{OnDraw,OnPlay,OnDiscard}` →
`TriggerEvent` queued on `LastCallState::triggers`, projected into
`PublicView`, acked per-seat by `ack_trigger()`, cleared when every alive
seat has acked. `seat_phase()` already returns `Acting` for anyone who still
owes an ack.

Three things stop it doing anything in a real game:

1. **No route calls `ack_trigger`.** There is no `POST .../lastcall/ack`, so
   a fired trigger can never be dismissed.
2. **Nothing enforces the block.** `advance_beat()` and `all_ready()` don't
   consult `triggers`, so `seat_phase()` says the table is waiting while the
   beat advances anyway. `blocking_seats()` is currently advisory.
3. **The three `TRIGGERS` entries point at card ids that aren't in the
   catalog** — `beer-salute`, `cider-round`, `liquor-road`. They're
   placeholders proving the shape; no real card fires one.

**To finish it:** add the ack route, make the advance predicates consult the
queue, and write the cards. That's the smallest piece of remaining work and
it unlocks your "salute the leader" case.

---

## Not built yet

### 0. Nothing renders a reveal

The `Reveal` engine is built and tested, but no surface draws one: the
per-viewer hand pane doesn't call `reveals_for()`, and `PublicView::reveals`
reaches the big screen unread. Same shape as the trigger queue above — the
logic lands before the UI, deliberately.

### 1. Resolve-stage attribution — "whose card hit whom"

This is the real gap in the log. Today:

- `LogEntry::Play { seat, title, target }` names the card.
- `LogEntry::Hit { source, target, amount }` names the damage.

Nothing links them. A UI can't say *"ALEX's LAST ORDERS hit SAM for 4"* — it
would have to guess by correlating adjacent entries, and a table-wide card
producing six `Hit` lines makes that guess wrong.

The blocker is documented in the code: `Effect::source_play` holds a
`Play.order_key`, which **resets to 1 at every reveal**, so it is not an
identity and two rounds' effects can collide on it.

**What it needs:** a stable play id — either a monotonic counter on
`LastCallState`, or `(round, order_key)` carried as a pair — threaded onto
`Play`, `Effect`, and the `Hit`/`Heal`/`Shield`/`Drain` log variants. Once a
log line can name the play that caused it, the visual layer is a rendering
job rather than a guessing job.

Do this one **before** the sound work. Sound cues want the same
"what just happened, caused by whom" signal, and building it twice would be
the mistake.

### 2. Sound + visual cues on cards

There **is** a sound system, but it's the wrong shape for this:
`templates/room.html` plays `/assets/sounds/<name>.mp3` from a `data-sound`
attribute on a **clicked button**, against a fixed 6-file whitelist in
`routes.rs`. It answers "I tapped something", not "something happened to me".

**What it needs:**

- A `sfx` field on `CardDef` (catalog-side, alongside `fx`/`rfx`/`chfx` — so
  it's never stored in the blob and a re-cut sound reaches games already in
  flight).
- Cues riding on the broadcast payload rather than inferred client-side. The
  Last Call SSE broadcast is a **full re-render** of the public view, so a
  naive implementation replays every sound on every reconnect. Cues need to
  be keyed to `seq` and deduped by the client against the last one it played.
- Widening the sound whitelist, or replacing it with a directory scan plus an
  extension check — it currently rejects anything not in the six names.

### 3. `Card::targets` should be an enum

It's a `String` with exactly four values (`self`/`one`/`all`/`right`),
validated only by a test in `lc_cards.rs`. A typo anywhere outside that test
silently falls into the "every other target class requires `None`" branch —
i.e. a mistargeted card fails open rather than failing to compile. Small,
mechanical, worth doing while touching targeting anyway.

---

## Suggested order

1. **Stable play id + attribution on log entries** — unblocks both the visual
   log and the sound cues.
2. **Finish the trigger queue** — ack route, advance predicates, real cards.
3. **Cue channel + `CardDef.sfx`** — now that there's something to key off.
4. **`targets` enum** — any time.

Nothing here needs UI work to land. Each one is testable in the engine, and
the visual pass reads the result off `PublicView` afterwards.
