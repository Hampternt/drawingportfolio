const fs = require('fs');
const { join } = require('path');
class DCLogic { constructor(p) { this.props = p || {}; } setState(o) { Object.assign(this.state, o); } }
const src = fs.readFileSync(join(__dirname, 'model.js'), 'utf8') + fs.readFileSync(join(__dirname, 'board.js'), 'utf8');
const Component = eval(src + '\n;Component');
eval(src);   // the model verbs, for asserting against what the board did

let fails = 0, checks = 0;
const ok = (c, m) => { checks++; if (!c) { fails++; console.log('  FAIL  ' + m); } };
const eq = (a, b, m) => ok(JSON.stringify(a) === JSON.stringify(b), m + '  got=' + JSON.stringify(a) + ' want=' + JSON.stringify(b));

const c = new Component({ accent: '#B48EF7', capacity: 8 });
let v = c.renderVals();

// ── every hole in the markup has a producer ─────────────────────────────────
const markup = fs.readFileSync(join(__dirname, 'board.html'), 'utf8');
const scopes = {};
for (const m of markup.matchAll(/<sc-for\s+list="\{\{([^}]+)\}\}"\s+as="([^"]+)"/g)) scopes[m[2]] = m[1];
function dig(obj, path) { return path.split('.').reduce((o, k) => (o == null ? undefined : o[k]), obj); }
// Sample the first item of a list that actually carries the path being checked.
// Taking [0] blindly fails on a heterogeneous list — the scene's first part is a
// wall with no stripes, and that says nothing about whether the stripes resolve.
function listAt(listPath) {
  const parts = listPath.split('.');
  if (scopes[parts[0]]) {
    const outer = listFor(scopes[parts[0]]);
    return outer.flatMap(o => dig(o, parts.slice(1).join('.')) || []);
  }
  return dig(v, listPath) || [];
}
const memo = {};
function listFor(p) { return memo[p] || (memo[p] = listAt(p)); }
const missing = [];
const holeSrc = markup.replace(/hint-placeholder-val="\{\{[^}]*\}\}"/g, '');
for (const m of holeSrc.matchAll(/\{\{([^}]+)\}\}/g)) {
  const path = m[1].trim(), parts = path.split('.');
  let val;
  if (scopes[parts[0]]) {
    const rest = parts.slice(1).join('.');
    const items = listFor(scopes[parts[0]]);
    ok(items.length > 0, 'the list behind ' + path + ' is not empty');
    val = items.some(it => dig(it, rest) !== undefined) ? true : undefined;
  } else val = dig(v, path);
  if (val === undefined) missing.push(path);
}
eq(missing, [], 'every {{hole}} resolves');
eq((markup.match(/<sc-for/g) || []).length, (markup.match(/<\/sc-for>/g) || []).length, 'sc-for tags balance');
eq((markup.match(/<sc-if/g) || []).length, (markup.match(/<\/sc-if>/g) || []).length, 'sc-if tags balance');
const bodyOnly = markup.split('</helmet>')[1];
ok(!/<style|<script/i.test(bodyOnly), 'no style or script blocks in the board itself');

// Every part of the picture has to answer every hole, or the runtime renders
// the literal word "undefined" into the van.
ok(v.scene.parts.every(p => typeof p.style === 'string' && Array.isArray(p.kids)
  && typeof p.text === 'string' && typeof p.tap === 'function'), 'every scene part is complete');
ok(v.scene.parts.every(p => p.kids.every(k => typeof k.style === 'string')), 'every stripe carries a style');
ok(v.queue.every(q => ['tap', 'side', 'rear', 'reopen'].every(f => typeof q[f] === 'function')),
  'every queue row carries all four handlers whatever state it is in');

