# Last Call — generic card draft

A wave of cards that trade in **information and card movement** rather than
more numbers on the HP pool. None of them are in a deck: they ship
`copies: 0`, the convention the three challenge prototypes already use —
catalog-present so the engine can exercise them, shoe-absent until someone
balances them into a deck.

Status: **all four primitives are built** — `targets: "other"`, `Reveal`,
`Pour` and `Swap` — with nine cards in the catalog at `copies: 0`. What is
left is the UI: nothing renders a reveal, and nothing offers the pickers the
two parking waves need. Each group names the one engine primitive it needs; see the end
for what building those costs.

> Because they ship `copies: 0`, none of these can be drawn in a real game
> yet — same status as the challenge prototypes. The engine resolves them;
> nothing deals them, and nothing draws them on screen.

---

## Targeting model

Your clarification, written down. Current classes plus one new one:

| Class | Who it can name | Notes |
|---|---|---|
| `self` | you only | |
| `one` | **any player, you included** | Already the rule — `set_target` allows self-targeting today. |
| `other` | any player **except you** | **New.** For cards that need two *different* people. |
| `all` | the whole table, you included | |
| `right` | auto: first live seat to your right | Duel challenges only, enforced by a catalog test. |

`other` is the only addition, and it's the honest way to express "this card
needs someone else" — a swap with yourself is a no-op, and a private reveal
of your own hand is a card that does nothing. Cheap to add: one arm in
`set_target`, one entry in `test_targets_are_a_known_class`.

---

## A. Reveal cards

**Needs:** a `Reveal` op with two scopes — `Table` (everyone sees) and
`Caster` (only the player who played it sees).

| id | Title | Kind | Targets | Cost | Effect |
|---|---|---|---|---|---|
| `gen-01` | **OPEN BOOK** | UTIL | `one` | 2 | Target's hand is revealed to the whole table for the rest of the round. |
| `gen-02` | **BARMAN'S EYE** | UTIL | `other` | 2 | You alone see the target's hand for the rest of the round. Nobody else is told what you saw. |
| `gen-03` | **NOTHING UP MY SLEEVES** | UTIL | `self` | 1 | Reveal your *own* hand to the table, then draw 2. |
| `gen-04` | **SHOW US ONE** | UTIL | `all` | 2 | Every player picks one card from their own hand and reveals it to the table. |

Flavour:

- **OPEN BOOK** — "Everyone leans in. There is nowhere left to put your elbows."
- **BARMAN'S EYE** — "You know. They know you know. Nobody else does."
- **NOTHING UP MY SLEEVES** — "Honesty is a tempo play. Draw two for the trouble."
- **SHOW US ONE** — "Everybody picks their least embarrassing card. That is itself information."

`gen-03` targeting `self` is deliberate and not a wasted card — voluntarily
opening your hand is a **pact move**, and drawing 2 pays for it. `gen-04`'s
"pick your own" is what makes it interesting: what someone *chooses* to show
says more than a random card would.

### Two design decisions these force

**Snapshot, not a live window.** A reveal captures what the target held **at
resolve** — not a live view of their hand. A live window would leak the next
draw beat and keep leaking as their hand changes, which is a much bigger card
than the cost suggests. Snapshot is also what "reveal" means at a real table:
you show, they look, you take it back.

**It lasts the FOLLOWING round, not "the rest of this one."** This draft
originally said "rest of the round" and that turned out to be unbuildable:
reveals land during `resolve()`, and `resolve()` rolls the round over a few
statements later, so a reveal pruned at the end of its own round is dropped
the instant it is taken — a 2-pull no-op. A snapshot taken in round N is
readable through round N+1, which is the Diplomacy where you actually act on
it, and dropped at the rollover after that.

**A private reveal must never touch the log.** `LogEntry` is public and
permanent. `gen-02` may log *that* it was played (`Play` already names card
and target) but never what was seen. `gen-01`'s contents are public, so
logging those is fine. This is the same public/private line `TabSettle`
already walks — it logs the seat, never the tab.

---

## B. Trade cards

**Built**, as `SwapDef { take, give }` + `SwapState`. Two flags rather than
three card types, because the three cards here are exactly the three useful
combinations — and that fell out of the build rather than being designed in.

- **The take is always at random.** Letting the caster pick out of another
  hand would need that hand revealed to them first, which is group A's job
  and a more expensive card. Keeping the take blind is what stops this wave
  quietly becoming a better `Barman's Eye`.
- **Only the give parks.** `Pickpocket` takes and gives nothing, so it makes
  no choice and resolves on the spot. The other two park on one decider.
- **The taken card is private,** and it needed its own projection type
  (`PublicSwap`) to stay that way. A `#[serde(skip)]` would also have dropped
  it from the blob, and the park has to survive a reload — so the secret is
  dropped at the projection, where every other secret in this engine is.
- **Declining costs the pulls.** You paid to look. The card goes back to its
  owner's hand, not to a pile — it was never discarded.

| id | Title | Kind | Targets | Cost | Effect |
|---|---|---|---|---|---|
| `gen-05` | **SWAP YOU FOR IT** | UTIL | `other` | 2 | Draw one card at random from the target's hand. **Keep it** and give one card of your choice back to them, **or decline** — the card returns and no cards move. |
| `gen-06` | **A GIFT** | UTIL | `other` | 1 | Give the target one card of your choice. They cannot refuse. |
| `gen-07` | **PICKPOCKET** | UTIL | `other` | 3 | Take one card at random from the target's hand. Nothing goes back. |

Flavour:

- **SWAP YOU FOR IT** — "You take a look at what they've got. You do not have to like it."
- **A GIFT** — "A present. Absolutely no strings. Enjoy it."
- **PICKPOCKET** — "They will notice. Not immediately."

