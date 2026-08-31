//! Sorting & Loading Assistant — the crate sort, on a tablet.
//!
//! A route's crates arrive stacked on pallets in whatever order they were
//! picked. They have to leave stacked in the van in *reverse delivery* order,
//! which means a physical sort: pull from the pallet tops, park what is not
//! needed yet in a standby slot, load the rest into the van in a sequence that
//! keeps adjacent rows within the stability limit.
//!
//! The plan for that sort is generated elsewhere and arrives here as one JSON
//! document per session. This module stores it, checks it, and serves the
//! board that is worked from while the sorting actually happens.
//!
//! **Where the work is split.** Rust owns the document: parsing it, checking
//! it, and rendering the static panels (the manifest, the stops, the pallet
//! reference, the sanity checks). The board's *live* half — the pick
//! checklist, the van diagram, the standby slots, the progress counters — is
//! rendered by `static/sorting.js` from the payload embedded in the page. That
//! line is drawn where it is for one reason: a tick has to feel instant with a
//! glove on and one bar of signal, which rules out a round trip to re-render.
//! The server is told about the tick afterwards, and the page keeps working if
//! it is not listening.
//!
//! **Why the checks are here and not in the generator.** The generator is
//! trusted to be right and mostly is; the checks exist for the times it is
//! not, and a check that runs in the same process that produced the answer is
//! not an independent check. These re-derive the arithmetic from the stored
//! document — crate counts across manifest, plan and pick sequence; the ±N
//! stability rule; whether every standby crate is picked back up — and say so
//! plainly on the board before anyone lifts anything.

use crate::{
    middleware::{AuthSession, OptionalAdmin},
    AppState,
};
use askama::Template;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
    routing::{delete, get, post},
    Form, Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// A pasted document larger than this is not a route plan. The global
/// `DefaultBodyLimit` is 35 MB because image uploads need it; a sorting plan
/// for a full day is tens of kilobytes, so this is the limit that actually
/// applies here.
const MAX_PAYLOAD_BYTES: usize = 2 * 1024 * 1024;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/sorting", get(index_page).post(create_session))
        .route("/sorting/{id}", get(board_page))
        .route("/api/sorting/sessions/{id}", delete(delete_session))
        .route("/api/sorting/sessions/{id}/steps/{step}", post(set_step))
        .route("/api/sorting/sessions/{id}/reset", post(reset_steps))
}

