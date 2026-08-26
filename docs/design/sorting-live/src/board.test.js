const fs = require('fs');
const { join } = require('path');
class DCLogic { constructor(p) { this.props = p || {}; } setState(o) { Object.assign(this.state, o); } }
const Component = eval(fs.readFileSync(join(__dirname, 'model.js'), 'utf8') + fs.readFileSync(join(__dirname, 'board.js'), 'utf8') + '\n;Component');

let fails = 0, checks = 0;
const ok = (c, m) => { checks++; if (!c) { fails++; console.log('  FAIL  ' + m); } };
const eq = (a, b, m) => ok(JSON.stringify(a) === JSON.stringify(b), m + '  got=' + JSON.stringify(a) + ' want=' + JSON.stringify(b));

const c = new Component({ accent: '#B48EF7', capacity: 8 });
let v = c.renderVals();

// ── every hole in the markup has a producer ─────────────────────────────────
const markup = fs.readFileSync(join(__dirname, 'board.html'), 'utf8');
// build the scope map from <sc-for list="{{x}}" as="y">
const scopes = {};
for (const m of markup.matchAll(/<sc-for\s+list="\{\{([^}]+)\}\}"\s+as="([^"]+)"/g)) scopes[m[2]] = m[1];
function dig(obj, path) {
  return path.split('.').reduce((o, k) => (o == null ? undefined : o[k]), obj);
}
function sample(listPath) {
  // resolve a list path that may itself be scoped (e.g. band.cells, cell.slots)
  const parts = listPath.split('.');
  if (scopes[parts[0]]) { const outer = sample(scopes[parts[0]]); return outer == null ? undefined : dig(outer, parts.slice(1).join('.'))?.[0]; }
  return dig(v, listPath)?.[0];
}
const missing = [];
const holeSrc = markup.replace(/hint-placeholder-val="\{\{[^}]*\}\}"/g, '');
for (const m of holeSrc.matchAll(/\{\{([^}]+)\}\}/g)) {
  const path = m[1].trim();
  const parts = path.split('.');
  let val;
  if (scopes[parts[0]]) {
    const item = sample(scopes[parts[0]]);
    val = item == null ? undefined : dig(item, parts.slice(1).join('.'));
  } else val = dig(v, path);
  if (val === undefined) missing.push(path);
}
eq(missing, [], 'every {{hole}} resolves');

// (the cross-artboard version of this check lives in _validate.js, which sees
//  both markups — this file only reads the landscape one)

// ── sc-for / sc-if balance ──────────────────────────────────────────────────
eq((markup.match(/<sc-for/g) || []).length, (markup.match(/<\/sc-for>/g) || []).length, 'sc-for tags balance');
eq((markup.match(/<sc-if/g) || []).length, (markup.match(/<\/sc-if>/g) || []).length, 'sc-if tags balance');
for (const m of markup.matchAll(/<sc-for\s+list="\{\{[^}]+\}\}"\s+as="[^"]+"([^>]*)>/g)) {
  ok(/hint-placeholder-count=/.test(m[1]), 'sc-for carries a placeholder hint: ' + m[0].slice(0, 60));
}

// ── the seeded board reads the way it should ────────────────────────────────
eq(v.stats.map(s => s.label + ' ' + s.value),
   ['POSITIONS LEFT 13', 'SIDE DOOR 3 left', 'CRATES IN 13', 'STOPS 1 / 6'],
   'header stats — nine rows two across, five already holding something');
ok(/R3 · R/.test(v.bar.eyebrow), 'the console points at the innermost free position');
eq(v.bar.title, 'Jåtten Skole', 'and names who the loading order says goes there');
ok(/3 positions left · 3 staged here/.test(v.sideDoor.note), 'the side-door budget counts both sides of it');
ok(!/will still be standing/.test(v.sideDoor.note), 'and stays quiet while three fit in three');
eq(v.sideSpots.map(s => s.pushLabel), ['Push in → R3 · R', 'Push in anyway', 'Push in anyway'],
   'a 2-crate remainder takes a position of its own — nothing is forcing a mix');
