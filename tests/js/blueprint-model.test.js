// Checks for the blueprint planner's model, run on plain node by
// ./scripts/verify.sh. It loads the served file, static/blueprint-model.js,
// never a copy of it. Prints one line per failure and a final count line.
const { join } = require('path');
const fs = require('fs');
const M = require(join(__dirname, '..', '..', 'static', 'blueprint-model.js'));
const DEMO = JSON.parse(fs.readFileSync(join(__dirname, '..', '..', 'docs', 'design', 'blueprint-planner', 'demo-plan.json'), 'utf8'));

let fails = 0, checks = 0;
function ok(cond, msg) { checks++; if (!cond) { fails++; console.log('  FAIL  ' + msg); } }
function eq(a, b, msg) { ok(JSON.stringify(a) === JSON.stringify(b), msg + '  got=' + JSON.stringify(a) + ' want=' + JSON.stringify(b)); }
function near(a, b, msg, tol) { ok(Math.abs(a - b) <= (tol || 1e-9), msg + '  got=' + a + ' want=' + b); }

// ── 1. Node size matches the reviewed canvas ───────────────────────────────
// The design canvas (canvas/Plan.dc.html) measured in px at 1u = 16px. The
// model works in u. This is that px formula, kept as the oracle: the two must
// agree on every node, or the app and the design have drifted apart.
function canvasMeasure(name, n, doc) {
  const U = 16, CH = 7.2, CH_TITLE = 7.8, CH_NOTE = 6.6, CH_BADGE = 6.6;
  const outs = Object.entries(n.out || {}), ins = Object.entries(n.in || {});
  const len = ([p, t]) => p.length + 1 + String(t).length;
  const longest = Math.max(0, ...outs.map(len), ...ins.map(len));
  const portPx = 2 + 14 + longest * CH + 14;
  const status = n.status || '';
  const badgePx = status ? 23 + status.length * CH_BADGE : 0;
  const iconPx = n.kind && n.kind !== 'machine' ? 22 : 0;
  const titlePx = 2 + 12 + iconPx + name.length * CH_TITLE + (badgePx ? 8 + badgePx : 0) + 10;
  let footPx = 0;
  if (n.kind === 'blueprint') {
    const sub = doc.graphs[n.graph];
    const foot = sub ? 'graph ' + n.graph + ' · ' + Object.keys(sub.nodes || {}).length + ' nodes' : 'graph ' + (n.graph || '?') + ' · missing';
    footPx = 2 + 12 + foot.length * CH_NOTE + 8 + (14 + 12 * CH_NOTE) + 8;
  }
  const w = Math.max(9 * U, Math.ceil(Math.max(portPx, titlePx, footPx) / U) * U);
  const noteLines = n.note ? M.wrapWords(n.note, Math.floor((w - 24) / CH_NOTE)) : [];
  const rows = Math.max(outs.length + ins.length, 1);
  const raw = 2 + 32 + 8 + rows * 24 + 8 + (noteLines.length ? noteLines.length * 16 + 6 : 0) + (n.kind === 'blueprint' ? 28 : 0);
  return { w: w / U, h: Math.ceil(raw / U), lines: noteLines.length };
}