`gen-05` is your card, with the decline branch made explicit: if you keep it
you **must** give one back, so hand sizes are preserved and the swap is a
true trade. If you decline, both hands end unchanged and you're out the pulls
— the card is a swap you're allowed to back out of, which makes paying for it
a real gamble rather than a free peek.

`gen-06` looks like a gift and isn't: the hand cap is **12**, and cards over
it are discarded at end of round. Handing someone their twelfth and thirteenth
card is an attack wearing a bow. `gen-07` is strictly better than `gen-05`
(take, no give, no decline) so it costs the most in the set.

---

## C. Drink-and-discard cards

**Built**, as `PourDef` + `PourState`. The engine parks the round on several
choices at once — the challenge vote's shape, but waiting on N players
instead of an electorate.

Three things the build settled:

- **The drink lands immediately; only the discard parks.** Nobody chooses how
  much they drink, so there is nothing to wait for there. It goes through
  `drain`, so it empties real pulls and moves `drinks`.
- **You owe what you can pay.** A seat holding fewer cards than `n` pitches
  everything it has, and a seat holding nothing is left off the debt list
  entirely — it can't pay a debt it was never given, and listing it would
  park the room forever.
- **Two parks can hold the same round.** One player pours while another
  challenges. Whichever settles *last* runs the rollover; a settle that rolled
  over unconditionally would advance the round out from under the other park
  and strand it. Both guards exist, and both orders are tested — a test that
  only ever settled the pour first left the challenge-side guard deletable
  with the suite green.

Your rule, made the whole mechanic: **the drink count and the discard count
are the same number.** One parameter, N.

| id | Title | Kind | Targets | Cost | Effect |
|---|---|---|---|---|---|
| `gen-08` | **ONE FOR THE ROAD** | UTIL | `all` | 1 | Everyone drinks 1 and discards 1 card of their choice. You included. |
| `gen-09` | **CLOSING TIME** | UTIL | `all` | 3 | Everyone drinks 3 and discards 3. You included. |
| `gen-10` | **SPILLAGE** | UTIL | `one` | 2 | Target drinks 2 and discards 2. |

Flavour:

- **ONE FOR THE ROAD** — "A small round, and a small price. Nobody is exempt, you least of all."
- **CLOSING TIME** — "Glasses down, hands empty. The lights are coming on."
- **SPILLAGE** — "Straight down the front. Some of that was your good stuff."

Three notes:

- **The drink must go through `drain`,** not a log line. It's a real cost —
  it empties vessels, it feeds `drinks`, and an empty vessel is how you get
  pushed into a finish-and-draw. Anything that "makes you drink" without
  moving pulls is decoration.
- **The handicap has to apply.** Every other cost in this game scales by
  `handicap_pct`, and a card that makes people drink is exactly where that
  matters most. A table-wide N that ignores it silently overrules everyone
  who set one.
- **`all` includes the caster**, following `soft-08` Mother Hen ("you
  included"). That self-cost is most of what balances `gen-09`.

---

## D. Needs one more thing

| id | Title | Kind | Targets | Cost | Effect |
|---|---|---|---|---|---|
| `gen-11` | **THE USUAL** | UTIL | `other` | 1 | Name a deck. The target reveals to you every card they hold from it. |

"They always order the same thing. You are counting."

A cheaper, narrower `BARMAN'S EYE` — and it needs something none of the
others do: **a play parameter that isn't a target.** Today an `ArmedCard`
carries a card and an optional seat, full stop. A card that also carries "and
the deck I'm naming" needs that struct widened. Worth knowing before it gets
designed into more cards; parked here as the one card that asks for it.

---

## What building this costs

Four new primitives, roughly in ascending order of work:

| Primitive | Cards | Notes |
|---|---|---|
| `targets: "other"` | all of B, D | **Built.** Not the "one guard arm" this doc first claimed — the class had to reach seven engine sites, the renderer and the seat picker. Only the self-rejection is genuinely one arm; the rest went through a new `targets_a_seat` predicate so a future class can't miss one. |
| `Reveal { scope }` | A, D | **Built** for group A (`gen-04` still needs group C's machinery, `gen-11` needs a play parameter). The private channel **already exists**: `broadcast_lc` publishes only the public panel, and each phone re-fetches its own hand through `hand_pane_html(…, player_id)`, rendered server-side per viewer. A caster-only reveal is genuinely private there, not client-side hidden. |
| `TableDiscard { n }` | C | **Built** as `Pour`. Also needed a settle ROUTE, unlike the reveal wave: a pour parks the round, and `test/grant` can push a `copies: 0` prototype into a hand, so an unreachable settle path is a room that never moves again rather than a merely inert feature. |
| `Swap` | B | **Built.** The random take comes off the engine's `LcRng`, so a theft replays identically from the seed. |

**`EffectOp` cannot express any of these.** Its five ops — `Damage`, `Heal`,
`Shield`, `Dot`, `PullDrain` — all move a number on HP or pulls. Nothing in
the vocabulary moves a *card* or reveals *information*. That's the real
finding of this draft: this isn't a wave of cards, it's a second effect
system next to the numeric one.

The pattern to copy is `ChallengeDef`, not `FxDef`: challenges already park
the game on a human decision, carry catalog-side rules that never enter the
blob, and use a `key` so a stale screen can't answer the wrong prompt. Every
card in B and C wants exactly that shape.

**Order built:** `other` → `Reveal` → `Pour` → `Swap`. What remains is UI —
and for the two parking waves it is not optional: a pour or a trade played
without a picker freezes the round. The settle routes exist and are tested
end to end, so the way out is reachable, but these cards must stay at
`copies: 0` until something calls them.
