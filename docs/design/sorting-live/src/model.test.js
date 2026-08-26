const fs = require('fs');
const { join } = require('path');
eval(fs.readFileSync(join(__dirname, 'model.js'), 'utf8'));
// the harness predates the do* naming; keep the old verbs pointing at the core
function assign(st, spot, cust) { doAssign(st, spot, cust); }
function bump(st, spot, d) { doBump(st, spot, d); }
function pushIn(st, spot, take) { return doPush(st, spot, take); }
function closeOut(st, cust) { st.closed[cust] = true; }

// The generated Stavanger plan was drawn for a seven-row van, so the replay
// below has to be run against that van, not against whatever the default is.
const DEFAULT_ROWS = ROWS;
configure({ rows: 7 });

let fails = 0, checks = 0;
function ok(cond, msg) { checks++; if (!cond) { fails++; console.log('  FAIL  ' + msg); } }
function eq(a, b, msg) { ok(JSON.stringify(a) === JSON.stringify(b), msg + '  got=' + JSON.stringify(a) + ' want=' + JSON.stringify(b)); }

// ── 0. codes have to survive a real route list ──────────────────────────────
// Three letters off the first word renders half a Norwegian route identically,
// and a code is what gets read before crates are carried into a building.
{
  const route = {
    a: 'Rema 1000 Hillevåg', b: 'Rema 1000 Madla', c: 'Rema 1000 Mariero',
    d: 'Jåtten Skole – K2054', e: 'Jåtten Barnehage', f: 'Coop Extra', g: 'Coop Prix',
    h: 'Marlink AS', i: 'Frøystad Barnehage SA'
  };
  const code = makeCodes(route), vals = Object.keys(code).map(k => code[k]);
  eq(new Set(vals).size, vals.length, 'every code on a colliding route is distinct');
  ok(vals.every(v => v.length >= 2 && v.length <= 5), 'and none of them runs long');
  // the shared word carries nothing, so it must not be what the code is built from
  eq([code.a, code.b, code.c], ['RHI', 'RMA', 'RMAR'], 'three Remas separate on the place name');
  eq([code.d, code.e], ['JSK', 'JBA'], 'and two Jåttens on what follows Jåtten');
  eq([code.f, code.g], ['CEX', 'CPR'], 'bare numbers like “1000” are skipped as meaningless');
  // a route with no collisions keeps the plain three letters
  const plain = makeCodes({ a: 'Olavstoppen', b: 'Marlink AS', c: 'Sverdrup Steel AS' });
  eq([plain.a, plain.b, plain.c], ['OLA', 'MAR', 'SVE'], 'an uncolliding route stays readable');
}

// ── 1. zones and fill order ──────────────────────────────────────────────────
eq(zone('side'), ['r1-left','r1-right','r2-left','r2-right','r3-left','r3-right','r4-left','r4-right'],
   'side zone is rows 1-4, left before right');
eq(zone('back'), ['r5-left','r5-right','r6-left','r6-right','r7-left','r7-right'],
   'back zone is rows 5-7');
ok(zone('side').every(id => rowOf(id) <= 4), 'no side position past row 4');
ok(zone('back').every(id => rowOf(id) >= 5), 'no back position before row 5');

// ── 2. the planned route, replayed live ─────────────────────────────────────
// Same six stops and same crate counts as the generated plan, driven only by
// live actions. The van it builds must come out identical.
var st = emptyState();
function stage(spot, cust, n) { assign(st, spot, cust); for (var i = 0; i < n; i++) bump(st, spot, 1); }
function push(spot, take) { return pushIn(st, spot, take); }

eq(expectedNext(st), 'OLA', 'the last delivery is loaded first');
eq(unstagedNext(st), 'OLA', 'an empty spot offers the queue head');

stage('side-1', 'OLA', 3);
eq(pushState(st, 'side-1').kind, 'ready', 'queue head pushes green');
eq(pushState(st, 'side-1').target, 'r1-left', 'innermost side position is r1 left');
eq(push('side-1'), 'r1-left', 'OLA 3 -> r1L');

// A second customer can be built at another spot while the first is still open.
stage('side-2', 'JAT', 3);
eq(pushState(st, 'side-2').kind, 'order', 'out-of-order push is amber, not blocked');
ok(/Olavstoppen goes in first/.test(pushState(st, 'side-2').why), 'names who goes first and where they are');

for (var i = 0; i < 3; i++) bump(st, 'side-1', 1);
eq(push('side-1'), 'r1-right', 'OLA 3 -> r1R');
for (var i = 0; i < 2; i++) bump(st, 'side-1', 1);
eq(push('side-1'), 'r2-left', 'OLA 2 -> r2L');
for (var i = 0; i < 2; i++) bump(st, 'side-1', 1);
eq(push('side-1'), 'r2-right', 'OLA 2 -> r2R');
closeOut(st, 'OLA');