{
  const all = [];
  Object.keys(DEMO.graphs).forEach((g) => Object.keys(DEMO.graphs[g].nodes).forEach((name) => all.push([name, DEMO.graphs[g].nodes[name], DEMO])));
  const stress = M.stressDoc(40, 60);
  Object.keys(stress.graphs.main.nodes).forEach((name) => all.push([name, stress.graphs.main.nodes[name], stress]));
  // Long names, long types, a missing sub-graph: the edges of the formula.
  const edge = { v: 1, root: 'main', graphs: { main: { nodes: {}, wires: [] } }, layout: {} };
  edge.graphs.main.nodes.a_very_long_machine_name = { kind: 'machine', status: 'building', in: { x: 'T' }, out: {} };
  edge.graphs.main.nodes.wide = { kind: 'machine', in: { payload: 'HashMap<String, Vec<Option<Row>>>' }, out: {} };
  edge.graphs.main.nodes.lost = { kind: 'blueprint', graph: 'nowhere', status: 'idea', in: {}, out: {}, note: 'a note long enough to wrap over more than one line of the node' };
  edge.graphs.main.nodes.bare = { kind: 'machine' };
  Object.keys(edge.graphs.main.nodes).forEach((name) => all.push([name, edge.graphs.main.nodes[name], edge]));

  all.forEach(([name, n, doc]) => {
    const m = M.measure(name, n, doc), c = canvasMeasure(name, n, doc);
    eq([m.w, m.h, m.noteLines.length], [c.w, c.h, c.lines], 'measure(' + name + ') agrees with the canvas');
    ok(Number.isInteger(m.w) && Number.isInteger(m.h), name + ' is a whole number of units');
    ok(m.w >= 9, name + ' is at least 9u wide');
  });
  ok(all.length > 50, 'the size check covered ' + all.length + ' nodes');
}

// ── 2. Ports sit on the node edges, one row each, outputs first ─────────────
{
  const n = DEMO.graphs.main.nodes.router, m = M.measure('router', n, DEMO);
  eq(M.portAt(m, 'out', 'page'), [m.w, 0.0625 + 2 + 0.5 + 0.75], 'the first output is on the right edge of row 0');
  eq(M.portAt(m, 'out', 'ai')[1], 0.0625 + 2 + 0.5 + 2 * 1.5 + 0.75, 'the third output is two rows down');
  eq(M.portAt(m, 'in', 'req'), [0, 0.0625 + 2 + 0.5 + 3 * 1.5 + 0.75], 'the input follows the outputs, on the left edge');
  eq(M.portAt(m, 'in', 'nope'), null, 'an unknown port has no position');
  eq(M.portAt(m, 'in', 'page'), null, 'an output name asked for as an input has no position');
  const geo = M.geometry(DEMO, 'main');
  eq(M.portXY(geo, 'router', 'in', 'req'), [13, 5 + 0.0625 + 2 + 0.5 + 3 * 1.5 + 0.75], 'portXY adds the layout position');
  // Every port of every demo node lies inside the node's height.
  Object.keys(geo).forEach((name) => M.portList(geo[name].m).forEach((p) => {
    const y = M.portAt(geo[name].m, p.side, p.port)[1];
    ok(y > 2 && y < geo[name].m.h, name + '.' + p.port + ' is inside its node');
  }));
}

// ── 3. Validation: dangling wires and type mismatches ───────────────────────
{
  eq(M.summarize(DEMO, 'main'), { nodes: 8, wires: 12, dangling: 1, mismatch: 1 }, 'the demo plan counts');
  eq(M.checkGraph(DEMO, 'main').filter((w) => w.mismatch).map((w) => [w.a, w.b, w.ft, w.tt]),
    [['router.ai', 'token_gate.bearer', 'Request', 'Header']], 'its one mismatch: a Request into a Header');
  const checks = M.checkGraph(DEMO, 'main');
  const d = checks.filter((w) => w.dangling);
  eq(d.map((w) => [w.a, w.b, w.why]), [['exporter.outline', 'claude.context', 'no node "claude"']], 'the one dangling wire, and why');
  ok(d[0].fromOk && !d[0].toOk, 'its source end still resolves, so a stub can be drawn');

  const doc = M.clone(DEMO);
  doc.graphs.main.nodes.render.in.rows = 'vec < post >';
  ok(!M.checkGraph(doc, 'main').some((w) => w.b === 'render.rows' && w.mismatch), 'case and spaces are folded before compare');
  doc.graphs.main.nodes.render.in.rows = 'Vec<Clean>';
  eq(M.checkGraph(doc, 'main').filter((w) => w.mismatch).map((w) => w.b), ['render.rows', 'token_gate.bearer'], 'a different type is a mismatch');
  doc.graphs.main.wires.push(['ghost.x', 'render.view'], ['handler.nope', 'render.view'], ['handler.view', 'render.nope']);
  eq(M.checkGraph(doc, 'main').slice(-3).map((w) => w.why), ['no node "ghost"', 'no out port "handler.nope"', 'no in port "render.nope"'], 'each dangling reason');
  eq(M.summarize(doc, 'main').dangling, 4, 'and they are counted, not dropped');
  eq(M.summarize(DEMO, 'nothing'), { nodes: 0, wires: 0, dangling: 0, mismatch: 0 }, 'an unknown graph counts as empty');
}