eq(v.sideSpots[0].hasAlt, false, 'and no combine is dangled at it');
ok(/they are on SIDE 1/.test(v.sideSpots[2].sub), 'and the out-of-order one names where the blocker is');
eq(v.bands.map(b => b.label), ['RIGHT', 'LEFT'], 'kerb side is the upper band');
eq(v.bands[0].cells.length, 9, 'nine positions down each band');
eq(v.bands[0].cells.slice(0, 4).map(x => x.pillText), ['IN', 'IN', 'NEXT', 'EMPTY'], 'right band pills');
eq(v.bands[1].cells.slice(0, 4).map(x => x.pillText), ['IN', 'IN', 'IN', 'EMPTY'], 'left band pills');
eq(v.bands[0].cells.slice(4).every(x => x.pillText === 'EMPTY'), true, 'and the rest are still empty');
eq(v.doorways.show, false, 'the doorways stay out of the way while there is floor');
eq(v.queue.map(q => q.state), ['DONE  \u21BA', 'LOADING NOW', 'ON SIDE 2', 'ON SIDE 3', 'WAITING', 'WAITING'], 'load-order strip');
eq(v.queue[0].where, 'R1 · L  R1 · R  R2 · L  R2 · R', 'a closed stop shows where its crates went');

// ── Done is an assertion, and assertions get walked back ────────────────────
const g = new Component({ accent: '#B48EF7', capacity: 8 });
let y = g.renderVals();
eq(y.bar.title, 'Jåtten Skole', 'Olavstoppen is closed out, so Jåtten is up');
y.queue[0].pick();                                 // more Olavstoppen crates just turned up
y = g.renderVals();
eq(g.state.st.closed.OLA, undefined, 'tapping a DONE stop reopens it');
eq(y.bar.title, 'Olavstoppen', 'and the loading order takes them back');
eq(y.queue[0].state, 'LOADING NOW', 'the strip agrees');
ok(/goes in first — they are on/.test(y.sideSpots[0].sub) || y.sideSpots[0].pushLabel !== 'Push in → R3 · R',
   'and the staged stacks are told to wait for them again');
y.bar.undo();
eq(g.renderVals().queue[0].state, 'DONE  \u21BA', 'undo puts the reopen back too');

// ── a crate that is not on this route ───────────────────────────────────────
eq(v.flag.label, '⚑ Odd crate', 'the flag starts quiet');
const h = new Component({ accent: '#B48EF7', capacity: 8 });
h.renderVals().flag.tap();
h.renderVals().flag.tap();
eq(h.renderVals().flag.label, '⚑ 2 odd', 'and counts what it is told');
ok(/F7768E/.test(h.renderVals().flag.style), 'going red once there is something to report');

// ── the console: four crates straight in, then done ─────────────────────────
for (let i = 0; i < 4; i++) { c.renderVals().bar.plus(); }
v = c.renderVals();
eq(c.state.st.open.n, 4, 'four taps, four crates');
ok(/4 stacked here/.test(v.bar.sub), 'and the console says so');
eq(v.bands[0].cells[2].pillText, 'OPEN', 'the position it is going into reads OPEN');
eq(v.bands[0].cells[2].sub, '4 crates', 'and shows the running count');
v.bar.close();
v = c.renderVals();
eq(c.state.st.van['r3-right'].map(l => [l.cust, l.n]), [['JAT', 4]], 'Done sealed the position');
eq(c.state.st.closed.JAT, true, 'and closed the stop out');
eq(v.bar.title, 'Hinna', 'the console moves to the next stop by itself');
// The plan generator put Hinna on top of Jåtten here. The board no longer
// does that by itself: two customers on one stack is a mis-delivery waiting to
// happen, and there is floor to spare.
eq(v.sideSpots[1].pushLabel, 'Push in → R4 · L', 'Hinna takes a position rather than riding on Jåtten');

// ── undo puts it back ───────────────────────────────────────────────────────
v.bar.undo();
v = c.renderVals();
eq(c.state.st.closed.JAT, undefined, 'undo reopens the stop');
eq(c.state.st.open.n, 4, 'and hands back the four crates');
eq(v.bar.title, 'Jåtten Skole', 'console follows it back');

