# Last Call — Plan F: the catalog and the damage scale (slice 3c)

> **For agentic workers:** REQUIRED SUB-SKILLS: `plan-economics` (this repo's
> task classes, review policy and plan sizing) then
> superpowers:subagent-driven-development to execute task-by-task.

**Goal:** Replace the deliberately-adversarial 20-card placeholder catalog with
the real one — every card for all five decks, each with a concrete effect (op,
magnitude, duration) on a stated damage scale — and make `resolve()` play the
card, not the card's kind.

**Architecture:** `lc_cards.rs` stays the single static source of card truth
and grows an effect column: `CardDef` gains `fx: Option<FxDef>` and
`copies: u8`, and the engine resolves plays through a catalog-side lookup
(`card_fx(id)`), never through data stored in the blob — so a balance retune
reaches in-flight games instantly and no `state_json` migration ever happens.
Plan D's placeholder kind→op mapping (`DMG_PER_COST` and friends) dies; one new
op, `EffectOp::PullDrain`, is added **with** engine support because two deck
roles (Trickster, Support's sabotage) are unwritable without it.

**Slice:** When this plan is done the game has real content: 40 distinct cards
across five decks, a 40-card copy-weighted shoe per deck, curated 5-card
opening hands, per-card damage/heal/shield/dot/drain numbers on an explicit
scale, and a keyword vocabulary that is either test-enforced or explicitly
cosmetic. Plan E (routes, ticker, SSE) samples draws from `lc_cards::shoe()`;
the reaction timing window stays a later plan (reactions ship inert, D9).

**Execution order — binding:** This plan runs **after Plan D** (it consumes
`EffectOp`, `resolve()`, `finish_and_draw`, and retunes tests Plan D writes).
It does **not** require Plan E and SHOULD run before it, so E's draw route is
built against the real shoe rather than the 4-card placeholder. It is
independent of Plan C (hand-group UI); if C or E has already executed, any of
their tests that encode the placeholder catalog's shape join Task 1's retune
sweep.

**Ledger:** `.superpowers/sdd/2026-08-11-last-call-plan-f-catalog/progress.md`
(gitignored).

---

## Proposed design decisions — awaiting user review

The bundle has **no card list, no card text and no damage numbers anywhere**
(DDv1 §11: *"Nothing else can be balanced until this exists"*). Everything
below is authored here, as data a playtest can move. The walkthrough was read
for tone only; its numbers were not copied — where a number below matches it,
the derivation in F3 is the reason.

### F1 — Effects live in the catalog, not on `Card`

`Card` is serde-**strict** by design (doc comment on `LastCallState`: nested
structs stay strict; a missing field inside one is a corrupt blob, not version
skew). Adding an effect field to `Card` would either break every stored blob
or, with a default, freeze stale magnitudes into old hands so a balance patch
splits the table into two rulebooks. So effects are a catalog-side lookup:
`lc_cards::card_fx(&card.id) -> Option<FxDef>`. The blob keeps carrying
identity and display text; the rules always come from the binary. A card id
the catalog no longer knows resolves as inert — a deliberate fail-soft for
skew, noted in the engine comment.

### F2 — Catalog shape: 8 distinct cards × copies (6,6,6,6,5,5,4,2) = 40 per deck

`LC_DECK_SIZE` stays **40** and is now real: a test pins each deck's copy sum
to it. Per deck: **4 commons ×6** (the deck's bread-and-butter, the cheap end
of its cost spread), **2 uncommons ×5**, **1 rare ×4** (the deck's identity
piece), **1 reaction ×2**. Rationale: with `DRAW_PER_VESSEL = 5`, eight
distinct cards mean a fresh draw almost always lands 2+ distinct commons
(readable at a party), while the ×4 rare stays an event; 40 distinct cards
total is the largest list one review pass can actually argue with. Repeats are
how a shoe works — copy count is impact inverted.

### F3 — The damage scale: par is 2 damage per pull, against HP 15

Stated so the numbers can be argued with:

- **Par:** a plain single-target immediate hit converts pulls to damage at
  **2.0 per pull** at handicap 100 (`cost` pulls → `2 × cost` damage).
  HP 15 / 2.0 = 7.5 pulls to solo-kill from full — one Beer vessel, or two
  Liquor measures. One attacker converting ~3 pulls a round kills in ~3 rounds
  through no defense; two focused attackers in ~1.5. With par-rate heals and
  shields answering, expect first elimination around round 3–4 and a 4-player
  game at 5–7 rounds ≈ 20–35 minutes at DDv2 §5's beat clocks.
- **Deck rates** (offense per pull / vessel potential):

  | Deck | Pulls | Immediate dmg/pull | Persistent | Shape |
  | --- | --- | --- | --- | --- |
  | Beer | 8 | 2.0, **no hit above 4** (§3.1 "no bomb") | dot 1×5r (rare) | biggest tank, flattest curve — 16 dmg potential per vessel |
  | Cider | 10 | 2.0 (bomb 6 @ cost 3) | dots at par | tempo: drains deny 1.5–2 pulls per pull spent |
  | Wine | 6 | 2.0 (Corked) | dots **2.0–3.0 total/pull**, back-loaded | the premium pays for the delay and the expiry-on-elimination risk |
  | Liquor | 4 | **2.5–2.67** (5@2, 7@3, rare 8@3) | dot at par | burst premium pays for the 4-pull tank and whole-measure refills (§3.3) |
  | Soft | 6 | 2.0 but cost-1 chip only | shields **2.5–3.0/pull** | support premium pays for waste risk (shields expire); drains are its sabotage |

> **ERRATUM (2026-08-12, whole-plan review).** The Liquor row states immediate dmg/pull as **2.5–2.67 (5@2, 7@3, rare 8@3)**, but 7@3 = 2.33 dmg/pull, outside the stated band. The code matches F12 exactly; only the prose derivation here was wrong. For the record: 5@2 = 2.5, 8@3 = 2.67, 7@3 = 2.33 (not in band).

- **Handicap never touches these numbers** — §11 is cost-only, rounded up. A
  150% player pays 3 pulls for a 4-damage card and self-throttles to 1.33
  dmg/pull; magnitudes are never scaled.
- Every magnitude, duration and copy count is table data in `lc_cards.rs` —
  a playtest moves one literal.

### F4 — One new op, `EffectOp::PullDrain`, with engine support

Cider's role is *"Trickster. Redirect, swap, theft"* and Soft's includes
*"sabotage"* — with only Damage/Heal/Shield/Dot, both collapse into worse Beer.
Swap/theft/redirect need hand- and queue-manipulation systems the engine
doesn't have (cut, per the no-flavor-only rule). **Pull drain** — remove
`magnitude` pulls from the target's fullest vessel — is pure, deterministic,
on-theme, and interacts with the economy (denies tempo and finish-&-draw
positioning without touching HP, which §11 reserves for cards). It is
immediate-only, never stored. Serde name `"pull_drain"`. Task 2 implements it.

### F5 — Reactions are in the catalog, inert, ×2 copies — flagged

One reaction per deck ships as real data (title, text, cost) so the render
surfaces keep being exercised, but `fx: None` and D9's `NotPlayable` stand
until the reaction slice. **Consequence to weigh:** 2/40 = 5% of every shoe is
a dead draw until then. The toggle is one number — set `copies` to 0 to keep
them out of the shoe (they'd remain preview-only). Proposed: keep ×2, so the
later reaction slice is a pure buff to cards people already hold. Reaction
texts say outright they are inert ("arms in a later update") — no rules-text
pretending.

### F6 — Opening hands are curated, fixed 5-card lists

`set_vessel` deals `opening_hand(deck)` — a fixed list per deck (D15's "real
opening hand"): commons plus one stronger card, no reactions, always including
the deck's `-01`. Deterministic on purpose: the engine is no-RNG, a curated
opener guarantees a playable first round (an attack and a defensive piece in
every hand), and draw variance arrives anyway with finish-&-draw, whose cards
Plan E samples from `shoe()`. Randomized openers, if playtest wants them, are
a Plan E route change (`set_vessel` would grow a `dealt` parameter) — deferred.
Cider's and Wine's openers deliberately include their strong card (Windfall /
Corked): a guaranteed turn-one threat creates immediate table politics, and it
is affordable (3 of 10 / 3 of 6 pulls).

| Deck | Opening five |
| --- | --- |
| Beer | Nudge, Grind, Second Wind, Head of Foam, Steady Pour |
| Cider | Sticky Pour, Spilled, Watered Down, Sour Turn, **Windfall** |
| Wine | Decant, Let It Breathe, Tannin Bite, Cellar Chill, **Corked** |
| Liquor | Shot Called, **Double**, Hangover, Chaser, Dutch Courage |
| Soft | Water Round, Designated, Snack Table, Cut Them Off, Splash of Cold Water |

### F7 — Keywords are two-tier: 7 mechanical (test-enforced) + 6 tone (cosmetic)

- **Mechanical** — `aoe, burst, dot, shield, heal, drain, reaction` — each is a
  predicate over the card's data, enforced **bidirectionally** by a test:
  `aoe` ⇔ `targets == "all"`; `dot`/`shield`/`heal`/`drain` ⇔ the fx op;
  `burst` ⇔ single-target immediate damage ≥ 5 (`BURST_KW_MIN_DAMAGE`);
  `reaction` ⇔ `CardKind::Reaction`. A chip that lies about its op cannot pass
  the suite.
- **Tone** — `loud, public, petty, slow, showy, quiet` — explicitly cosmetic
  flavor vocabulary, whitelisted by a test, carrying **no rules**. Documented
  in the module header so nobody later "implements" `petty`.

Thirteen words total; any keyword outside both lists fails the build.

### F8 — Shield cards protect in their own round; dots still queue

Plan D's queue-at-step-3 rule exists so a curse never ticks in its creation
round — correct for dots, wrong for shields: a shield revealed this round that
only materialises next round can never do its job against the simultaneous
reveal. So a **Shield play registers its effect immediately** at its play's
resolution slot (replace-not-stack by `(op, subject)`, D10 unchanged), and
protects every *later* play in `order_key` order. Deliberate tension: 7.1's
bigger-spender-first means a cheap shield resolves **after** a big attack — to
pre-shield, the defender must outspend. Two Task 2 tests pin both orders.

### F9 — `targets == "all"` includes the source (D2), and aoe cards price it in

"One For The Table" damages its own player; "Snack Table" heals them. The self-
hit on aoe attacks and aoe drains is part of the card's cost and the game's
joke; noted per card in the table. No `table`-targeted cards exist (the class
resolves as a no-op — that would be flavor-only, so it is unused).

### F10 — Plan D's D8 constants die

`DMG_PER_COST`, `HEAL_PER_COST`, `DOT_PER_COST`, `CURSE_ROUNDS` are deleted in
Task 2 — the per-card fx table is the tunable now. `LC_DECK_SIZE` (40) stays
and is test-pinned to the copy sums. The beer/cider/soft magnitudes below
deliberately coincide with the old 2×cost mapping, which is why Plan D's
resolve tests survive the swap with their expected values intact (only Wine
and Liquor carry premiums, and no D test pins those decks' magnitudes).

### F11 — The §14 conservation invariant stays deferred

DDv2 §14's "cards in play + hands + discards = deck size, per deck" needs a
shoe that tracks identity. The shoe remains a count (D6): `shoe(deck)` exposes
the copy-expanded 40 for Plan E to sample **with replacement**, so `copies`
express *relative frequency*, not strict supply — you can, rarely, see a fifth
Windfall. Honest limitation, flagged; a per-identity shoe is a later engine
change if playtest cares.

### F12 — The full catalog

`INERT` marks the five reaction cards (F5). Effect column: `Damage n` /
`Heal n` (immediate), `Shield n, r rounds` / `Dot n × r rounds` (persist via
`expires_round = round + r`), `Drain n pulls` (immediate). Copies per F2.

| Deck | Id | Title | Kind | Cost | Targets | Effect | Copies | Keywords | Text |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Beer | beer-01 | Nudge | Atk | 1 | one | Damage 2 | 6 | — | Deal 2 damage. Small, boring, and there is always another one. |
| Beer | beer-02 | Grind | Atk | 2 | one | Damage 4 | 6 | slow | Deal 4 damage. Nothing flashy, the tab just keeps running. |
| Beer | beer-03 | Second Wind | Buff | 1 | self | Heal 2 | 6 | heal | Heal 2. Shake it off, it was only a nudge. |
| Beer | beer-04 | Head of Foam | Buff | 2 | self | Shield 4, 2 rounds | 6 | shield | Gain a shield that absorbs 4 damage over the next 2 rounds. |
| Beer | beer-05 | One For The Table | Atk | 2 | all | Damage 1 (each, incl. you) | 5 | aoe | Deal 1 damage to every player at the table. Yes, including you. |
| Beer | beer-06 | Steady Pour | Buff | 2 | self | Heal 4 | 5 | heal | Heal 4. Slow beer, long night. |
| Beer | beer-07 | Tab Runs Long | Curse | 2 | one | Dot 1 × 5 rounds | 4 | dot, slow | Deal 1 damage a round for 5 rounds. It did not seem like much at the time. |
| Beer | beer-08 | Coaster | Reaction | 1 | self | **INERT** | 2 | reaction | Reaction: inert until the response window ships. Slide it over your glass and wait. |
| Cider | cider-01 | Sticky Pour | Curse | 1 | one | Dot 1 × 2 rounds | 6 | dot | Deal 1 damage a round for 2 rounds. Something inconvenient, later. |
| Cider | cider-02 | Spilled | Util | 1 | one | Drain 2 pulls | 6 | drain, petty | Drain 2 pulls from a player's fullest vessel. Whoops. |
| Cider | cider-03 | Watered Down | Util | 2 | one | Drain 3 pulls | 6 | drain | Drain 3 pulls from a player's fullest vessel. They will taste it eventually. |
| Cider | cider-04 | Windfall | Atk | 3 | one | Damage 6 | 4 | burst, loud, public, petty, showy | Deal 6 damage. The whole orchard at once, and everyone hears it land. |
| Cider | cider-05 | Sour Turn | Atk | 2 | one | Damage 4 | 6 | — | Deal 4 damage. Sweet up front, then it bites. |
| Cider | cider-06 | Happy Hour Panic | Util | 3 | all | Drain 1 pull (each, incl. you) | 5 | aoe, drain | Drain 1 pull from every player at the table, you included. Last orders moved up. |
| Cider | cider-07 | Two Straws | Curse | 2 | one | Dot 2 × 2 rounds | 5 | dot | Deal 2 damage a round for 2 rounds. Double the trouble, half the dignity. |
| Cider | cider-08 | Not So Fast, Friend | Reaction | 2 | one | **INERT** | 2 | reaction | Reaction: inert until the response window ships. Keep it where they can see it. |
| Wine | wine-01 | Decant | Curse | 2 | one | Dot 2 × 2 rounds | 6 | dot | Deal 2 damage a round for 2 rounds. Poured slowly, from a great height, while keeping eye contact the whole time, a patient problem that keeps arriving well after the glass is set down. |
| Wine | wine-02 | Let It Breathe | Curse | 2 | one | Dot 1 × 4 rounds | 6 | dot, slow | Deal 1 damage a round for 4 rounds. It only improves with time. |
| Wine | wine-03 | Tannin Bite | Atk | 2 | one | Damage 4 | 6 | — | Deal 4 damage. Dry, sharp, structured. |
| Wine | wine-04 | Corked | Atk | 3 | one | Damage 6 | 5 | burst | Deal 6 damage. Control, delivered as damage. |
| Wine | wine-05 | House Rules Amendment | Util | 3 | all | Drain 1 pull (each, incl. you) | 5 | aoe, drain, public | Drain 1 pull from every player at the table, you included. Motion carried. |
| Wine | wine-06 | Cellar Chill | Curse | 3 | one | Dot 2 × 3 rounds | 6 | dot | Deal 2 damage a round for 3 rounds. Best served cold and repeatedly. |
| Wine | wine-07 | The Long Decant of Winter | Curse | 3 | one | Dot 3 × 3 rounds | 4 | dot, slow, showy | Deal 3 damage a round for 3 rounds. A vintage grudge, opened at last. |
| Wine | wine-08 | Send It Back | Reaction | 2 | one | **INERT** | 2 | reaction | Reaction: inert until the response window ships. Summon the sommelier. |
| Liquor | liquor-01 | Shot Called | Atk | 2 | one | Damage 5 | 6 | burst | Deal 5 damage. Loud and immediate. |
| Liquor | liquor-02 | Double | Atk | 3 | one | Damage 7 | 5 | burst, loud | Deal 7 damage. Louder and more immediate. |
| Liquor | liquor-03 | Hangover | Curse | 2 | one | Dot 2 × 2 rounds | 6 | dot | Deal 2 damage a round for 2 rounds. Payable next round, with interest. |
| Liquor | liquor-04 | Chaser | Buff | 2 | self | Heal 4 | 6 | heal | Heal 4. Something soft to land on. |
| Liquor | liquor-05 | Neat, No Ice, No Mercy | Atk | 3 | all | Damage 2 (each, incl. you) | 5 | aoe, loud | Deal 2 damage to every player at the table, you included. A round of shots is still a round. |
| Liquor | liquor-06 | Dutch Courage | Buff | 3 | self | Shield 7, 2 rounds | 6 | shield | Gain a shield that absorbs 7 damage over the next 2 rounds. Liquid confidence, briefly real. |
| Liquor | liquor-07 | Last Call | Atk | 3 | one | Damage 8 | 4 | burst, loud, showy, public | Deal 8 damage. The biggest hit in the game, named after the night's last mistake. |
| Liquor | liquor-08 | Spit It Out | Reaction | 2 | self | **INERT** | 2 | reaction | Reaction: inert until the response window ships. Undignified but effective. |
| Soft | soft-01 | Water Round | Buff | 1 | one | Heal 2 | 6 | heal | Heal 2 on any player. Someone feels better. |
| Soft | soft-02 | Designated | Buff | 1 | one | Shield 3, 2 rounds | 6 | shield | Shield any player for 3 damage over the next 2 rounds. You take it for them. |
| Soft | soft-03 | Snack Table | Buff | 2 | all | Heal 1 (each, incl. you) | 6 | aoe, heal | Heal 1 on every player at the table, you included. Crisps solve most things. |
| Soft | soft-04 | The Long Sober Look Across The Table | Reaction | 1 | self | **INERT** | 2 | reaction | Reaction: inert until the response window ships. You know what you did. |
| Soft | soft-05 | Cut Them Off | Util | 2 | one | Drain 3 pulls | 6 | drain, petty | Drain 3 pulls from a player's fullest vessel. It is for their own good. |
| Soft | soft-06 | Splash of Cold Water | Atk | 1 | one | Damage 2 | 5 | — | Deal 2 damage. Rude, refreshing, effective. |
| Soft | soft-07 | Glass Wall | Buff | 2 | one | Shield 5, 2 rounds | 5 | shield | Shield any player for 5 damage over the next 2 rounds. Politely impenetrable. |
| Soft | soft-08 | Mother Hen | Buff | 2 | all | Shield 2, 2 rounds (each, incl. you) | 4 | aoe, shield, quiet | Shield every player for 2 damage over the next 2 rounds, you included. Everyone gets a coaster. |

**Role notes worth arguing with:** Beer has no card above 4 damage (§3.1's "no
bomb" is a test). Soft deals damage only through its cost-1 chip — it can close
a game, slowly, but its win line is outlasting (heals/shields at premium rate,
drains as denial); if Soft feels like a penalty at the table, the balance is
wrong per §3.4's intent and these numbers move. Wine's rare (Dot 3×3 = 9 total,
3.0/pull) is the game's most efficient damage and its slowest — it dies with
its target's elimination and with Wine's 6-pull vessel it is half your round.
Liquor's 8-damage rare is the single biggest hit; over half of HP 15 in one
card is intended to be the table's scariest moment.

**§9 adversarial coverage, now on real cards** (Task 3 pins all of it):
title bands — ≤14 chars: most titles; 15–24: One For The Table (17), Happy
Hour Panic (16), Not So Fast, Friend (19), House Rules Amendment (21), Neat,
No Ice, No Mercy (22), Splash of Cold Water (20); >24: The Long Decant of
Winter (25), The Long Sober Look Across The Table (36). Let It Breathe sits
exactly on the 14-char band edge. Body overflow (>108 chars): wine-01 Decant.
Zero keywords: beer-01, cider-05, wine-03, soft-06. Many keywords: cider-04
(5 chips → the `+n` fold), liquor-07 (4).

---

## Global Constraints

Every task's requirements implicitly include this section.

### Scope

Content and its engine hookup only:

- `drinkinggame/src/lc_cards.rs` — rewritten (data + API).
- `drinkinggame/src/last_call.rs` — `EffectOp::PullDrain`, the `set_vessel`
  opening-deal swap, the `resolve()` fx hookup, and mechanical retunes of
  existing tests that encoded the placeholder catalog's shape.
- `drinkinggame/src/lc_render.rs`, `drinkinggame/src/lc_preview.rs`,
  `drinkinggame/tests/http.rs` — test/label retunes only, enumerated per task.

No routes, no SSE, no templates, no CSS, no JS, no migration. If a task finds
itself editing a handler body, it has crossed the line and must stop and
report. Plan E owns sampling draws at the route layer.

### Binding rules

- **No flavor-only rules text.** Every non-reaction card's `text` states its
  actual effect and matches its `fx` (self-reviewed against the F12 table;
  reactions state their inertness outright).
- **Serde version skew:** nothing is added to `Card` or any nested struct.
  Effects are catalog-side (F1). `EffectOp` gains a variant — serde name
  `"pull_drain"` — which is safe: enums grow forward, and no stored blob can
  contain it before this binary writes one.
- **§9 floor holds forever:** at least one title per §7.5 ramp band, one
  overflowing body, one zero-keyword and one many-keyword card — pinned by
  Task 3's tests so future catalog edits keep the ramp exercised.
- **All numbers are data:** magnitudes, durations, copies, openers and the
  keyword vocabulary are consts/static tables in `lc_cards.rs`.
- Keep `drinkinggame` clippy-clean; the pre-existing distinct-warning count
  (17, all in `drawingportfolio`) must not grow.

### Verification

**Verification for every task:** `./scripts/verify.sh` — all green, output
quoted in the report. Never a bare `cargo test`.

**Baseline before Task 1:** whatever Plan D's ledger records at its close —
the pre-C/D figure was 371 tests; the invariants are *verify green* and *17
distinct clippy warnings, `drinkinggame` clean*, not a fixed test count. Read
the number from `.superpowers/sdd/2026-08-11-last-call-plan-d-loop-engine/progress.md`
and record it in this plan's ledger before starting.

**Browser checkpoint: one**, after Task 4 — the preview page's catalog-driven
groups shift from 20 cards to 40. **No `cargo sqlx prepare`** (no migration;
`drinkinggame` is runtime-checked).

---

### Task 1: The real catalog — data, representation, opening hands

**Class:** B (logic, tests specified below)

**Why this class:** static data plus pure lookups, every invariant written as
a test with expected values; the engine edit is a one-line iterator swap whose
new expected counts are enumerated.

**Files:**
- Modify: `drinkinggame/src/lc_cards.rs` (rewrite)
- Modify: `drinkinggame/src/last_call.rs` (`EffectOp::PullDrain`; `set_vessel`
  deal source; `preview_state` fixture slices; enumerated test retunes)
- Modify: `drinkinggame/src/lc_render.rs` (two keyword-fold test retunes)
- Modify: `drinkinggame/tests/http.rs` (one stale comment)
- Test: `drinkinggame/src/lc_cards.rs` (`#[cfg(test)] mod tests`, rewritten)

**Interfaces:**
- Consumes: `Card`, `CardKind`, `Deck`, `EffectOp`, `LC_DECK_SIZE` from
  `last_call.rs` (Plan D's Produces).
- Produces (exact — Tasks 2–3 and Plan E build against these):

```rust
// last_call.rs — variant added, same serde convention ("pull_drain"):
pub enum EffectOp { Damage, Heal, Shield, Dot, PullDrain }

// lc_cards.rs:
/// A card's rules, resolved by id at play time — never stored in the blob
/// (see the version-skew note on LastCallState; a retune must reach
/// in-flight games).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FxDef {
    pub op: EffectOp,
    pub magnitude: i32,
    /// 0 = immediate. n >= 1 = persists; expires_round = round + n.
    pub rounds: u32,
}

pub struct CardDef {
    pub id: &'static str,
    pub deck: Deck,
    pub kind: CardKind,
    pub cost: u8,
    pub targets: &'static str,
    pub title: &'static str,
    pub text: &'static str,
    pub keywords: &'static [&'static str],
    pub copies: u8,          // shoe frequency; per-deck sum == LC_DECK_SIZE
    pub fx: Option<FxDef>,   // None ⇔ kind == Reaction (inert, F5)
}

pub const CATALOG: [CardDef; 40];
pub fn deck_cards(deck: Deck) -> Vec<Card>;   // 8 distinct, catalog order
pub fn card_by_id(id: &str) -> Option<Card>;  // unchanged signature
pub fn card_fx(id: &str) -> Option<FxDef>;    // None for reactions AND unknown ids
pub fn opening_hand(deck: Deck) -> Vec<Card>; // the curated 5 (F6)
pub fn deck_copies(deck: Deck) -> u16;        // == LC_DECK_SIZE, test-pinned
pub fn shoe(deck: Deck) -> Vec<Card>;         // copy-expanded 40 — Plan E samples this
```

`to_card` derives `Card.duration` from fx: `rounds >= 1` →
`Some(format!("{rounds} ROUNDS"))`, else `None` — the display field stays
honest without a parallel table.

- [ ] **Step 1: Rewrite `lc_cards.rs`**

Module doc: the real catalog, the F2 shape, the F3 scale in one sentence, the
F7 keyword tiers (tone words carry no rules), and the F11 with-replacement
caveat on `shoe()`. Then the types above, these helpers:

```rust
const fn fx(op: EffectOp, magnitude: i32, rounds: u32) -> Option<FxDef> {
    Some(FxDef { op, magnitude, rounds })
}
const fn dmg(m: i32) -> Option<FxDef> { fx(EffectOp::Damage, m, 0) }
const fn heal(m: i32) -> Option<FxDef> { fx(EffectOp::Heal, m, 0) }
const fn shield(m: i32, r: u32) -> Option<FxDef> { fx(EffectOp::Shield, m, r) }
const fn dot(m: i32, r: u32) -> Option<FxDef> { fx(EffectOp::Dot, m, r) }
const fn drain(m: i32) -> Option<FxDef> { fx(EffectOp::PullDrain, m, 0) }

/// F7 — mechanical keywords, each a tested predicate over card data.
pub const MECHANICAL_KW: [&str; 7] =
    ["aoe", "burst", "dot", "shield", "heal", "drain", "reaction"];
/// F7 — tone keywords: cosmetic vocabulary, whitelisted, NO rules attach.
pub const TONE_KW: [&str; 6] = ["loud", "public", "petty", "slow", "showy", "quiet"];
/// Single-target immediate damage at or above this carries `burst`.
pub const BURST_KW_MIN_DAMAGE: i32 = 5;

const OPENERS: [(Deck, [&'static str; 5]); 5] = [
    (Deck::Beer,   ["beer-01", "beer-02", "beer-03", "beer-04", "beer-06"]),
    (Deck::Cider,  ["cider-01", "cider-02", "cider-03", "cider-05", "cider-04"]),
    (Deck::Wine,   ["wine-01", "wine-02", "wine-03", "wine-06", "wine-04"]),
    (Deck::Liquor, ["liquor-01", "liquor-02", "liquor-03", "liquor-04", "liquor-06"]),
    (Deck::Soft,   ["soft-01", "soft-02", "soft-03", "soft-05", "soft-06"]),
];
```

The catalog itself, verbatim (this is the F12 table as Rust — the two must
agree; the executor transcribes, never re-derives):

```rust
pub const CATALOG: [CardDef; 40] = [
    // ---- Beer — Attrition, costs 1-2, 8 pulls. Par 2 dmg/pull, no hit > 4.
    CardDef { id: "beer-01", deck: Deck::Beer, kind: CardKind::Atk, cost: 1,
        targets: "one", title: "Nudge", copies: 6, keywords: &[], fx: dmg(2),
        text: "Deal 2 damage. Small, boring, and there is always another one." },
    CardDef { id: "beer-02", deck: Deck::Beer, kind: CardKind::Atk, cost: 2,
        targets: "one", title: "Grind", copies: 6, keywords: &["slow"], fx: dmg(4),
        text: "Deal 4 damage. Nothing flashy, the tab just keeps running." },
    CardDef { id: "beer-03", deck: Deck::Beer, kind: CardKind::Buff, cost: 1,
        targets: "self", title: "Second Wind", copies: 6, keywords: &["heal"], fx: heal(2),
        text: "Heal 2. Shake it off, it was only a nudge." },
    CardDef { id: "beer-04", deck: Deck::Beer, kind: CardKind::Buff, cost: 2,
        targets: "self", title: "Head of Foam", copies: 6, keywords: &["shield"],
        fx: shield(4, 2),
        text: "Gain a shield that absorbs 4 damage over the next 2 rounds." },
    CardDef { id: "beer-05", deck: Deck::Beer, kind: CardKind::Atk, cost: 2,
        targets: "all", title: "One For The Table", copies: 5, keywords: &["aoe"],
        fx: dmg(1),
        text: "Deal 1 damage to every player at the table. Yes, including you." },
    CardDef { id: "beer-06", deck: Deck::Beer, kind: CardKind::Buff, cost: 2,
        targets: "self", title: "Steady Pour", copies: 5, keywords: &["heal"], fx: heal(4),
        text: "Heal 4. Slow beer, long night." },
    CardDef { id: "beer-07", deck: Deck::Beer, kind: CardKind::Curse, cost: 2,
        targets: "one", title: "Tab Runs Long", copies: 4, keywords: &["dot", "slow"],
        fx: dot(1, 5),
        text: "Deal 1 damage a round for 5 rounds. It did not seem like much at the time." },
    CardDef { id: "beer-08", deck: Deck::Beer, kind: CardKind::Reaction, cost: 1,
        targets: "self", title: "Coaster", copies: 2, keywords: &["reaction"], fx: None,
        text: "Reaction: inert until the response window ships. Slide it over your glass and wait." },
    // ---- Cider — Trickster, costs 1-3, 10 pulls. Par hits + pull drains.
    CardDef { id: "cider-01", deck: Deck::Cider, kind: CardKind::Curse, cost: 1,
        targets: "one", title: "Sticky Pour", copies: 6, keywords: &["dot"], fx: dot(1, 2),
        text: "Deal 1 damage a round for 2 rounds. Something inconvenient, later." },
    CardDef { id: "cider-02", deck: Deck::Cider, kind: CardKind::Util, cost: 1,
        targets: "one", title: "Spilled", copies: 6, keywords: &["drain", "petty"],
        fx: drain(2),
        text: "Drain 2 pulls from a player's fullest vessel. Whoops." },
    CardDef { id: "cider-03", deck: Deck::Cider, kind: CardKind::Util, cost: 2,
        targets: "one", title: "Watered Down", copies: 6, keywords: &["drain"],
        fx: drain(3),
        text: "Drain 3 pulls from a player's fullest vessel. They will taste it eventually." },
    CardDef { id: "cider-04", deck: Deck::Cider, kind: CardKind::Atk, cost: 3,
        targets: "one", title: "Windfall", copies: 4,
        keywords: &["burst", "loud", "public", "petty", "showy"], fx: dmg(6),
        text: "Deal 6 damage. The whole orchard at once, and everyone hears it land." },
    CardDef { id: "cider-05", deck: Deck::Cider, kind: CardKind::Atk, cost: 2,
        targets: "one", title: "Sour Turn", copies: 6, keywords: &[], fx: dmg(4),
        text: "Deal 4 damage. Sweet up front, then it bites." },
    CardDef { id: "cider-06", deck: Deck::Cider, kind: CardKind::Util, cost: 3,
        targets: "all", title: "Happy Hour Panic", copies: 5, keywords: &["aoe", "drain"],
        fx: drain(1),
        text: "Drain 1 pull from every player at the table, you included. Last orders moved up." },
    CardDef { id: "cider-07", deck: Deck::Cider, kind: CardKind::Curse, cost: 2,
        targets: "one", title: "Two Straws", copies: 5, keywords: &["dot"], fx: dot(2, 2),
        text: "Deal 2 damage a round for 2 rounds. Double the trouble, half the dignity." },
    CardDef { id: "cider-08", deck: Deck::Cider, kind: CardKind::Reaction, cost: 2,
        targets: "one", title: "Not So Fast, Friend", copies: 2, keywords: &["reaction"],
        fx: None,
        text: "Reaction: inert until the response window ships. Keep it where they can see it." },
    // ---- Wine — Control, costs 2-3, 6 pulls. Dots at 2.0-3.0 total/pull.
    CardDef { id: "wine-01", deck: Deck::Wine, kind: CardKind::Curse, cost: 2,
        targets: "one", title: "Decant", copies: 6, keywords: &["dot"], fx: dot(2, 2),
        text: "Deal 2 damage a round for 2 rounds. Poured slowly, from a great height, \
               while keeping eye contact the whole time, a patient problem that keeps \
               arriving well after the glass is set down." },
    CardDef { id: "wine-02", deck: Deck::Wine, kind: CardKind::Curse, cost: 2,
        targets: "one", title: "Let It Breathe", copies: 6, keywords: &["dot", "slow"],
        fx: dot(1, 4),
        text: "Deal 1 damage a round for 4 rounds. It only improves with time." },
    CardDef { id: "wine-03", deck: Deck::Wine, kind: CardKind::Atk, cost: 2,
        targets: "one", title: "Tannin Bite", copies: 6, keywords: &[], fx: dmg(4),
        text: "Deal 4 damage. Dry, sharp, structured." },
    CardDef { id: "wine-04", deck: Deck::Wine, kind: CardKind::Atk, cost: 3,
        targets: "one", title: "Corked", copies: 5, keywords: &["burst"], fx: dmg(6),
        text: "Deal 6 damage. Control, delivered as damage." },
    CardDef { id: "wine-05", deck: Deck::Wine, kind: CardKind::Util, cost: 3,
        targets: "all", title: "House Rules Amendment", copies: 5,
        keywords: &["aoe", "drain", "public"], fx: drain(1),
        text: "Drain 1 pull from every player at the table, you included. Motion carried." },
    CardDef { id: "wine-06", deck: Deck::Wine, kind: CardKind::Curse, cost: 3,
        targets: "one", title: "Cellar Chill", copies: 6, keywords: &["dot"], fx: dot(2, 3),
        text: "Deal 2 damage a round for 3 rounds. Best served cold and repeatedly." },
    CardDef { id: "wine-07", deck: Deck::Wine, kind: CardKind::Curse, cost: 3,
        targets: "one", title: "The Long Decant of Winter", copies: 4,
        keywords: &["dot", "slow", "showy"], fx: dot(3, 3),
        text: "Deal 3 damage a round for 3 rounds. A vintage grudge, opened at last." },
    CardDef { id: "wine-08", deck: Deck::Wine, kind: CardKind::Reaction, cost: 2,
        targets: "one", title: "Send It Back", copies: 2, keywords: &["reaction"], fx: None,
        text: "Reaction: inert until the response window ships. Summon the sommelier." },
    // ---- Liquor — Burst, costs 2-3, 4 pulls. Premium 2.5-2.67 dmg/pull.
    CardDef { id: "liquor-01", deck: Deck::Liquor, kind: CardKind::Atk, cost: 2,
        targets: "one", title: "Shot Called", copies: 6, keywords: &["burst"], fx: dmg(5),
        text: "Deal 5 damage. Loud and immediate." },
    CardDef { id: "liquor-02", deck: Deck::Liquor, kind: CardKind::Atk, cost: 3,
        targets: "one", title: "Double", copies: 5, keywords: &["burst", "loud"],
        fx: dmg(7),
        text: "Deal 7 damage. Louder and more immediate." },
    CardDef { id: "liquor-03", deck: Deck::Liquor, kind: CardKind::Curse, cost: 2,
        targets: "one", title: "Hangover", copies: 6, keywords: &["dot"], fx: dot(2, 2),
        text: "Deal 2 damage a round for 2 rounds. Payable next round, with interest." },
    CardDef { id: "liquor-04", deck: Deck::Liquor, kind: CardKind::Buff, cost: 2,
        targets: "self", title: "Chaser", copies: 6, keywords: &["heal"], fx: heal(4),
        text: "Heal 4. Something soft to land on." },
    CardDef { id: "liquor-05", deck: Deck::Liquor, kind: CardKind::Atk, cost: 3,
        targets: "all", title: "Neat, No Ice, No Mercy", copies: 5,
        keywords: &["aoe", "loud"], fx: dmg(2),
        text: "Deal 2 damage to every player at the table, you included. A round of shots is still a round." },
    CardDef { id: "liquor-06", deck: Deck::Liquor, kind: CardKind::Buff, cost: 3,
        targets: "self", title: "Dutch Courage", copies: 6, keywords: &["shield"],
        fx: shield(7, 2),
        text: "Gain a shield that absorbs 7 damage over the next 2 rounds. Liquid confidence, briefly real." },
    CardDef { id: "liquor-07", deck: Deck::Liquor, kind: CardKind::Atk, cost: 3,
        targets: "one", title: "Last Call", copies: 4,
        keywords: &["burst", "loud", "showy", "public"], fx: dmg(8),
        text: "Deal 8 damage. The biggest hit in the game, named after the night's last mistake." },
    CardDef { id: "liquor-08", deck: Deck::Liquor, kind: CardKind::Reaction, cost: 2,
        targets: "self", title: "Spit It Out", copies: 2, keywords: &["reaction"], fx: None,
        text: "Reaction: inert until the response window ships. Undignified but effective." },
    // ---- Soft — Support, costs 1-2, 6 pulls. Shields at premium, one chip.
    CardDef { id: "soft-01", deck: Deck::Soft, kind: CardKind::Buff, cost: 1,
        targets: "one", title: "Water Round", copies: 6, keywords: &["heal"], fx: heal(2),
        text: "Heal 2 on any player. Someone feels better." },
    CardDef { id: "soft-02", deck: Deck::Soft, kind: CardKind::Buff, cost: 1,
        targets: "one", title: "Designated", copies: 6, keywords: &["shield"],
        fx: shield(3, 2),
        text: "Shield any player for 3 damage over the next 2 rounds. You take it for them." },
    CardDef { id: "soft-03", deck: Deck::Soft, kind: CardKind::Buff, cost: 2,
        targets: "all", title: "Snack Table", copies: 6, keywords: &["aoe", "heal"],
        fx: heal(1),
        text: "Heal 1 on every player at the table, you included. Crisps solve most things." },
    CardDef { id: "soft-04", deck: Deck::Soft, kind: CardKind::Reaction, cost: 1,
        targets: "self", title: "The Long Sober Look Across The Table", copies: 2,
        keywords: &["reaction"], fx: None,
        text: "Reaction: inert until the response window ships. You know what you did." },
    CardDef { id: "soft-05", deck: Deck::Soft, kind: CardKind::Util, cost: 2,
        targets: "one", title: "Cut Them Off", copies: 6, keywords: &["drain", "petty"],
        fx: drain(3),
        text: "Drain 3 pulls from a player's fullest vessel. It is for their own good." },
    CardDef { id: "soft-06", deck: Deck::Soft, kind: CardKind::Atk, cost: 1,
        targets: "one", title: "Splash of Cold Water", copies: 5, keywords: &[],
        fx: dmg(2),
        text: "Deal 2 damage. Rude, refreshing, effective." },
    CardDef { id: "soft-07", deck: Deck::Soft, kind: CardKind::Buff, cost: 2,
        targets: "one", title: "Glass Wall", copies: 5, keywords: &["shield"],
        fx: shield(5, 2),
        text: "Shield any player for 5 damage over the next 2 rounds. Politely impenetrable." },
    CardDef { id: "soft-08", deck: Deck::Soft, kind: CardKind::Buff, cost: 2,
        targets: "all", title: "Mother Hen", copies: 4,
        keywords: &["aoe", "shield", "quiet"], fx: shield(2, 2),
        text: "Shield every player for 2 damage over the next 2 rounds, you included. Everyone gets a coaster." },
];
```

`to_card` keeps its shape, plus the duration derivation. New fns are thin:
`card_fx` = `CATALOG.iter().find(|d| d.id == id).and_then(|d| d.fx)`;
`opening_hand` maps its `OPENERS` row through `card_by_id(...).expect(...)`;
`deck_copies` sums `copies as u16`; `shoe` flat-maps each def to
`copies` clones. String continuation in `wine-01`'s text uses `\` exactly as
`LONG_BODY` did.

- [ ] **Step 2: `EffectOp::PullDrain` and the opening-deal swap**

In `last_call.rs`: add the `PullDrain` variant (snake_case serde gives
`"pull_drain"`; extend Plan D's `test_effect_op_serde_names` with
`assert_eq!(serde_json::to_string(&EffectOp::PullDrain).unwrap(), "\"pull_drain\"");`).
In `set_vessel`, change the deal source only —
`for card in crate::lc_cards::deck_cards(deck)` becomes
`for card in crate::lc_cards::opening_hand(deck)` — the dedupe, the shoe
debit and the seq bump stay exactly as Plan D left them. Update the doc
comment: the deal is now F6's curated opener, no longer a slice-1 stub.

Note for the executor: until Task 2 swaps `resolve()`, the engine still maps
kinds to D8 numbers — a shield-kind Buff heals, drains are inert. That skew
lives for exactly one commit and no test observes it.

- [ ] **Step 3: Rewrite the `lc_cards` tests**

Replace the module's tests with (expected values final):

```rust
#[test]
fn test_catalog_shape_and_copy_sums() {
    assert_eq!(CATALOG.len(), 40);
    for deck in Deck::ALL {
        assert_eq!(deck_cards(deck).len(), 8, "{deck:?}");
        assert_eq!(deck_copies(deck), crate::last_call::LC_DECK_SIZE, "{deck:?}");
        assert_eq!(shoe(deck).len(), crate::last_call::LC_DECK_SIZE as usize);
    }
    let ids: HashSet<&str> = CATALOG.iter().map(|d| d.id).collect();
    assert_eq!(ids.len(), CATALOG.len());
    for def in CATALOG.iter() {
        assert!(def.id.starts_with(def.deck.slug()), "{}", def.id);
        assert!((1..=6).contains(&def.copies), "{}", def.id);
    }
}

#[test]
fn test_catalog_costs_match_deck_spread() {
    // Same spreads table as before (Beer 1..=2, Cider 1..=3, Wine 2..=3,
    // Liquor 2..=3, Soft 1..=2), minus the stale len-4 assert.
}

#[test]
fn test_fx_matches_kind() {
    for def in CATALOG.iter() {
        match (def.kind, def.fx) {
            (CardKind::Atk, Some(f)) => assert_eq!(f.op, EffectOp::Damage, "{}", def.id),
            (CardKind::Buff, Some(f)) => assert!(
                matches!(f.op, EffectOp::Heal | EffectOp::Shield), "{}", def.id),
            (CardKind::Curse, Some(f)) => assert_eq!(f.op, EffectOp::Dot, "{}", def.id),
            (CardKind::Util, Some(f)) => assert_eq!(f.op, EffectOp::PullDrain, "{}", def.id),
            (CardKind::Reaction, None) => {}
            (kind, fx) => panic!("{}: {kind:?} with fx {fx:?}", def.id),
        }
    }
}

#[test]
fn test_fx_rounds_match_op() {
    for f in CATALOG.iter().filter_map(|d| d.fx) {
        assert!(f.magnitude >= 1);
        match f.op {
            EffectOp::Damage | EffectOp::Heal | EffectOp::PullDrain =>
                assert_eq!(f.rounds, 0),
            EffectOp::Shield | EffectOp::Dot => assert!(f.rounds >= 1),
        }
    }
}

#[test]
fn test_targets_are_a_known_class() {
    for def in CATALOG.iter() {
        assert!(["self", "one", "all"].contains(&def.targets), "{}", def.id);
    }
}

#[test]
fn test_opening_hands() {
    for deck in Deck::ALL {
        let hand = opening_hand(deck);
        assert_eq!(hand.len(), 5, "{deck:?}");
        let first = format!("{}-01", deck.slug());
        assert!(hand.iter().any(|c| c.id == first), "{deck:?} lacks {first}");
        for c in &hand {
            assert_eq!(c.deck, deck);
            assert_ne!(c.kind, CardKind::Reaction, "{} in {deck:?} opener", c.id);
        }
    }
}

#[test]
fn test_duration_is_derived_from_fx() {
    assert_eq!(card_by_id("wine-02").unwrap().duration.as_deref(), Some("4 ROUNDS"));
    assert_eq!(card_by_id("beer-04").unwrap().duration.as_deref(), Some("2 ROUNDS"));
    assert_eq!(card_by_id("beer-01").unwrap().duration, None);
    assert_eq!(card_by_id("soft-04").unwrap().duration, None);
}

#[test]
fn test_catalog_titles_use_char_counts_not_bytes() { /* keep as-is */ }

#[test]
fn test_beer_has_no_bomb() { // DDv2 §3.1: "Attrition. No bomb in the list."
    for def in CATALOG.iter().filter(|d| d.deck == Deck::Beer) {
        if let Some(f) = def.fx {
            if f.op == EffectOp::Damage && def.targets == "one" {
                assert!(f.magnitude <= 4, "{}", def.id);
            }
        }
    }
}
```

(`card_fx` is exercised by Task 2's engine tests; §9 coverage and the keyword
contract are Task 3's.)

- [ ] **Step 4: The retune sweep — every test that encoded the 4-card placeholder**

Enumerated; each is a mechanical expected-value or fixture edit, no behavior
change. In `last_call.rs`:

- `preview_state`: bob's oversized hand becomes the first **4** Cider ids
  repeated three times (`deck_cards(Deck::Cider).into_iter().take(4).collect::<Vec<_>>()`
  fed to `repeat_n(…, 3)` — still 12; update the comment) and
  `st.discards` becomes `deck_cards(Deck::Beer).into_iter().take(4).collect()`
  (still count 4). Both slices exist so every downstream render/http count
  assertion is untouched.
- `test_preview_state_covers_every_variant`: the oversized-hand filter becomes
  `p.hand.len() > 10` with a comment — the two-deck player legitimately holds
  10 now (two 5-card openers), and the fixture's "exactly one oversized hand"
  means bob's 12.
- Plan D Task 2 tests (opener is 5 cards, not 4): in
  `test_set_vessel_activates_and_debits_the_shoe` the counts become
  `LC_DECK_SIZE - 5` (35) and `LC_DECK_SIZE - 10` (30); re-registration still
  deals 0. In `test_finish_and_draw_refills_and_draws` the batch becomes
  `deck_cards(Deck::Beer)[..5].to_vec()` (shoe 35 → expected batch 5), and the
  asserts become `hand.len() == 10`, `deck_count == 30`.
  `test_one_finish_and_draw_per_round`: same `[..5]` batch construction.
  `test_finish_and_draw_validates_the_batch`: the too-few case uses
  `deck_cards(Deck::Beer)[..4].to_vec()`, the wrong-deck case `[..4]` plus one
  cider card. `test_short_shoe_draws_partial`: `[..5]` and `[..3]` slices,
  final `hand.len() == 8`.
- Plan D Task 3/4/5 tests (opener size 5): `test_arm_moves_hand_to_armed`
  hand 4 (was 3); `test_disarm_returns_the_card` hand 5 (was 4);
  `test_arm_guard_order` gains one setup line before the `NotPlayable` assert —
  `st.players[2].hand.push(crate::lc_cards::card_by_id("soft-04").unwrap());`
  (Soft's opener excludes its reaction, and `UnknownCard` outranks
  `NotPlayable` in the guard order); `test_reveal_charges_orders_and_flips`
  cara's hand 5 (was 4); `test_elimination_is_immediate_...` discards 7 (was
  6 — bob's hand is 4 after arming) and the `bob_hand` comment; `test_soft_cap_
  discards_newest_first` builds 16 via `repeat_n(deck_cards(Deck::Cider), 2)`
  (8 distinct ×2). All other Plan D expected values — beer-01 dmg 2, beer-02
  dmg 4, cider-04 dmg 6, soft-01 heal 2, cider-01 dot 1 expiring round 3,
  liquor-02 killing an hp-4 player, pulls 8/10/6, "Nudge" in the revealed
  JSON — hold **by construction** (F10) and must not be touched.

In `lc_render.rs`: the two cider-04 keyword tests (~lines 967, 979) expect 5
keywords now — 3 chips rendered plus a `+2` fold (was 6 and `+3`). Everything
else in that file is count-agnostic or uses ids/titles that survive
("wine-01" still the overflow body, "The Long Sober Look Across The Table"
still the sm-band title, the leak tests' `beer-01`/"Nudge"/"Sticky" needles
still name real cards).

In `tests/http.rs`: no expected-value changes — hands still contain each
deck's `-01` (openers guarantee it) and `data-count` asserts are dynamic. The
stale "4 placeholder cards" wording in the comment near line 4261 is updated.

Then run `./scripts/verify.sh` and chase any count assertion this list missed
(if Plan C or E landed first, their catalog-shape fixtures surface here) —
every such edit is the same mechanical kind, recorded in the task report.

- [ ] **Step 5: Commit**

```bash
git add drinkinggame/src/lc_cards.rs drinkinggame/src/last_call.rs drinkinggame/src/lc_render.rs drinkinggame/tests/http.rs
git commit -m "feat(lastcall): the real catalog — 40 cards, copy-weighted shoes, curated openers, per-card fx data"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 2: Per-card resolution — `resolve()` plays the card, not the kind

**Class:** B (logic, tests specified below)

**Why this class:** a pure rewrite of the resolution program's effect step
with every semantic pinned by a test and expected values written here; no
concurrency, auth or broadcast.

**Files:**
- Modify: `drinkinggame/src/last_call.rs`
- Test: `drinkinggame/src/last_call.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: Task 1's `card_fx`, `FxDef`, `EffectOp::PullDrain`; Plan D's
  `resolve()` structure, `apply_damage`, ordered `plays`.
- Produces: `resolve()` resolving per-card fx — no public signature changes.
  Private helper `fn drain_pulls(player: &mut LcPlayer, n: i32)`.

- [ ] **Step 1: Replace the D8 mapping inside `resolve()` step 1**

For each play (same skip/fizzle/subject rules as Plan D — D2 subjects, dead
sources skipped, dead "one"-targets fizzle):

```rust
match crate::lc_cards::card_fx(&play.card.id) {
    // Reactions are inert (D9/F5); an id the catalog no longer knows is
    // treated the same — deliberate fail-soft for version skew (F1).
    None => {}
    Some(f) => for subject in subjects {
        match f.op {
            EffectOp::Damage => apply_damage(subject, f.magnitude),
            EffectOp::Heal => /* hp += magnitude; no ceiling (TBD-3) */,
            EffectOp::PullDrain => drain_pulls(subject, f.magnitude),
            // F8: shields register NOW, so they absorb later plays in this
            // round's order — replace-not-stack by (op, subject), D10.
            EffectOp::Shield => /* upsert Effect { op: Shield, magnitude,
                expires_round: self.round + f.rounds, .. } */,
            // Dots still queue (append at step 3): never tick in their own
            // creation round.
            EffectOp::Dot => /* queue Effect { op: Dot, magnitude,
                expires_round: self.round + f.rounds, .. } */,
        }
    }
}
```

`drain_pulls(player, n)`: `n` times, pick the vessel with the greatest
`pulls_left` (tie → lowest index) and decrement it; stop early when every
vessel is at 0. Steps 2–7 of Plan D's `resolve()` (dot ticks, queued-effect
append with replace, expiry, soft cap, seq, outcome freeze, rollover) are
untouched.

- [ ] **Step 2: Delete the D8 constants**

Remove `DMG_PER_COST`, `HEAL_PER_COST`, `DOT_PER_COST`, `CURSE_ROUNDS` — no
non-comment references remain after Step 1 (grep to confirm). Update the two
Plan D test comments that mention them (`test_heal_has_no_ceiling`'s
"1×HEAL_PER_COST", `test_curse_ticks_...`'s "1 + CURSE_ROUNDS") to cite the
catalog fx instead. Their asserted values are unchanged (F10).

- [ ] **Step 3: Tests**

```rust
#[test]
fn test_liquor_hits_above_par() { // F3's burst premium is engine-real
    let mut st = LastCallState::new(vec![(1, "alice".into()), (2, "bob".into())], 42);
    st.set_vessel(1, Deck::Liquor, "shot").unwrap();
    st.beat = Beat::Lock;
    st.arm(1, "liquor-01").unwrap(); // cost 2, Damage 5 — not 2 x cost
    st.set_target(1, "liquor-01", Some(1)).unwrap();
    st.lock_in(1).unwrap();
    st.advance_beat().unwrap();
    st.advance_beat().unwrap();
    st.resolve().unwrap();
    assert_eq!(st.players[1].hp, 10); // 15 - 5
    assert_eq!(st.players[0].vessels[0].pulls_left, 2); // 4 - 2: cost, not fx
}

#[test]
fn test_shield_card_protects_in_its_own_round_when_it_outspends() { // F8
    let mut st = at_lock();
    // soft-07 is not in Soft's opener (F6) — deal it into cara's hand.
    st.players[2].hand.push(crate::lc_cards::card_by_id("soft-07").unwrap());
    // cara (Soft) spends 3 pulls (soft-07 + soft-01), alice (Beer) spends 2:
    // cara resolves first, Glass Wall lands on bob before Grind does.
    st.arm(3, "soft-07").unwrap();
    st.set_target(3, "soft-07", Some(1)).unwrap();
    st.arm(3, "soft-01").unwrap();
    st.set_target(3, "soft-01", Some(2)).unwrap();
    st.lock_in(3).unwrap();
    st.arm(1, "beer-02").unwrap(); // Damage 4 -> bob
    st.set_target(1, "beer-02", Some(1)).unwrap();
    st.lock_in(1).unwrap();
    st.advance_beat().unwrap();
    st.advance_beat().unwrap();
    st.resolve().unwrap();
    assert_eq!(st.players[1].hp, 15);                 // fully absorbed
    assert_eq!(st.effects.len(), 1);                  // shield survives, worn
    assert_eq!(st.effects[0].magnitude, 1);           // 5 - 4
    assert_eq!(st.players[2].hp, 17);                 // soft-01 healed cara
}

#[test]
fn test_a_cheap_shield_resolves_after_the_big_hit() { // F8's tension
    let mut st = at_lock();
    st.arm(3, "soft-02").unwrap(); // 1 pull, Shield 3 -> bob
    st.set_target(3, "soft-02", Some(1)).unwrap();
    st.lock_in(3).unwrap();
    st.arm(1, "beer-02").unwrap(); // 2 pulls, Damage 4 -> bob
    st.set_target(1, "beer-02", Some(1)).unwrap();
    st.lock_in(1).unwrap();
    st.advance_beat().unwrap();
    st.advance_beat().unwrap();
    st.resolve().unwrap();
    assert_eq!(st.players[1].hp, 11);       // alice outspent: hit lands first
    assert_eq!(st.effects[0].magnitude, 3); // the late shield arrives intact
}

#[test]
fn test_drain_hits_the_fullest_vessel_and_floors_at_zero() { // F4
    let mut st = at_lock();
    // Alice's second vessel is built by hand: set_vessel is Draw-gated and
    // the fixture is already at Lock.
    st.players[0].vessels.push(Vessel {
        deck: Deck::Soft, pulls_max: 6, pulls_left: 3, container: "cup".into(),
    });
    st.arm(2, "cider-03").unwrap(); // Drain 3 -> alice
    st.set_target(2, "cider-03", Some(0)).unwrap();
    st.lock_in(2).unwrap();
    st.advance_beat().unwrap();
    st.advance_beat().unwrap();
    st.resolve().unwrap();
    // Beer vessel was fullest (8 vs 3): drained to 5, Soft cup untouched.
    assert_eq!(st.players[0].vessels[0].pulls_left, 5);
    assert_eq!(st.players[0].vessels[1].pulls_left, 3);
    assert_eq!(st.players[0].hp, 15); // drains never touch HP

    // Floor: drain more than remains.
    let mut st = at_lock();
    st.players[0].vessels[0].pulls_left = 2;
    st.arm(2, "cider-03").unwrap();
    st.set_target(2, "cider-03", Some(0)).unwrap();
    st.lock_in(2).unwrap();
    st.advance_beat().unwrap();
    st.advance_beat().unwrap();
    st.resolve().unwrap();
    assert_eq!(st.players[0].vessels[0].pulls_left, 0);
}

#[test]
fn test_aoe_includes_the_source() { // F9 / D2
    let mut st = at_lock();
    st.arm(1, "beer-05").unwrap(); // Damage 1 to all, no target needed
    st.lock_in(1).unwrap();
    st.advance_beat().unwrap();
    st.advance_beat().unwrap();
    st.resolve().unwrap();
    for p in &st.players {
        assert_eq!(p.hp, 14, "seat {}", p.seat);
    }
}

#[test]
fn test_a_reaction_play_resolves_inert() { // F5 — even if one sneaks in
    let mut st = at_lock();
    st.plays.push(Play {
        card: crate::lc_cards::card_by_id("beer-08").unwrap(),
        source_seat: 0, target: Some(1), paid_from: Deck::Beer, order_key: 1,
    });
    st.beat = Beat::Resolve;
    st.resolve().unwrap();
    assert!(st.players.iter().all(|p| p.hp == 15));
    assert_eq!(st.discards.len(), 1); // still discarded (8.4)
}

#[test]
fn test_dot_duration_is_per_card() { // F10: no CURSE_ROUNDS
    // cider-07 (Dot 2 x 2) beside cider-01 (Dot 1 x 2): the magnitude comes
    // from the card, not from cost x DOT_PER_COST. cider-07 is not in
    // Cider's opener (F6) — deal it into bob's hand.
    let mut st = at_lock();
    st.players[1].hand.push(crate::lc_cards::card_by_id("cider-07").unwrap());
    st.arm(2, "cider-07").unwrap();
    st.set_target(2, "cider-07", Some(0)).unwrap();
    st.lock_in(2).unwrap();
    st.advance_beat().unwrap();
    st.advance_beat().unwrap();
    st.resolve().unwrap();
    assert_eq!(st.effects[0].magnitude, 2);      // per-card, not 1 x cost
    assert_eq!(st.effects[0].expires_round, 3);  // round 1 + rounds 2
}
```

- [ ] **Step 4: Commit**

```bash
git add drinkinggame/src/last_call.rs
git commit -m "feat(lastcall): resolve() plays per-card fx — PullDrain, same-round shields, D8 mapping dies"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 3: The keyword contract and the §9 coverage floor

**Class:** B (logic, tests specified below)

**Why this class:** every rule is a predicate over static data with the
expected outcome stated; the tests are the spec.

**Files:**
- Modify: `drinkinggame/src/lc_cards.rs` (tests only, plus any keyword edit
  the predicates surface)
- Test: `drinkinggame/src/lc_cards.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: Task 1's `CATALOG`, `MECHANICAL_KW`, `TONE_KW`,
  `BURST_KW_MIN_DAMAGE`; `lc_render::{TITLE_CLAMP_CHARS, BODY_CLAMP_CHARS}`
  semantics via literal thresholds (14/24/108 — the same numbers
  `title_ramp_class` branches on).
- Produces: nothing new — a durable test floor.

- [ ] **Step 1: The keyword contract tests**

```rust
#[test]
fn test_keywords_come_from_the_two_vocabularies() { // F7
    for def in CATALOG.iter() {
        for kw in def.keywords {
            assert!(
                MECHANICAL_KW.contains(kw) || TONE_KW.contains(kw),
                "{}: unknown keyword {kw}", def.id
            );
        }
        assert!(def.keywords.len() <= 5, "{}", def.id);
    }
}

#[test]
fn test_mechanical_keywords_are_bidirectional() { // F7 — chips cannot lie
    for def in CATALOG.iter() {
        let has = |kw: &str| def.keywords.contains(&kw);
        assert_eq!(has("aoe"), def.targets == "all", "{}", def.id);
        assert_eq!(has("reaction"), def.kind == CardKind::Reaction, "{}", def.id);
        let op = |o: EffectOp| def.fx.is_some_and(|f| f.op == o);
        assert_eq!(has("dot"), op(EffectOp::Dot), "{}", def.id);
        assert_eq!(has("shield"), op(EffectOp::Shield), "{}", def.id);
        assert_eq!(has("heal"), op(EffectOp::Heal), "{}", def.id);
        assert_eq!(has("drain"), op(EffectOp::PullDrain), "{}", def.id);
        let bursty = def.targets == "one"
            && def.fx.is_some_and(|f| {
                f.op == EffectOp::Damage && f.magnitude >= BURST_KW_MIN_DAMAGE
            });
        assert_eq!(has("burst"), bursty, "{}", def.id);
    }
}
```

- [ ] **Step 2: The §9 coverage floor, durable**

Property-shaped (≥ 1 per case), so a future catalog edit can add or retitle
cards freely but can never silently stop exercising a rendering branch:

```rust
#[test]
fn test_catalog_covers_every_title_band() { // §7.5 ramp, forever
    let len = |d: &CardDef| d.title.chars().count();
    assert!(CATALOG.iter().any(|d| len(d) <= 14));
    assert!(CATALOG.iter().any(|d| (15..=24).contains(&len(d))));
    assert!(CATALOG.iter().any(|d| len(d) > 24));
}

#[test]
fn test_catalog_has_an_overflowing_body() {
    // >108 chars = BODY_CLAMP_CHARS: the 3-line clamp and data-expandable
    // must stay proven against rendered content (spec §9).
    let overflowing: Vec<&str> = CATALOG.iter()
        .filter(|d| d.text.chars().count() > 108)
        .map(|d| d.id)
        .collect();
    assert!(overflowing.contains(&"wine-01"), "got {overflowing:?}");
}

#[test]
fn test_catalog_keeps_the_keyword_extremes() {
    assert!(CATALOG.iter().any(|d| d.keywords.is_empty()));
    // > 3 keywords is what makes card_face emit the +n fold chip.
    assert!(CATALOG.iter().any(|d| d.keywords.len() > 3));
}
```

- [ ] **Step 3: Commit**

```bash
git add drinkinggame/src/lc_cards.rs
git commit -m "test(lastcall): keyword contract is bidirectional; the §9 adversarial floor is pinned durable"
```

**Acceptance:** `./scripts/verify.sh` — all green.

---

### Task 4: The preview page tells the truth about the catalog

**Class:** A (compiler/lint-gated)

**Why this class:** label strings and doc comments; the preview route is
already catalog-driven, so the 40 cards flow through untouched builders and
Askama/format machinery that the compiler and existing tests gate.

**Files:**
- Modify: `drinkinggame/src/lc_preview.rs`
- Modify: `drinkinggame/src/lc_cards.rs` (module doc only, if Task 1's header
  needs the final counts corrected)

**Interfaces:**
- Consumes: Task 1's `CATALOG` (the text-cases group iterates it already).
- Produces: nothing — display strings only.

- [ ] **Step 1: Update the stale copy**

In `lc_preview.rs`: the text-cases group's row label "Catalog — all 20 cards,
deliberately adversarial (spec §9)" becomes dynamic —
`&format!("Catalog — all {} distinct cards (x copies make the 40-card shoe), spec §9 coverage now on real content", CATALOG.len())`
— and the group note's "deliberately adversarial 20" sentence is rewritten to
say the catalog is real content whose §9 coverage is test-pinned
(Task 3). Sweep the file for other placeholder-era counts ("20", "four cards
per deck") in labels, notes and comments; `sample_cards`' ids
(`beer-01`, `wine-04`, `cider-01`) all still exist and stay.

- [ ] **Step 2: Commit**

```bash
git add drinkinggame/src/lc_preview.rs drinkinggame/src/lc_cards.rs
git commit -m "docs(lastcall): preview copy reflects the real 40-card catalog"
```

**Acceptance:** `./scripts/verify.sh` — all green.

**Browser checkpoint (the plan's only one), after this task:** open
`http://localhost:3001/lastcall/preview` (`cargo run -p drinkinggame`) in a
real, focused browser and eyeball the text-cases group: 40 cards render; the
three ramp sizes visibly differ (Nudge at 30px, One For The Table at 24px,
The Long Sober Look Across The Table at 20px); wine-01's body clamps at three
lines and carries the expandable marking; cider-04 shows three chips plus
`+2`. Ten minutes; record findings in the ledger.

---

## Before the plan is done

- Every task has a class (B, B, B, A) and every acceptance is
  `./scripts/verify.sh` — no per-task reviewer per `plan-economics` §3/§4;
  **one whole-plan review of the branch diff at the end** on the most capable
  model. Content judgment (are the numbers fun?) is explicitly *not* the
  reviewer's job — that is the F12 table's user review plus a playtest.
- No `cargo sqlx prepare` (no migration; `drinkinggame` is runtime-checked).
- Interfaces line up: Task 1's `card_fx`/`FxDef` are what Task 2's `resolve()`
  consumes; Task 1's `MECHANICAL_KW`/`TONE_KW`/`BURST_KW_MIN_DAMAGE` are what
  Task 3's predicates read; Plan E's named surface is `shoe(deck)` +
  `card_by_id` + the unchanged `finish_and_draw` contract.
- The F12 review table and Task 1's Rust CATALOG were cross-checked row by
  row (id, kind, cost, targets, effect, copies, keywords, text) during
  plan-writing; the executor transcribes the Rust block verbatim and never
  re-derives from the table.
- Self-check sums: per deck 8 distinct, copies 6+6+6+6+5+5+4+2 = 40 =
  `LC_DECK_SIZE`; five openers of 5 with no reactions; every §9 floor case
  named in the decisions section exists in the data.
- Plan D's placeholder-era wording this plan supersedes: D6's
  "`LC_DECK_SIZE` (40, placeholder) — Plan F resets" (kept at 40, now
  test-pinned), D8's mapping (deleted, F10), D15's "until Plan F defines a
  real opening hand" (F6). Reaction inertness (D9) and the no-stack rule
  (D10) stand.
- `drinkinggame` stays clippy-clean; distinct-warning count stays 17.
