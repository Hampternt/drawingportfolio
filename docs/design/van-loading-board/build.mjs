// Builds "Van loading board.dc.html" — a design document for the live load
// board, on the Hampter design system.
//
// The boards on this page are not drawings of the screen. They ARE the screen:
// ../sorting-live/src/board.html rendered by ssr.mjs against the value tree
// ../sorting-live/src/board.js produces, from states built by tapping the real
// model in ../sorting-live/src/model.js. A test in that folder checks the
// static render against the browser's own DOM node for node, so a design
// document that disagreed with the prototype would be a build failure rather
// than something to notice later.
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { boardTemplate, render } from './ssr.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const LIVE = join(here, '..', 'sorting-live', 'src');
const read = f => readFileSync(join(LIVE, f), 'utf8');

class DCLogic { constructor(p) { this.props = p || {}; } setState(o) { Object.assign(this.state, o); } }
const M = {};
(new Function('exports', 'DCLogic', read('model.js') + read('board.js') + `
  Object.assign(exports, { Component, VIEW, SCENE, DOCK, CUST, STOPS, QUEUE, ORDER, SPOTS, DOORS,
    ROWS, CAP, SIDE_DOOR_ROWS, THIN, configure, emptyState, pushState, beginState, topUpState,
    stackHosts, depthFaults, windowAt, suggestAt, posLabel, heightOf, stopOf, doAssign, doBump,
    doPush, doClose, doBegin, COUNTS, PALLETS });
`))(M, DCLogic);
M.configure({});
const TPL = boardTemplate(read('board.html'));
const SET = boardTemplate(read('settings.html'));

// ── a board state, built by tapping the real model ──────────────────────────
function board(mutate, props = {}) {
  M.configure({});
  const c = new M.Component(Object.assign({ accent: '#B48EF7' }, props));
  const st = M.emptyState();
  if (mutate) mutate(st);
  const live = M.SPOTS.filter(s => st.staged[s.id])[0];
  c.state = { st, focus: live ? live.id : null, target: null, host: null, flash: null, hist: [] };
  return c;
}
// box-sizing is not border-box on this page the way it is inside the board's own
// helmet, so the frame carries its border explicitly rather than growing by it.
const frame = html => `<div style="box-sizing:border-box;width:1442px;height:842px;flex:none;
  border-radius:14px;overflow:hidden;border:1px solid var(--border-default)">${html}</div>`;
const draw = c => frame(render(TPL, c.renderVals()));
const drawSettings = c => { c.state.screen = 'settings'; const h = frame(render(SET, c.renderVals())); c.state.screen = 'board'; return h; };

// ── tokens, quoted from the design system rather than guessed ───────────────
const T = {
  raised: 'var(--surface-raised)', card: 'var(--surface-card)', void: 'var(--surface-void)',
  border: 'var(--border-default)', hair: 'var(--border-subtle)',
  strong: 'var(--text-strong)', body: 'var(--text-body)', muted: 'var(--text-muted)',
  faint: 'var(--text-faint)', accent: 'var(--accent)', accentText: 'var(--text-accent)',
  ok: 'var(--status-success)', warn: 'var(--status-warning)',
  danger: 'var(--status-danger)', info: 'var(--status-info)',
};
const label = `font:var(--type-label);letter-spacing:var(--tracking-caps);text-transform:uppercase`;
const mono = `font:var(--type-mono);color:${T.accentText}`;
const esc = s => String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
const code = s => `<code style="${mono}">${esc(s)}</code>`;