// ── the picture stays inside the board ──────────────────────────────────────
// Sheared boxes do not report their painted extent to any layout engine, so
// this is the only thing that catches a stack drawn out through the frame.
function boxOf(style) {
  const wh = /width:([-\d.]+)px;height:([-\d.]+)px/.exec(style);
  const mx = /transform:matrix\(([^)]+)\)/.exec(style);
  const lt = /left:([-\d.]+)(?:px)?;top:([-\d.]+)(?:px)?/.exec(style);
  if (!lt) return null;
  const L = +lt[1], T = +lt[2];
  if (!mx || !wh) return { x: [L, L], y: [T, T] };
  const [a, b, cc, d, e, f] = mx[1].split(',').map(Number);
  const W = +wh[1], H = +wh[2];
  const pts = [[0, 0], [W, 0], [0, H], [W, H]].map(([x, y]) => [a * x + cc * y + e + L, b * x + d * y + f + T]);
  return { x: [Math.min(...pts.map(p => p[0])), Math.max(...pts.map(p => p[0]))],
           y: [Math.min(...pts.map(p => p[1])), Math.max(...pts.map(p => p[1]))] };
}
function sceneBounds(vals) {
  const off = /left:([-\d.]+)px;top:([-\d.]+)px/.exec(vals.scene.box);
  const ox = +off[1], oy = +off[2];
  let x0 = Infinity, x1 = -Infinity, y0 = Infinity, y1 = -Infinity;
  for (const p of vals.scene.parts) {
    const b = boxOf(p.style);
    if (!b) continue;
    x0 = Math.min(x0, b.x[0] + ox); x1 = Math.max(x1, b.x[1] + ox);
    y0 = Math.min(y0, b.y[0] + oy); y1 = Math.max(y1, b.y[1] + oy);
  }
  return { x0, x1, y0, y1 };
}
{
  const b = sceneBounds(v);
  ok(b.x0 >= -1 && b.x1 <= 1441, 'the picture stays inside the board horizontally  ' + Math.round(b.x0) + '..' + Math.round(b.x1));
  ok(b.y0 >= -1 && b.y1 <= 841, 'the picture stays inside the board vertically  ' + Math.round(b.y0) + '..' + Math.round(b.y1));
  ok(b.x1 <= 1440 - 376 - 16 + 1, 'and clear of the queue rail  right edge ' + Math.round(b.x1));
}
// The dock stands inside the picture's bounding box, on the empty pavement wedge
// between the two clusters of spots. Nothing may be drawn into it — and the
// clearance is single digits, so this is checked against the worst board the van
// can produce, not against the demo state.
// The painted corners, not the axis-aligned box. A sheared floor slab's bounding
// box covers most of the picture while the slab itself covers a parallelogram
// nowhere near the dock, so a bbox test here answers the wrong question.
function polyOf(style) {
  const wh = /width:([-\d.]+)px;height:([-\d.]+)px/.exec(style);
  const mx = /transform:matrix\(([^)]+)\)/.exec(style);
  const lt = /left:([-\d.]+)(?:px)?;top:([-\d.]+)(?:px)?/.exec(style);
  if (!lt) return null;
  const L = +lt[1], T = +lt[2];
  if (!mx || !wh) return [[L, T]];
  const [a, b, c, d, e, f] = mx[1].split(',').map(Number);
  const W = +wh[1], H = +wh[2];
  return [[0, 0], [W, 0], [W, H], [0, H]].map(([x, y]) => [a * x + c * y + e + L, b * x + d * y + f + T]);
}
function convexHit(A, B) {
  for (const poly of [A, B]) {
    for (let i = 0; i < poly.length; i++) {
      const p = poly[i], q = poly[(i + 1) % poly.length];
      const ax = -(q[1] - p[1]), ay = q[0] - p[0];
      if (!ax && !ay) continue;
      let a0 = Infinity, a1 = -Infinity, b0 = Infinity, b1 = -Infinity;
      for (const [x, y] of A) { const v = x * ax + y * ay; a0 = Math.min(a0, v); a1 = Math.max(a1, v); }
      for (const [x, y] of B) { const v = x * ax + y * ay; b0 = Math.min(b0, v); b1 = Math.max(b1, v); }
      const eps = 1 * Math.hypot(ax, ay);
      if (a1 <= b0 + eps || b1 <= a0 + eps) return false;
    }
  }
  return true;
}
function dockOverlaps(vals) {
  const D = { x: +/left:(\d+)px/.exec(vals.con.box)[1], y: +/top:(\d+)px/.exec(vals.con.box)[1],
              w: +/width:(\d+)px/.exec(vals.con.box)[1], h: +/height:(\d+)px/.exec(vals.con.box)[1] };
  const off = /left:([-\d.]+)px;top:([-\d.]+)px/.exec(vals.scene.box);
  const ox = +off[1], oy = +off[2], hits = [];
  const rect = [[D.x, D.y], [D.x + D.w, D.y], [D.x + D.w, D.y + D.h], [D.x, D.y + D.h]];
  for (const p of vals.scene.parts) {
    if (/pointer-events:none/.test(p.style) && p.text === '') continue;   // the tether ends on the dock by design
    const poly = polyOf(p.style);
    if (!poly || poly.length < 4) continue;
    if (convexHit(poly.map(([x, y]) => [x + ox, y + oy]), rect)) {
      hits.push(p.text || Math.round(poly[0][0] + ox) + ',' + Math.round(poly[0][1] + oy));
    }
  }
  return hits;
}
{
  const t = new Component({ accent: '#B48EF7' });
  configure({});
  const st = emptyState();
  for (let r = 1; r <= 9; r++) for (const col of ['left', 'right']) st.van['r' + r + '-' + col] = [{ cust: 'OLA', n: 8 }];
  SPOTS.forEach(sp => { st.staged[sp.id] = { cust: 'SVE', n: 8 }; });
  st.van['door-side'] = [{ cust: 'JAT', n: 2 }];
  st.van['door-back'] = [{ cust: 'HIN', n: 2 }];
  t.state = { st: st, focus: 'side-1', target: null, host: null, flash: null, hist: [] };
  const hits = dockOverlaps(t.renderVals());
  eq(hits.slice(0, 4), [], 'a van stacked to the roof with every spot piled high stays out of the dock');
  eq(dockOverlaps(new Component({ accent: '#B48EF7', tier: 2 }).renderVals()).slice(0, 4), [],
    'and so does the planned forecast drawn over an empty one');
}
// A seven-row van has to fit the same frame, and a full one has to fit too.
for (const rows of [7, 9, 11]) {
  const t = new Component({ accent: '#B48EF7', rows: rows });
  const tv = t.renderVals();
  const b = sceneBounds(tv);
  ok(b.x0 >= -1 && b.x1 <= 1441 && b.y0 >= -1 && b.y1 <= 841, rows + ' rows still fits the frame');
}
{ // a van loaded to the roof is the tallest the picture ever gets
  const t = new Component({ accent: '#B48EF7' });
  configure({});
  const st = emptyState();
  for (let r = 1; r <= 9; r++) for (const col of ['left', 'right']) st.van['r' + r + '-' + col] = [{ cust: 'OLA', n: 8 }];
  st.staged['side-1'] = { cust: 'SVE', n: 8 };
  t.state = { st: st, focus: 'side-1', target: null, host: null, hist: [] };
  const b = sceneBounds(t.renderVals());
  ok(b.y0 >= -1 && b.y1 <= 841, 'a van stacked to the roof still fits  ' + Math.round(b.y0) + '..' + Math.round(b.y1));
}