eq(expectedNext(st), 'JAT', 'Done advances the queue');
eq(pushState(st, 'side-2').kind, 'ready', 'and the amber clears');
eq(push('side-2'), 'r3-left', 'JAT 3 -> r3L');
for (var i = 0; i < 2; i++) bump(st, 'side-2', 1);
eq(push('side-2'), 'r3-right', 'JAT 2 -> r3R');
closeOut(st, 'JAT');

// Hinna is delivered before Jaatten, so it goes ON TOP rather than in a thin
// row of its own. The methodology's combine rule, enforced live.
stage('side-3', 'HIN', 2);
eq(canStackOn(st, 'r3-left', 'HIN').ok, true, 'earlier-delivered may go on top');
eq(canStackOn(st, 'r3-left', 'OLA').ok, false, 'later-delivered may not go on top');
ok(/goes underneath, not on top/.test(canStackOn(st, 'r3-left', 'OLA').why), 'and says why');
st.van['r3-left'].push({ cust: 'HIN', n: 2 }); st.staged['side-3'] = null;
eq(heightOf(st, 'r3-left'), 5, 'r3L is Jaatten 3 + Hinna 2');
closeOut(st, 'HIN');

stage('side-1', 'SVE', 4);
eq(push('side-1'), 'r4-left', 'SVE 4 -> r4L');
for (var i = 0; i < 3; i++) bump(st, 'side-1', 1);
eq(push('side-1'), 'r4-right', 'SVE 3 -> r4R');
closeOut(st, 'SVE');

// ── 3. the side door shuts exactly when rows 1-4 are full ───────────────────
eq(sideDoorOpen(st), false, 'side door shuts once rows 1-4 are full');
stage('side-2', 'FRO', 2);
eq(pushState(st, 'side-2').kind, 'physical', 'a stack stranded at the side is carried round, not forced in');
ok(/travel past what is already aboard/.test(pushState(st, 'side-2').why), 'and it says why it cannot go in');
ok(/Carry this round to the back/.test(pushState(st, 'side-2').why), 'and what to do about it');

// A single crate is the exception — the side well is right there, and walking
// it round the van earns nothing.
stage('side-2', 'HIN', 1);
var lone = pushState(st, 'side-2');
eq(lone.kind, 'doorway', 'one crate goes to the side door instead');
eq(lone.label, 'Put it at the side door', 'and the button says so');
ok(/keeps Hinna off anybody/.test(lone.why), 'because it also keeps them off somebody else’s stack');
ok(/freeze ware shares this space/.test(lone.why), 'with the freeze ware flagged as sharing it');
st.staged['side-2'] = null;

stage('back-1', 'FRO', 2);
eq(pushState(st, 'back-1').target, 'r5-left', 'back door starts at r5');
eq(push('back-1'), 'r5-left', 'FRO 2 -> r5L');
for (var i = 0; i < 2; i++) bump(st, 'back-1', 1);
eq(push('back-1'), 'r5-right', 'FRO 2 -> r5R');
closeOut(st, 'FRO');

stage('back-2', 'MAR', 2);
eq(push('back-2'), 'r6-left', 'MAR 2 -> r6L');
bump(st, 'back-2', 1);
eq(push('back-2'), 'r6-right', 'MAR 1 -> r6R');
closeOut(st, 'MAR');

// ── 4. the van it built matches the generated plan, cell for cell ───────────
var WANT = {
  'r1-left': [['OLA',3]], 'r1-right': [['OLA',3]],
  'r2-left': [['OLA',2]], 'r2-right': [['OLA',2]],
  'r3-left': [['JAT',3],['HIN',2]], 'r3-right': [['JAT',2]],
  'r4-left': [['SVE',4]], 'r4-right': [['SVE',3]],
  'r5-left': [['FRO',2]], 'r5-right': [['FRO',2]],
  'r6-left': [['MAR',2]], 'r6-right': [['MAR',1]],
  'r7-left': [], 'r7-right': []
};
ORDER.forEach(function (id) {
  eq(st.van[id].map(l => [l.cust, l.n]), WANT[id], 'live replay rebuilt ' + id);
});

// ── 5. stability, checked the way the doc states it ─────────────────────────
['left','right'].forEach(function (col) {
  for (var r = 1; r < ROWS; r++) {
    var a = 'r' + r + '-' + col, b = 'r' + (r + 1) + '-' + col;
    if (isEmpty(st, a) || isEmpty(st, b)) continue;          // empty is exempt
    var d = Math.abs(heightOf(st, a) - heightOf(st, b));
    ok(d <= STAB, 'column ' + col + ' rows ' + r + '/' + (r + 1) + ' are ' + d + ' apart');
  }
});
// left is never compared to right
ok(Math.abs(heightOf(st, 'r4-left') - heightOf(st, 'r4-right')) === 1, 'r4 L/R differ, and that is fine');