// ── the states this document is drawn from ──────────────────────────────────
const midState = st => {
  st.van['r1-left'] = [{ cust: 'OLA', n: 3 }];
  st.van['r1-right'] = [{ cust: 'OLA', n: 3 }];
  st.van['r2-left'] = [{ cust: 'OLA', n: 2 }];
  st.van['r2-right'] = [{ cust: 'OLA', n: 2 }];
  st.van['r3-left'] = [{ cust: 'JAT', n: 3 }];
  st.van['r3-right'] = [{ cust: 'JAT', n: 2 }];
  st.closed.OLA = true; st.closed.JAT = true;
  st.staged['side-1'] = { cust: 'HIN', n: 2 };
  st.staged['side-2'] = { cust: 'SVE', n: 4 };
};
const MID = board(midState);
const MID2 = board(midState, { tier: 2 });
// Further on, with the side door shut behind rows 1–4 and the last stop being
// packed at the back.
const LATER = board(st => {
  st.van['r1-left'] = [{ cust: 'OLA', n: 5 }];
  st.van['r1-right'] = [{ cust: 'OLA', n: 5 }];
  st.van['r2-left'] = [{ cust: 'JAT', n: 5 }];
  st.van['r2-right'] = [{ cust: 'HIN', n: 2 }];
  st.van['r3-left'] = [{ cust: 'SVE', n: 4 }];
  st.van['r3-right'] = [{ cust: 'SVE', n: 3 }];
  st.van['r4-left'] = [{ cust: 'FRO', n: 4 }];
  st.van['r4-right'] = [{ cust: 'FRO', n: 4 }];
  st.closed.OLA = true; st.closed.JAT = true; st.closed.HIN = true; st.closed.SVE = true;
  st.staged['back-1'] = { cust: 'MAR', n: 3 };
}, { tier: 3 });

const SETTINGS = board(midState);

// ── the projection, quoted from the code that computes it ───────────────────
const GEO = MID.renderVals().scene.geo;
const n1 = x => (Math.round(x * 10) / 10).toFixed(1);
function projection() {
  const V = M.VIEW;
  const rows = [
    ['u', 'one column, left wall → right wall', `+${V.cx}, +${V.cy}`, `+${n1(GEO.cx)}, +${n1(GEO.cy)}`,
     'right and <em>down</em> — the kerb side is nearer the camera'],
    ['v', 'one row, cab → back doors', `−${V.rx}, +${V.ry}`, `−${n1(GEO.rx)}, +${n1(GEO.ry)}`,
     'left and down — the near end swings away from the corner you stand at'],
    ['w', 'one crate of height', `0, −${V.ch}`, `0, −${n1(GEO.ch)}`, 'straight up, always'],
  ].map(([a, what, authored, scaled, why]) => `<tr>
    <td style="padding:9px 14px 9px 0;border-top:1px solid ${T.hair};font:var(--type-mono);color:${T.accentText}">${a}</td>
    <td style="padding:9px 14px 9px 0;border-top:1px solid ${T.hair};color:${T.body}">${what}</td>
    <td style="padding:9px 14px 9px 0;border-top:1px solid ${T.hair};font:var(--type-mono);color:${T.muted};white-space:nowrap">${authored}</td>
    <td style="padding:9px 14px 9px 0;border-top:1px solid ${T.hair};font:var(--type-mono);color:${T.strong};white-space:nowrap">${scaled}</td>
    <td style="padding:9px 0;border-top:1px solid ${T.hair};color:${T.muted}">${why}</td>
  </tr>`).join('');
  return `<table style="border-collapse:collapse;width:100%;max-width:1180px;font:var(--type-body-sm);text-align:left">
    <thead><tr>${['axis', 'step', 'authored px', `at k = ${n1(GEO.k * 100) / 100}`, 'direction on screen']
      .map(h => `<th style="padding:0 14px 8px 0;${label};color:${T.faint};font-weight:400">${h}</th>`).join('')}</tr></thead>
    <tbody>${rows}</tbody></table>`;
}
function corners() {
  const P = (u, v, w = 0) => [GEO.ox + u * GEO.cx - v * GEO.rx, GEO.oy + u * GEO.cy + v * GEO.ry - (w || 0) * GEO.ch];
  const pts = [
    ['cab, left wall', P(0, 0), 'the top of the picture — the corner furthest from you'],
    ['cab, kerb wall', P(2, 0), 'the far end of the side door'],
    ['back doors, left wall', P(0, M.ROWS), ''],
    ['back doors, kerb wall', P(2, M.ROWS), 'the corner you are standing at'],
    ['an eight-high stack at R1 · L', P(0, 0, M.CAP), 'the tallest the picture ever gets'],
  ].map(([what, [x, y], why]) => `<tr>
    <td style="padding:9px 14px 9px 0;border-top:1px solid ${T.hair};color:${T.body}">${what}</td>
    <td style="padding:9px 14px 9px 0;border-top:1px solid ${T.hair};font:var(--type-mono);color:${T.strong};white-space:nowrap">${Math.round(x)}, ${Math.round(y)}</td>
    <td style="padding:9px 0;border-top:1px solid ${T.hair};color:${T.muted}">${why}</td>
  </tr>`).join('');
  return `<table style="border-collapse:collapse;width:100%;max-width:1180px;font:var(--type-body-sm);text-align:left">
    <tbody>${pts}</tbody></table>`;
}
// How much of a stack the row in front of it takes away, at each legal height
// difference — the number that decides whether this view is usable at all.
function occlusion() {
  const rows = [];
  for (let d = 0; d <= 5; d++) {
    const crest = GEO.ry - d * GEO.ch;
    rows.push(`<tr>
      <td style="padding:8px 14px 8px 0;border-top:1px solid ${T.hair};font:var(--type-mono);color:${d > 3 ? T.danger : T.strong}">${d > 3 ? '(' + d + ')' : d}</td>
      <td style="padding:8px 14px 8px 0;border-top:1px solid ${T.hair};font:var(--type-mono);color:${crest < 6 ? T.danger : T.body}">${n1(crest)}px</td>
      <td style="padding:8px 0;border-top:1px solid ${T.hair};color:${T.muted}">${
        d === 0 ? 'the whole front face of the deeper stack shows'
        : d === 3 ? 'the largest gap the ±3 rule permits, and the crest still clears'
        : d === 4 ? 'only reachable by breaking the rule — and past here it does start to hide'
        : d === 5 ? 'the deeper stack is behind the one in front of it entirely' : ''}</td>
    </tr>`);
  }
  return `<table style="border-collapse:collapse;max-width:760px;font:var(--type-body-sm);text-align:left">
    <thead><tr>${['crates taller', 'crest above the row in front', '']
      .map(h => `<th style="padding:0 14px 8px 0;${label};color:${T.faint};font-weight:400">${h}</th>`).join('')}</tr></thead>
    <tbody>${rows.join('')}</tbody></table>`;
}

