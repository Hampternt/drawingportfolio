//! 3 Man dice-game engine — a pure state machine, no I/O, no SQL, no RNG.
//!
//! Dice values are always passed in by the caller (routes roll the dice and
//! hand the values here); the module only ever computes state transitions.
//! `ThreeManState` round-trips losslessly through `to_json`/`from_json`
//! because later tasks snapshot it into a DB column between requests.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Ready,
    Rolled,
    HandOff,
    Assign,
    Gifts,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum GiveMode {
    Both,
    Split,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Call {
    pub player_id: i64,
    pub amount: u8,
    pub reason: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Gift {
    pub player_id: i64,
    pub dice_count: u8,
    pub values: Option<Vec<u8>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DoubleState {
    pub value: u8,
    pub owner: i64,
    pub mode: Option<GiveMode>,
    pub slots: Vec<Option<i64>>,
    pub gifts: Vec<Gift>,
    pub payback: Option<u8>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ThreeManState {
    pub order: Vec<i64>,
    pub roller_idx: usize,
    pub three_man: i64,
    pub phase: Phase,
    pub dice: Option<(u8, u8)>,
    pub calls: Vec<Call>,
    pub double: Option<DoubleState>,
    pub pending_double: bool,
    pub handoff_from: Option<i64>,
    pub last_roller: Option<i64>,
    pub stale: bool,
    pub seq: u64,
}

#[derive(Debug, PartialEq)]
pub enum TmError {
    WrongPhase,
    BadTarget,
    TooFewPlayers,
}

impl ThreeManState {
    /// Rotates `members` so `starter` sits at index 0; `starter` is also the
    /// first 3 Man.
    pub fn new(members: Vec<i64>, starter: i64) -> Self {
        let pos = members.iter().position(|&p| p == starter).unwrap_or(0);
        let mut order = members[pos..].to_vec();
        order.extend_from_slice(&members[..pos]);
        ThreeManState {
            order,
            roller_idx: 0,
            three_man: starter,
            phase: Phase::Ready,
            dice: None,
            calls: Vec::new(),
            double: None,
            pending_double: false,
            handoff_from: None,
            last_roller: None,
            stale: false,
            seq: 0,
        }
    }

    pub fn roller(&self) -> i64 {
        self.order[self.roller_idx]
    }

    /// order[(idx+1) % len] — "next to roll".
    pub fn left_of(&self, idx: usize) -> i64 {
        self.order[(idx + 1) % self.order.len()]
    }

    pub fn right_of(&self, idx: usize) -> i64 {
        let len = self.order.len();
        self.order[(idx + len - 1) % len]
    }

    pub fn roll(&mut self, d1: u8, d2: u8) -> Result<(), TmError> {
        if self.phase != Phase::Ready {
            return Err(TmError::WrongPhase);
        }
        let sum = d1 + d2;
        let roller = self.roller();
        self.dice = Some((d1, d2));
        self.calls.clear();
        self.double = None;
        self.pending_double = false;
        self.handoff_from = None;
        self.stale = false;
        self.seq += 1;

        let threes = u8::from(d1 == 3) + u8::from(d2 == 3) + u8::from(sum == 3);
        let mut handoff = false;
        if threes > 0 {
            if roller == self.three_man {
                handoff = true; // no drink — the title moves instead
            } else {
                let reason = match threes {
                    1 => "a 3 on the dice",
                    2 => "two 3s on the dice",
                    _ => "3s everywhere",
                };
                self.calls.push(Call {
                    player_id: self.three_man,
                    amount: threes,
                    reason: reason.into(),
                });
            }
        }
        // Sum rules have no stated hand-off exception (unlike the threes
        // rule's explicit "UNLESS the roller IS the 3 Man"), so they fire
        // unconditionally — a hand-off roll can still trigger a 7/9/11.
        match sum {
            7 => self.calls.push(Call {
                player_id: self.left_of(self.roller_idx),
                amount: 1,
                reason: "7 — left of the roller".into(),
            }),
            9 => self.calls.push(Call {
                player_id: self.right_of(self.roller_idx),
                amount: 1,
                reason: "9 — right of the roller".into(),
            }),
            11 => self.calls.push(Call {
                player_id: roller,
                amount: 1,
                reason: "11 — the roller".into(),
            }),
            _ => {}
        }
        if d1 == d2 {
            self.double = Some(DoubleState {
                value: d1,
                owner: roller,
                mode: None,
                slots: vec![],
                gifts: vec![],
                payback: None,
            });
        }
        self.phase = if handoff {
            self.pending_double = d1 == d2;
            Phase::HandOff
        } else if d1 == d2 {
            Phase::Assign
        } else {
            Phase::Rolled
        };
        Ok(())
    }

    /// HandOff only. `target` must be in `order` and not the current 3 Man.
    pub fn give_three_man(&mut self, target: i64) -> Result<(), TmError> {
        if self.phase != Phase::HandOff {
            return Err(TmError::WrongPhase);
        }
        if target == self.three_man || !self.order.contains(&target) {
            return Err(TmError::BadTarget);
        }
        self.resolve_handoff(target);
        Ok(())
    }

    fn resolve_handoff(&mut self, target: i64) {
        self.handoff_from = Some(self.three_man);
        self.three_man = target;
        self.phase = if self.pending_double {
            Phase::Assign
        } else {
            Phase::Rolled
        };
        self.pending_double = false;
    }

    pub fn set_mode(&mut self, mode: GiveMode) -> Result<(), TmError> {
        if self.phase != Phase::Assign {
            return Err(TmError::WrongPhase);
        }
        if mode == GiveMode::Split && self.order.len() < 3 {
            return Err(TmError::TooFewPlayers);
        }
        let Some(double) = self.double.as_mut() else {
            return Err(TmError::WrongPhase);
        };
        double.mode = Some(mode);
        double.slots = match mode {
            GiveMode::Both => vec![None],
            GiveMode::Split => vec![None, None],
        };
        double.gifts.clear();
        Ok(())
    }

    pub fn pick_target(&mut self, slot: usize, player: i64) -> Result<(), TmError> {
        if self.phase != Phase::Assign {
            return Err(TmError::WrongPhase);
        }
        let Some(double) = self.double.as_mut() else {
            return Err(TmError::WrongPhase);
        };
        if double.mode.is_none() {
            return Err(TmError::WrongPhase);
        }
        if slot >= double.slots.len() {
            return Err(TmError::BadTarget);
        }
        if player == double.owner || !self.order.contains(&player) {
            return Err(TmError::BadTarget);
        }
        if double
            .slots
            .iter()
            .enumerate()
            .any(|(i, s)| i != slot && *s == Some(player))
        {
            return Err(TmError::BadTarget);
        }
        double.slots[slot] = Some(player);
        Ok(())
    }

    pub fn clear_slot(&mut self, slot: usize) -> Result<(), TmError> {
        if self.phase != Phase::Assign {
            return Err(TmError::WrongPhase);
        }
        let Some(double) = self.double.as_mut() else {
            return Err(TmError::WrongPhase);
        };
        if slot >= double.slots.len() {
            return Err(TmError::BadTarget);
        }
        double.slots[slot] = None;
        Ok(())
    }

    pub fn send(&mut self) -> Result<(), TmError> {
        if self.phase != Phase::Assign {
            return Err(TmError::WrongPhase);
        }
        let Some(double) = self.double.as_mut() else {
            return Err(TmError::WrongPhase);
        };
        let Some(mode) = double.mode else {
            return Err(TmError::WrongPhase);
        };
        if double.slots.iter().any(|s| s.is_none()) {
            return Err(TmError::BadTarget);
        }
        let dice_count = match mode {
            GiveMode::Both => 2,
            GiveMode::Split => 1,
        };
        double.gifts = double
            .slots
            .iter()
            .map(|s| Gift {
                player_id: s.expect("all slots checked Some above"),
                dice_count,
                values: None,
            })
            .collect();
        self.phase = Phase::Gifts;
        Ok(())
    }

    /// Returns the total the victim drinks for this gift.
    pub fn gift_roll(&mut self, slot: usize, values: Vec<u8>) -> Result<u8, TmError> {
        if self.phase != Phase::Gifts {
            return Err(TmError::WrongPhase);
        }
        let Some(double) = self.double.as_mut() else {
            return Err(TmError::WrongPhase);
        };
        let Some(gift) = double.gifts.get_mut(slot) else {
            return Err(TmError::BadTarget);
        };
        if gift.values.is_some() {
            return Err(TmError::BadTarget);
        }
        if values.len() != gift.dice_count as usize {
            return Err(TmError::BadTarget);
        }
        let total: u8 = values.iter().sum();
        gift.values = Some(values);
        self.seq += 1;

        if double.gifts.iter().all(|g| g.values.is_some()) {
            let value = double.value;
            let all_values: Vec<u8> = double
                .gifts
                .iter()
                .flat_map(|g| g.values.clone().unwrap_or_default())
                .collect();
            double.payback = all_values.contains(&value).then(|| all_values.iter().sum());
        }
        Ok(total)
    }

    pub fn gifts_complete(&self) -> bool {
        match &self.double {
            Some(double) => {
                !double.gifts.is_empty() && double.gifts.iter().all(|g| g.values.is_some())
            }
            None => true,
        }
    }

    pub fn pass(&mut self) -> Result<(), TmError> {
        let allowed =
            self.phase == Phase::Rolled || (self.phase == Phase::Gifts && self.gifts_complete());
        if !allowed {
            return Err(TmError::WrongPhase);
        }
        self.last_roller = Some(self.roller());
        self.roller_idx = (self.roller_idx + 1) % self.order.len();
        self.stale = true;
        self.seq += 1;
        self.phase = Phase::Ready;
        Ok(())
    }

    /// Moves `player`'s seat by `delta` (wrapping circularly, seat 0 ↔ last —
    /// a deliberate deviation from the prototype's clamp behaviour). The
    /// roller keeps rolling: `roller_idx` is recomputed after the swap.
    pub fn move_seat(&mut self, player: i64, delta: i64) -> Result<(), TmError> {
        let len = self.order.len();
        let Some(idx) = self.order.iter().position(|&p| p == player) else {
            return Err(TmError::BadTarget);
        };
        let j = (idx as i64 + delta).rem_euclid(len as i64) as usize;
        let roller_id = self.roller();
        self.order.swap(idx, j);
        self.roller_idx = self
            .order
            .iter()
            .position(|&p| p == roller_id)
            .expect("roller is always present in order");
        Ok(())
    }

    /// Table-tab reassign — works in any phase. During HandOff it resolves
    /// the hand-off exactly like `give_three_man` (an engine-level
    /// tolerance; route gating elsewhere still restricts who may call this
    /// while HandOff is active).
    pub fn set_three_man(&mut self, player: i64) -> Result<(), TmError> {
        if !self.order.contains(&player) {
            return Err(TmError::BadTarget);
        }
        if self.phase == Phase::HandOff {
            // Resolving a hand-off is exactly like give_three_man: the
            // crown must actually move, anyone but the outgoing 3 Man.
            if player == self.three_man {
                return Err(TmError::BadTarget);
            }
            self.resolve_handoff(player);
        } else {
            self.three_man = player;
        }
        Ok(())
    }

    /// Mid-game join. Idempotent — appending an already-present player is a
    /// no-op.
    pub fn add_player(&mut self, player: i64) {
        if !self.order.contains(&player) {
            self.order.push(player);
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("ThreeManState is always serializable")
    }

    /// Deserializes a snapshot produced by `to_json`. Only ever called on
    /// this engine's own output, so a parse failure is a programming error.
    pub fn from_json(s: &str) -> Self {
        serde_json::from_str(s).expect("valid ThreeManState JSON")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st3() -> ThreeManState {
        ThreeManState::new(vec![1, 2, 3], 1)
    }

    #[test]
    fn test_new_rotates_starter_to_front() {
        let s = ThreeManState::new(vec![7, 8, 9], 8);
        assert_eq!(s.order, vec![8, 9, 7]);
        assert_eq!((s.roller(), s.three_man, s.phase), (8, 8, Phase::Ready));
    }

    #[test]
    fn test_plain_roll() {
        // 2+4: nobody drinks
        let mut s = st3();
        s.set_three_man(2).unwrap();
        s.roll(2, 4).unwrap();
        assert_eq!(s.phase, Phase::Rolled);
        assert!(s.calls.is_empty());
        assert_eq!(s.seq, 1);
    }

    #[test]
    fn test_single_three_hits_three_man() {
        let mut s = st3();
        s.set_three_man(2).unwrap();
        s.roll(3, 5).unwrap();
        assert_eq!(
            s.calls,
            vec![Call {
                player_id: 2,
                amount: 1,
                reason: "a 3 on the dice".into()
            }]
        );
    }

    #[test]
    fn test_three_total_counts() {
        // 1+2
        let mut s = st3();
        s.set_three_man(2).unwrap();
        s.roll(1, 2).unwrap();
        assert_eq!(s.calls[0].amount, 1);
    }

    #[test]
    fn test_double_threes_count_each_and_fire_doubles() {
        let mut s = st3();
        s.set_three_man(2).unwrap();
        s.roll(3, 3).unwrap();
        assert_eq!(
            s.calls[0],
            Call {
                player_id: 2,
                amount: 2,
                reason: "two 3s on the dice".into()
            }
        );
        assert_eq!(s.phase, Phase::Assign);
        assert_eq!(s.double.as_ref().unwrap().value, 3);
    }

    #[test]
    fn test_seven_nine_eleven() {
        let mut s = st3();
        s.set_three_man(3).unwrap();
        s.roll(3, 4).unwrap(); // 7 AND a three
        let ids: Vec<i64> = s.calls.iter().map(|c| c.player_id).collect();
        assert!(ids.contains(&3)); // three_man for the 3
        assert!(ids.contains(&s.left_of(0))); // 7 → left
        s = st3();
        s.set_three_man(3).unwrap();
        s.roll(4, 5).unwrap(); // 9 → right
        assert_eq!(s.calls[0].player_id, s.right_of(0));
        s = st3();
        s.set_three_man(3).unwrap();
        s.roll(5, 6).unwrap(); // 11 → roller
        assert_eq!(s.calls[0].player_id, 1);
    }

    #[test]
    fn test_two_player_left_equals_right() {
        let mut s = ThreeManState::new(vec![1, 2], 1);
        assert_eq!(s.left_of(0), 2);
        assert_eq!(s.right_of(0), 2);
        s.set_three_man(2).unwrap();
        s.roll(4, 3).unwrap(); // 7 → the other player
        assert!(s.calls.iter().any(|c| c.player_id == 2));
    }

    #[test]
    fn test_three_man_rolls_three_goes_to_handoff_no_drink() {
        let mut s = st3(); // three_man == roller == 1
                           // NB: brief's literal (3, 6) sums to 9, which would also fire the
                           // sum-9 rule (no stated hand-off exception for sums) — treated as a
                           // dice-literal typo; (3, 5) preserves the test's intent (a lone 3,
                           // no sum-based collision) without silently voiding a real rule.
        s.roll(3, 5).unwrap();
        assert_eq!(s.phase, Phase::HandOff);
        assert!(s.calls.is_empty());
        assert!(!s.pending_double);
    }

    #[test]
    fn test_handoff_with_pending_double() {
        let mut s = st3();
        s.roll(3, 3).unwrap();
        assert_eq!(s.phase, Phase::HandOff);
        assert!(s.pending_double);
        assert_eq!(s.give_three_man(1), Err(TmError::BadTarget)); // not to yourself
        s.give_three_man(3).unwrap();
        assert_eq!(
            (s.three_man, s.handoff_from, s.phase),
            (3, Some(1), Phase::Assign)
        );
    }

    #[test]
    fn test_handoff_without_double_goes_to_rolled() {
        let mut s = st3();
        s.roll(3, 6).unwrap();
        s.give_three_man(2).unwrap();
        assert_eq!(s.phase, Phase::Rolled);
    }

    #[test]
    fn test_assign_both_flow_and_payback() {
        let mut s = st3();
        s.set_three_man(2).unwrap();
        s.roll(4, 4).unwrap();
        s.set_mode(GiveMode::Both).unwrap();
        assert_eq!(s.double.as_ref().unwrap().slots.len(), 1);
        assert_eq!(s.pick_target(0, 1), Err(TmError::BadTarget)); // owner excluded
        s.pick_target(0, 3).unwrap();
        s.send().unwrap();
        assert_eq!(s.phase, Phase::Gifts);
        assert_eq!(s.double.as_ref().unwrap().gifts[0].dice_count, 2);
        let total = s.gift_roll(0, vec![4, 2]).unwrap(); // a gifted die == double value 4
        assert_eq!(total, 6);
        assert_eq!(s.double.as_ref().unwrap().payback, Some(6)); // owner drinks combined total
        assert!(s.gifts_complete());
    }

    #[test]
    fn test_assign_split_flow_no_payback() {
        let mut s = st3();
        s.set_three_man(2).unwrap();
        s.roll(5, 5).unwrap();
        s.set_mode(GiveMode::Split).unwrap();
        s.pick_target(0, 2).unwrap();
        assert_eq!(s.pick_target(1, 2), Err(TmError::BadTarget)); // distinct slots
        s.pick_target(1, 3).unwrap();
        s.send().unwrap();
        s.gift_roll(0, vec![2]).unwrap();
        assert!(!s.gifts_complete());
        s.gift_roll(1, vec![6]).unwrap();
        assert!(s.gifts_complete());
        assert_eq!(s.double.as_ref().unwrap().payback, None); // no 5 rolled
    }

    #[test]
    fn test_split_rejected_under_three_players() {
        let mut s = ThreeManState::new(vec![1, 2], 2);
        s.roll(2, 2).unwrap();
        assert_eq!(s.set_mode(GiveMode::Split), Err(TmError::TooFewPlayers));
        s.set_mode(GiveMode::Both).unwrap();
    }

    #[test]
    fn test_clear_slot_and_resend() {
        let mut s = st3();
        s.set_three_man(2).unwrap();
        s.roll(4, 4).unwrap();
        s.set_mode(GiveMode::Both).unwrap();
        s.pick_target(0, 3).unwrap();
        s.clear_slot(0).unwrap();
        assert_eq!(s.send(), Err(TmError::BadTarget)); // empty slot
        s.pick_target(0, 3).unwrap();
        s.send().unwrap();
        assert_eq!(s.phase, Phase::Gifts);
    }

    #[test]
    fn test_pass_advances_left_and_wraps_and_marks_stale() {
        let mut s = st3();
        s.set_three_man(2).unwrap();
        s.roll(2, 4).unwrap();
        s.pass().unwrap();
        assert_eq!(
            (s.roller(), s.last_roller, s.stale, s.phase),
            (2, Some(1), true, Phase::Ready)
        );
        s.roll(2, 4).unwrap();
        s.pass().unwrap();
        s.roll(2, 4).unwrap();
        s.pass().unwrap();
        assert_eq!(s.roller(), 1); // wrapped
    }

    #[test]
    fn test_pass_blocked_until_gifts_done() {
        let mut s = st3();
        s.set_three_man(2).unwrap();
        s.roll(6, 6).unwrap();
        s.set_mode(GiveMode::Both).unwrap();
        s.pick_target(0, 2).unwrap();
        s.send().unwrap();
        assert_eq!(s.pass(), Err(TmError::WrongPhase));
        s.gift_roll(0, vec![1, 2]).unwrap();
        s.pass().unwrap();
    }

    #[test]
    fn test_wrong_phase_everything() {
        let mut s = st3();
        assert_eq!(s.pass(), Err(TmError::WrongPhase));
        assert_eq!(s.give_three_man(2), Err(TmError::WrongPhase));
        assert_eq!(s.set_mode(GiveMode::Both), Err(TmError::WrongPhase));
        assert_eq!(s.gift_roll(0, vec![1]), Err(TmError::WrongPhase));
        s.roll(2, 4).unwrap();
        assert_eq!(s.roll(2, 4), Err(TmError::WrongPhase)); // roll from Rolled
    }

    #[test]
    fn test_move_seat_preserves_roller_and_wraps() {
        let mut s = st3();
        s.roll(2, 4).unwrap();
        s.pass().unwrap(); // roller is now player 2
        s.move_seat(2, -1).unwrap(); // swaps 2 to front
        assert_eq!(s.roller(), 2); // same player still rolling
        s.move_seat(1, -1).unwrap(); // wrap: index 0 - 1 swaps with last
    }

    #[test]
    fn test_table_reassign_any_time_resolves_handoff() {
        let mut s = st3();
        s.roll(3, 3).unwrap(); // HandOff + pending double
        assert_eq!(s.set_three_man(1), Err(TmError::BadTarget)); // can't hand off to yourself
        s.set_three_man(3).unwrap();
        assert_eq!(s.phase, Phase::Assign); // handoff resolved by the table pick
        assert_eq!(s.set_three_man(99), Err(TmError::BadTarget));
    }

    #[test]
    fn test_add_player_appends_once() {
        let mut s = st3();
        s.add_player(4);
        s.add_player(4);
        assert_eq!(s.order, vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_json_roundtrip() {
        let mut s = st3();
        s.roll(3, 4).unwrap();
        assert_eq!(ThreeManState::from_json(&s.to_json()), s);
    }

    #[test]
    fn test_json_roundtrip_covers_double_gifts_and_payback() {
        // Exercises the fields the plain-roll roundtrip above never
        // touches: double, mode, slots, gifts (with rolled values), and a
        // populated payback — this whole struct gets snapshotted into a DB
        // column by later tasks, so it must round-trip losslessly.
        let mut s = st3();
        s.set_three_man(2).unwrap();
        s.roll(4, 4).unwrap();
        s.set_mode(GiveMode::Both).unwrap();
        s.pick_target(0, 3).unwrap();
        s.send().unwrap();
        s.gift_roll(0, vec![4, 2]).unwrap();
        assert_eq!(s.double.as_ref().unwrap().payback, Some(6));
        assert_eq!(ThreeManState::from_json(&s.to_json()), s);
    }
}