// ── a spot push-in lands where the guard said it would ──────────────────────
v.bar.close();                                   // JAT sealed into r3R, closed
v = c.renderVals();
eq(v.sideSpots[0].head, 'Jåtten Skole', 'SIDE 1 still holds Jåtten leftovers');
eq(v.sideSpots[0].pushLabel, 'Push in anyway', 'which is now out of order — Hinna is up');
v.sideSpots[1].push();                           // Hinna, 2 crates
v = c.renderVals();
eq(c.state.st.van['r4-left'].map(l => [l.cust, l.n]), [['HIN', 2]], 'Hinna went in on her own');
eq(c.state.st.van['r3-left'].map(l => [l.cust, l.n]), [['JAT', 3]], 'and Jåtten was left alone');

// ── run the side door out and check the hard block ──────────────────────────
const d = new Component({ accent: '#B48EF7', capacity: 8 });
for (const id of ['r3-right', 'r4-left', 'r4-right']) d.state.st.van[id] = [{ cust: 'SVE', n: 3 }];
let w = d.renderVals();
eq(w.stats[1].value, 'shut', 'side door reads shut once rows 1-4 are full');
ok(/Rows 1–4 are full/.test(w.sideDoor.note), 'and says why');
eq(w.sideSpots.map(s => s.pushLabel), ['Round the back', 'Round the back', 'Round the back'],
   'every stranded side stack gets the same hard block');
ok(/Carry this round to the back/.test(w.sideSpots[0].sub), 'with the instruction attached');
eq(w.bar.eyebrow.indexOf('BACK DOORS') > -1, true, 'and the console switches to the back door');
ok(/R5 · L/.test(w.bar.eyebrow), 'starting at row 5');

// ── picking a position by hand ──────────────────────────────────────────────
const e = new Component({ accent: '#B48EF7', capacity: 8 });
let x = e.renderVals();
eq(x.bands[0].cells[2].pillText, 'NEXT', 'the board picks R3 · R by itself');
eq(x.bands[0].cells[3].sub, 'send here', 'a free position further out invites a tap');
eq(x.bands[0].cells[3].pos, 'R', 'and at nine across the cell shows only its column — the row is in the header');
eq(x.bands[0].cells[4].sub, 'empty',
   'but one past the side door does not — that stack could not get there');
x.bands[0].cells[4].pick();
eq(e.state.target, null, 'and tapping it picks nothing');
x.bands[0].cells[3].pick();                       // choose R4 · R by hand
x = e.renderVals();
eq(e.state.target, 'r4-right', 'the tap stuck');
eq(x.bands[0].cells[3].pillText, 'PICKED', 'and the position says so');
eq(x.bands[0].cells[2].pillText, 'EMPTY', 'while the automatic one steps aside');
ok(/YOUR PICK/.test(x.bar.eyebrow), 'the console follows the pick');
ok(/R4 · R/.test(x.bar.eyebrow), 'to the position that was picked');
ok(/R3 · R stays free/.test(x.sideSpots[0].sub), 'and the spot spells out what the gap costs');
x.bands[0].cells[3].pick();                       // tapping the picked one clears it
x = e.renderVals();
eq(e.state.target, null, 'tapping it again hands the choice back');
eq(x.bands[0].cells[2].pillText, 'NEXT', 'and the automatic target returns');
x.bands[1].cells[0].pick();                       // a filled position is not a target
eq(e.state.target, null, 'a filled position cannot be picked');

// ── the van is a setting, and there is more than one van ────────────────────
const small = new Component({ rows: 5, capacity: 6, sideDoorRows: 3, sideSpots: 2, backSpots: 2 });
let z = small.renderVals();
eq(z.bands[0].cells.length, 5, 'five rows means five cells a band');
eq(z.heads.map(h => h.label), ['R1', 'R2', 'R3', 'R4', 'R5'], 'and five row heads');
eq(z.sideSpots.length, 2, 'two side spots when that is what is fitted');
eq(z.backSpots.length, 2, 'and two at the back');
eq(z.bands[0].cells.map(c => c.pos).join(' '), 'R1 · R R2 · R R3 · R R4 · R R5 · R', 'positions renumber');
eq(z.stats[0].value, '5', 'positions left counts the smaller van — 10 total, 5 already in');
eq(z.rowsA.length, 3, 'the side door reaches three rows here');
eq(z.rowsB.length, 2, 'and the back doors take the other two');
ok(/flex:3 1 0/.test(z.zoneA.style) && /flex:2 1 0/.test(z.zoneB.style),
   'so the portrait zones split three to two, not four to three');