// ── the ladder, generated by putting the model in each state ────────────────
function ladder() {
  const full = st => { for (let r = 1; r <= M.ROWS; r++) for (const c of ['left', 'right']) st.van[`r${r}-${c}`] = [{ cust: 'OLA', n: 4 }]; };
  const sideFull = st => { for (let r = 1; r <= M.SIDE_DOOR_ROWS; r++) for (const c of ['left', 'right']) st.van[`r${r}-${c}`] = [{ cust: 'OLA', n: 4 }]; };
  const rows = [
    ['Ready, counted', st => { M.doAssign(st, 'side-1', 'OLA'); M.doBump(st, 'side-1', 4); }],
    ['Ready, nothing counted', st => { M.doAssign(st, 'side-1', 'OLA'); }],
    ['Too tall for the window', st => { M.doAssign(st, 'side-1', 'OLA'); M.doBump(st, 'side-1', 10); }],
    ['Too thin for the window', st => {
      st.van['r1-left'] = [{ cust: 'OLA', n: 8 }]; st.van['r1-right'] = [{ cust: 'OLA', n: 8 }];
      M.doAssign(st, 'side-1', 'JAT'); M.doBump(st, 'side-1', 1);
    }],
    ['The window has closed entirely', st => {
      st.van['r1-left'] = [{ cust: 'OLA', n: 8 }]; st.van['r1-right'] = [{ cust: 'OLA', n: 8 }];
      st.van['r3-left'] = [{ cust: 'HIN', n: 1 }];
      M.doAssign(st, 'side-1', 'JAT'); M.doBump(st, 'side-1', 4);
    }],
    ['Would break depth order', st => {
      M.doBegin(st, 'OLA', 'back'); M.doBump(st, 'back-1', 4); M.doPush(st, 'back-1'); M.doClose(st, 'back-1');
      M.doBegin(st, 'JAT', 'side'); M.doBump(st, 'side-1', 4);
    }, 'side-1'],
    ['Out of order, the stop being skipped is still on a spot', st => {
      M.doAssign(st, 'side-1', 'OLA'); M.doBump(st, 'side-1', 3); M.doPush(st, 'side-1', 3);
      M.doAssign(st, 'side-1', 'OLA'); M.doBump(st, 'side-1', 2);
      M.doAssign(st, 'side-2', 'JAT'); M.doBump(st, 'side-2', 3);
    }, 'side-2'],
    ['Out of order, nothing of theirs has gone in', st => { M.doAssign(st, 'side-1', 'JAT'); M.doBump(st, 'side-1', 3); }],
    ['Position hand-picked', st => { M.doAssign(st, 'side-1', 'OLA'); M.doBump(st, 'side-1', 3); }, 'side-1', 'r3-left'],
    ['Side rows full, and a back spot free', st => { sideFull(st); M.doAssign(st, 'side-1', 'SVE'); M.doBump(st, 'side-1', 4); }],
    ['Side rows full, nowhere to carry it to', st => {
      sideFull(st); M.doAssign(st, 'back-1', 'FRO'); M.doAssign(st, 'back-2', 'MAR');
      M.doAssign(st, 'side-1', 'SVE'); M.doBump(st, 'side-1', 4);
    }, 'side-1'],
    ['Side rows full, one crate', st => { sideFull(st); M.doAssign(st, 'side-1', 'SVE'); M.doBump(st, 'side-1', 1); }],
    ['Every position taken', st => { full(st); M.doAssign(st, 'back-1', 'SVE'); M.doBump(st, 'back-1', 3); }],
    ['Every position and both doorways taken', st => {
      full(st);
      st.van['door-side'] = [{ cust: 'JAT', n: 2 }]; st.van['door-back'] = [{ cust: 'HIN', n: 2 }];
      M.doAssign(st, 'back-1', 'SVE'); M.doBump(st, 'back-1', 3);
    }, 'back-1'],
  ];
  return `<div style="display:flex;flex-direction:column;gap:2px">` + rows.map(([what, mutate, focus, target]) => {
    const c = board(mutate);
    if (focus) c.state.focus = focus;
    if (target) c.state.target = target;
    const con = c.renderVals().con;
    return `<div style="display:flex;gap:18px;align-items:flex-start;padding:14px 0;border-top:1px solid ${T.hair}">
      <span style="width:250px;flex:none;font:600 var(--text-14)/1.35 var(--font-sans);color:${T.body}">${esc(what)}</span>
      <div style="flex:none;background:#0B0910;border-radius:12px;padding:10px">
        <div style="${con.pushStyle}"><span style="${con.pushBig}">${esc(con.pushLabel)}</span><span style="${con.pushSub}">${esc(con.pushNote)}</span></div>
      </div>
      <p style="margin:0;flex:1;font:var(--type-body-sm);color:${T.muted};max-width:60ch;text-wrap:pretty">${esc(con.why) || '<span style="color:' + T.faint + '">— nothing to say</span>'}</p>
    </div>`;
  }).join('') + `</div>`;
}