// ── depth order is paint order ──────────────────────────────────────────────
// Nearer stacks have to be emitted later or they are painted over by the rows
// they stand in front of, and the picture reads inside out.
{
  const t = new Component({ accent: '#B48EF7' });
  configure({});
  const st = emptyState();
  for (let r = 1; r <= 9; r++) for (const col of ['left', 'right']) st.van['r' + r + '-' + col] = [{ cust: 'OLA', n: 3 }];
  t.state = { st: st, focus: null, target: null, host: null, hist: [] };
  const scn = t.renderVals().scene;
  // stack tops are the only opaque quads carrying the customer's full colour
  const tops = scn.parts.map((p, i) => ({ i: i, b: boxOf(p.style), s: p.style }))
    .filter(x => x.s.indexOf('rgb(111,191,151)') > -1 && x.s.indexOf('matrix') > -1);
  ok(tops.length === 18, 'every loaded position draws a top face  got ' + tops.length);
  // The invariant is not that each stack sits lower than the last — two cells on
  // the same diagonal tie, and ties are free because they do not overlap. It is
  // that wherever two of them DO overlap on screen, the nearer one was emitted
  // second, so it paints over the one it stands in front of.
  const depth = [];
  for (let r = 0; r < 9; r++) for (let cl = 0; cl < 2; cl++) depth.push(r + cl);
  let inverted = 0;
  for (let i = 0; i < tops.length; i++) for (let j = i + 1; j < tops.length; j++) {
    const a = tops[i].b, b = tops[j].b;
    const overlaps = a.x[0] < b.x[1] - 0.5 && b.x[0] < a.x[1] - 0.5
                  && a.y[0] < b.y[1] - 0.5 && b.y[0] < a.y[1] - 0.5;
    if (overlaps && depth[j] < depth[i]) inverted++;
  }
  eq(inverted, 0, 'where two stacks overlap, the nearer one is painted second');
}