eq(z.bands[0].cells[0].slots.length, 6, 'a lower roof means shorter slot columns');
ok(/reaches rows 1–3/i.test(z.sideDoor.label) || /ROWS 1–3/.test(z.sideDoor.label),
   'and the side-door line quotes the rows it actually reaches: ' + z.sideDoor.label);

// a van with no side door at all is a legitimate setup, not a broken one
const noDoor = new Component({ rows: 7, sideDoorRows: 0, sideSpots: 1, backSpots: 2 });
const nd = noDoor.renderVals();
eq(nd.rowsA.length, 0, 'no rows behind the side door');
eq(nd.rowsB.length, 7, 'every row is back-door work');
ok(/BACK DOORS/.test(nd.bar.eyebrow), 'and everything loads from the back');
eq(nd.stats[1].value, 'shut', 'the side door reads shut from the start');

// ── the doorway, when the floor runs short ──────────────────────────────────
// Built deliberately rather than off the demo seed: each of these is a specific
// moment the doorway either is or is not the answer.
function blank(c) {
  c.renderVals();                                   // configure + normalize first
  Object.keys(c.state.st.van).forEach(k => { c.state.st.van[k] = []; });
  Object.keys(c.state.st.staged).forEach(k => { c.state.st.staged[k] = null; });
  c.state.st.closed = {}; c.state.st.open = { n: 0 }; c.state.hist = [];
  return c;
}
function fill(c, ids, cust, n) { ids.forEach(id => { c.state.st.van[id] = [{ cust, n }]; }); }
function stage(c, spot, cust, n) { c.state.st.staged[spot] = { cust, n }; }

// A: space is tight but not gone — the doorway sits beside the normal push
const tightVan = blank(new Component({ rows: 5, sideDoorRows: 4, sideSpots: 2, backSpots: 2 }));
fill(tightVan, ['r1-left', 'r1-right', 'r2-left', 'r2-right', 'r3-left', 'r3-right'], 'OLA', 3);
tightVan.state.st.closed.OLA = true;
stage(tightVan, 'side-1', 'JAT', 2);
let t = tightVan.renderVals();
eq(t.stats[0].value, '4', 'four numbered positions left');
ok(t.warn.show, 'and fewer of those than stops still to load');
// More stacks at the side than the door can still reach: the ones past the
// count are the ones that will be left standing, and they get named.
const squeeze = blank(new Component({ rows: 5, sideDoorRows: 4, sideSpots: 3, backSpots: 2 }));
fill(squeeze, ['r1-left', 'r1-right', 'r2-left', 'r2-right', 'r3-left', 'r3-right', 'r4-left'], 'OLA', 3);
squeeze.state.st.closed.OLA = true;
stage(squeeze, 'side-1', 'JAT', 2);
stage(squeeze, 'side-2', 'HIN', 2);
stage(squeeze, 'side-3', 'SVE', 2);
const sq = squeeze.renderVals();
ok(/1 position left · 3 staged here/.test(sq.sideDoor.note), 'the count is stated');
ok(/SIDE 2 \(Hinna\) and SIDE 3 \(Sverdrup\) will still be standing here when it shuts/.test(sq.sideDoor.note),
   'and so is who it lands on — in loading order, so the first one in is the one that fits');
eq(t.doorways.show, true, 'so the doorways come out of hiding');
eq(t.doorways.tiles.map(d => d.name), ['SIDE DOORWAY', 'BACK DOORWAY'], 'one at each door');
eq(t.doorways.tiles.map(d => d.head), ['free', 'free'], 'both empty to start');
ok(/a crate can stand here/.test(t.doorways.tiles[0].sub),
   'the side well says what it still is once no more can be pushed in past it');