// ── the two rail buttons, in every state they have ──────────────────────────
function railStates() {
  const cases = [
    ['Waiting, and next in loading order', st => {}, 'OLA'],
    ['Waiting, but somebody loads first', st => {}, 'JAT'],
    ['Already being packed at this door', st => { M.doAssign(st, 'side-1', 'OLA'); }, 'OLA'],
    ['Every spot at this door is holding somebody', st => {
      M.doAssign(st, 'side-1', 'OLA'); M.doAssign(st, 'side-2', 'JAT'); M.doAssign(st, 'side-3', 'HIN');
    }, 'SVE'],
    ['Rows 1–4 full — the door is shut, the well is not', st => {
      for (let r = 1; r <= 4; r++) for (const c of ['left', 'right']) st.van[`r${r}-${c}`] = [{ cust: 'OLA', n: 4 }];
    }, 'SVE'],
    ['Rows 1–4 full and the well taken too', st => {
      for (let r = 1; r <= 4; r++) for (const c of ['left', 'right']) st.van[`r${r}-${c}`] = [{ cust: 'OLA', n: 4 }];
      st.van['door-side'] = [{ cust: 'JAT', n: 2 }];
    }, 'SVE'],
    ['Closed out', st => { M.doAssign(st, 'side-1', 'OLA'); M.doBump(st, 'side-1', 3); M.doPush(st, 'side-1', 3); M.doClose(st, 'side-1'); }, 'OLA'],
  ];
  return `<div style="display:flex;flex-direction:column;gap:2px">` + cases.map(([what, mutate, cust]) => {
    const c = board(mutate);
    const row = c.renderVals().queue[M.QUEUE.indexOf(cust)];
    const bs = M.beginState(c.state.st, cust, 'side');
    const buttons = row.hasDoors
      ? `<div style="${row.sideStyle}">${esc(row.sideLabel)}</div><div style="${row.rearStyle}">${esc(row.rearLabel)}</div>`
      : `<div style="${row.reopenStyle}">reopen</div>`;
    return `<div style="display:flex;gap:18px;align-items:center;padding:14px 0;border-top:1px solid ${T.hair}">
      <span style="width:250px;flex:none;font:600 var(--text-14)/1.35 var(--font-sans);color:${T.body}">${esc(what)}</span>
      <span style="width:76px;flex:none;font:var(--type-mono);color:${T.accentText}">${esc(bs.kind)}</span>
      <div style="flex:none;background:#0B0910;border-radius:12px;padding:10px;display:flex;gap:5px">${buttons}</div>
      <p style="margin:0;flex:1;font:var(--type-body-sm);color:${T.muted};max-width:56ch;text-wrap:pretty">${esc(bs.why) || '<span style="color:' + T.faint + '">— nothing to say</span>'}</p>
    </div>`;
  }).join('') + `</div>`;
}