// ── the flow the driver actually taps ───────────────────────────────────────
function fresh() {
  configure({});
  const t = new Component({ accent: '#B48EF7' });
  t.state = { st: emptyState(), focus: null, target: null, host: null, hist: [] };
  return t;
}
const begin = (t, cust, door) => {
  const q = t.renderVals().queue[QUEUE.indexOf(cust)];
  (door === 'side' ? q.side : q.rear)();
};
const bump = (t, n) => { for (let i = 0; i < n; i++) t.renderVals().con.plus(); };
const push = t => t.renderVals().con.push();
const top = t => t.renderVals().con.top();
const done = t => t.renderVals().con.done();

{
  const t = fresh();
  eq(t.renderVals().con.eyebrow, 'NOTHING ON A PACKING SPOT', 'with nothing staged the console asks for a stop');
  begin(t, 'OLA', 'side');
  eq(t.state.focus, 'side-1', 'starting a customer at the side door claims the first side spot');
  eq(t.state.st.staged['side-1'].cust, 'OLA', 'and stages them on it');
  ok(t.renderVals().con.eyebrow.indexOf('SIDE 1') === 0, 'the console picks the spot up');

  // one tap per position, which is the whole point of the rework
  bump(t, 10);
  eq(t.renderVals().con.pushLabel, 'Push in 5 of 10', 'ten crates will not stand in one position, so the push offers the split');
  push(t);
  eq(heightOf(t.state.st, 'r1-left'), 5, 'the first half goes in the deepest position');
  eq(t.state.st.held['r1-right'], 'OLA', 'and the rest of it is held next door');
  eq(t.renderVals().con.pushLabel, 'Push in 5', 'the second tap offers what is left');
  push(t);
  eq(heightOf(t.state.st, 'r1-right'), 5, 'a second tap uses a second position');
  eq(Object.keys(t.state.st.held).filter(k => isEmpty(t.state.st, k)).length, 0, 'and the hold is released once used');
  done(t);
  ok(t.state.st.closed['OLA'], 'Done closes the stop out');
  eq(t.state.focus, null, 'and hands the console back');

  // the rest of the route, the same way
  begin(t, 'JAT', 'side'); bump(t, 5); push(t); done(t);
  begin(t, 'HIN', 'side'); bump(t, 2); push(t); done(t);
  begin(t, 'SVE', 'side'); bump(t, 7); push(t); done(t);
  begin(t, 'FRO', 'side'); bump(t, 4); push(t); done(t);
  eq(heightOf(t.state.st, 'r2-left'), 5, 'Jåtten lands behind Olavstoppen');
  eq(heightOf(t.state.st, 'r3-left'), 7, 'and Sverdrup two rows on');

  // Marlink is three crates against a seven, which the ±3 rule will not have
  begin(t, 'MAR', 'side'); bump(t, 3);
  const con = t.renderVals().con;
  eq(con.pushLabel, 'Push in anyway', 'a thin stack beside a tall one is allowed but never quiet');
  ok(con.topNote !== 'no stack for it', 'and the board offers the stack it could go on instead');
  eq(con.topLabel, '+3 on top', 'a small order goes up whole, not one crate at a time');
  eq(con.topNote, 'R2 · R', 'onto the outermost stack that will legally take it');
  ok(con.showWhy && /wrong crate/.test(con.why), 'and says what mixing a stack costs');
  top(t);
  const layers = t.state.st.van['r2-right'];
  eq(layers.map(l => l.cust), ['HIN', 'MAR'], 'the earlier delivery ends up on top');
  ok(stopOf(layers[1].cust).i < stopOf(layers[0].cust).i, 'which is the only order that comes off cleanly');
  done(t);

  // the whole van, checked against the rule it was built under
  let legal = true;
  for (const col of ['left', 'right']) {
    for (let r = 1; r < 9; r++) {
      const a = t.state.st.van['r' + r + '-' + col], b = t.state.st.van['r' + (r + 1) + '-' + col];
      if (!a.length || !b.length) continue;
      if (Math.abs(heightOf(t.state.st, 'r' + r + '-' + col) - heightOf(t.state.st, 'r' + (r + 1) + '-' + col)) > 3) legal = false;
    }
  }
  ok(legal, 'every neighbouring pair in a column finishes within three of each other');
  eq(cratesIn(t.state.st), 31, 'and every crate of every stop is aboard');
  eq(t.renderVals().con.eyebrow, 'EVERY STOP CLOSED OUT', 'the console says so when there is nothing left');
}