// ── 6. split, when one stack would spike the column ─────────────────────────
var s2 = emptyState();
assign(s2, 'side-1', 'OLA'); for (var i = 0; i < 2; i++) bump(s2, 'side-1', 1);
pushIn(s2, 'side-1');                                        // r1L = 2
eq(heightOf(s2, 'r1-left'), 2, 'r1L seeded at 2');
assign(s2, 'side-1', 'OLA'); for (var i = 0; i < 3; i++) bump(s2, 'side-1', 1);
pushIn(s2, 'side-1');                                        // r1R = 3
closeOut(s2, 'OLA');
assign(s2, 'side-2', 'JAT'); for (var i = 0; i < 8; i++) bump(s2, 'side-2', 1);
var ps = pushState(s2, 'side-2');
eq(ps.kind, 'split', '8 crates next to a 2 offers a split');
eq(ps.target, 'r2-left', 'the split targets r2L');
eq(ps.take, 4, 'divided evenly rather than filled to the ceiling');
eq(ps.plan.heights, [4, 4], 'four and four, so the column steps 2 → 4 → 4');
// The spill is the next position in FILL order, which here is the same row's
// other column. That is free floor: ±3 compares front to back within one
// column and never left to right, so the two halves do not constrain one
// another at all.
eq(ps.plan.cells, ['r2-left', 'r2-right'], 'the spill goes across the row, not down the column');
eq(colOf(ps.plan.cells[0]) === colOf(ps.plan.cells[1]), false, 'so the two halves are in different columns');
ok(/Split it 4 \+ 4/.test(ps.why), 'and the reason spells the split out');
doPush(s2, 'side-2', ps.take, null, ps.plan.cells[1]);
eq(heightOf(s2, 'r2-left'), 4, 'four went in');
eq(s2.staged['side-2'].n, 4, 'four are still on the spot');
eq(s2.held['r2-right'], 'JAT', 'and r2R is held for the rest of them');
eq(isHeldAgainst(s2, 'r2-right', 'HIN'), true, 'somebody else is routed past the held cell');
eq(isHeldAgainst(s2, 'r2-right', 'JAT'), false, 'while the split’s owner walks straight into it');
eq(resolveTarget(s2, 'side', null, 'JAT'), 'r2-right', 'which is where the rest of the order goes');
eq(pushState(s2, 'side-2').target, 'r2-right', 'so the rest of the order lands beside the first half');
doPush(s2, 'side-2');
eq(heightOf(s2, 'r2-right'), 4, 'and lands there');
eq(s2.held['r2-right'], undefined, 'releasing the hold as it goes');

// ── the split that used to manufacture the violation it prevents ────────────
// Fill to min(roof, front + 3) and spill puts 8 behind a closed 8 and then 2
// behind that: a gap of six, made by the mechanism meant to enforce three.
var s5 = emptyState();
assign(s5, 'side-1', 'OLA'); for (var i = 0; i < 8; i++) bump(s5, 'side-1', 1);
pushIn(s5, 'side-1');                                        // r1L = 8, closed at the roof
closeOut(s5, 'OLA');
assign(s5, 'side-2', 'JAT'); for (var i = 0; i < 10; i++) bump(s5, 'side-2', 1);
var big = pushState(s5, 'side-2');
eq(big.kind, 'split', 'ten behind a closed eight is a split');
eq(big.plan.heights, [5, 5], 'five and five — never eight then two');
ok(Math.abs(8 - big.plan.heights[0]) <= 3, 'the first step is inside the rule');
ok(Math.abs(big.plan.heights[0] - big.plan.heights[1]) <= 3, 'and so is the second');
eq(windowAt(s5, 'r2-left').lo, 5, 'the floor beside an eight is five');
eq(windowAt(s5, 'r2-left').hi, 8, 'and the ceiling is the roof');

// nothing legal to ramp into is said plainly rather than fudged
var s6 = emptyState();
assign(s6, 'side-1', 'OLA'); for (var i = 0; i < 8; i++) bump(s6, 'side-1', 1);
pushIn(s6, 'side-1');
for (var i = 0; i < 8; i++) bump(s6, 'side-1', 1);
pushIn(s6, 'side-1');                                        // r1R too, so the target is r2L
closeOut(s6, 'OLA');
s6.van['r3-left'] = [{ cust: 'MAR', n: 3 }];                 // and the left column is blocked
s6.van['r4-left'] = [{ cust: 'MAR', n: 3 }];
assign(s6, 'side-2', 'JAT'); for (var i = 0; i < 20; i++) bump(s6, 'side-2', 1);
eq(pushState(s6, 'side-2').kind, 'nofit', 'twenty with nowhere to ramp is not pretended otherwise');
ok(/cannot be made to step down/.test(pushState(s6, 'side-2').why), 'and it says so');