ok(/blocks the door/.test(t.doorways.tiles[1].sub), 'and the back one says what it costs');
eq(t.doorways.tiles[0].hasFreeze, true, 'the side well carries the freeze-ware fact');
eq(t.doorways.tiles[1].hasFreeze, false, 'the back one does not');
eq(t.doorways.tiles[0].freezeLabel, '❄ freeze ware', 'set, because it usually is');
t.doorways.tiles[0].freezeTap();
eq(tightVan.renderVals().doorways.tiles[0].freezeLabel, '❄ none today', 'and it can be cleared for the day');
t.doorways.tiles[0].freezeTap();
// Space is tight, so the doorway is offered — but as the alternative, with a
// position of its own still the primary.
eq(t.sideSpots[0].pushLabel, 'Push in → R4 · L', 'a position of its own stays the primary');
eq(t.sideSpots[0].hasAlt, true, 'with the doorway beside it');
eq(t.sideSpots[0].altLabel, 'Doorway', 'as an escape, not an order');
ok(/floor is short/.test(t.sideSpots[0].sub) || /doorway/i.test(t.sideSpots[0].sub) === false,
   'and the sub-line explains the squeeze rather than the rule');
t.sideSpots[0].alt();
t = tightVan.renderVals();
eq(t.doorways.tiles[0].head, 'Jåtten Skole', 'and taking it stands the stack in the side well');
eq(t.stats[0].value, '4', 'without spending a numbered position on it');

// B: the back rows are gone, so the doorway is the only move left
const full = blank(new Component({ rows: 6, sideDoorRows: 4, sideSpots: 2, backSpots: 2 }));
fill(full, ['r5-left', 'r5-right', 'r6-left', 'r6-right'], 'FRO', 3);
stage(full, 'back-1', 'OLA', 2);
let f = full.renderVals();
eq(f.backSpots[0].pushLabel, 'Stand it in the doorway', 'with the back rows full the doorway is the move');
ok(/in the way at every stop before that/.test(f.backSpots[0].sub),
   'and it says the cost — Olavstoppen is stop 6, so it blocks every stop before it');
f.backSpots[0].push();
f = full.renderVals();
eq(f.doorways.tiles[1].head, 'Olavstoppen', 'the stack is standing in the back doorway');
ok(/in the way until then/.test(f.doorways.tiles[1].sub), 'and the tile keeps saying so');
eq(f.doorways.tiles[1].clearLabel, 'Take it out', 'with a way back out');
f.doorways.tiles[1].clear();
eq(full.renderVals().doorways.tiles[1].head, 'free', 'which empties it again');

// B2: the same doorway, with the stop that actually belongs in it
stage(full, 'back-1', 'MAR', 2);
f = full.renderVals();
ok(/before anything else — the doorway is the right place for it/.test(f.backSpots[0].sub),
   'the first delivery is the one the doorway is for');
f.backSpots[0].push();
f = full.renderVals();
ok(/out first, so it is never in the way/.test(f.doorways.tiles[1].sub), 'and the tile agrees');

// C: a shut side door cannot be loaded through, doorway or not
const shut = blank(new Component({ rows: 5, sideDoorRows: 2, sideSpots: 2, backSpots: 2 }));
fill(shut, ['r1-left', 'r1-right', 'r2-left', 'r2-right'], 'OLA', 3);
stage(shut, 'side-1', 'HIN', 2);
let sh = shut.renderVals();
eq(sh.sideSpots[0].pushLabel, 'Round the back', 'two crates get carried round, not stood in the doorway');
ok(/travel past what is already aboard/.test(sh.sideSpots[0].sub), 'and it says why nothing more goes in this way');
eq(sh.sideSpots[0].hasAlt, false, 'with no doorway dangled — there is still back-door floor');

// One crate is the exception: the side well is right there, and it is the
// place a single box belongs anyway.
stage(shut, 'side-1', 'HIN', 1);
sh = shut.renderVals();
eq(sh.sideSpots[0].pushLabel, 'Put it at the side door', 'a lone crate goes in the well instead');
ok(/keeps Hinna off anybody/.test(sh.sideSpots[0].sub), 'because it also keeps them off another stack');
ok(/freeze ware shares this space/.test(sh.sideSpots[0].sub), 'and the freeze ware is flagged');
sh.sideSpots[0].push();
sh = shut.renderVals();
eq(sh.doorways.tiles[0].head, 'Hinna', 'and that is where it lands');
ok(/one crate, easy to reach/.test(sh.doorways.tiles[0].sub), 'read as a good use of the space, not a warning');

// D: a van with no side door has one doorway, not two
const nd2 = blank(new Component({ rows: 5, sideDoorRows: 0, sideSpots: 1, backSpots: 2 }));
eq(nd2.renderVals().doorways.tiles.map(d => d.name), ['BACK DOORWAY'], 'no side door, no side doorway');