// ── the guards, one at a time ───────────────────────────────────────────────
{
  const t = fresh();
  begin(t, 'JAT', 'side');            // Olavstoppen loads first
  eq(t.renderVals().con.pushLabel, 'Push in anyway', 'loading out of order is amber, not blocked');
  ok(/Olavstoppen/.test(t.renderVals().con.why), 'and names who should have gone in');
}
{
  const t = fresh();
  // fill rows 1–4 so the side door shuts
  for (let r = 1; r <= 4; r++) for (const col of ['left', 'right']) t.state.st.van['r' + r + '-' + col] = [{ cust: 'OLA', n: 4 }];
  const q = t.renderVals().queue[QUEUE.indexOf('SVE')];
  ok(/dashed/.test(q.sideStyle), 'with rows 1–4 full the side button stops looking like the ordinary way in');
  begin(t, 'SVE', 'side'); bump(t, 4);
  // The board used to say "round the back" and offer no way to do it, which
  // stranded whoever was mid-order when rows 1–4 filled.
  const con = t.renderVals().con;
  eq(con.pushLabel, 'Carry round the back', 'the refusal comes with the move that answers it');
  eq(con.pushNote, 'to BACK 1', 'and says where to carry it');
  con.push();
  eq(t.state.st.staged['side-1'], null, 'tapping it gives up the side spot');
  eq(t.state.st.staged['back-1'].n, 4, 'and the four crates already stacked come with them');
  eq(t.state.focus, 'back-1', 'with the console following');
  // (still amber, because nothing has closed Olavstoppen out — but it is a push)
  ok(/^Push in/.test(t.renderVals().con.pushLabel), 'and a push that will now work');
  eq(t.renderVals().con.pushNote, 'R5 · L', 'aimed at the first row the back doors reach');
  eq(t.renderVals().con.topNote, 'no stack for it', 'and the top-up keeps its slot even with no host, so nothing shifts');
}
{
  // With no back spot free either, it is a genuine hard no.
  const t = fresh();
  for (let r = 1; r <= 4; r++) for (const col of ['left', 'right']) t.state.st.van['r' + r + '-' + col] = [{ cust: 'OLA', n: 4 }];
  begin(t, 'FRO', 'back'); begin(t, 'MAR', 'back');
  begin(t, 'SVE', 'side'); bump(t, 4);
  eq(t.renderVals().con.pushLabel, 'Round the back', 'with nowhere to carry it to, it stays a refusal');
  ok(t.renderVals().con.pushStyle.indexOf('247,118,142') > -1, 'in the colour of a hard no');
}
{
  // Carrying round is an answer to a shut door, not to a full van: with every
  // position taken the refusal is the vehicle, and walking a stack round it
  // achieves nothing.
  const t = fresh();
  for (let r = 1; r <= 9; r++) for (const col of ['left', 'right']) t.state.st.van['r' + r + '-' + col] = [{ cust: 'OLA', n: 4 }];
  t.state.st.van['door-side'] = [{ cust: 'JAT', n: 2 }];
  t.state.st.van['door-back'] = [{ cust: 'HIN', n: 2 }];
  begin(t, 'SVE', 'back'); bump(t, 3);
  const con = t.renderVals().con;
  eq(con.pushLabel, 'Van full', 'a full van says so');
  ok(con.pushStyle.indexOf('247,118,142') > -1, 'in red');
  ok(!/Carry/.test(con.pushLabel), 'and never offers to walk it round to a door with no floor behind it');
}
{
  // A push with nothing counted is allowed — it is the fast flow — but it
  // records a question mark, so it is never the green one.
  const t = fresh();
  begin(t, 'OLA', 'side');
  const con = t.renderVals().con;
  eq(con.pushLabel, 'Push in', 'an uncounted push is offered');
  ok(/not counted/.test(con.pushNote), 'and says what it will record');
  ok(con.pushStyle.indexOf('#4FD6A8') < 0, 'without pretending it is the recommended move');
  con.push();
  eq(heightOf(t.state.st, 'r1-left'), null, 'what lands is an unknown, not a zero');
  bump(t, 3);
  ok(t.renderVals().con.pushStyle.indexOf('#4FD6A8') > -1, 'counting it turns the button green');
}
{
  const t = fresh();
  for (let r = 1; r <= 4; r++) for (const col of ['left', 'right']) t.state.st.van['r' + r + '-' + col] = [{ cust: 'OLA', n: 4 }];
  begin(t, 'SVE', 'side'); bump(t, 1);
  const con = t.renderVals().con;
  ok(/side door/i.test(con.pushLabel), 'one crate behind a shut side door belongs in the well');
  ok(con.pushStyle.indexOf('#4FD6A8') > -1, 'and that is a good idea, not a warning');
}
{ // the doorway is offered when the floor runs short, and only then
  const t = fresh();
  for (let r = 1; r <= 9; r++) for (const col of ['left', 'right']) t.state.st.van['r' + r + '-' + col] = [{ cust: 'OLA', n: 4 }];
  begin(t, 'SVE', 'back'); bump(t, 3);
  ok(/doorway/i.test(t.renderVals().con.pushLabel), 'with every position taken the back doorway is the last floor');
}
{ // undo puts the whole board back one tap
  const t = fresh();
  begin(t, 'OLA', 'back'); bump(t, 4);
  const before = JSON.stringify(t.state.st.van);
  push(t);
  ok(JSON.stringify(t.state.st.van) !== before, 'a push changes the van');
  t.renderVals().con.undo();
  eq(JSON.stringify(t.state.st.van), before, 'and Undo puts it back exactly');
}
{ // a closed stop can be reopened, because Done is an assertion and not a fact
  const t = fresh();
  begin(t, 'OLA', 'side'); bump(t, 3); push(t); done(t);
  const row = t.renderVals().queue[QUEUE.indexOf('OLA')];
  ok(row.hasReopen && !row.hasDoors, 'a closed stop offers reopen instead of the two doors');
  row.reopen();
  ok(!t.state.st.closed['OLA'], 'and one tap has it open again');
}
{ // tapping the van picks a position by hand
  const t = fresh();
  begin(t, 'OLA', 'side'); bump(t, 3);
  eq(t.renderVals().con.pushNote, 'R1 · L', 'the board aims at the innermost free position');
  t.pickCell(t.state.st, 'r3-left');
  const con = t.renderVals().con;
  eq(con.pushNote, 'R3 · L', 'tapping an empty position aims there instead');
  ok(/stays free/.test(con.why), 'and says what the gap will cost');
  t.pickCell(t.state.st, 'r3-left');
  eq(t.renderVals().con.pushNote, 'R1 · L', 'tapping it again hands the choice back');
}
{ // a customer already on a spot is focused rather than staged twice
  const t = fresh();
  begin(t, 'OLA', 'side');
  begin(t, 'OLA', 'back');
  eq(SPOTS.filter(s => t.state.st.staged[s.id]).length, 1, 'a stop can only be on one packing spot');
}
{ // the back spots take a customer too, and aim at the rows the back doors reach
  const t = fresh();
  begin(t, 'OLA', 'back');
  eq(t.state.focus, 'back-1', 'the rear button claims a back spot');
  bump(t, 4);
  eq(t.renderVals().con.pushNote, 'R5 · L', 'and the back doors start at the first row they can reach');
}

