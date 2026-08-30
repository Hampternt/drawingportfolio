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
| **Trading cards between hands** | `lc_cards::SwapDef`, `SwapState`, `swap_resolve()` | Take at random and/or give by choice. The take is off the engine's `LcRng`, so a theft replays from the seed; the taken card is private and gets its own projection type to stay that way. |
| **Rounds of drinks** | `lc_cards::PourDef`, `PourState`, `pour_discard()` | Drink `n`, pitch `n` cards of your choice. The drink lands at resolve; the discard parks the round, and `parked()` is now the one definition of "waiting on people" so a pour and a challenge can hold the same round without either rolling it over early. |
| **Round resolution report** | `lc_report.rs` — `Blow`/`BlowKind`/`Resolution`, `ack_resolution()` | What landed on whom this round, with the card and the player behind each blow. Ordered (a timeline, so a UI can step it), carries the outcomes the log omits by design (blocked, fizzled, cancelled), and parks the round until the seats it landed on confirm. |
| **Confirm gate at resolve** | `Resolution::owed`/`acked`, `POST …/resolution-ack` | Only seats a blow landed on gate the round; a round that lands nothing does not park at all. A tap from a seat that owed nothing is accepted and changes nothing, so one button serves every viewer. |
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

### 0. Nothing renders a reveal, a pour or a trade

**The round report is the exception** — it renders. `resolution_strip` draws
the receipt into the banner slot (above the challenge strip, since the report
parks last and so is what the table is actually waiting on), and the GOT IT
button rides the house `data-lc-post` contract. That was not optional: the
report parks on nearly every round, so shipping it without a way to press the
button would have frozen every game rather than leaving an inert feature.

Everything else below still has no surface.

The `Reveal` and `Pour` engines are built and tested, but no surface draws
one: the per-viewer hand pane doesn't call `reveals_for()`, `PublicView`'s
`reveals`/`pours` reach the big screen unread, and nothing offers the discard
picker a parked pour needs.

The parking waves are the sharp edge. A pour or a trade **freezes the
round**, and `test/grant` can push a `copies: 0` prototype into a hand, so one
played without a picker is a room that never moves again. Both settle routes
(`pour-discard`, `swap`) exist and are tested end to end, so the way out is
reachable — but until a UI calls them, these cards must stay at `copies: 0`.

Note also that `EffectOp` still cannot express any of this. There are now
five non-numeric effect systems keyed off the catalog (`rfx`, `chfx`, `rvfx`,
`pofx`, `swfx`) beside `fx`, each resolved by id and each with its own block
in `resolve()`. A catalog test pins that a card carries **exactly one** of the
three newest, because two would fire two blocks. If a sixth arrives, that is
the moment to collapse them into one `enum` rather than adding a seventh
`Option` field.

The `Reveal` engine is built and tested, but no surface draws one: the
per-viewer hand pane doesn't call `reveals_for()`, and `PublicView::reveals`
reaches the big screen unread. Same shape as the trigger queue above — the
logic lands before the UI, deliberately.

### 1. Resolve-stage attribution — **built** (`lc_report.rs`)

This was the real gap in the log, and it is now filled by a second record
beside the log rather than by a richer log.

The problem was that `LogEntry::Play` named the card and `LogEntry::Hit`
named the damage, with nothing linking them — a table-wide card producing six
`Hit` lines makes any correlate-adjacent-entries guess wrong. `Effect::source_play`
looked like the join key and was not one: `order_key` resets to 1 at every
reveal.

**What was built instead of a stable play id.** A `Resolution` carrying an
ordered `Vec<Blow>`, each naming `card_id`, `title`, `source`, `subject`,
`kind`, `amount` and `absorbed`. Three reasons it is not the log:

- The log is permanent and capped at `LC_LOG_CAP`, evicting oldest first, so
  any identity written into it can outlive the row it points at. A report is
  bounded by its round.
- **The log deliberately omits the null results.** `damage()` logs a `Hit`
  only when HP actually moved, so a fully shielded hit logs *nothing* — the
  player who blocked an attack has no evidence they were attacked. Same for a
  fizzle and a cancelled play. Those are exactly what a "what happened to me"
  screen must show and exactly what a permanent log should not accumulate.
- It is ordered, which is what lets a UI step through a round one beat at a
  time — and therefore what lets a sound land on the right moment.

`Effect::source_card` was added alongside: a dot ticks rounds after the play
that laid it is gone, so the tick needs the card id the `order_key` could
never be. That is the honest version of what `source_play` only looked like it
did; `source_play` is still not an identity and still should not be treated as
one.

**What is NOT in the report,** deliberately: a challenge's penalty, which
lands at the verdict after the report is already built and has the challenge
screen of its own; and the give half of a trade, which is settled later for
the same reason. A report that reopened after people had confirmed it would be
asking them to confirm something they never saw.

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
