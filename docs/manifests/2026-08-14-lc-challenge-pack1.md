# LC challenge cards — Pack 1: challenge engine + bare loop

**Status:** COMPLETE 2026-08-14 — review wave applied, merged → `dev`
**Container:** `2026-08-14-lc-challenge-cards.md`
**Branch:** `feat/lc-challenge-cards`

Observable: in a test-mode room, play the prototype challenge card; at
Resolve the game freezes into a challenge phase — contestants named in the
public banner, eligible players get vote buttons, verdict lands when every
vote is in, penalty applies, round rolls over. Minimal styling.

## Engine decisions (from the 2026-08-14 survey)

- The phase is a **pre-rollover freeze**, not a mid-resolve pause:
  `resolve()` runs numeric resolution to completion, then — if challenge
  plays were collected — parks `beat = Resolve` with a populated
  `challenges` queue, exactly like the D16 outcome freeze. The rollover is
  extracted into a shared helper both paths call.
- `lc_advance_chain` gets a `challenge_pending()` gate beside the outcome
  gate (without it the chain's Resolve arm would loop forever).
- Blob carries **no rules**: `ChallengeState { card_id, instigator,
  opponent: Option<usize>, votes, round }` (+ top-level
  `challenges: Vec<_>`, container-level serde default covers it).
  `LcPlayer.rules: Vec<PlayerRule { card_id, expires_round }>` gets a
  field-level `#[serde(default)]` (LcPlayer is strict-nested).
- Rules live catalog-side: `chfx: Option<ChallengeDef>` on `CardDef`,
  `card_chfx(id)` lookup, unknown id ⇒ inert (fail-soft, rfx precedent).
  `ChallengeDef { contest: Contest (Duel|Solo), penalty: Penalty
  (Damage|Drain|Drink|PersonalRule) }`.
- New target class `"right"`: no target picker; opponent = first **Alive**
  seat walking `(seat + n - k) % n` (mirror of the HostileRedirect walk),
  computed at challenge **activation** (post-numeric-resolve), so
  mid-resolve eliminations are respected.
- Reflect (`Send It Back`) may answer a challenge play despite
  `target == None` (guard carve-out); its effect is a **role swap**
  (instigator ↔ opponent), captured at collection time from the Step 0
  `reflected` set.
- Votes: Alive non-contestants only (ghosts sit out v1 — haunting is their
  channel). Duel vote = which contestant wins; solo vote = pass/fail.
  Majority; **tie ⇒ both contestants penalized**; zero eligible voters ⇒
  the challenge fizzles (logged, no penalty). Dead instigator at
  activation ⇒ fizzle; neighbor walk landing back on the instigator ⇒
  fizzle.
- Settling the last challenge runs the shared rollover — unless the
  penalty produced an outcome, in which case the D16 freeze wins.
- Prototype card ships at `copies: 0` — in the catalog, out of the shoe;
  test mode is the way to play it until Pack 3.

## Items

1. **Catalog surface.** `CardKind::Challenge`; `Contest`/`Penalty`/
   `ChallengeDef` types; `chfx` column + `card_chfx()`; invariant tests
   (`chfx.is_some() ⇔ kind == Challenge`, challenge ⇒ `fx: None`,
   `rfx: None`); one prototype duel card at `copies: 0` (catalog sums
   stay 40). Done: `card_chfx` resolves the prototype; catalog tests pass.
2. **Blob state.** `ChallengeState` + `challenges` queue on
   `LastCallState`; `PlayerRule` + `LcPlayer.rules`; serde round-trip +
   old-blob backfill tests. Done: `from_json` on a pre-challenge blob
   yields empty queue/rules.
3. **Resolve integration.** Arm/lock accept Challenge kind ("right" needs
   no target); Step 1 collects challenge plays (no numeric fx, reflect
   swap honored, cancelled honored); post-Step-7 activation freeze;
   `rollover()` extracted; `challenge_pending()` gate in
   `lc_advance_chain`; fizzle rules. Done: unit tests prove freeze,
   fizzles, and that a cancel-answered challenge never activates.
   ⚠ risky item — touches `resolve()`; review individually at pack
   review.
4. **Vote + verdict engine.** `challenge_vote(player_id, choice)` with
   guards (active challenge, alive, non-contestant, once); settle on
   all-votes-in: majority/tie, penalty application (Damage→
   `apply_damage`, Drain→`drain_pulls`, Drink→log only, PersonalRule→
   `rules` + rollover expiry), queue pop, rollover or outcome freeze;
   `LogEntry` variants (challenge opened / verdict / fizzle). Done: unit
   tests cover duel win, tie-both-lose, solo fail, each penalty kind,
   elimination-by-penalty freeze. ⚠ risky item — same review flag.
5. **Route + bare UI.** `POST /room/{code}/lastcall/challenge-vote`
   (PlayerSession, room lock, `map_lc` codes, full broadcast tier);
   banner line + tally in the public panel; vote buttons in the private
   hand fragment for eligible viewers; verdict/fizzle log rendering;
   minimal CSS for any new component root (CSS-root test polices this);
   test-mode path to get the prototype card into a hand; `tests/http.rs`
   coverage (guard, non-member 403, double-vote 409, LcPublic carries no
   hand secrets). Done: http tests pass; bare loop playable.
6. **Browser walkthrough.** Test-mode room, full challenge round driven
   from one browser; evidence noted here. Done: seen working, screenshot
   or transcript note in the ledger.

## Ledger

- 2026-08-14 — manifest written after engine survey; items 1–6 scoped.
- 2026-08-14 — items 1–5 done, one commit each (`7911872` catalog,
  `d03e9a3` blob state, `9b89104` resolve integration, `4614e00` vote
  engine, `68ee225` route + bare UI, `a001362` gate housekeeping).
  Deviations from scope: THREE prototypes shipped, not one — `soft-09`
  (Solo/Drink) and `beer-09` (Duel/Rule) added so both contest shapes and
  all four penalty kinds are engine-tested; personal-rule expiry pruning
  landed with the rollover extraction (item 3) rather than item 4.
- 2026-08-14 — **pack gate**: `./scripts/verify.sh` → "VERIFY OK — fmt,
  clippy, tests, JS syntax all clean." Workspace total 783 tests
  (was 765): +13 engine unit tests, +4 http tests, +1 catalog test.
  Clippy back to the documented 21 distinct warnings (cleared one
  `from_ref` warning inherited from a1aff65, pre-container).
- 2026-08-14 — **item 6 browser walkthrough** (test-mode room FRZU,
  three seats driven from one browser): granted `liquor-09` via
  `test/grant`; card rendered in the hand wheel with CHALLENGE chip;
  tray tap → target overlay showed only the EVERYONE row for the
  `"right"` class → armed → LOCK IN; all locked → Reveal; all READY →
  Resolve parked. Banner strip read "CHALLENGE — TEST-3 VS TEST-2 ·
  Bar Court · VOTES 0/1"; contestant hand showed "THE TABLE IS
  DECIDING", voter hand showed the two verdict buttons; tapping
  "TEST-3 WINS" settled: log "TEST-3 CHALLENGES TEST-2 — Bar Court" →
  "TEST-2 LOSES Bar Court" → "— ROUND 2 —", TEST-2 HP 15→11, beat DRAW.
  Right-neighbour derivation confirmed (seat 0 → seat 2 at a 3-table).
- Polish debt handed to Pack 2: the arm flash caption reads
  "YOU → EVERYONE" for a `"right"` card (JS target-class fall-through —
  functional, mislabeled); the ARMED chip likewise shows "ALL"; personal
  rules have no visible badge yet (verdict log + card text only).
- 2026-08-14 — **pack review** (`/code-review high`, items 3–4 flagged):
  10 verified findings, all resolved in the review-wave commit.
  Fixes: SEND BACK now offered against Duel challenges (scope_legal
  mirror) and refused against Solo ones on both ends (contest-keyed, not
  target-keyed — a Solo's `"self"` class carries a target and dodged the
  first guard); electorate FROZEN at activation on `ChallengeState`
  (late joiners can neither block the settle nor swing it; settle prunes
  dead electors from queued challenges); votes carry a `challenge=<key>`
  identity token (`challenge_seq` counter — stale-screen votes 409);
  settle mirrors the Step-6 dead-partner pact sweep; parked-round pact
  breaks re-stamped for the landing round (G5); reshuffle no longer
  reclaims `copies: 0` prototypes; accidental design-handoff zip
  untracked + gitignored. Rulings, documented in code: challenges are
  exempt from pact betrayal (auto-target ≠ chosen aim); shields DO
  absorb challenge penalties (pinned by test). Nine regression tests;
  792 workspace tests; VERIFY OK.