const list = (items, mark) => `<div style="display:flex;flex-direction:column;gap:2px;max-width:1180px">
${items.map(([t, d, good]) => `<div style="display:flex;gap:14px;align-items:flex-start;padding:12px 0;border-top:1px solid ${T.hair}">
  ${mark ? `<span style="width:20px;flex:none;color:${good ? T.ok : T.warn};font-size:15px">${good ? '✓' : '—'}</span>` : ''}
  <span style="width:270px;flex:none;font:600 var(--text-14)/1.3 var(--font-sans);color:${T.body}">${esc(t)}</span>
  <p style="margin:0;flex:1;font:var(--type-body-sm);color:${T.muted};max-width:64ch;text-wrap:pretty">${d}</p>
</div>`).join('')}
</div>`;

// ── the document ────────────────────────────────────────────────────────────
const section = (id, tag, title, note, body) => `
<section id="${id}" data-screen-label="${esc(tag + ' ' + title)}" style="display:flex;flex-direction:column;gap:14px">
  <div style="display:flex;align-items:baseline;gap:12px;flex-wrap:wrap">
    <span style="${label};color:${T.accent}">${esc(tag)}</span>
    <h2 style="margin:0;font:var(--type-h2);color:${T.strong}">${esc(title)}</h2>
  </div>
  <p style="margin:0;max-width:80ch;font:var(--type-body);color:${T.muted};text-wrap:pretty">${note}</p>
  ${body}
</section>`;

const doc = `<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<script src="./support.js"></script>
</head>
<body>
<x-dc>
<helmet>
<meta name="design_doc_mode" content="canvas">
<script src="./ds-base.js"></script>
<style>
body{margin:0;background:var(--surface-void)}
</style>
</helmet>
<div style="padding:64px;display:flex;flex-direction:column;gap:64px;background:var(--surface-void);font:var(--type-body);color:${T.body}">

<div style="display:flex;flex-direction:column;gap:10px;max-width:82ch">
  <h1 style="margin:0;font:var(--type-h1);color:${T.strong}">Van loading board<span style="color:${T.accent}">.</span></h1>
  <p style="margin:0;color:${T.muted};text-wrap:pretty">The screen a driver holds at the pallet while loading a delivery van, drawn from the van&rsquo;s own rear-right corner. Nine rows by two columns, loaded in reverse delivery order, with a side door that reaches only the first four rows.</p>
  <p style="margin:0;color:${T.faint};text-wrap:pretty">The boards on this page are not drawings of the screen — they are the screen. ${code('ssr.mjs')} renders ${code('sorting-live/src/board.html')} against the value tree ${code('board.js')} produces, from states built by tapping the rule set in ${code('model.js')}, and a test checks that static render against the browser&rsquo;s own DOM node for node. Rebuild with ${code('node build.mjs')}.</p>
</div>

${section('S1', 'S1', 'The board, mid-load', `Route list only: nothing is known until it is tapped in. Olavstoppen and Jåtten are closed out three rows deep, Hinna is built on one packing spot and Sverdrup on the next. <strong style="color:${T.body}">Stack height is drawn as height</strong> — there is no gauge, because the picture is one. The ±3 stability rule is what makes it legible: it keeps neighbouring stacks within three crates of each other, so the van reads as a staircase rather than a wall.`, draw(MID))}

${section('S2', 'S2', 'The same board, with the counts scanned', `Nothing has moved — the only difference from S1 is that a total per customer was read in before the doors opened, so the board can run the live rules forward and say what belongs in the empty positions. A planned stack is a <strong style="color:${T.info}">translucent volume with a face down to the floor</strong>, not a lid hanging in mid-air over the position two rows behind it, and the next position merges the two things it has to say into one chip: <code style="${mono}">NEXT · HIN 2</code>. Same rules, same fill order, same split — a route sorted with a manifest and one sorted blind end up in the same van.`, draw(MID2))}

${section('S3', 'S3', 'Later, with the side door shut', `Rows 1–4 are full, so nothing more can be pushed in that way — it would have to travel past what is already aboard. The sill has gone from amber to red, the still-empty side positions have gone with it, and Marlink is being packed at the back instead. With the pallets read in too, the dock names the one to pull from next.`, draw(LATER))}

${section('S4', 'S4', 'The rules, as a screen', `Every row on here was already a parameter or a constant; the screen is what makes it the driver&rsquo;s rather than mine. The left column is the van — and it will not shrink past what is already loaded, because the reshape would drop those positions and a lost stack is not something to find out about at the stop. The right column is <strong style="color:${T.body}">what the board reaches for first</strong>, which was an if-chain and is now an order you can move. The preview is the live session, not an illustration of one.