{
  // The longest reason the board can produce, in the most cramped state, still
  // has to leave the buttons inside the frame.
  const t = fresh();
  for (let r = 1; r <= 9; r++) for (const col of ['left', 'right']) t.state.st.van['r' + r + '-' + col] = [{ cust: 'OLA', n: 5 }];
  begin(t, 'SVE', 'back'); bump(t, 3);
  const con = t.renderVals().con;
  ok(/doorway/i.test(con.pushLabel), 'a full van offers the doorway');
  ok(con.why.length > 80, 'with a reason long enough to wrap');
  const cy = /top:(\d+)px/.exec(con.box), ch = /height:(\d+)px/.exec(con.box);
  ok(cy && ch && +cy[1] + +ch[1] <= 840, 'and the console is a fixed box that cannot grow past the board');
  ok(/line-clamp/.test(con.whyStyle), 'because the reason is clamped rather than allowed to push it');
}

// ── the two counts that cross before anybody notices ────────────────────────
{
  const t = fresh();
  // three stacks at the side door, one position left that it can still reach
  for (let r = 1; r <= 3; r++) for (const col of ['left', 'right']) t.state.st.van['r' + r + '-' + col] = [{ cust: 'OLA', n: 4 }];
  t.state.st.van['r4-left'] = [{ cust: 'OLA', n: 4 }];
  begin(t, 'JAT', 'side'); begin(t, 'HIN', 'side'); begin(t, 'SVE', 'side');
  const w = t.renderVals().warn;
  ok(w.show, 'the board says when the side door will shut on stacks still standing at it');
  ok(/1 position left through the side door and 3 stacks/.test(w.text), 'counting both sides of it');
  ok(/SIDE 2 \(Hinna\) and SIDE 3 \(Sverdrup\)/.test(w.text),
    'and naming the ones that lose, in loading order — not just the shortfall');
  ok(!/\) and SIDE \d+ \(.*\) and /.test(w.text), 'as a list, not "A and B and C"');
  ok(!/SIDE 1/.test(w.text), 'the one that still fits is not flagged');
}
{
  const t = fresh();
  for (let r = 1; r <= 9; r++) t.state.st.van['r' + r + '-left'] = [{ cust: 'OLA', n: 4 }];
  for (let r = 1; r <= 8; r++) t.state.st.van['r' + r + '-right'] = [{ cust: 'OLA', n: 4 }];
  ok(/1 position left and 5 stops with nothing aboard/.test(t.renderVals().warn.text),
    'and says when the van will run out before the route does');
}
{
  eq(fresh().renderVals().warn.show, false, 'an empty van with a whole route ahead of it warns about nothing');
}