// ── the two consoles are laid out differently on purpose ────────────────────
ok(/flex-direction:column/.test(v.bar.boxV), 'upright, the console stacks');
ok(!/flex-direction:column/.test(v.bar.box), 'landscape, it runs along one row');
ok(px(v.bar.boxV, 'height') >= px(v.bar.plusStyle, 'height') * 2,
   'and the column box is tall enough for two rows of controls');

// ── mixing customers is a remedy, and never a silent one ────────────────────
// The ±3 rule can leave a stack no room of its own. That is the one thing that
// makes combining the primary — and it goes in amber, saying what it costs.
const forced = blank(new Component({ rows: 9, sideDoorRows: 4, sideSpots: 2, backSpots: 2 }));
fill(forced, ['r1-left', 'r1-right'], 'OLA', 6);
forced.state.st.closed.OLA = true;
stage(forced, 'side-1', 'JAT', 2);
let fr = forced.renderVals();
eq(fr.sideSpots[0].pushLabel, 'Stack on R1 · L', 'two crates beside a six have nowhere of their own to go');
ok(/two customers on one stack is how the wrong crate gets carried in/.test(fr.sideSpots[0].sub),
   'and the board says what mixing costs rather than just doing it');
eq(fr.sideSpots[0].altLabel, 'R2 · L', 'with the position of its own still one tap away');
fr.sideSpots[0].push();
fr = forced.renderVals();
eq(fr.bands[1].cells[0].pillText, 'MIXED', 'a position holding two customers says so, in the pill');
ok(/two customers/.test(fr.bands[1].cells[0].sub), 'and again underneath');
eq(fr.bands[0].cells[0].pillText, 'IN', 'while a single-customer position reads plainly');

// a lone crate settles the same problem without mixing anything
const loneFix = blank(new Component({ rows: 9, sideDoorRows: 4, sideSpots: 2, backSpots: 2 }));
fill(loneFix, ['r1-left', 'r1-right'], 'OLA', 6);
loneFix.state.st.closed.OLA = true;
stage(loneFix, 'side-1', 'JAT', 1);
const lf = loneFix.renderVals();
eq(lf.sideSpots[0].pushLabel, 'Put it at the side door', 'one crate goes to the door rather than onto a stack');
ok(/easier to reach at the side door anyway/.test(lf.sideSpots[0].sub), 'and says why that is better');

// ── the else-if chain stays a chain ─────────────────────────────────────────
// A stray `if` spliced into it once orphaned every branch below, so: a kind
// that lives at the bottom of the chain, in the state that used to break it.
const chain = blank(new Component({ rows: 4, sideDoorRows: 2, sideSpots: 2, backSpots: 2 }));
fill(chain, ['r1-left', 'r1-right', 'r2-left', 'r2-right', 'r3-left', 'r3-right', 'r4-left'], 'OLA', 3);
chain.state.st.van['door-side'] = [{ cust: 'MAR', n: 1 }];   // well already taken
stage(chain, 'side-1', 'SVE', 4);
const ch = chain.renderVals();
ok(ch.warn.show, 'space is tight');
eq(ch.sideSpots[0].pushLabel, 'Round the back', 'and the last branch of the chain still renders its label');
ok(ch.sideSpots[0].pushLabel !== '—', 'rather than falling through to nothing');
ok(/F7768E/.test(ch.sideSpots[0].pushStyle), 'in the red it is supposed to wear');

// ── the band's back spot cannot flex against nine cells ─────────────────────
// It did, and collapsed to 126px with its text column at zero width — a bug
// that only a browser could show, so the width is asserted here.
{
  const r = new Component({}).renderVals();
  ok(/width:244px/.test(r.bands[0].spot.tile), 'the back spot is a fixed width at the end of the band');
  ok(/flex:none/.test(r.bands[0].spot.tile), 'and does not flex against the cells');
  ok(/flex:1 1 0/.test(r.sideSpots[0].tile), 'while the side spots, which share a row, still do');
  ok(boxH(r.bands[0].spot.tile) === boxH(r.bands[0].cells[0].tile),
     'and it is the same height as a cell, so the band does not grow around it');
  ok(/display:none/.test(r.bands[0].cells[8].slotCol), 'an untouched position draws no stack column');
  ok(!/display:none/.test(r.bands[0].cells[0].slotCol), 'a loaded one does');
}