// ── 4. Colour comes from the folded type label ──────────────────────────────
{
  eq(M.hueOf('Vec<Row>'), M.hueOf('vec < row >'), 'two labels that count as one type share a hue');
  ok(M.hueOf('Request') !== M.hueOf('Viewer'), 'different types usually differ');
  for (let i = 0; i < 500; i++) {
    const h = M.hueOf('T' + i);
    if (h < 30 || h > 329) { ok(false, 'hue ' + h + ' for T' + i + ' is in the rose band'); break; }
  }
  ok(true, 'no hue lands in the rose band kept for mismatches');
  eq(M.fnv1a(''), 0x811c9dc5, 'fnv1a of the empty string is the offset basis');
  eq(M.fnv1a('a'), 0xe40c292c, 'fnv1a("a") is the published value');
}

// ── 5. Wire curves ──────────────────────────────────────────────────────────
{
  const f = M.wireCurve(0, 0, 20, 4);
  eq([f.c1x, f.c1y, f.c2x, f.c2y], [10, 0, 10, 4], 'forward: c = dx/2, tangents horizontal');
  const s = M.wireCurve(0, 0, 2, 0);
  eq([s.c1x, s.c2x], [3, -1], 'a short forward wire still bows out at least 3u');
  const b = M.wireCurve(10, 0, 0, 5);
  eq([b.c1x, b.c2x], [18, -8], 'backward: c = max(6u, |dx|/2 + 3u), so it loops out of the node');
  eq(M.pathD(f), 'M0 0C10 0 10 4 20 4', 'the path string');
  eq(M.curveMid(f), [10, 2], 'the midpoint, for a mismatch label');
}

// ── 6. Screen and world, at every zoom the spike promises ───────────────────
{
  const unit = 16;
  [0.2, 0.3, 1, 3].forEach((z) => {
    const view = { x: 123.5, y: -40, z: z };
    const s = M.toScreen(view, unit, 7.25, 3);
    const w = M.toWorld(view, unit, s[0], s[1]);
    near(w[0], 7.25, 'world → screen → world round-trips x at zoom ' + z, 1e-9);
    near(w[1], 3, 'and y', 1e-9);
  });
  // Zooming at the pointer keeps the world point under it where it was.
  const v0 = { x: 50, y: 20, z: 1 };
  const before = M.toWorld(v0, unit, 400, 300);
  [0.3, 3, 10, 0.01].forEach((z) => {
    const v1 = M.zoomAt(v0, unit, 400, 300, z);
    const after = M.toWorld(v1, unit, 400, 300);
    near(after[0], before[0], 'zoomAt(' + z + ') pins the point under the pointer (x)', 1e-9);
    near(after[1], before[1], 'zoomAt(' + z + ') pins it (y)', 1e-9);
    ok(v1.z >= M.ZOOM_MIN && v1.z <= M.ZOOM_MAX, 'zoomAt clamps ' + z + ' to the range');
  });
  // Fit centres the plan and never zooms past 100%.
  const geo = M.geometry(DEMO, 'main'), b = M.bounds(geo);
  const fv = M.fitView(b, unit, 1200, 700);
  ok(fv.z <= 1 && fv.z >= M.ZOOM_MIN, 'fit stays in range');
  const tl = M.toScreen(fv, unit, b.x1, b.y1), br = M.toScreen(fv, unit, b.x2, b.y2);
  ok(tl[0] >= 31.9 && tl[1] >= 31.9 && br[0] <= 1200 - 31.9 && br[1] <= 700 - 31.9, 'fit keeps a 2u margin all round');
  near((tl[0] + br[0]) / 2, 600, 'fit centres horizontally', 1e-6);
  eq(M.fitView(b, unit, 100000, 100000).z, 1, 'a small plan in a big viewport fits at 100%');
  eq(M.fitView(null, unit, 800, 600), { x: 32, y: 32, z: 1 }, 'an empty plan fits at 100%');
}