// ── The document ──────────────────────────────────────────────────────────
//
// Every field is `#[serde(default)]`. A plan that is missing its loading
// diagram should still give you a working checklist, and one missing its pick
// sequence should still show you the manifest — the alternative is a 400 that
// tells someone standing at a pallet to go and fix their JSON. What a document
// may *not* do is be empty in all three of manifest, plan and pick sequence;
// `SortingPlan::looks_like_a_plan` is where that line sits.

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SortingPlan {
    #[serde(default)]
    pub session: SessionMeta,
    #[serde(default)]
    pub stops: Vec<Stop>,
    #[serde(default)]
    pub manifest: Vec<ManifestEntry>,
    #[serde(default, rename = "palletStacks")]
    pub pallet_stacks: Vec<PalletStack>,
    #[serde(default, rename = "vanConfig")]
    pub van_config: VanConfig,
    #[serde(default, rename = "loadingPlan")]
    pub loading_plan: LoadingPlan,
    #[serde(default, rename = "pickSequence")]
    pub pick_sequence: Vec<PickStep>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SessionMeta {
    #[serde(default, rename = "routeName")]
    pub route_name: String,
    #[serde(default)]
    pub date: String,
    #[serde(default, rename = "startTime")]
    pub start_time: String,
    #[serde(default, rename = "endTime")]
    pub end_time: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Stop {
    #[serde(default, rename = "stopNumber")]
    pub stop_number: i64,
    #[serde(default, rename = "deliveryIndex")]
    pub delivery_index: i64,
    #[serde(default)]
    pub customer: String,
    #[serde(default)]
    pub address: String,
    #[serde(default, rename = "orderNumber", deserialize_with = "loose_string")]
    pub order_number: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ManifestEntry {
    #[serde(default)]
    pub customer: String,
    #[serde(default, rename = "orderNumber", deserialize_with = "loose_string")]
    pub order_number: String,
    #[serde(default)]
    pub count: i64,
    /// `"confirmed"` or `"minimum"`. Anything else is treated as confirmed —
    /// this is a flag for a human's attention, not a state machine.
    #[serde(default)]
    pub confidence: String,
    /// Absent means "yes". A crate is only interesting here when it is
    /// explicitly marked as matching no stop on the route.
    #[serde(default = "yes", rename = "onRoute")]
    pub on_route: bool,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct PalletStack {
    #[serde(default, rename = "stackId")]
    pub stack_id: String,
    #[serde(default, rename = "sourceImage")]
    pub source_image: String,
    #[serde(default, rename = "topToBottom")]
    pub top_to_bottom: Vec<StackItem>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct StackItem {
    #[serde(default)]
    pub customer: String,
    #[serde(default, rename = "orderNumber", deserialize_with = "loose_string")]
    pub order_number: String,
    #[serde(default)]
    pub count: i64,
}

/// The van's geometry. Its `Default` is the real van's, not zeroes — a
/// document that omits `vanConfig` entirely should draw the van everyone
/// actually loads, and a diagram with zero rows would just be a blank panel
/// with no explanation.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VanConfig {
    #[serde(default = "default_rows", rename = "totalRows")]
    pub total_rows: i64,
    #[serde(default = "default_height", rename = "maxHeight")]
    pub max_height: i64,
    #[serde(default = "default_stability", rename = "stabilityLimit")]
    pub stability_limit: i64,
    #[serde(default, rename = "sideDoor")]
    pub side_door: SideDoor,
    #[serde(default)]
    pub standby: Standby,
}

impl Default for VanConfig {
    fn default() -> Self {
        Self {
            total_rows: default_rows(),
            max_height: default_height(),
            stability_limit: default_stability(),
            side_door: SideDoor::default(),
            standby: Standby::default(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SideDoor {
    #[serde(default, rename = "columnAccess")]
    pub column_access: Vec<String>,
    #[serde(default)]
    pub rows: Vec<i64>,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Standby {
    #[serde(default = "default_side_slots", rename = "sideSlots")]
    pub side_slots: i64,
    #[serde(default = "default_back_slots", rename = "backSlots")]
    pub back_slots: i64,
    #[serde(default = "yes", rename = "slotsCanStack")]
    pub slots_can_stack: bool,
}

impl Default for Standby {
    fn default() -> Self {
        Self {
            side_slots: default_side_slots(),
            back_slots: default_back_slots(),
            slots_can_stack: true,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct LoadingPlan {
    #[serde(default)]
    pub rows: Vec<PlanRow>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct PlanRow {
    #[serde(default)]
    pub row: i64,
    #[serde(default)]
    pub entries: Vec<PlanEntry>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct PlanEntry {
    #[serde(default)]
    pub customer: String,
    #[serde(default)]
    pub color: String,
    #[serde(default)]
    pub left: i64,
    #[serde(default)]
    pub right: i64,
    #[serde(default)]
    pub uncertain: bool,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct PickStep {
    #[serde(default)]
    pub step: i64,
    #[serde(default)]
    pub customer: String,
    #[serde(default)]
    pub quantity: i64,
    #[serde(default)]
    pub from: Endpoint,
    #[serde(default)]
    pub to: Endpoint,
    /// Whether the generator considered this already done. Seeds the ticks on
    /// first upload and is never read again — progress lives in
    /// `sorting_step_state` from that point on.
    #[serde(default)]
    pub completed: bool,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Endpoint {
    #[serde(default, rename = "type")]
    pub kind: String,
    #[serde(default, rename = "stackId")]
    pub stack_id: String,
    #[serde(default)]
    pub row: Option<i64>,
    #[serde(default)]
    pub column: String,
    #[serde(default)]
    pub slot: String,
}

fn yes() -> bool {
    true
}
fn default_rows() -> i64 {
    7
}
fn default_height() -> i64 {
    8
}
fn default_stability() -> i64 {
    3
}
fn default_side_slots() -> i64 {
    3
}
fn default_back_slots() -> i64 {
    2
}

/// Accepts an order number written either as a JSON string or as a JSON
/// number, and yields a string either way.
///
/// Order numbers are ten digits. Written unquoted they parse as `f64` and come
/// back as `1000703538.0`, or lose their last digits entirely past 2^53 — and
/// the failure is silent, producing a number that no longer matches the label
/// on the crate. Taking either form and normalising here is cheaper than
/// finding that out on a pallet.
fn loose_string<'de, D>(d: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize as _;
    Ok(match serde_json::Value::deserialize(d)? {
        serde_json::Value::String(s) => s,
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    })
}

impl SortingPlan {
    /// Whether this document carries any of the three things a plan is for.
    /// All-empty means something else was pasted — a route list, a chat reply,
    /// last week's notes — and saying so beats storing it and rendering an
    /// empty board that looks like a bug.
    fn looks_like_a_plan(&self) -> bool {
        !self.manifest.is_empty()
            || !self.pick_sequence.is_empty()
            || !self.loading_plan.rows.is_empty()
    }

    /// Crates that have to reach the van: the manifest, minus anything flagged
    /// as matching no stop on the route.
    fn crates_on_route(&self) -> i64 {
        self.manifest
            .iter()
            .filter(|m| m.on_route)
            .map(|m| m.count.max(0))
            .sum()
    }
}

// ── Plan checks ───────────────────────────────────────────────────────────

/// How much a finding should interrupt someone who is already holding a crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckLevel {
    /// The plan cannot be worked as written — following it loses crates or
    /// runs off the end of the van.
    Critical,
    /// The plan is workable but disagrees with itself, or with the van's
    /// stated limits.
    Warning,
    /// Something to look at once, which the generator already knew about.
    Note,
}

impl CheckLevel {
    fn class(self) -> &'static str {
        match self {
            CheckLevel::Critical => "is-critical",
            CheckLevel::Warning => "is-warning",
            CheckLevel::Note => "is-note",
        }
    }
    fn label(self) -> &'static str {
        match self {
            CheckLevel::Critical => "STOP",
            CheckLevel::Warning => "CHECK",
            CheckLevel::Note => "NOTE",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Check {
    pub level: CheckLevel,
    pub title: String,
    pub detail: String,
}

/// Per-row column heights, `row -> (left, right)`, summed over the row's
/// entries. Rows absent from the plan are absent here too; callers decide what
/// an absent row means, because "empty" and "not mentioned" differ depending
/// on the question.
fn column_heights(plan: &LoadingPlan) -> BTreeMap<i64, (i64, i64)> {
    let mut out: BTreeMap<i64, (i64, i64)> = BTreeMap::new();
    for row in &plan.rows {
        let cell = out.entry(row.row).or_insert((0, 0));
        for e in &row.entries {
            cell.0 += e.left.max(0);
            cell.1 += e.right.max(0);
        }
    }
    out
}

/// The ±N rule, exactly as the methodology states it: within one column, a
/// stack may be at most `stability_limit` taller or shorter than the stack
/// **immediately in front of or behind it in that same column**.
///
/// Two things this deliberately does not do, both of them ways an earlier
/// version of this function cried wolf:
///
/// * **Left is never compared to right.** They are separate columns and may
///   differ by any amount. Averaging the two into one "row height", or
///   comparing across the aisle, invents violations that are not there.
/// * **An empty position is not a stack.** A position with nothing in it has
///   nothing to be unstable, so it is neither compared to its neighbours nor
///   bridged across — two stacks with a gap between them are not touching, so
///   neither supports the other. Only a pair where *both* positions hold
///   crates is checked. That covers the empty tail of a light route and a
///   genuinely empty row mid-van alike, without needing to know which is which.
pub fn stability_violations(plan: &LoadingPlan, config: &VanConfig) -> Vec<String> {
    let heights = column_heights(plan);
    let limit = config.stability_limit.max(0);
    let mut out = Vec::new();
    for row in 1..config.total_rows.max(1) {
        let (al, ar) = heights.get(&row).copied().unwrap_or((0, 0));
        let (bl, br) = heights.get(&(row + 1)).copied().unwrap_or((0, 0));
        for (name, a, b) in [("left", al, bl), ("right", ar, br)] {
            if a == 0 || b == 0 {
                continue;
            }
            if (a - b).abs() > limit {
                out.push(format!(
                    "{name} column: row {row} is {a} high, row {} is {b} — a gap of {}, over the limit of {limit}",
                    row + 1,
                    (a - b).abs()
                ));
            }
        }
    }
    out
}

/// Reconciles the pick sequence against the loading plan **cell by cell**:
/// for each (row, column, customer), what the sequence delivers there against
/// what the plan says goes there.
///
/// The totals check above is not enough on its own, and this is the case that
/// proves it. A sequence can move exactly the right number of crates into the
/// van, for exactly the right customers, and still put them in the wrong
/// columns — the arithmetic balances and the plan is still unworkable. The
/// board would show it as a diagram that refuses to fill in: crates ticked off
/// the list that never go solid, because there is no room for them where the
/// step says to put them.
///
/// One caveat worth stating: a row's crates are usually split across several
/// steps, which is expected and fine. This only reports a cell where the two
/// documents disagree once *every* step for it has been counted.
pub fn cell_delivery_mismatches(plan: &SortingPlan) -> Vec<String> {
    let mut planned: BTreeMap<(i64, String, String), i64> = BTreeMap::new();
    for row in &plan.loading_plan.rows {
        for e in &row.entries {
            if e.left > 0 {
                *planned
                    .entry((row.row, "left".to_string(), e.customer.clone()))
                    .or_insert(0) += e.left;
            }
            if e.right > 0 {
                *planned
                    .entry((row.row, "right".to_string(), e.customer.clone()))
                    .or_insert(0) += e.right;
            }
        }
    }

    let mut delivered: BTreeMap<(i64, String, String), i64> = BTreeMap::new();
    for s in &plan.pick_sequence {
        if s.to.kind != "van" {
            continue;
        }
        let Some(row) = s.to.row else { continue };
        *delivered
            .entry((row, s.to.column.clone(), s.customer.clone()))
            .or_insert(0) += s.quantity.max(0);
    }

    let mut keys: BTreeSet<&(i64, String, String)> = BTreeSet::new();
    keys.extend(planned.keys());
    keys.extend(delivered.keys());

    let mut out = Vec::new();
    for key in keys {
        let want = planned.get(key).copied().unwrap_or(0);
        let got = delivered.get(key).copied().unwrap_or(0);
        if want == got {
            continue;
        }
        let (row, col, customer) = key;
        let where_ = format!("row {row} {col}");
        out.push(if want == 0 {
            format!(
                "{where_}: the sequence brings {got} for {customer}, but the plan puts none of theirs there"
            )
        } else if got == 0 {
            format!("{where_}: the plan holds {want} for {customer}, and no step delivers them")
        } else {
            format!(
                "{where_}: the sequence brings {got} for {customer}, the plan has room for {want}"
            )
        });
    }
    out
}

/// Columns stacked past `maxHeight`, and rows numbered past `totalRows`.
pub fn geometry_violations(plan: &LoadingPlan, config: &VanConfig) -> Vec<String> {
    let heights = column_heights(plan);
    let mut out = Vec::new();
    for (row, (l, r)) in &heights {
        if *row < 1 || *row > config.total_rows {
            out.push(format!(
                "row {row} is outside the van — it has {} rows",
                config.total_rows
            ));
            continue;
        }
        if *l > config.max_height {
            out.push(format!(
                "row {row} left is {l} crates high, over the maximum of {}",
                config.max_height
            ));
        }
        if *r > config.max_height {
            out.push(format!(
                "row {row} right is {r} crates high, over the maximum of {}",
                config.max_height
            ));
        }
    }
    out
}

/// What the standby slots do over the course of the sort.
#[derive(Debug, Default)]
pub struct StandbyTrace {
    /// Things that cannot happen as written.
    pub problems: Vec<String>,
    /// Slots still holding crates when the sequence runs out.
    pub left_behind: BTreeMap<String, i64>,
    /// The most slots occupied at any one moment.
    pub peak_slots: usize,
    /// The step number at which that peak was reached.
    pub peak_at_step: i64,
}

/// Walks the pick sequence and reports what the standby slots do over it.
///
/// Three things go wrong here and all three are silent in the document itself:
///
/// * a crate taken out of a slot nothing put anything into;
/// * a crate parked in a slot nothing ever comes back for — the one that
///   matters most, because it means the plan finishes with crates still
///   standing on the floor of the van and nobody notices until the round is
///   short;
/// * more slots wanted at once than the van has. That last one is why this
///   tracks a running peak rather than just the ending state: a plan can park
///   and retrieve perfectly, ending empty, and still call for six slots at a
///   moment when only five exist. It is a problem to solve on the floor, not
///   something to discover halfway through the pallet.
pub fn standby_trace(plan: &SortingPlan) -> StandbyTrace {
    let mut steps: Vec<&PickStep> = plan.pick_sequence.iter().collect();
    steps.sort_by_key(|s| s.step);

    let mut occupancy: BTreeMap<String, i64> = BTreeMap::new();
    let mut out = StandbyTrace::default();
    let known = known_slots(&plan.van_config.standby);

    for s in steps {
        if s.from.kind == "standby" {
            let slot = slot_name(&s.from);
            let held = occupancy.entry(slot.clone()).or_insert(0);
            *held -= s.quantity.max(0);
            if *held < 0 {
                out.problems.push(format!(
                    "step {}: takes {} from {slot}, which is empty at that point",
                    s.step, s.quantity
                ));
                *held = 0;
            }
        }
        if s.to.kind == "standby" {
            let slot = slot_name(&s.to);
            if !known.contains(&slot) {
                out.problems.push(format!(
                    "step {}: parks in {slot}, which is not one of the {} standby slots",
                    s.step,
                    known.len()
                ));
            }
            *occupancy.entry(slot).or_insert(0) += s.quantity.max(0);
        }

        // Measured after every step, not only after a park: a step that empties
        // a slot lowers the count, and the peak is about the worst moment, not
        // the last one.
        let in_use = occupancy.values().filter(|held| **held > 0).count();
        if in_use > out.peak_slots {
            out.peak_slots = in_use;
            out.peak_at_step = s.step;
        }
    }

    occupancy.retain(|_, held| *held > 0);
    out.left_behind = occupancy;
    out
}

fn slot_name(e: &Endpoint) -> String {
    if e.slot.is_empty() {
        "an unnamed slot".to_string()
    } else {
        e.slot.clone()
    }
}

fn known_slots(s: &Standby) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for i in 1..=s.side_slots.max(0) {
        out.insert(format!("side-{i}"));
    }
    for i in 1..=s.back_slots.max(0) {
        out.insert(format!("back-{i}"));
    }
    out
}

/// Everything worth saying about a plan before it is worked, most urgent
/// first.
///
/// Pure, and deliberately so: it is the one part of this module that decides
/// whether someone trusts what is on the screen, and a pure function is one a
/// test can pin down completely.
pub fn run_checks(plan: &SortingPlan) -> Vec<Check> {
    let mut out: Vec<Check> = Vec::new();

    // ── Structural: does the checklist address one move per step? ──
    //
    // Progress is keyed on step number, so two steps sharing one is not a
    // cosmetic duplicate — ticking either ticks both, and one of the two
    // moves silently disappears from the board.
    let mut seen: BTreeSet<i64> = BTreeSet::new();
    let mut dupes: BTreeSet<i64> = BTreeSet::new();
    for s in &plan.pick_sequence {
        if !seen.insert(s.step) {
            dupes.insert(s.step);
        }
    }
    if !dupes.is_empty() {
        out.push(Check {
            level: CheckLevel::Critical,
            title: "Two moves share a step number".into(),
            detail: format!(
                "Step {} appears more than once. Ticking one ticks the other, so a move would drop off the checklist. Regenerate the plan before working it.",
                join_ints(&dupes)
            ),
        });
    }

    // ── Does everyone on the route get loaded? ──
    let planned: BTreeSet<String> = plan
        .loading_plan
        .rows
        .iter()
        .flat_map(|r| r.entries.iter())
        .filter(|e| e.left > 0 || e.right > 0)
        .map(|e| e.customer.clone())
        .collect();
    if !plan.loading_plan.rows.is_empty() {
        let missing: Vec<String> = plan
            .manifest
            .iter()
            .filter(|m| m.on_route && m.count > 0 && !planned.contains(&m.customer))
            .map(|m| m.customer.clone())
            .collect();
        if !missing.is_empty() {
            out.push(Check {
                level: CheckLevel::Critical,
                title: "Counted, but nowhere in the van".into(),
                detail: format!(
                    "{} has crates on the manifest and no place in the loading plan. As written, they get left at the depot.",
                    missing.join(", ")
                ),
            });
        }
    }

    // ── Standby: nothing parked and forgotten, and never more than fits ──
    let standby = standby_trace(plan);
    for p in standby.problems {
        out.push(Check {
            level: CheckLevel::Critical,
            title: "The standby slots do not add up".into(),
            detail: p,
        });
    }
    if !standby.left_behind.is_empty() {
        let list: Vec<String> = standby
            .left_behind
            .iter()
            .map(|(slot, n)| format!("{n} in {slot}"))
            .collect();
        out.push(Check {
            level: CheckLevel::Critical,
            title: "Crates end the sort still in standby".into(),
            detail: format!(
                "The sequence finishes with {}. Nothing picks them back up, so they never reach the van.",
                list.join(", ")
            ),
        });
    }
    let slots_available = known_slots(&plan.van_config.standby).len();
    if standby.peak_slots > slots_available {
        out.push(Check {
            level: CheckLevel::Critical,
            title: "The sort needs more standby room than the van has".into(),
            detail: format!(
                "At step {} the plan has {} slots in use at once, and there are {}. This is a problem to solve on the floor before starting, not halfway through the pallet.",
                standby.peak_at_step, standby.peak_slots, slots_available
            ),
        });
    }

    // ── Arithmetic: manifest vs plan vs sequence ──
    let manifest_total = plan.crates_on_route();
    let plan_total: i64 = plan
        .loading_plan
        .rows
        .iter()
        .flat_map(|r| r.entries.iter())
        .map(|e| e.left.max(0) + e.right.max(0))
        .sum();
    let loaded_total: i64 = plan
        .pick_sequence
        .iter()
        .filter(|s| s.to.kind == "van")
        .map(|s| s.quantity.max(0))
        .sum();

    let has_minimums = plan.manifest.iter().any(|m| m.confidence == "minimum");
    if !plan.loading_plan.rows.is_empty()
        && !plan.manifest.is_empty()
        && manifest_total != plan_total
    {
        // A "minimum" count is explicitly a floor, so the plan holding *more*
        // than the manifest is the expected shape of that flag, not an error.
        let expected_gap = has_minimums && plan_total > manifest_total;
        out.push(Check {
            level: if expected_gap {
                CheckLevel::Note
            } else {
                CheckLevel::Critical
            },
            title: "Manifest and loading plan disagree".into(),
            detail: format!(
                "The manifest counts {manifest_total} crates for this route; the loading plan places {plan_total}.{}",
                if expected_gap {
                    " Some counts are minimums, so a higher plan figure is expected — worth one look."
                } else {
                    ""
                }
            ),
        });
    }
    if !plan.pick_sequence.is_empty()
        && !plan.loading_plan.rows.is_empty()
        && loaded_total != plan_total
    {
        out.push(Check {
            level: CheckLevel::Critical,
            title: "The sequence does not load the plan".into(),
            detail: format!(
                "The loading plan places {plan_total} crates in the van; the pick sequence moves {loaded_total} into it."
            ),
        });
    }

    // ── Does the sequence put things where the diagram says? ──
    if !plan.pick_sequence.is_empty() && !plan.loading_plan.rows.is_empty() {
        let cells = cell_delivery_mismatches(plan);
        if !cells.is_empty() {
            out.push(Check {
                level: CheckLevel::Critical,
                title: "The sequence and the diagram disagree on where crates go".into(),
                detail: format!(
                    "{}. Crates ticked off the list would have nowhere to land, and the van diagram will not fill in.",
                    cells.join("; ")
                ),
            });
        }
    }

    // ── The van's own limits ──
    for v in geometry_violations(&plan.loading_plan, &plan.van_config) {
        out.push(Check {
            level: CheckLevel::Critical,
            title: "Outside the van's limits".into(),
            detail: v,
        });
    }
    for v in stability_violations(&plan.loading_plan, &plan.van_config) {
        out.push(Check {
            level: CheckLevel::Warning,
            title: "Stability rule broken".into(),
            detail: v,
        });
    }

    // ── Steps that point somewhere that does not exist ──
    let bad_rows: Vec<String> = plan
        .pick_sequence
        .iter()
        .filter(|s| s.to.kind == "van")
        .filter_map(|s| {
            let row = s.to.row?;
            (row < 1 || row > plan.van_config.total_rows)
                .then(|| format!("step {} → row {row}", s.step))
        })
        .collect();
    if !bad_rows.is_empty() {
        out.push(Check {
            level: CheckLevel::Critical,
            title: "A move points past the end of the van".into(),
            detail: format!(
                "{}. The van has {} rows.",
                bad_rows.join(", "),
                plan.van_config.total_rows
            ),
        });
    }

    // ── The two the methodology says to eyeball ──
    let off_route: Vec<String> = plan
        .manifest
        .iter()
        .filter(|m| !m.on_route)
        .map(|m| {
            if m.order_number.is_empty() {
                m.customer.clone()
            } else {
                format!("{} ({})", m.customer, m.order_number)
            }
        })
        .collect();
    if !off_route.is_empty() {
        out.push(Check {
            level: CheckLevel::Warning,
            title: "On a crate, not on the route".into(),
            detail: format!(
                "{} — matched no stop in the route list. Either the crate is not ours today, or the stop is missing from the list.",
                off_route.join(", ")
            ),
        });
    }

    let minimums: Vec<String> = plan
        .manifest
        .iter()
        .filter(|m| m.confidence == "minimum")
        .map(|m| format!("{} (at least {})", m.customer, m.count))
        .collect();
    if !minimums.is_empty() {
        out.push(Check {
            level: CheckLevel::Note,
            title: "Counts that are a floor, not a total".into(),
            detail: format!(
                "The photo did not reach the pallet base for {}. Count these on the pallet before trusting the number.",
                minimums.join(", ")
            ),
        });
    }

    // ── Missing halves of the document ──
    if plan.pick_sequence.is_empty() {
        out.push(Check {
            level: CheckLevel::Warning,
            title: "No pick sequence".into(),
            detail:
                "This plan has no step-by-step sort, so the checklist is empty. The van diagram and manifest still work."
                    .into(),
        });
    }
    if plan.loading_plan.rows.is_empty() {
        out.push(Check {
            level: CheckLevel::Warning,
            title: "No loading plan".into(),
            detail: "This plan has no final placement, so the van diagram is empty.".into(),
        });
    }

    out
}

fn join_ints(v: &BTreeSet<i64>) -> String {
    v.iter()
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

// ── Relaxed JSON ──────────────────────────────────────────────────────────

/// Strips `//` and `/* */` comments and trailing commas, leaving strings
/// alone.
///
/// The plan is written by hand into a chat and copied out of it, and the
/// document it is copied from is specified as JSONC — with comments on the
/// fields, exactly the sort a person keeps when pasting. `serde_json` rejects
/// both, with a byte offset and no hint of which. Accepting them costs this
/// function; refusing them costs someone at a pallet at 07:00 hand-deleting
/// comments on a tablet keyboard.
///
/// It is a lexer, not a parser: it tracks whether it is inside a string and
/// whether the previous character was a backslash, which is all that is needed
/// to know if a `/` opens a comment or is simply part of an address.
pub fn relax_json(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut chars = src.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;

    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match c {
            '"' => {
                in_string = true;
                out.push(c);
            }
            '/' if chars.peek() == Some(&'/') => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut prev = '\0';
                for c in chars.by_ref() {
                    if prev == '*' && c == '/' {
                        break;
                    }
                    // Newlines are kept so byte offsets in a later parse error
                    // still land on roughly the right line.
                    if c == '\n' {
                        out.push('\n');
                    }
                    prev = c;
                }
            }
            _ => out.push(c),
        }
    }

    strip_trailing_commas(&out)
}

/// Removes a comma that is followed only by whitespace and a closing bracket.
fn strip_trailing_commas(src: &str) -> String {
    let bytes: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let mut in_string = false;
    let mut escaped = false;

    for (i, &c) in bytes.iter().enumerate() {
        if in_string {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        if c == '"' {
            in_string = true;
            out.push(c);
            continue;
        }
        if c == ',' {
            let next = bytes[i + 1..].iter().find(|c| !c.is_whitespace());
            if matches!(next, Some(']') | Some('}')) {
                continue;
            }
        }
        out.push(c);
    }
    out
}

// ── Parsing a pasted document ─────────────────────────────────────────────

/// What a paste turned into: the normalised JSON to store, the typed view of
/// it, and its two summary numbers.
#[derive(Debug)]
struct ParsedPlan {
    normalised: String,
    plan: SortingPlan,
    total_steps: i64,
    total_crates: i64,
}

/// Parses a pasted document, or explains in one sentence why it is not one.
///
/// The stored text is the *re-serialised* document, not the paste. Three
/// reasons: it is known-valid, because it came back out of a parser; it has no
/// comments or trailing commas left to confuse the browser's `JSON.parse`; and
/// every `<` in it can be escaped on the way into the page knowing it can only
/// be inside a string, which is what makes embedding it in a `<script>` safe.
fn parse_plan(raw: &str) -> Result<ParsedPlan, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("Nothing pasted — the box was empty.".into());
    }
    if trimmed.len() > MAX_PAYLOAD_BYTES {
        return Err(format!(
            "That is {} KB. A route plan is a few dozen; this looks like the wrong thing.",
            trimmed.len() / 1024
        ));
    }

    let relaxed = relax_json(trimmed);
    let value: serde_json::Value = serde_json::from_str(&relaxed).map_err(|e| {
        format!("That is not valid JSON — {e}. Paste the whole object, braces included.")
    })?;

    if !value.is_object() {
        return Err("The plan has to be a JSON object — the one starting with \"session\".".into());
    }

    let plan: SortingPlan = serde_json::from_value(value.clone())
        .map_err(|e| format!("The JSON parsed, but not as a sorting plan — {e}."))?;

    if !plan.looks_like_a_plan() {
        return Err(
            "No manifest, no loading plan and no pick sequence — this parsed, but it is not a sorting plan."
                .into(),
        );
    }

    let total_steps = plan.pick_sequence.len() as i64;
    let total_crates = plan.crates_on_route();
    let normalised = serde_json::to_string(&value).map_err(|e| e.to_string())?;

    Ok(ParsedPlan {
        normalised,
        plan,
        total_steps,
        total_crates,
    })
}

// ── HTML helpers ──────────────────────────────────────────────────────────

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Escapes a JSON document for embedding in `<script type="application/json">`.
///
/// The element's content is raw text — the browser scans it for `</script`
/// and nothing else — so the whole job is making sure no `<` survives. In
/// valid JSON every `<` is inside a string literal, where `<` means
/// exactly the same thing, so this changes what the parser sees not at all.
/// `>` and `&` go too, which costs nothing and takes the page out of reach of
/// the older `<!--` comment-swallowing behaviour as well.
fn json_for_script(s: &str) -> String {
    s.replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026")
}

fn checks_html(checks: &[Check]) -> String {
    if checks.is_empty() {
        return r#"<div class="sort-check sort-check--clear">
  <span class="sort-check__tag">CLEAR</span>
  <div class="sort-check__body">
    <strong>Nothing to flag.</strong>
    <p>Counts reconcile, the van's limits hold, and every standby crate is picked back up.</p>
  </div>
</div>"#
            .to_string();
    }
    let mut order = checks.to_vec();
    order.sort_by_key(|c| match c.level {
        CheckLevel::Critical => 0,
        CheckLevel::Warning => 1,
        CheckLevel::Note => 2,
    });
    let mut out = String::new();
    for c in order {
        out.push_str(&format!(
            r#"<div class="sort-check {cls}">
  <span class="sort-check__tag">{tag}</span>
  <div class="sort-check__body">
    <strong>{title}</strong>
    <p>{detail}</p>
  </div>
</div>"#,
            cls = c.level.class(),
            tag = c.level.label(),
            title = html_escape(&c.title),
            detail = html_escape(&c.detail),
        ));
    }
    out
}

fn manifest_html(plan: &SortingPlan) -> String {
    if plan.manifest.is_empty() {
        return r#"<p class="sort-empty">No crate manifest in this plan.</p>"#.to_string();
    }
    let mut rows = String::new();
    for m in &plan.manifest {
        let mut flags = String::new();
        if m.confidence == "minimum" {
            flags.push_str(r#"<span class="sort-flag sort-flag--min">at least</span>"#);
        }
        if !m.on_route {
            flags.push_str(r#"<span class="sort-flag sort-flag--off">not on route</span>"#);
        }
        rows.push_str(&format!(
            r#"<tr{cls}>
  <td class="sort-t__name">{customer}</td>
  <td class="sort-t__num">{order}</td>
  <td class="sort-t__count">{count}</td>
  <td class="sort-t__flags">{flags}</td>
</tr>"#,
            cls = if !m.on_route || m.confidence == "minimum" {
                r#" class="is-flagged""#
            } else {
                ""
            },
            customer = html_escape(&m.customer),
            order = html_escape(&m.order_number),
            count = m.count,
            flags = flags,
        ));
    }
    format!(
        r#"<table class="sort-table">
  <thead><tr><th>Customer</th><th>Order</th><th>Crates</th><th></th></tr></thead>
  <tbody>{rows}</tbody>
</table>"#
    )
}

fn stops_html(plan: &SortingPlan) -> String {
    if plan.stops.is_empty() {
        return r#"<p class="sort-empty">No route list in this plan.</p>"#.to_string();
    }
    let mut stops: Vec<&Stop> = plan.stops.iter().collect();
    stops.sort_by_key(|s| s.delivery_index);
    let mut rows = String::new();
    for s in stops {
        rows.push_str(&format!(
            r#"<li class="sort-stop">
  <span class="sort-stop__idx">{idx}</span>
  <div class="sort-stop__body">
    <strong>{customer}</strong>
    <span class="sort-stop__addr">{address}</span>
  </div>
  <span class="sort-stop__num">{order}</span>
</li>"#,
            idx = s.delivery_index,
            customer = html_escape(&s.customer),
            address = html_escape(&s.address),
            order = html_escape(&s.order_number),
        ));
    }
    format!(r#"<ol class="sort-stops">{rows}</ol>"#)
}

fn stacks_html(plan: &SortingPlan) -> String {
    if plan.pallet_stacks.is_empty() {
        return r#"<p class="sort-empty">No pallet photos in this plan.</p>"#.to_string();
    }
    let mut out = String::new();
    for st in &plan.pallet_stacks {
        let mut items = String::new();
        for (i, it) in st.top_to_bottom.iter().enumerate() {
            items.push_str(&format!(
                r#"<li class="sort-stackitem"><span class="sort-stackitem__depth">{depth}</span><span class="sort-stackitem__name">{name}</span><span class="sort-stackitem__count">×{count}</span></li>"#,
                depth = i + 1,
                name = html_escape(&it.customer),
                count = it.count,
            ));
        }
        out.push_str(&format!(
            r#"<div class="sort-stack">
  <div class="sort-stack__head">
    <span class="sort-stack__id">Stack {id}</span>
    <span class="sort-stack__src">{src}</span>
  </div>
  <ol class="sort-stack__list">{items}</ol>
</div>"#,
            id = html_escape(&st.stack_id),
            src = html_escape(&st.source_image),
            items = items,
        ));
    }
    format!(
        r#"<div class="sort-stacks"><p class="sort-hint">Listed top of the pile first — the order a stack can actually be pulled apart in.</p>{out}</div>"#
    )
}

/// `Wed 19 Aug` from `2026-08-19`, or the raw string when it is not a date.
fn date_label(iso: &str) -> String {
    match chrono::NaiveDate::parse_from_str(iso, "%Y-%m-%d") {
        Ok(d) => d.format("%a %-d %b").to_string(),
        Err(_) => iso.to_string(),
    }
}

// ── Templates ─────────────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "sorting/index.html")]
struct IndexTemplate {
    /// `base.html` reads this for its `IS_ADMIN` constant, which gates the
    /// command palette's admin-only entries. It means art-admin, not "signed
    /// in" — every signed-in user reaches this page.
    is_admin: bool,
    sessions: Vec<SessionCard>,
    error: String,
    /// Kept so a rejected paste is still in the box to fix, rather than gone.
    draft: String,
}

/// A session as the index lists it, with its numbers already turned into the
/// strings the template prints. Templates here get values, not arithmetic.
struct SessionCard {
    id: i64,
    route_name: String,
    date_label: String,
    session_date: String,
    total_steps: i64,
    completed_steps: i64,
    total_crates: i64,
    percent: i64,
    status: String,
}

#[derive(Template)]
#[template(path = "sorting/board.html")]
struct BoardTemplate {
    is_admin: bool,
    session_id: i64,
    route_name: String,
    date_label: String,
    time_window: String,
    payload_json: String,
    progress_json: String,
    checks_html: String,
    critical_count: usize,
    attention_count: usize,
    manifest_html: String,
    stops_html: String,
    stacks_html: String,
}

// ── Route handlers ────────────────────────────────────────────────────────

async fn index_page(
    session: AuthSession,
    OptionalAdmin(is_admin): OptionalAdmin,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    Html(render_index(&state, session.user(), is_admin, "", "").await)
}

async fn render_index(
    state: &Arc<AppState>,
    user: crate::models::UserId,
    is_admin: bool,
    error: &str,
    draft: &str,
) -> String {
    let sessions = crate::db::list_sorting_sessions(&state.pool, user)
        .await
        .into_iter()
        .map(|s| {
            let percent = if s.total_steps > 0 {
                (s.completed_steps * 100 / s.total_steps).clamp(0, 100)
            } else {
                0
            };
            let status = if s.total_steps == 0 {
                "No sequence".to_string()
            } else if s.completed_steps == 0 {
                "Not started".to_string()
            } else if s.completed_steps >= s.total_steps {
                "Done".to_string()
            } else {
                format!("{} left", s.total_steps - s.completed_steps)
            };
            SessionCard {
                id: s.id,
                date_label: date_label(&s.session_date),
                route_name: if s.route_name.is_empty() {
                    "Unnamed route".to_string()
                } else {
                    s.route_name
                },
                session_date: s.session_date,
                total_steps: s.total_steps,
                completed_steps: s.completed_steps,
                total_crates: s.total_crates,
                percent,
                status,
            }
        })
        .collect();

    IndexTemplate {
        is_admin,
        sessions,
        error: error.to_string(),
        draft: draft.to_string(),
    }
    .render()
    .unwrap_or_else(|e| format!("template error: {e}"))
}

#[derive(Deserialize)]
struct CreateForm {
    payload: String,
}

/// Takes a pasted plan and opens its board.
///
/// A rejected paste re-renders this page with the reason *and the paste still
/// in the box*. Losing a document someone just copied from a phone, because a
/// comma was in the wrong place, is the kind of small cruelty that gets a tool
/// abandoned.
async fn create_session(
    session: AuthSession,
    OptionalAdmin(is_admin): OptionalAdmin,
    State(state): State<Arc<AppState>>,
    Form(form): Form<CreateForm>,
) -> impl IntoResponse {
    let parsed = match parse_plan(&form.payload) {
        Ok(p) => p,
        Err(e) => {
            return Html(render_index(&state, session.user(), is_admin, &e, &form.payload).await)
                .into_response()
        }
    };

    let route_name = parsed.plan.session.route_name.clone();
    let date = if parsed.plan.session.date.is_empty() {
        chrono::Utc::now().format("%Y-%m-%d").to_string()
    } else {
        parsed.plan.session.date.clone()
    };

    let id = match crate::db::insert_sorting_session(
        &state.pool,
        &route_name,
        &date,
        parsed.total_steps,
        parsed.total_crates,
        &parsed.normalised,
        session.user(),
    )
    .await
    {
        Ok(id) => id,
        Err(e) => {
            return Html(
                render_index(
                    &state,
                    session.user(),
                    is_admin,
                    &format!("The plan parsed but could not be saved — {e}."),
                    &form.payload,
                )
                .await,
            )
            .into_response()
        }
    };

    // Carry over anything the generator already marked done, so a plan that
    // was part-worked before it was uploaded does not start from zero.
    for s in parsed.plan.pick_sequence.iter().filter(|s| s.completed) {
        crate::db::set_sorting_step(&state.pool, id, s.step, true, session.user()).await;
    }

    Redirect::to(&format!("/sorting/{id}")).into_response()
}

async fn board_page(
    session: AuthSession,
    OptionalAdmin(is_admin): OptionalAdmin,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let Some(row) = crate::db::get_sorting_session(&state.pool, id, session.user()).await else {
        // 404, never 403: a signed-in user guessing ids should not learn which
        // of them exist. Same rule the post visibility model uses.
        return (StatusCode::NOT_FOUND, "not found").into_response();
    };

    // The payload came back out of `serde_json` on the way in, so it parses.
    // A failure here would mean the column was edited underneath us; an empty
    // plan renders an empty board with its checks explaining why, which beats
    // a 500.
    let plan: SortingPlan = serde_json::from_str(&row.payload).unwrap_or_default();
    let checks = run_checks(&plan);
    let critical_count = checks
        .iter()
        .filter(|c| c.level == CheckLevel::Critical)
        .count();
    let attention_count = checks
        .iter()
        .filter(|c| c.level != CheckLevel::Note)
        .count();

    let completed = crate::db::get_completed_steps(&state.pool, id, session.user()).await;
    let progress_json = serde_json::to_string(&completed).unwrap_or_else(|_| "[]".to_string());

    let time_window = match (
        plan.session.start_time.as_str(),
        plan.session.end_time.as_str(),
    ) {
        ("", "") => String::new(),
        (a, "") => a.to_string(),
        ("", b) => b.to_string(),
        (a, b) => format!("{a}–{b}"),
    };

    let tpl = BoardTemplate {
        is_admin,
        session_id: id,
        route_name: if row.route_name.is_empty() {
            "Unnamed route".to_string()
        } else {
            row.route_name.clone()
        },
        date_label: date_label(&row.session_date),
        time_window,
        payload_json: json_for_script(&row.payload),
        progress_json: json_for_script(&progress_json),
        checks_html: checks_html(&checks),
        critical_count,
        attention_count,
        manifest_html: manifest_html(&plan),
        stops_html: stops_html(&plan),
        stacks_html: stacks_html(&plan),
    };

    Html(
        tpl.render()
            .unwrap_or_else(|e| format!("template error: {e}")),
    )
    .into_response()
}

#[derive(Deserialize)]
struct StepBody {
    completed: bool,
}

#[derive(Serialize)]
struct StepReply {
    ok: bool,
    completed: usize,
}

/// Ticks or unticks one step.
///
/// Answers with the authoritative completed count so the tablet can correct
/// itself after a spell offline, without re-fetching the board.
async fn set_step(
    session: AuthSession,
    State(state): State<Arc<AppState>>,
    Path((id, step)): Path<(i64, i64)>,
    Json(body): Json<StepBody>,
) -> impl IntoResponse {
    if !crate::db::set_sorting_step(&state.pool, id, step, body.completed, session.user()).await {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    }
    let completed = crate::db::get_completed_steps(&state.pool, id, session.user()).await;
    Json(StepReply {
        ok: true,
        completed: completed.len(),
    })
    .into_response()
}

async fn reset_steps(
    session: AuthSession,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    if !crate::db::reset_sorting_steps(&state.pool, id, session.user()).await {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    }
    Redirect::to(&format!("/sorting/{id}")).into_response()
}

async fn delete_session(
    session: AuthSession,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    if !crate::db::delete_sorting_session(&state.pool, id, session.user()).await {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    }
    StatusCode::NO_CONTENT.into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Relaxed JSON ──────────────────────────────────────────────────────

    #[test]
    fn test_relax_json_strips_line_comments_but_not_urls_in_strings() {
        // The spec's own example document is JSONC, so a paste arrives with
        // comments on the fields. An address with a `//` in it must survive.
        let src = r#"{
  // one entry per stop
  "a": 1, /* and a block one */
  "b": "https://example.com/x"
}"#;
        let out = relax_json(src);
        let v: serde_json::Value = serde_json::from_str(&out).expect("must parse");
        assert_eq!(v["a"], 1);
        assert_eq!(v["b"], "https://example.com/x");
    }

    #[test]
    fn test_relax_json_keeps_a_comment_marker_inside_a_string() {
        let src = r#"{"note": "closes // once rows are full"}"#;
        let v: serde_json::Value = serde_json::from_str(&relax_json(src)).unwrap();
        assert_eq!(v["note"], "closes // once rows are full");
    }

    #[test]
    fn test_relax_json_drops_trailing_commas() {
        let src = r#"{"a": [1, 2, 3,], "b": {"c": 1,},}"#;
        let v: serde_json::Value = serde_json::from_str(&relax_json(src)).unwrap();
        assert_eq!(v["a"][2], 3);
        assert_eq!(v["b"]["c"], 1);
    }

    #[test]
    fn test_relax_json_leaves_a_comma_inside_a_string_alone() {
        // The naive "comma before a bracket" rule would eat this one.
        let src = r#"{"addr": "Slettestrandveien 2, 4032 Stavanger"}"#;
        let v: serde_json::Value = serde_json::from_str(&relax_json(src)).unwrap();
        assert_eq!(v["addr"], "Slettestrandveien 2, 4032 Stavanger");
    }

    #[test]
    fn test_relax_json_leaves_an_escaped_quote_alone() {
        let src = r#"{"a": "he said \"hi\" // not a comment"}"#;
        let v: serde_json::Value = serde_json::from_str(&relax_json(src)).unwrap();
        assert_eq!(v["a"], r#"he said "hi" // not a comment"#);
    }

    // ── Script embedding ──────────────────────────────────────────────────

    #[test]
    fn test_json_for_script_neutralises_a_closing_script_tag() {
        // A customer name containing `</script>` would otherwise end the block
        // and drop the rest of the document into the page as markup.
        let raw = r#"{"customer":"Acme </script><img src=x onerror=alert(1)>"}"#;
        let safe = json_for_script(raw);
        assert!(!safe.contains('<'), "no raw < may survive: {safe}");
        // And it still means exactly what it meant before.
        let v: serde_json::Value = serde_json::from_str(&safe).unwrap();
        assert_eq!(v["customer"], "Acme </script><img src=x onerror=alert(1)>");
    }

    // ── Order numbers ─────────────────────────────────────────────────────

    #[test]
    fn test_order_number_survives_being_written_unquoted() {
        // Ten digits unquoted is a JSON number. Read as f64 it comes back as
        // 1000703538.0 and stops matching the label on the crate.
        let plan: SortingPlan = serde_json::from_str(
            r#"{"manifest":[{"customer":"A","orderNumber":1000703538,"count":1}]}"#,
        )
        .unwrap();
        assert_eq!(plan.manifest[0].order_number, "1000703538");
    }

    #[test]
    fn test_on_route_defaults_to_true_when_absent() {
        // Absent must mean "fine", or every plan without the field reads as an
        // entire route of off-route crates.
        let plan: SortingPlan =
            serde_json::from_str(r#"{"manifest":[{"customer":"A","count":2}]}"#).unwrap();
        assert!(plan.manifest[0].on_route);
        assert_eq!(plan.crates_on_route(), 2);
    }

    #[test]
    fn test_off_route_crates_are_not_counted_as_due_on_the_van() {
        let plan: SortingPlan = serde_json::from_str(
            r#"{"manifest":[{"customer":"A","count":2},{"customer":"B","count":5,"onRoute":false}]}"#,
        )
        .unwrap();
        assert_eq!(plan.crates_on_route(), 2);
    }

    #[test]
    fn test_van_config_defaults_to_the_real_van_not_zeroes() {
        // A plan with no vanConfig must still draw a van. Zero rows would be a
        // blank panel with nothing to explain it.
        let plan: SortingPlan = serde_json::from_str(r#"{"manifest":[]}"#).unwrap();
        assert_eq!(plan.van_config.total_rows, 7);
        assert_eq!(plan.van_config.max_height, 8);
        assert_eq!(plan.van_config.stability_limit, 3);
        assert_eq!(plan.van_config.standby.side_slots, 3);
        assert_eq!(plan.van_config.standby.back_slots, 2);
    }

    // ── Stability ─────────────────────────────────────────────────────────

    fn plan_with_rows(rows: &[(i64, i64, i64)]) -> LoadingPlan {
        LoadingPlan {
            rows: rows
                .iter()
                .map(|(row, l, r)| PlanRow {
                    row: *row,
                    entries: vec![PlanEntry {
                        customer: format!("C{row}"),
                        color: "#888".into(),
                        left: *l,
                        right: *r,
                        uncertain: false,
                    }],
                })
                .collect(),
        }
    }

    #[test]
    fn test_stability_accepts_a_gap_exactly_at_the_limit() {
        // The rule is "no more than 3", so 3 itself is legal. An off-by-one
        // here cries wolf on every correctly generated plan.
        let plan = plan_with_rows(&[(1, 5, 5), (2, 2, 5)]);
        assert!(stability_violations(&plan, &VanConfig::default()).is_empty());
    }

    #[test]
    fn test_stability_catches_a_gap_one_over_the_limit() {
        let plan = plan_with_rows(&[(1, 5, 5), (2, 1, 5)]);
        let v = stability_violations(&plan, &VanConfig::default());
        assert_eq!(v.len(), 1, "{v:?}");
        assert!(v[0].starts_with("left column"), "{v:?}");
    }

    #[test]
    fn test_stability_ignores_the_empty_tail_of_a_light_load() {
        // Rows 4-7 unplanned is a light day, not an instability. Comparing the
        // last loaded row against a row nobody planned would flag every short
        // route as broken and teach everyone to ignore the panel.
        let plan = plan_with_rows(&[(1, 6, 6), (2, 5, 5), (3, 4, 4)]);
        assert!(stability_violations(&plan, &VanConfig::default()).is_empty());
    }

    #[test]
    fn test_stability_exempts_a_genuinely_empty_row_mid_van() {
        // Straight from the methodology: the rule "only applies between two
        // rows that both actually hold a stack" — an empty position has
        // nothing in it to be unstable. This shipped the other way round
        // first, reporting two violations here, which would have had someone
        // restacking a van that was fine.
        let plan = plan_with_rows(&[(1, 6, 0), (2, 0, 0), (3, 6, 0)]);
        assert!(stability_violations(&plan, &VanConfig::default()).is_empty());
    }

    #[test]
    fn test_stability_skips_an_empty_column_but_not_its_loaded_neighbour() {
        // Per column, independently: the left column here has a gap at row 2
        // and is exempt across it, while the right column is loaded all the
        // way through and its 6-to-1 drop is a real violation. Averaging the
        // two into one row height, or comparing left against right, misses it.
        let plan = plan_with_rows(&[(1, 6, 6), (2, 0, 1), (3, 6, 1)]);
        let v = stability_violations(&plan, &VanConfig::default());
        assert_eq!(v.len(), 1, "{v:?}");
        assert!(v[0].starts_with("right column"), "{v:?}");
    }

    #[test]
    fn test_stability_never_compares_left_against_right() {
        // "Left and right stacks in the same row are never compared to each
        // other. They can differ by any amount."
        let plan = plan_with_rows(&[(1, 8, 1), (2, 8, 1)]);
        assert!(stability_violations(&plan, &VanConfig::default()).is_empty());
    }

    #[test]
    fn test_stability_sums_every_entry_in_a_row() {
        // A row holds one entry per customer; the height is the sum, and
        // reading only the first entry would under-report every mixed row.
        let plan = LoadingPlan {
            rows: vec![
                PlanRow {
                    row: 1,
                    entries: vec![
                        PlanEntry {
                            customer: "A".into(),
                            color: "#111".into(),
                            left: 3,
                            right: 0,
                            uncertain: false,
                        },
                        PlanEntry {
                            customer: "B".into(),
                            color: "#222".into(),
                            left: 4,
                            right: 0,
                            uncertain: false,
                        },
                    ],
                },
                PlanRow {
                    row: 2,
                    entries: vec![PlanEntry {
                        customer: "C".into(),
                        color: "#333".into(),
                        left: 1,
                        right: 0,
                        uncertain: false,
                    }],
                },
            ],
        };
        let v = stability_violations(&plan, &VanConfig::default());
        assert_eq!(v.len(), 1, "7 against 1 is a gap of 6: {v:?}");
    }

    #[test]
    fn test_geometry_catches_an_overstacked_column_and_a_row_off_the_end() {
        let plan = plan_with_rows(&[(1, 9, 2), (9, 1, 1)]);
        let v = geometry_violations(&plan, &VanConfig::default());
        assert_eq!(v.len(), 2, "{v:?}");
        assert!(v.iter().any(|s| s.contains("9 crates high")), "{v:?}");
        assert!(v.iter().any(|s| s.contains("outside the van")), "{v:?}");
    }

    // ── Standby ───────────────────────────────────────────────────────────

    /// The customer every `step()` below moves crates for. One name, shared
    /// with the `PlanEntry`s in these tests on purpose: the cell reconciliation
    /// matches the sequence to the diagram *by customer*, so a helper that
    /// invented a name per step would report a mismatch in every test that
    /// pairs the two.
    const WHO: &str = "A";

    fn step(n: i64, qty: i64, from: Endpoint, to: Endpoint) -> PickStep {
        step_for(n, WHO, qty, from, to)
    }

    fn step_for(n: i64, customer: &str, qty: i64, from: Endpoint, to: Endpoint) -> PickStep {
        PickStep {
            step: n,
            customer: customer.to_string(),
            quantity: qty,
            from,
            to,
            completed: false,
        }
    }
    fn pallet(id: &str) -> Endpoint {
        Endpoint {
            kind: "pallet".into(),
            stack_id: id.into(),
            ..Default::default()
        }
    }
    fn standby(slot: &str) -> Endpoint {
        Endpoint {
            kind: "standby".into(),
            slot: slot.into(),
            ..Default::default()
        }
    }
    fn van(row: i64, col: &str) -> Endpoint {
        Endpoint {
            kind: "van".into(),
            row: Some(row),
            column: col.into(),
            ..Default::default()
        }
    }

    #[test]
    fn test_standby_trace_is_clear_when_everything_parked_is_picked_back_up() {
        let plan = SortingPlan {
            pick_sequence: vec![
                step(1, 3, pallet("A"), standby("side-1")),
                step(2, 1, pallet("A"), van(1, "left")),
                step(3, 3, standby("side-1"), van(2, "left")),
            ],
            ..Default::default()
        };
        let t = standby_trace(&plan);
        let (problems, left) = (t.problems, t.left_behind);
        assert!(problems.is_empty(), "{problems:?}");
        assert!(left.is_empty(), "{left:?}");
    }

    #[test]
    fn test_standby_trace_reports_crates_left_standing_at_the_end() {
        // The failure this exists for: the plan reads fine, the sort finishes,
        // and three crates are still on the floor of the van.
        let plan = SortingPlan {
            pick_sequence: vec![
                step(1, 3, pallet("A"), standby("side-2")),
                step(2, 1, pallet("A"), van(1, "left")),
            ],
            ..Default::default()
        };
        let left = standby_trace(&plan).left_behind;
        assert_eq!(left.get("side-2"), Some(&3));
    }

    #[test]
    fn test_standby_trace_reports_taking_from_an_empty_slot() {
        let plan = SortingPlan {
            pick_sequence: vec![step(1, 2, standby("side-1"), van(1, "left"))],
            ..Default::default()
        };
        let problems = standby_trace(&plan).problems;
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("empty at that point"), "{problems:?}");
    }

    #[test]
    fn test_standby_trace_walks_steps_in_step_order_not_array_order() {
        // A regenerated plan can come back with its steps out of order. Read
        // as given, step 3's pickup would run before step 1's drop-off and
        // report a phantom underflow.
        let plan = SortingPlan {
            pick_sequence: vec![
                step(3, 3, standby("side-1"), van(2, "left")),
                step(1, 3, pallet("A"), standby("side-1")),
            ],
            ..Default::default()
        };
        let t = standby_trace(&plan);
        let (problems, left) = (t.problems, t.left_behind);
        assert!(problems.is_empty(), "{problems:?}");
        assert!(left.is_empty(), "{left:?}");
    }

    #[test]
    fn test_standby_trace_reports_the_peak_slots_in_use_at_once() {
        // Three blocking customers pulled off before the one that is needed —
        // the shape every real pallet produces, and the number the methodology
        // says to watch.
        let plan = SortingPlan {
            pick_sequence: vec![
                step_for(1, "A", 2, pallet("B"), standby("side-1")),
                step_for(2, "B", 3, pallet("B"), standby("side-2")),
                step_for(3, "C", 4, pallet("C"), standby("side-3")),
                step_for(4, "A", 2, standby("side-1"), van(3, "left")),
            ],
            ..Default::default()
        };
        let t = standby_trace(&plan);
        assert_eq!(t.peak_slots, 3, "three slots were held at once");
        assert_eq!(t.peak_at_step, 3, "the peak is reached at step 3");
        assert!(t.problems.is_empty(), "{:?}", t.problems);
    }

    #[test]
    fn test_a_plan_wanting_more_standby_room_than_exists_is_critical() {
        // A plan can park and retrieve perfectly, end with every slot empty,
        // and still call for six slots at a moment when five exist. Only a
        // running peak sees it; the ending state says everything is fine.
        let mut steps = Vec::new();
        for i in 1..=6 {
            steps.push(step_for(
                i,
                "A",
                1,
                pallet("P"),
                standby(&format!("side-{i}")),
            ));
        }
        let plan = SortingPlan {
            van_config: VanConfig {
                standby: Standby {
                    side_slots: 3,
                    back_slots: 2,
                    slots_can_stack: true,
                },
                ..Default::default()
            },
            pick_sequence: steps,
            ..Default::default()
        };
        let t = standby_trace(&plan);
        assert_eq!(t.peak_slots, 6);

        let checks = run_checks(&plan);
        assert!(
            checks
                .iter()
                .any(|c| c.level == CheckLevel::Critical && c.title.contains("more standby room")),
            "{checks:?}"
        );
    }

    #[test]
    fn test_standby_trace_rejects_a_slot_the_van_does_not_have() {
        let plan = SortingPlan {
            pick_sequence: vec![step(1, 1, pallet("A"), standby("side-9"))],
            ..Default::default()
        };
        let problems = standby_trace(&plan).problems;
        assert!(
            problems.iter().any(|p| p.contains("side-9")),
            "{problems:?}"
        );
    }

    // ── The check panel ───────────────────────────────────────────────────

    #[test]
    fn test_duplicate_step_numbers_are_critical() {
        // Progress is keyed on the step number, so two moves sharing one means
        // ticking either ticks both and a move vanishes from the board.
        let plan = SortingPlan {
            pick_sequence: vec![
                step_for(1, "A", 1, pallet("A"), van(1, "left")),
                step_for(1, "B", 1, pallet("B"), van(1, "right")),
            ],
            ..Default::default()
        };
        let checks = run_checks(&plan);
        assert!(
            checks
                .iter()
                .any(|c| c.level == CheckLevel::Critical && c.title.contains("step number")),
            "{checks:?}"
        );
    }

    #[test]
    fn test_a_customer_counted_but_never_placed_is_critical() {
        let plan = SortingPlan {
            manifest: vec![
                ManifestEntry {
                    customer: "Loaded AS".into(),
                    count: 1,
                    on_route: true,
                    ..Default::default()
                },
                ManifestEntry {
                    customer: "Forgotten AS".into(),
                    count: 2,
                    on_route: true,
                    ..Default::default()
                },
            ],
            loading_plan: LoadingPlan {
                rows: vec![PlanRow {
                    row: 1,
                    entries: vec![PlanEntry {
                        customer: "Loaded AS".into(),
                        color: "#111".into(),
                        left: 1,
                        right: 0,
                        uncertain: false,
                    }],
                }],
            },
            ..Default::default()
        };
        let checks = run_checks(&plan);
        let hit = checks
            .iter()
            .find(|c| c.title.contains("nowhere in the van"))
            .expect("must be reported");
        assert_eq!(hit.level, CheckLevel::Critical);
        assert!(hit.detail.contains("Forgotten AS"), "{}", hit.detail);
        assert!(
            !hit.detail.contains("Loaded AS"),
            "the placed customer must not be named: {}",
            hit.detail
        );
    }

    #[test]
    fn test_a_minimum_count_softens_a_plan_that_holds_more_than_the_manifest() {
        // "minimum" means the photo did not reach the pallet base, so the plan
        // carrying more crates than the manifest is that flag working as
        // intended — a note, not a stop.
        let plan = SortingPlan {
            manifest: vec![ManifestEntry {
                customer: "A".into(),
                count: 2,
                confidence: "minimum".into(),
                on_route: true,
                ..Default::default()
            }],
            loading_plan: LoadingPlan {
                rows: vec![PlanRow {
                    row: 1,
                    entries: vec![PlanEntry {
                        customer: "A".into(),
                        color: "#111".into(),
                        left: 4,
                        right: 0,
                        uncertain: false,
                    }],
                }],
            },
            ..Default::default()
        };
        let checks = run_checks(&plan);
        let hit = checks
            .iter()
            .find(|c| c.title.contains("disagree"))
            .expect("the mismatch is still reported");
        assert_eq!(hit.level, CheckLevel::Note);
    }

    #[test]
    fn test_the_plan_holding_fewer_than_a_minimum_count_is_still_critical() {
        // A minimum is a floor. The plan carrying *less* than it cannot be
        // explained by the flag, so this one keeps its teeth.
        let plan = SortingPlan {
            manifest: vec![ManifestEntry {
                customer: "A".into(),
                count: 4,
                confidence: "minimum".into(),
                on_route: true,
                ..Default::default()
            }],
            loading_plan: LoadingPlan {
                rows: vec![PlanRow {
                    row: 1,
                    entries: vec![PlanEntry {
                        customer: "A".into(),
                        color: "#111".into(),
                        left: 2,
                        right: 0,
                        uncertain: false,
                    }],
                }],
            },
            ..Default::default()
        };
        let checks = run_checks(&plan);
        let hit = checks
            .iter()
            .find(|c| c.title.contains("disagree"))
            .unwrap();
        assert_eq!(hit.level, CheckLevel::Critical);
    }

    #[test]
    fn test_a_clean_plan_produces_no_checks_at_all() {
        // The panel's credibility rests on this: a correct plan must come back
        // silent, or every real finding is buried in noise nobody reads.
        let plan = SortingPlan {
            manifest: vec![ManifestEntry {
                customer: "A".into(),
                order_number: "1".into(),
                count: 2,
                confidence: "confirmed".into(),
                on_route: true,
            }],
            loading_plan: LoadingPlan {
                rows: vec![PlanRow {
                    row: 1,
                    entries: vec![PlanEntry {
                        customer: "A".into(),
                        color: "#111".into(),
                        left: 1,
                        right: 1,
                        uncertain: false,
                    }],
                }],
            },
            pick_sequence: vec![
                step(1, 1, pallet("A"), van(1, "left")),
                step(2, 1, pallet("A"), van(1, "right")),
            ],
            ..Default::default()
        };
        let checks = run_checks(&plan);
        assert!(checks.is_empty(), "expected a clean bill: {checks:?}");
    }

    #[test]
    fn test_off_route_and_minimum_are_both_surfaced() {
        // The two the methodology names as worth a human glance.
        let plan = SortingPlan {
            manifest: vec![
                ManifestEntry {
                    customer: "Stranger AS".into(),
                    order_number: "999".into(),
                    count: 1,
                    on_route: false,
                    ..Default::default()
                },
                ManifestEntry {
                    customer: "Deep Pallet AS".into(),
                    count: 3,
                    confidence: "minimum".into(),
                    on_route: true,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let checks = run_checks(&plan);
        assert!(
            checks
                .iter()
                .any(|c| c.title.contains("not on the route") && c.detail.contains("Stranger AS")),
            "{checks:?}"
        );
        assert!(
            checks
                .iter()
                .any(|c| c.title.contains("floor") && c.detail.contains("Deep Pallet AS")),
            "{checks:?}"
        );
    }

    #[test]
    fn test_cell_mismatch_is_caught_even_when_the_totals_balance() {
        // The case the totals check cannot see, and the one that actually
        // showed up on the first real plan put through this board: exactly ten
        // crates for exactly the right customers, all in the wrong columns.
        // Row 1 holds 2 left and 1 right; the sequence sends all 3 to the left.
        let plan = SortingPlan {
            loading_plan: LoadingPlan {
                rows: vec![PlanRow {
                    row: 1,
                    entries: vec![PlanEntry {
                        customer: "A".into(),
                        color: "#111".into(),
                        left: 2,
                        right: 1,
                        uncertain: false,
                    }],
                }],
            },
            pick_sequence: vec![step(1, 3, pallet("A"), van(1, "left"))],
            ..Default::default()
        };

        // The totals agree — 3 placed, 3 delivered — so nothing else fires.
        let mismatches = cell_delivery_mismatches(&plan);
        assert_eq!(mismatches.len(), 2, "{mismatches:?}");
        assert!(
            mismatches.iter().any(|m| m.contains("row 1 left")
                && m.contains("brings 3")
                && m.contains("room for 2")),
            "{mismatches:?}"
        );
        assert!(
            mismatches
                .iter()
                .any(|m| m.contains("row 1 right") && m.contains("no step delivers")),
            "{mismatches:?}"
        );

        let checks = run_checks(&plan);
        assert!(
            checks
                .iter()
                .any(|c| c.level == CheckLevel::Critical && c.title.contains("where crates go")),
            "{checks:?}"
        );
    }

    #[test]
    fn test_a_cell_filled_by_several_steps_is_not_a_mismatch() {
        // A row is normally loaded over more than one trip. Reporting each
        // partial step would flag every plan ever generated.
        let plan = SortingPlan {
            loading_plan: LoadingPlan {
                rows: vec![PlanRow {
                    row: 2,
                    entries: vec![PlanEntry {
                        customer: "A".into(),
                        color: "#111".into(),
                        left: 4,
                        right: 0,
                        uncertain: false,
                    }],
                }],
            },
            pick_sequence: vec![
                step(1, 1, pallet("A"), van(2, "left")),
                step(2, 3, pallet("A"), van(2, "left")),
            ],
            ..Default::default()
        };
        assert!(cell_delivery_mismatches(&plan).is_empty());
    }

    #[test]
    fn test_a_cell_nobody_planned_for_is_reported() {
        let plan = SortingPlan {
            loading_plan: LoadingPlan {
                rows: vec![PlanRow {
                    row: 1,
                    entries: vec![PlanEntry {
                        customer: "A".into(),
                        color: "#111".into(),
                        left: 1,
                        right: 0,
                        uncertain: false,
                    }],
                }],
            },
            pick_sequence: vec![
                step(1, 1, pallet("A"), van(1, "left")),
                step(2, 2, pallet("A"), van(5, "right")),
            ],
            ..Default::default()
        };
        let m = cell_delivery_mismatches(&plan);
        assert_eq!(m.len(), 1, "{m:?}");
        assert!(
            m[0].contains("row 5 right") && m[0].contains("puts none"),
            "{m:?}"
        );
    }

    #[test]
    fn test_a_sequence_that_does_not_load_the_plan_is_critical() {
        let plan = SortingPlan {
            loading_plan: LoadingPlan {
                rows: vec![PlanRow {
                    row: 1,
                    entries: vec![PlanEntry {
                        customer: "A".into(),
                        color: "#111".into(),
                        left: 3,
                        right: 0,
                        uncertain: false,
                    }],
                }],
            },
            pick_sequence: vec![step(1, 1, pallet("A"), van(1, "left"))],
            ..Default::default()
        };
        let checks = run_checks(&plan);
        assert!(
            checks
                .iter()
                .any(|c| c.level == CheckLevel::Critical && c.title.contains("does not load")),
            "{checks:?}"
        );
    }

    // ── Parsing a paste ───────────────────────────────────────────────────

    #[test]
    fn test_parse_rejects_something_that_is_not_a_plan() {
        // Pasting the route-list screenshot's JSON, or a chat reply, must say
        // so rather than storing an empty board.
        let err = parse_plan(r#"{"hello":"world"}"#).expect_err("must be refused");
        assert!(err.contains("not a sorting plan"), "{err}");
    }

    #[test]
    fn test_parse_rejects_an_empty_paste() {
        assert!(parse_plan("   \n  ").is_err());
    }

    #[test]
    fn test_parse_rejects_a_bare_array() {
        let err = parse_plan("[1,2,3]").expect_err("must be refused");
        assert!(err.contains("JSON object"), "{err}");
    }

    #[test]
    fn test_parse_accepts_a_plan_with_only_a_manifest() {
        // Half a document is still worth a board — the checks say what is
        // missing rather than the upload refusing it.
        let p = parse_plan(r#"{"manifest":[{"customer":"A","count":2}]}"#).expect("accepted");
        assert_eq!(p.total_crates, 2);
        assert_eq!(p.total_steps, 0);
    }

    #[test]
    fn test_parse_normalises_away_comments_so_the_stored_text_is_plain_json() {
        // The browser's JSON.parse gets the stored text directly and has no
        // relax_json of its own.
        let p = parse_plan(
            r#"{
              // the tally
              "manifest": [{"customer":"A","count":1},],
            }"#,
        )
        .expect("accepted");
        assert!(!p.normalised.contains("//"), "{}", p.normalised);
        let v: serde_json::Value = serde_json::from_str(&p.normalised).expect("plain JSON");
        assert_eq!(v["manifest"][0]["count"], 1);
    }

    #[test]
    fn test_parse_counts_steps_from_the_sequence_length() {
        let p = parse_plan(
            r#"{"pickSequence":[
                 {"step":1,"customer":"A","quantity":1,"from":{"type":"pallet","stackId":"A"},"to":{"type":"van","row":1,"column":"left"}},
                 {"step":2,"customer":"B","quantity":2,"from":{"type":"pallet","stackId":"A"},"to":{"type":"standby","slot":"side-1"}}
               ]}"#,
        )
        .expect("accepted");
        assert_eq!(p.total_steps, 2);
        assert_eq!(p.plan.pick_sequence[1].to.slot, "side-1");
    }

    #[test]
    fn test_date_label_falls_back_to_the_raw_string() {
        assert_eq!(date_label("2026-08-19"), "Wed 19 Aug");
        assert_eq!(date_label("whenever"), "whenever");
    }

    // ── The routes, end to end ────────────────────────────────────────────

    use axum::body::Body;
    use axum::http::{Request, StatusCode as HttpStatus};
    use sqlx::sqlite::SqlitePoolOptions;
    use tower::ServiceExt;

    async fn app_with_pool() -> (Router, crate::db::DbPool) {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        crate::db::run_migrations(&pool).await;
        let storage = crate::storage::ObjectStorage::from_env().await;
        let rp_origin = url::Url::parse("http://localhost:3000").unwrap();
        let webauthn = webauthn_rs::prelude::WebauthnBuilder::new("localhost", &rp_origin)
            .unwrap()
            .build()
            .unwrap();
        let state = Arc::new(crate::AppState {
            pool: pool.clone(),
            storage,
            webauthn,
        });
        (router().with_state(state), pool)
    }

    /// A session cookie for a member — a plain signed-in user, no art admin.
    /// This whole section is reachable by exactly that, which is what these
    /// tests are here to hold in place.
    async fn member_cookie(
        pool: &crate::db::DbPool,
        name: &str,
    ) -> (String, crate::models::UserId) {
        let id = crate::db::create_user(pool, name, "hash").await.unwrap();
        let sess = format!("{name}-sess");
        crate::db::create_session(pool, &sess, "2099-01-01T00:00:00", id).await;
        (format!("session={sess}"), crate::models::UserId(id))
    }

    fn req(method: &str, uri: &str, cookie: Option<&str>, body: &str, json: bool) -> Request<Body> {
        let mut b = Request::builder().method(method).uri(uri).header(
            "content-type",
            if json {
                "application/json"
            } else {
                "application/x-www-form-urlencoded"
            },
        );
        if let Some(c) = cookie {
            b = b.header("cookie", c);
        }
        b.body(Body::from(body.to_string())).unwrap()
    }

    // `r##` rather than `r#`: the plan carries CSS colours, and a `"#` inside
    // an `r#"..."#` literal closes it.
    const SAMPLE: &str = r##"{"session":{"routeName":"Stavanger Route","date":"2026-08-19"},
      "manifest":[{"customer":"Marlink AS","orderNumber":"1000703538","count":1}],
      "loadingPlan":{"rows":[{"row":1,"entries":[{"customer":"Marlink AS","color":"#5b9279","left":1,"right":0}]}]},
      "pickSequence":[{"step":1,"customer":"Marlink AS","quantity":1,
        "from":{"type":"pallet","stackId":"A"},"to":{"type":"van","row":1,"column":"left"}}]}"##;

    /// Every route wants a session. The logged-out case is the one a test
    /// suite passes accidentally, because `AuthSession` redirects rather than
    /// erroring — so this asserts the redirect explicitly, on all six.
    #[tokio::test]
    async fn test_every_route_refuses_a_logged_out_visitor() {
        let (app, _pool) = app_with_pool().await;
        let reqs = vec![
            req("GET", "/sorting", None, "", false),
            req("POST", "/sorting", None, "payload=%7B%7D", false),
            req("GET", "/sorting/1", None, "", false),
            req("DELETE", "/api/sorting/sessions/1", None, "", false),
            req(
                "POST",
                "/api/sorting/sessions/1/steps/1",
                None,
                r#"{"completed":true}"#,
                true,
            ),
            req("POST", "/api/sorting/sessions/1/reset", None, "", false),
        ];
        for r in reqs {
            let method = r.method().clone();
            let uri = r.uri().clone();
            let resp = app.clone().oneshot(r).await.unwrap();
            assert_eq!(
                resp.status(),
                HttpStatus::SEE_OTHER,
                "{method} {uri} must bounce a logged-out visitor to the login"
            );
        }
    }

    /// A member — not an admin — is who this section is for.
    #[tokio::test]
    async fn test_a_plain_member_can_create_and_open_a_board() {
        let (app, pool) = app_with_pool().await;
        let (cookie, _driver) = member_cookie(&pool, "driver").await;

        let body = format!("payload={}", urlencode(SAMPLE));
        let resp = app
            .clone()
            .oneshot(req("POST", "/sorting", Some(&cookie), &body, false))
            .await
            .unwrap();
        assert_eq!(resp.status(), HttpStatus::SEE_OTHER, "upload must redirect");
        let location = resp
            .headers()
            .get("location")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_string();
        assert_eq!(location, "/sorting/1");

        let resp = app
            .clone()
            .oneshot(req("GET", &location, Some(&cookie), "", false))
            .await
            .unwrap();
        assert_eq!(resp.status(), HttpStatus::OK);
    }

    /// The board is addressed by a sequential id, so this is the request that
    /// matters: a signed-in member reaching for a session that is not theirs.
    /// 404, never 403 — 403 would confirm the id exists.
    #[tokio::test]
    async fn test_one_member_cannot_reach_another_members_route() {
        let (app, pool) = app_with_pool().await;
        let (owner, driver) = member_cookie(&pool, "driver").await;
        let (stranger, _) = member_cookie(&pool, "stranger").await;

        let body = format!("payload={}", urlencode(SAMPLE));
        app.clone()
            .oneshot(req("POST", "/sorting", Some(&owner), &body, false))
            .await
            .unwrap();

        for (method, uri, payload, json) in [
            ("GET", "/sorting/1", "", false),
            (
                "POST",
                "/api/sorting/sessions/1/steps/1",
                r#"{"completed":true}"#,
                true,
            ),
            ("POST", "/api/sorting/sessions/1/reset", "", false),
            ("DELETE", "/api/sorting/sessions/1", "", false),
        ] {
            let resp = app
                .clone()
                .oneshot(req(method, uri, Some(&stranger), payload, json))
                .await
                .unwrap();
            assert_eq!(
                resp.status(),
                HttpStatus::NOT_FOUND,
                "{method} {uri} must 404 for another member"
            );
        }

        // And the owner's plan is untouched by any of it.
        let steps = crate::db::get_completed_steps(&pool, 1, driver).await;
        assert!(steps.is_empty(), "a stranger's tick reached the table");
        assert!(crate::db::get_sorting_session(&pool, 1, driver)
            .await
            .is_some());
    }

    #[tokio::test]
    async fn test_a_rejected_paste_re_renders_the_page_with_the_text_still_in_it() {
        // Losing a document someone just copied from a phone, because a comma
        // was in the wrong place, is how a tool gets abandoned.
        let (app, pool) = app_with_pool().await;
        let (cookie, _driver) = member_cookie(&pool, "driver").await;

        let resp = app
            .oneshot(req(
                "POST",
                "/sorting",
                Some(&cookie),
                "payload=%7B%22nope%22%3A1%7D",
                false,
            ))
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            HttpStatus::OK,
            "a bad paste must not redirect"
        );
        let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        let html = String::from_utf8_lossy(&body);
        assert!(
            html.contains("not a sorting plan"),
            "the reason must be shown"
        );
        assert!(
            html.contains("nope"),
            "the paste must still be in the box to fix"
        );
    }

    #[tokio::test]
    async fn test_ticking_a_step_reports_the_authoritative_count() {
        // The tablet uses this to correct itself after a spell offline.
        let (app, pool) = app_with_pool().await;
        let (cookie, _driver) = member_cookie(&pool, "driver").await;
        let body = format!("payload={}", urlencode(SAMPLE));
        app.clone()
            .oneshot(req("POST", "/sorting", Some(&cookie), &body, false))
            .await
            .unwrap();

        let resp = app
            .clone()
            .oneshot(req(
                "POST",
                "/api/sorting/sessions/1/steps/1",
                Some(&cookie),
                r#"{"completed":true}"#,
                true,
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), HttpStatus::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), 1 << 16)
            .await
            .unwrap();
        let reply: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(reply["ok"], true);
        assert_eq!(reply["completed"], 1);
    }

    #[tokio::test]
    async fn test_an_upload_carries_over_steps_the_generator_marked_done() {
        // A plan part-worked before it was uploaded must not start from zero.
        let (app, pool) = app_with_pool().await;
        let (cookie, driver) = member_cookie(&pool, "driver").await;
        let plan = r#"{"pickSequence":[
            {"step":1,"customer":"A","quantity":1,"from":{"type":"pallet","stackId":"A"},"to":{"type":"van","row":1,"column":"left"},"completed":true},
            {"step":2,"customer":"B","quantity":1,"from":{"type":"pallet","stackId":"A"},"to":{"type":"van","row":1,"column":"right"}}]}"#;
        let body = format!("payload={}", urlencode(plan));
        app.oneshot(req("POST", "/sorting", Some(&cookie), &body, false))
            .await
            .unwrap();

        let steps = crate::db::get_completed_steps(&pool, 1, driver).await;
        assert_eq!(steps, vec![1]);
    }

    /// Minimal percent-encoding for the form bodies above — enough for JSON,
    /// which is all these tests post.
    fn urlencode(s: &str) -> String {
        let mut out = String::with_capacity(s.len() * 3);
        for b in s.bytes() {
            match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    out.push(b as char)
                }
                _ => out.push_str(&format!("%{b:02X}")),
            }
        }
        out
    }
}