// ── the slide ───────────────────────────────────────────────────────────────
// The runtime rebuilds the whole tree on every paint, so a JS-held clone would
// be destroyed ~16ms in. `translate` composes outside the sheared `transform`,
// and a freshly created element starts its animation on its own — so the full
// re-render drives the motion rather than breaking it.
{
  const t = fresh();
  begin(t, 'OLA', 'side'); bump(t, 3);
  eq(t.renderVals().scene.parts.filter(p => /sc-push/.test(p.style)).length, 0, 'nothing animates before the tap');
  push(t);
  const moving = t.renderVals().scene.parts.filter(p => /sc-push/.test(p.style));
  eq(moving.length, 3, 'the three faces of the stack that just landed carry the slide');
  ok(moving.every(p => /--dx:[-\d.]+px;--dy:[-\d.]+px/.test(p.style)), 'each with the offset from the spot it was built on');
  ok(!/transform:translate/.test(moving[0].style), 'and never inside transform, which would fight the shear');
  const dx = +/--dx:([-\d.]+)px/.exec(moving[0].style)[1];
  ok(Math.abs(dx) > 100, 'the offset is a real distance across the picture  ' + Math.round(dx));
  eq(t.renderVals().scene.parts.filter(p => /sc-push/.test(p.style)).length, 0,
    'and it is consumed on the first paint, so no later repaint replays it');
}
{
  // The keyframe it depends on has to exist, and it is the one thing that
  // cannot live in the board's own markup.
  const helmet = markup.split('</helmet>')[0];
  ok(/@keyframes sc-push/.test(helmet), 'the keyframe is declared in the helmet');
  ok(/translate: var\(--dx\) var\(--dy\)/.test(helmet), 'as an independent translate, not a transform');
}