// ── 7. Drops land on ports at zoom 0.3 and 3 ────────────────────────────────
// The S0 promise. The hit radius is fixed on screen, so the test drops the
// pointer a fixed number of *screen* pixels away from the dot and checks it
// still lands, then checks it misses just outside the radius.
{
  const unit = 16, geo = M.geometry(DEMO, 'main');
  const target = M.portXY(geo, 'render', 'in', 'view');
  [0.3, 1, 3].forEach((z) => {
    const r = M.HIT_SCREEN * unit; // screen px
    const inside = (r * 0.9) / (unit * z), outside = (r * 1.1) / (unit * z);
    const hit = M.nearestPort(geo, target[0] - inside, target[1], z, 'in', 'handler');
    eq(hit && [hit.node, hit.port], ['render', 'view'], 'a drop 0.9 radius off render.view lands at zoom ' + z);
    const miss = M.nearestPort(geo, target[0] - outside, target[1], z, 'in', 'handler');
    ok(!miss || miss.node !== 'render' || miss.port !== 'view', 'a drop 1.1 radius off misses at zoom ' + z);
  });
  // At 0.3 the rows are 7.2px apart on screen, so neighbours overlap: the
  // nearest port has to win.
  const rows = M.portXY(geo, 'render', 'in', 'rows');
  const between = M.nearestPort(geo, rows[0], rows[1] + 0.2, 0.3, 'in', 'db');
  eq(between && between.port, 'rows', 'with overlapping targets the nearest port wins');
  eq(M.nearestPort(geo, rows[0], rows[1], 1, 'out', null), null, 'an input is never offered when an output is wanted');
  eq(M.nearestPort(geo, rows[0], rows[1], 1, 'in', 'render'), null, 'the start node is skipped');
}

// ── 8. Box select and keyboard navigation ───────────────────────────────────
{
  const geo = M.geometry(DEMO, 'main');
  eq(M.nodesInRect(geo, 0, 0, 30, 30).sort(), ['nginx', 'router', 'session', 'token_gate'], 'a box over the left of the plan');
  eq(M.nodesInRect(geo, 30, 30, 0, 0).sort(), ['nginx', 'router', 'session', 'token_gate'], 'dragged up and left, the same box');
  eq(M.nodesInRect(geo, 200, 200, 210, 210), [], 'an empty box selects nothing');
  eq(M.readingOrder(geo), ['nginx', 'router', 'session', 'token_gate', 'handler', 'db', 'render', 'exporter'], 'Tab order: by x, then y, then name');
  eq(M.nearestInDirection(geo, 'router', 'right'), 'session', 'Ctrl + → from router');
  eq(M.nearestInDirection(geo, 'session', 'down'), 'token_gate', 'Ctrl + ↓ from session');
  eq(M.nearestInDirection(geo, 'nginx', 'left'), null, 'nothing left of the leftmost machine');
  eq(M.portList(geo.router.m).map((p) => p.side + ':' + p.port), ['out:page', 'out:api', 'out:ai', 'in:req'], '] walks outputs first');
}