<br><br>Two things are deliberately absent, and the last row says so out loud: the fill order, and depth order. Those are what the whole method is for, and a board that let you turn them off would be a board that could quietly load the van backwards.`, drawSettings(SETTINGS))}

${section('S5', 'S5', 'The projection', `A parallel projection from the van&rsquo;s rear-right corner, raised. It is <strong style="color:${T.body}">dimetric rather than true isometric</strong>, and it has to be: at 30° on both axes, nine rows of van is 950px wide and 850 tall before a crate goes in, which does not fit a 1440 × 840 board that also has to hold a queue. Two floor basis vectors and one vertical, scaled by a single factor to fit the box — the projection is linear in that factor, so the picture&rsquo;s bounding box is too, and the fit is one division rather than a search.`,
`<div style="display:flex;flex-direction:column;gap:26px">
  ${projection()}
  <div style="display:flex;flex-direction:column;gap:10px">
    <span style="${label};color:${T.faint}">where that puts things</span>
    ${corners()}
  </div>
  <div style="display:flex;flex-direction:column;gap:10px">
    <span style="${label};color:${T.faint}">occlusion, which is the risk this view runs</span>
    <p style="margin:0;max-width:80ch;font:var(--type-body-sm);color:${T.muted};text-wrap:pretty">A nearer stack stands in front of the one behind it. One crate of height is ${n1(GEO.ch)}px against a row step of ${n1(GEO.ry)}px, so a stack the maximum legal three taller than the one in front still shows a readable crest — and each position&rsquo;s identity chip rides the <em>back</em> edge of its top face, the side furthest from the advancing occluder. A stack that hides its neighbour outright is a stack that broke the rule.</p>
    ${occlusion()}
  </div>
  <div style="display:flex;flex-direction:column;gap:10px">
    <span style="${label};color:${T.faint}">the frame, 1440 × 840</span>
    ${list([
      ['Header — 24, 14', 'Over the top-left corner the diagonal never reaches.'],
      [`Picture — ${M.SCENE.x}, ${M.SCENE.y}, ${M.SCENE.w} × ${M.SCENE.h}`, `Fitted to ${Math.round(GEO.w)} × ${Math.round(GEO.h)} at k = ${n1(GEO.k * 100) / 100}. Height-bound: every pixel of box height converts straight into scale.`],
      [`Dock — ${M.DOCK.x}, ${M.DOCK.y}, ${M.DOCK.w} × ${M.DOCK.h}`, `Inside the picture, standing on the pavement wedge between the two clusters of packing spots — which is where the driver stands with the back doors open. Clearance is single digits, so nothing being drawn into it is asserted against a van stacked to the roof with every spot piled high, using a separating-axis test on the real parallelograms: a bounding box says the floor slab covers the dock when the slab is nowhere near it.`],
      ['Queue rail — right 16, top 14, 376 × 812', 'The hand that is not carrying a crate.'],
    ])}
  </div>