// ── the plan drawn from counts, and the board built by hand, agree ──────────
// planAhead runs the same window, the same split and the same fill order
// forward over counts that are already known, so a route sorted with a
// manifest and one sorted blind end up in the same van.
{
  const plan = planAhead({ OLA: 10, JAT: 5, HIN: 2, SVE: 7, FRO: 4, MAR: 3 });
  const used = ORDER.filter(id => plan.van[id].length);
  eq(used.length, 8, 'thirty-one crates land in eight of the eighteen positions');
  eq(plan.short.length, 0, 'and none of it is left over');
  let bad = 0;
  ['left', 'right'].forEach(col => {
    for (let r = 1; r < ROWS; r++) {
      const a = plan.van['r' + r + '-' + col].reduce((s, l) => s + l.n, 0);
      const b = plan.van['r' + (r + 1) + '-' + col].reduce((s, l) => s + l.n, 0);
      if (a && b && Math.abs(a - b) > STAB) bad++;
    }
  });
  eq(bad, 0, 'with no ±3 violation anywhere in either column');
  // the profile that beat the naive planner: 7 next to 3 two rows on
  const lumpy = planAhead({ OLA: 16, JAT: 2, HIN: 14, SVE: 1, FRO: 9, MAR: 5 });
  let lbad = 0;
  ['left', 'right'].forEach(col => {
    for (let r = 1; r < ROWS; r++) {
      const a = lumpy.van['r' + r + '-' + col].reduce((s, l) => s + l.n, 0);
      const b = lumpy.van['r' + (r + 1) + '-' + col].reduce((s, l) => s + l.n, 0);
      if (a && b && Math.abs(a - b) > STAB) lbad++;
    }
  });
  eq(lbad, 0, 'and a deliberately lumpy route ramps too');
  eq(lumpy.short.length, 0, 'without running out of van');
}

// ── 7. thin-stack warning, the other direction ──────────────────────────────
var s3 = emptyState();
assign(s3, 'side-1', 'OLA'); for (var i = 0; i < 8; i++) bump(s3, 'side-1', 1);
pushIn(s3, 'side-1');                                        // r1L = 8
assign(s3, 'side-1', 'OLA'); bump(s3, 'side-1', 1);
pushIn(s3, 'side-1');                                        // r1R = 1
closeOut(s3, 'OLA');
assign(s3, 'side-2', 'JAT'); bump(s3, 'side-2', 1);
var ts = pushState(s3, 'side-2');
eq(ts.kind, 'thin', 'a stack far shorter than its neighbour warns too');
ok(/7 apart, and 5 is the floor here/.test(ts.why), 'naming both the gap and the floor it breaks');

// ── 8. uncounted stacks are exempt, not guessed at ──────────────────────────
var s4 = emptyState();
assign(s4, 'side-1', 'OLA'); pushIn(s4, 'side-1');           // pushed with n = 0
eq(s4.van['r1-left'][0].n, null, 'an uncounted push records unknown, not zero');
eq(heightOf(s4, 'r1-left'), null, 'a column with an unknown has no height');
assign(s4, 'side-1', 'OLA'); for (var i = 0; i < 8; i++) bump(s4, 'side-1', 1);
eq(stabilityAt(s4, 'r1-right', 8).length, 0, 'and never triggers a violation it cannot prove');

// ── 9. positions left, the headline number ──────────────────────────────────
eq(positionsLeft(st, 'side') + positionsLeft(st, 'back'), 2, 'two positions left in the seven-row van');
eq(positionsLeft(emptyState(), 'side'), 8, 'eight through the side door');
eq(positionsLeft(emptyState(), 'back'), 6, 'six through the back of a seven-row van');

// ── and the van it is actually loaded into ──────────────────────────────────
eq(DEFAULT_ROWS, 9, 'the default van is nine rows, not the doc’s seven');
configure({});
eq(ORDER.length, 18, 'nine rows two across is eighteen positions');
eq(positionsLeft(emptyState(), 'side'), 8, 'the side door still reaches four rows — eight positions');
eq(positionsLeft(emptyState(), 'back'), 10, 'the extra length all lands behind the side door');
eq(zone('back')[0], 'r5-left', 'back-door work still starts at row 5');
eq(zone('back')[zone('back').length - 1], 'r9-right', 'and now runs to row 9');

console.log((fails ? 'FAILED ' : 'passed ') + (checks - fails) + '/' + checks + ' checks');
process.exit(fails ? 1 : 0);