// ── 9. Edits ────────────────────────────────────────────────────────────────
{
  const doc = M.clone(DEMO);
  eq(M.connect(doc, 'main', { node: 'render', side: 'in', port: 'rows' }, { node: 'db', side: 'out', port: 'rows' }), null, 'an existing wire is not doubled, from either end');
  eq(M.connect(doc, 'main', { node: 'render', side: 'in', port: 'view' }, { node: 'nginx', side: 'out', port: 'static' }), ['nginx.static', 'render.view'], 'dragging from an input stores [out, in]');
  eq(M.connect(doc, 'main', { node: 'db', side: 'out', port: 'rows' }, { node: 'db', side: 'in', port: 'query' }), null, 'a machine is not wired to itself');
  eq(M.connect(doc, 'main', { node: 'db', side: 'out', port: 'rows' }, { node: 'nginx', side: 'out', port: 'req' }), null, 'two outputs are not a wire');
  eq(M.summarize(doc, 'main').mismatch, 2, 'fan-in and a mismatch are both allowed, and the mismatch counts');

  M.moveNodes(doc, 'main', ['nginx', 'router'], 2, -1);
  eq([doc.layout['main/nginx'], doc.layout['main/router']], [[3, 5], [15, 4]], 'moveNodes shifts each by whole units');
  eq(DEMO.layout['main/nginx'], [1, 6], 'clone() kept the original untouched');

  const name = M.addNode(doc, 'main', 'nginx', [4.4, 7.6], { kind: 'machine', status: 'idea', in: {}, out: {} });
  eq([name, doc.layout['main/nginx_2']], ['nginx_2', [4, 8]], 'addNode picks a free name and snaps to the grid');

  M.removeNodes(doc, 'main', ['render']);
  ok(!doc.graphs.main.nodes.render && !doc.layout['main/render'], 'removeNodes drops the node and its position');
  ok(!doc.graphs.main.wires.some((w) => w[0].startsWith('render.') || w[1].startsWith('render.')), 'and every wire on it');
  ok(doc.graphs.main.wires.some((w) => w[1] === 'claude.context'), 'but leaves an unrelated dangling wire alone');
  const before = doc.graphs.main.wires.length;
  M.removeWire(doc, 'main', 0);
  eq(doc.graphs.main.wires.length, before - 1, 'removeWire drops one wire');
}

// ── 10. The demo plan's layout ──────────────────────────────────────────────
// Node widths grew when the review made them count the badge and footer, and
// db ended up on top of exporter. Nothing else would notice that again.
{
  const geo = M.geometry(DEMO, 'main'), keys = Object.keys(geo), hits = [];
  for (let i = 0; i < keys.length; i++) for (let j = i + 1; j < keys.length; j++) {
    const a = geo[keys[i]], b = geo[keys[j]], gap = 0.375; // the blueprint stack's offset
    if (a.x < b.x + b.m.w + gap && b.x < a.x + a.m.w + gap && a.y < b.y + b.m.h + gap && b.y < a.y + a.m.h + gap) hits.push(keys[i] + '/' + keys[j]);
  }
  eq(hits, [], 'no two demo machines overlap');
}

// ── 11. The load test plan ──────────────────────────────────────────────────
{
  const doc = M.stressDoc(200, 400);
  const s = M.summarize(doc, 'main');
  eq([s.nodes, s.wires, s.dangling, s.mismatch], [200, 400, 0, 0], '200 machines, 400 wires, none dangling or mismatched');
  eq(JSON.stringify(M.stressDoc(200, 400)), JSON.stringify(doc), 'the same plan on every load');
  const geo = M.geometry(doc, 'main');
  const keys = Object.keys(geo);
  let overlap = 0;
  for (let i = 0; i < keys.length; i++) for (let j = i + 1; j < keys.length; j++) {
    const a = geo[keys[i]], b = geo[keys[j]];
    if (a.x < b.x + b.m.w && b.x < a.x + a.m.w && a.y < b.y + b.m.h && b.y < a.y + a.m.h) overlap++;
  }
  eq(overlap, 0, 'no two machines overlap');
}

console.log((fails ? fails + ' FAILED, ' : '') + checks + ' checks' + (fails ? '' : ', all passed'));
process.exitCode = fails ? 1 : 0;