</div>`)}

${section('S6', 'S6', 'What every push button can say', `Amber means <em>allowed, and here is what it costs</em>. Red means <em>the van physically cannot</em>. <strong style="color:${T.body}">Every red state that names an action has a button that performs it</strong> — the board used to say &ldquo;round the back&rdquo; and offer no way to do it, which stranded whoever was mid-order when rows 1–4 filled. These rows are not written out: each one puts the model in a state and prints what the dock then says.`, ladder())}

${section('S7', 'S7', 'Starting a stop — the two rail buttons', `The route runs down the right in loading order, and the only decision the board cannot make is which door you are packing a stop at. Almost nothing here is a hard no; the buttons say which choice is worse and let the driver make it anyway. <strong style="color:${T.body}">Only two states are untappable</strong>, and neither of them is &ldquo;the side door is shut&rdquo; — rows 1–4 filling stops crates being pushed in that way, but the well is still floor, and it is where a single-crate stop belongs.`, railStates())}

${section('S8', 'S8', 'What changed, and what it cost', `The previous board was a plan view in flex bands, with a stack gauge in each cell and a console strip along the bottom.`, list([
  ['Height is the gauge', 'A stack drawn at its real height needs no bar beside it, and eighteen positions stop reading as graph paper. The space/identity toggle went with it: colour and a three-letter code answer “who”, the silhouette answers “how full”.', true],
  ['The spots are where they are', 'Three pads outboard of the kerb flank beside the rows the side door reaches, two on the ground aft of the back doors — and the pile you have built stands on its pad at true crate height, so the stack you made and the stack it is about to become are the same picture.', true],
  ['The controls stand where the driver does', 'One dock on the pavement wedge, tethered by a line to the spot it is driving and bordered in that customer’s colour. The user asked for the push button to be on the packing area; a control that jumps between five pads is a mis-tap generator, so it says which pad it belongs to instead of moving to it.', true],
  ['Depth order is checked', 'Nothing may sit deeper in the van than a stop delivered before it — the whole reason loading runs backwards, and until this revision nothing checked it. Working both doors at once produced a reverse-order load with a solid green button.', true],
  ['Blind is not empty', 'A push with nothing counted records an unknown, and the headline says <code style="' + mono + '">POSITIONS IN · 2 blind</code> rather than reporting nought crates with two stacks aboard.', true],
  ['The proposal is drawn before it is committed', 'A top-up shows the crates themselves standing on the host; a pushed stack slides in from the spot it was built on.', true],
  ['A skipped tap still cannot be detected', 'Nothing here fixes that — the app has no independent view of the van. The route rail is what it was tapped in as, which is honesty rather than a solution.', false],
  ['The rules are the driver’s', 'Stability, what counts as a small order, whether two customers may share a stack, whether a lone crate goes to the well — and the order the board reaches for them in. All of it was a constant.', true],
  ['Portrait has not been redrawn', 'The board is authored landscape and scales rather than reflows. Turning this projection through ninety degrees is a different picture, not the same one rotated.', false],
], true))}

${section('S9', 'S9', 'Notes on the design system', `Three deliberate departures, all because this screen is used standing up in a cold warehouse rather than at a desk.`, list([
  ['Controls are taller than the kit', 'The largest button in the system is ' + code('--control-height-lg') + ' at 42px. The push button here is 238 × 76, because the driver is often gloved and a dropped tap commits a crate to a position. Everything else — colour, type, radius, spacing — comes straight from the tokens.'],
  ['No hover state carries meaning', 'This is a touch screen with no pointer. Anything the kit expresses through ' + code(':hover') + ' has to also be visible at rest.'],
  ['The picture is not a component', 'Floor tiles and stack faces are sheared with ' + code('matrix()') + ' and cannot take type; every readable thing is a separate upright chip positioned at a projected point. Sheared text is unreadable at arm’s length in the rain.'],
]))}

</div>
</x-dc>
</body>
</html>
`;

writeFileSync(join(here, 'Van loading board.dc.html'), doc);
console.log('wrote Van loading board.dc.html —', doc.length, 'bytes');
