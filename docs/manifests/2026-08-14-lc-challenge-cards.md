# Last Call: challenge cards (container)

**Status:** ACTIVE 2026-08-14 — Pack 1 in progress
**Branch:** `feat/lc-challenge-cards` (from `dev`; merges → `dev` → `master`)
**Origin:** the deferred v2 plan recorded 2026-08-12 (memory
`lastcall-v2-challenge-cards`): cards whose effect happens in real life —
party challenges resolved at the table, not in the engine.

## Goal

A new family of Last Call cards that break out of pure numeric play:
**challenge duels** (instigator vs a seat-relative opponent, table votes the
winner — e.g. "argue you're gayer than the player to your right") and
**solo dares** (do/say/solve something, table votes pass/fail). The loser
takes a penalty that may be numeric (damage, pull drain) or social (drink,
a personal rule displayed on their seat).

## Decisions (theorycraft session 2026-08-14)

- **Stakes:** loser takes the penalty; penalty vocabulary is a catalog-side
  enum — `Damage`, `Drain` (pulls), `Drink` (real-life sips, engine only
  displays), `PersonalRule` (text + rounds, badge on the loser's seat,
  socially enforced).
- **Timing:** challenges interrupt at **Resolve** — after numeric
  resolution the game enters a challenge sub-phase (present → vote →
  verdict), then the round continues. Advancement follows the game's
  no-clock philosophy: the verdict lands when every eligible vote is in,
  not on a timer.
- **Vote:** non-contestants vote; simple majority; **tie = both take the
  penalty**. Contestants cannot vote.
- **"No you":** no new card — `wine-08 Send It Back` (`ReactionFx::Reflect`)
  answering a challenge swaps the roles: the reflector becomes the
  instigator. Falls out of Reflect's existing "resolves against its source"
  semantics; the challenge phase must read swapped roles.
- **Targeting:** duel opponent is seat-relative (right neighbour of the
  instigator at the game table), computed from seat order — no target
  picker.
- **HUD popup (user, 2026-08-14):** the challenge phase announces itself
  with an attention-grabbing overlay; built with **named hook points** so
  sound + visual effects can attach later without restructuring.
- **Persistent table-rule cards are a later container** — they need a
  display surface and an enforcement mechanic of their own.
  (`PersonalRule` as a *penalty* is the deliberately small exception:
  display-only, no enforcement machinery.)
- Catalog-side rules columns follow the `fx`/`rfx` precedent: a
  `ChallengeDef` resolved by card id at play time, never stored in the
  blob, so a retune reaches in-flight games. Unknown/missing defs resolve
  inert (fail-soft, same as reactions pre-Plan-I).

## Packs

Only the active pack gets an item manifest; the lists below are the
proposed shape, one level deep.

### Pack 1 — challenge engine + bare loop (ACTIVE)

Observable: in a test-mode room, play a challenge card; at Resolve the
phase runs end to end in the browser — contestants named, non-contestants
vote, verdict shown, penalty applied — with minimal (unstyled) UI.

Item manifest: `2026-08-14-lc-challenge-pack1.md` (created at pack start,
after the engine survey).

### Pack 2 — spectacle: HUD popup + vote surfaces

Observable: a challenge card triggers a full-screen announcement overlay
on every phone (with named sound/fx hook points), a styled vote UI, a live
tally on the spectator big screen, and personal-rule badges on seats.

### Pack 3 — catalog wave

Observable: the real challenge/dare cards (gayer-than duel included) are
in the shoe at balanced costs/copies; Send It Back's text acknowledges the
role swap. **Checkpoint: card theorycraft with the user before this pack's
items are written.**

## Ledger

- 2026-08-14 — container opened; branch created; engine survey dispatched.