// ── the two numbers that used to lie ────────────────────────────────────────
{
  const t = fresh();
  begin(t, 'OLA', 'side'); push(t); push(t);          // two blind pushes
  const stat = t.renderVals().stats.filter(s => /POSITIONS IN|CRATES IN/.test(s.label))[0];
  eq(stat.label, 'POSITIONS IN', 'with anything aboard uncounted the headline stops claiming a crate count');
  eq(stat.value, '2 · 2 blind', 'and says how much of it is a guess');
  done(t);
  ok(/DONE · 2 positions · uncounted/.test(t.renderVals().queue[QUEUE.indexOf('OLA')].state),
    'the queue row says the same rather than reporting nought crates');
  const t2 = fresh();
  begin(t2, 'OLA', 'side'); bump(t2, 3); push(t2);
  eq(t2.renderVals().stats.filter(s => /CRATES IN/.test(s.label))[0].value, '3',
    'and when everything aboard was counted it is a crate count again');
}
{
  // Depth order is stated on the board, not only guarded at the tap.
  const t = fresh();
  begin(t, 'OLA', 'back'); bump(t, 4); push(t); done(t);
  begin(t, 'JAT', 'side'); bump(t, 4); push(t);
  const w = t.renderVals().warn;
  ok(w.show && /Jåtten Skole at R1 · L is deeper than Olavstoppen at R5 · L/.test(w.text),
    'a load that will cost an unload says so, and keeps saying it');
  ok(/has to come off to reach them/.test(w.text), 'in terms of what it costs to fix');
}

// ── what the picture says about the door ────────────────────────────────────
{
  const t = fresh();
  const open = t.renderVals().scene.parts.filter(p => p.text === 'SIDE DOOR · ROWS 1–4');
  eq(open.length, 1, 'the side door is named on the sill while it is open');
  for (let r = 1; r <= 4; r++) for (const col of ['left', 'right']) t.state.st.van['r' + r + '-' + col] = [{ cust: 'OLA', n: 4 }];
  const shut = t.renderVals().scene.parts.filter(p => p.text === 'SIDE DOOR · SHUT');
  eq(shut.length, 1, 'and says so when it shuts');
}
{ // with counts in hand the empty positions show what belongs there
  const t = new Component({ accent: '#B48EF7', tier: 2 });
  configure({});
  t.state = { st: emptyState(), focus: null, target: null, host: null, hist: [] };
  const ghosts = t.renderVals().scene.parts.filter(p => /122,162,247/.test(p.style));
  ok(ghosts.length > 0, 'tier 2 draws the plan onto the empty van');
  const t1 = fresh();
  eq(t1.renderVals().scene.parts.filter(p => /122,162,247/.test(p.style)).length, 0,
    'and tier 1, which knows no counts, draws none');
}

console.log(fails ? `\n  ${fails} failed of ${checks}` : `passed ${checks}/${checks} checks`);
process.exit(fails ? 1 : 0);