// ── the vertical budget ─────────────────────────────────────────────────────
// These artboards cannot be rendered here, so the height arithmetic is checked
// instead of eyeballed: add up every fixed-height band and make sure the route
// strip is still left something to live in.
function boxH(style) { const m = style.match(/height:(\d+)px/); return m ? +m[1] : 0; }
{
  const b = new Component({});             // the real van: nine rows
  const r = b.renderVals();
  const GAP = 8, PAD = 14 * 2;
  const LABEL_ROW = 18, HEAD_ROW = 14, WARN = 34, DOORWAY = 54, BACK_BTN = 44;
  const fixed = PAD
    + BACK_BTN + GAP                       // header
    + boxH(r.bar.box) + GAP                   // console
    + LABEL_ROW + GAP                      // side-door line
    + boxH(r.sideSpots[0].tile) + GAP         // packing spots
    + HEAD_ROW + GAP                       // row numbers
    + boxH(r.bands[0].cells[0].tile) + GAP    // kerb band
    + boxH(r.bands[1].cells[0].tile) + GAP;   // driver band
  const worst = fixed + WARN + GAP + DOORWAY + GAP;   // both warning strips out
  ok(fixed <= 840, 'the landscape board fits 840 before the conditional strips: ' + fixed);
  ok(worst <= 840 - 55, 'and still leaves the route strip room with both strips showing: ' + worst);

  // across: nine positions, a row label, an arrow and a back spot
  const across = 1440 - 18 * 2 - 52 - GAP - GAP - 20 - GAP - 244;
  const cell = Math.floor((across - GAP * (9 - 1)) / 9);
  ok(cell >= 100, 'nine cells still get a workable width: ' + cell + 'px');
  ok(r.bands[0].cells[0].head.length <= 8 || cell >= 140,
     'and at that width the head is short enough to read: "' + r.bands[0].cells[0].head + '"');
  const who = new Component({});
  who.renderVals().modes[1].pick();
  const names = who.renderVals().bands[1].cells[0].head;
  ok(names.length <= 5, 'in “who goes where” a narrow cell falls back to a code: "' + names + '"');
}
{
  const p = new Component({});
  const r = p.renderVals();
  const GAP = 8, PAD = 14 * 2;
  const fixed = PAD
    + 44 + GAP                             // header
    + boxH(r.bar.boxV) + GAP                  // console, stacked
    + 20 + GAP                             // cab line
    + 26 + GAP                             // back-doors line
    + 150;                                 // back packing spots
  const zones = 1384 - fixed - 54 - GAP;   // doorway strip shows by default upright
  const rowH = Math.floor((zones - GAP * 9) / 9);
  ok(rowH >= 80, 'upright, nine rows still get a usable height each: ' + rowH + 'px');
  // eight slots that shrink rather than overflow the shorter cell
  ok(/flex:0 1 7px/.test(r.rowsA[0].cells[0].slots[0].style), 'slots shrink instead of overflowing');
  ok(/min-height:3px/.test(r.rowsA[0].cells[0].slots[0].style), 'down to a floor, not to nothing');
  ok(/justify-content:flex-end/.test(r.rowsA[0].cells[0].slotCol), 'and stack up from the bottom');
}

// ── touch targets ───────────────────────────────────────────────────────────
function px(style, prop) { const m = style.match(new RegExp(prop + ':(\\d+)px')); return m ? +m[1] : null; }
ok(px(v.bar.plusStyle, 'height') >= 48, 'the +1 target clears 48dp');
ok(px(v.bar.plusStyle, 'width') >= 200, 'and is the widest control on the bar');
v.sideSpots.forEach((s, i) => {
  ok(px(s.countStyle, 'height') >= 48, 'spot ' + (i + 1) + ' count target clears 48dp');
  ok(px(s.pushStyle, 'height') >= 44, 'spot ' + (i + 1) + ' push button is at least 44px');
});

console.log((fails ? 'FAILED ' : 'passed ') + (checks - fails) + '/' + checks + ' checks');
process.exit(fails ? 1 : 0);
