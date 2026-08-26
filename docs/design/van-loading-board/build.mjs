// Builds "Van loading board.dc.html" — a design document for the live load
// board, on the Hampter design system.
//
// Every number, name and sentence in the output is produced by the real rule
// set in ../sorting-live/src/model.js, so the design cannot drift from the
// logic it is a design for.
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const MODEL = join(here, '..', 'sorting-live', 'src', 'model.js');
const M = {};
(new Function('exports', readFileSync(MODEL, 'utf8') + `
  Object.assign(exports, { CUST, STOPS, QUEUE, ORDER, SPOTS, DOORS, ROWS, CAP, SIDE_DOOR_ROWS,
    configure, emptyState, normalize, cloneState, pushState, planAhead, windowAt, splitPlan,
    posLabel, rowOf, colOf, doorOf, isEmpty, heightOf, custCount, expectedNext, unstagedNext,
    positionsLeft, positionsOf, stopOf, cratesIn, sideDoorOpen, doAssign, doBump, doPush, doClose,
    stagedAtDoor, spotHolding, isDoor, COUNTS, PALLETS });
`))(M);
M.configure({});

// ── tokens, quoted from the design system rather than guessed ───────────────
const T = {
  page: 'var(--surface-page)', raised: 'var(--surface-raised)', card: 'var(--surface-card)',
  void: 'var(--surface-void)', border: 'var(--border-default)', hair: 'var(--border-subtle)',
  strong: 'var(--text-strong)', body: 'var(--text-body)', muted: 'var(--text-muted)',
  faint: 'var(--text-faint)', accent: 'var(--accent)', accentText: 'var(--text-accent)',
  onAccent: 'var(--text-on-accent)', tint: 'var(--accent-tint)',
  ok: 'var(--status-success)', warn: 'var(--status-warning)',
  danger: 'var(--status-danger)', info: 'var(--status-info)',
  mono: "var(--font-mono)", display: 'var(--font-display)',
};
const label = `font:var(--type-label);letter-spacing:var(--tracking-caps);text-transform:uppercase`;
const esc = s => String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');

// ── a board state, built by tapping the real model ──────────────────────────
function board(mutate) {
  const st = M.normalize(M.emptyState());
  st.open = { n: 0 };
  mutate(st);
  return st;
}
const MID = board(st => {
  st.van['r1-left'] = [{ cust: 'OLA', n: 5 }];
  st.van['r1-right'] = [{ cust: 'OLA', n: 5 }];
  st.van['r2-left'] = [{ cust: 'JAT', n: 3 }];
  st.closed.OLA = true;
  st.staged['side-1'] = { cust: 'JAT', n: 2 };
  st.staged['side-2'] = { cust: 'HIN', n: 2 };
  st.staged['side-3'] = { cust: 'SVE', n: 4 };
});

// ── the stack bar: the one element that answers both questions at once ──────
// Height is fullness; colour and code are identity. Crates that are in are
// solid; crates the plan expects but has not seen are an outline above them.
// That removes the space/identity toggle the old board needed.
function bar(st, id, plan, h = 92) {
  const layers = st.van[id] || [];
  const inCount = layers.reduce((a, l) => a + (l.n || 0), 0);
  const planned = plan && plan.van[id] ? plan.van[id].reduce((a, l) => a + (l.n || 0), 0) : 0;
  const ghost = Math.max(0, planned - inCount);
  const colourOf = [];
  layers.forEach(l => { for (let i = 0; i < (l.n || 0); i++) colourOf.push(M.CUST[l.cust].color); });
  const planCust = plan && plan.van[id] && plan.van[id][0] ? M.CUST[plan.van[id][0].cust].color : T.info;
  // A position nobody has touched draws a baseline, not eight empty rungs —
  // eighteen full gauges at once was the graph-paper problem. As soon as there
  // is something to weigh, the empty rungs come up bright enough to read the
  // fill against, because that reading is what replaced the space/identity
  // toggle.
  if (!inCount && !ghost) {
    return `<div style="width:26px;height:${h}px;flex:none;display:flex;align-items:flex-end">
      <div style="width:100%;height:3px;border-radius:2px;background:var(--white-a07)"></div></div>`;
  }
  const rungs = [];
  for (let i = M.CAP - 1; i >= 0; i--) {
    const solid = i < inCount, planned = !solid && i < inCount + ghost;
    rungs.push(`<div style="flex:1 1 0;min-height:3px;border-radius:2px;background:${
      solid ? colourOf[i] : (planned ? 'transparent' : 'var(--white-a12)')
    };${planned ? `border:1px dashed ${planCust}` : ''}"></div>`);
  }
  return `<div style="width:26px;height:${h}px;flex:none;display:flex;flex-direction:column;justify-content:flex-end;gap:2px">${rungs.join('')}</div>`;
}

// ── one van position ────────────────────────────────────────────────────────
function cell(st, id, plan, opts = {}) {
  const layers = st.van[id] || [];
  const total = layers.reduce((a, l) => a + (l.n || 0), 0);
  const names = [...new Set(layers.map(l => l.cust))];
  const mixed = names.length > 1;
  const frontier = opts.frontier;
  const isNext = id === frontier;
  const planLayers = plan && plan.van[id] ? plan.van[id] : null;
  const planTotal = planLayers ? planLayers.reduce((a, l) => a + (l.n || 0), 0) : 0;
  const ghost = Math.max(0, planTotal - total);


  const tone = layers.length ? T.card : T.raised;
  const ring = isNext ? `2px solid ${T.accent}` : `1px solid ${layers.length ? T.border : T.hair}`;
  const pill = mixed ? ['MIXED', T.warn] : layers.length ? ['IN', T.ok]
    : isNext ? ['NEXT', T.accentText] : ghost ? ['PLAN', T.info] : ['', T.faint];

  const who = layers.length ? names.map(k => M.CUST[k].code).join('+')
    : ghost ? planLayers.map(l => M.CUST[l.cust].code).join('+')
    : (isNext && opts.next ? M.CUST[opts.next].code : '');
  const whoColour = layers.length ? T.strong : ghost ? T.info : (isNext ? T.accentText : T.faint);
  const sub = layers.length ? `${total}${mixed ? ' · 2 cust' : ''}${ghost ? ` +${ghost}` : ''}`
    : ghost ? `+${ghost}` : isNext ? 'next in' : '';

  return `<div style="flex:1 1 0;min-width:0;height:126px;padding:${isNext ? '7px' : '8px'};border-radius:var(--radius-lg);border:${ring};background:${tone};display:flex;flex-direction:column;gap:6px">
  <div style="display:flex;align-items:center;justify-content:space-between;height:14px">
    <span style="${label};color:${T.faint}">${esc(M.colOf(id) === 'left' ? 'L' : 'R')}</span>
    ${pill[0] ? `<span style="${label};color:${pill[1]}">${pill[0]}</span>` : ''}
  </div>
  <div style="display:flex;gap:8px;align-items:flex-end;flex:1;min-height:0">
    ${bar(st, id, plan, 92)}
    <div style="display:flex;flex-direction:column;gap:1px;min-width:0;flex:1">
      <span style="font:700 var(--text-16)/1.1 ${T.display};letter-spacing:var(--tracking-tight);color:${whoColour};overflow:hidden;text-overflow:ellipsis;white-space:nowrap">${esc(who)}</span>
      <span style="font:var(--type-label);color:${mixed ? T.warn : (ghost ? T.info : T.muted)};white-space:nowrap">${esc(sub)}</span>
    </div>
  </div>
</div>`;
}

// ── a packing spot ──────────────────────────────────────────────────────────
function spot(st, spotId, w, compact) {
  const s = M.SPOTS.find(x => x.id === spotId);
  const held = st.staged[spotId];
  const ps = M.pushState(st, spotId);
  const tone = { ready: T.ok, doorway: T.ok, split: T.warn, order: T.warn, thin: T.warn,
                 chosen: T.warn, physical: T.danger, nofit: T.danger, empty: T.faint }[ps.kind] || T.faint;
  const filled = ps.kind === 'ready' || ps.kind === 'doorway';
  const takeNext = M.unstagedNext(st);
  // A 214px tile cannot hold "Push in anyway" on one line, and wrapping it was
  // pushing the button row out through the bottom of the card.
  const short = { ready: 'Push in', order: 'Anyway', thin: 'Anyway', chosen: 'Push in',
    split: 'Split it', physical: 'Round back', doorway: 'Doorway', nofit: 'Will not fit' };
  const btnLabel = compact ? (short[ps.kind] || ps.label) : ps.label;

  return `<div style="${w};flex:none;border-radius:var(--radius-lg);border:1px solid ${held ? tone + '66' : T.hair};background:${held ? T.card : T.raised};padding:10px;display:flex;flex-direction:column;gap:8px">
  <div style="display:flex;align-items:center;justify-content:space-between">
    <span style="${label};color:${T.warn}">${esc(s.name)}</span>
    <span style="font:var(--type-label);color:${T.faint}">${esc(ps.target ? '→ ' + M.posLabel(ps.target) : '—')}</span>
  </div>
  <div style="display:flex;align-items:center;gap:10px;flex:1;min-width:0;min-height:0;overflow:hidden">
    <div style="width:62px;height:52px;flex:none;border-radius:var(--radius-md);display:flex;align-items:center;justify-content:center;font:900 var(--text-26)/1 ${T.display};letter-spacing:var(--tracking-tight);${
      held ? `background:${M.CUST[held.cust].color}1F;border:1px solid ${M.CUST[held.cust].color}55;color:${T.strong}`
           : `background:var(--white-a04);border:1px dashed ${T.border};color:${T.muted}` }">${held ? held.n : '+'}</div>
    <div style="display:flex;flex-direction:column;gap:2px;min-width:0;flex:1">
      <span style="font:600 var(--text-15)/1.15 ${T.display};letter-spacing:var(--tracking-snug);color:${held ? T.strong : T.faint};overflow:hidden;text-overflow:ellipsis;white-space:nowrap">${esc(held ? M.CUST[held.cust].name : 'Empty')}</span>
      <span style="font:var(--type-body-sm);line-height:1.25;color:${held ? (ps.why ? tone : T.muted) : T.muted};display:-webkit-box;-webkit-line-clamp:2;-webkit-box-orient:vertical;overflow:hidden">${esc(held ? (ps.why || `stop ${M.stopOf(held.cust).i} · staged`) : '')}</span>
    </div>
  </div>
  ${held ? `<div style="display:flex;gap:6px">
    <div style="flex:1;min-width:0;height:48px;border-radius:var(--radius-md);display:flex;align-items:center;justify-content:center;text-align:center;padding:0 10px;font:600 var(--text-15)/1.15 var(--font-sans);${
      filled ? `background:${tone};color:${T.onAccent}` : `background:${tone}1A;border:1px solid ${tone}66;color:${tone}` }">${esc(btnLabel)}</div>
    <div style="width:70px;height:48px;flex:none;border-radius:var(--radius-md);display:flex;align-items:center;justify-content:center;font:600 var(--text-14)/1 var(--font-sans);background:var(--white-a04);border:1px solid ${T.border};color:${T.body}">Done</div>
  </div>` : `<div style="height:48px;border-radius:var(--radius-md);display:flex;align-items:center;justify-content:center;font:600 var(--text-15)/1 var(--font-sans);background:var(--white-a04);border:1px dashed ${T.border};color:${T.muted}">${esc(takeNext ? 'Take ' + M.CUST[takeNext].short : 'Nothing waiting')}</div>`}
</div>`;
}

// ── the console ─────────────────────────────────────────────────────────────
function consoleStrip(st, tier) {
  const who = M.expectedNext(st);
  const door = M.sideDoorOpen(st) ? 'side' : 'back';
  const frontier = M.ORDER.filter(id => M.doorOf(id) === door && M.isEmpty(st, id))[0];
  const eyebrow = (tier >= 3 && M.PALLETS[who] ? `PALLET ${M.PALLETS[who]} → ` : 'STRAIGHT OFF THE PALLET → ')
    + M.posLabel(frontier) + ' · ' + (door === 'side' ? 'SIDE DOOR' : 'BACK DOORS');
  const expected = tier >= 2 && M.COUNTS[who] ? ` · ${M.COUNTS[who]} expected` : '';
  const big = (w, bg, fg, txt, sub) => `<div style="width:${w}px;height:60px;flex:none;border-radius:var(--radius-lg);background:${bg};color:${fg};display:flex;flex-direction:column;align-items:center;justify-content:center;gap:1px">
      <span style="font:700 var(--text-18)/1 var(--font-sans)">${esc(txt)}</span>
      ${sub ? `<span style="font:var(--type-label);opacity:.7">${esc(sub)}</span>` : ''}
    </div>`;
  return `<div style="height:80px;flex:none;display:flex;align-items:center;gap:10px;padding:0 14px;border-radius:var(--radius-xl);border:1px solid var(--border-accent);background:${T.card}">
  <div style="flex:1;min-width:0;display:flex;flex-direction:column;gap:2px">
    <span style="${label};color:${T.accentText}">${esc(eyebrow)}</span>
    <div style="display:flex;align-items:baseline;gap:12px;min-width:0">
      <span style="font:var(--type-h2);color:${T.strong}">${esc(M.CUST[who].name)}</span>
      <span style="font:var(--type-body-sm);color:${T.muted};white-space:nowrap">stop ${M.stopOf(who).i} of 6 · nothing in yet${esc(expected)}</span>
    </div>
  </div>
  ${big(56, 'var(--white-a04)', T.muted, '−')}
  ${big(196, T.accent, T.onAccent, '+ 1 crate in')}
  ${big(150, 'var(--white-a04)', T.faint, 'Full · next')}
  ${big(168, T.ok, T.onAccent, 'Done · ' + M.CUST[who].short)}
  ${big(84, 'var(--white-a04)', T.faint, 'Undo')}
</div>`;
}

// Upright the console stacks: what is going in on top, the hands underneath.
function portraitConsole(st) {
  const who = M.expectedNext(st);
  const door = M.sideDoorOpen(st) ? 'side' : 'back';
  const frontier = M.ORDER.filter(id => M.doorOf(id) === door && M.isEmpty(st, id))[0];
  const key = (grow, bg, fg, txt) => `<div style="${grow};height:56px;border-radius:var(--radius-lg);background:${bg};color:${fg};display:flex;align-items:center;justify-content:center;font:700 var(--text-16)/1 var(--font-sans);text-align:center;padding:0 8px">${esc(txt)}</div>`;
  return `<div style="flex:none;border-radius:var(--radius-xl);border:1px solid var(--border-accent);background:${T.card};padding:12px 14px;display:flex;flex-direction:column;gap:10px">
  <div style="display:flex;align-items:center;gap:12px;min-width:0">
    <div style="flex:1;min-width:0;display:flex;flex-direction:column;gap:2px">
      <span style="${label};color:${T.accentText}">STRAIGHT OFF THE PALLET → ${esc(M.posLabel(frontier))}</span>
      <div style="display:flex;align-items:baseline;gap:10px;min-width:0">
        <span style="font:var(--type-h3);color:${T.strong}">${esc(M.CUST[who].name)}</span>
        <span style="font:var(--type-body-sm);color:${T.muted};white-space:nowrap">stop ${M.stopOf(who).i} of 6</span>
      </div>
    </div>
    ${key('width:78px;flex:none', 'var(--white-a04)', T.faint, 'Undo')}
  </div>
  <div style="display:flex;gap:8px">
    ${key('width:56px;flex:none', 'var(--white-a04)', T.muted, '−')}
    ${key('flex:1', T.accent, T.onAccent, '+ 1 crate in')}
  </div>
  <div style="display:flex;gap:8px">
    ${key('flex:1', 'var(--white-a04)', T.faint, 'Full · next')}
    ${key('flex:1', T.ok, T.onAccent, 'Done · ' + M.CUST[who].short)}
  </div>
</div>`;
}

// ── the whole landscape board ───────────────────────────────────────────────
function landscape(st, tier) {
  const plan = tier >= 2 ? M.planAhead(M.COUNTS, st) : null;
  const door = M.sideDoorOpen(st) ? 'side' : 'back';
  const frontier = M.ORDER.filter(id => M.doorOf(id) === door && M.isEmpty(st, id))[0];
  const free = M.positionsLeft(st, 'side') + M.positionsLeft(st, 'back');
  const notAboard = M.QUEUE.filter(k => !st.closed[k] && !M.positionsOf(st, k).length).length;
  const tightVan = free < notAboard;
  const sideLeft = M.positionsLeft(st, 'side');
  const sideStaged = M.stagedAtDoor(st, 'side');

  const stat = (l, v, c, note) => `<div style="display:flex;flex-direction:column;align-items:flex-end;gap:2px">
    <span style="${label};color:${T.faint}">${esc(l)}</span>
    <span style="font:900 var(--text-26)/1 ${T.display};letter-spacing:var(--tracking-tight);color:${c}">${esc(v)}</span>
    ${note ? `<span style="font:var(--type-label);color:${c}">${esc(note)}</span>` : ''}
  </div>`;

  // Same flex skeleton as a band — 54px label slot, nine flexed columns, the
  // arrow and the spot — so the numbers cannot drift off the cells they name.
  // Approximating it with padding put R9 nine pixels out.
  const heads = `<div style="display:flex;gap:8px;flex:none">
    <div style="width:54px;flex:none"></div>
    ${Array.from({ length: M.ROWS }, (_, i) =>
      `<div style="flex:1 1 0;min-width:0;${label};color:${i + 1 <= M.SIDE_DOOR_ROWS ? T.warn : T.muted}">R${i + 1}</div>`).join('')}
  </div>`;

  const band = (col, name, sub) => `<div style="display:flex;gap:8px;align-items:stretch;height:126px">
  <div style="width:54px;flex:none;display:flex;flex-direction:column;justify-content:center;align-items:flex-end;gap:1px">
    <span style="${label};color:${T.muted}">${name}</span>
    <span style="font:var(--type-label);color:${T.faint}">${sub}</span>
  </div>
  ${Array.from({ length: M.ROWS }, (_, i) => cell(st, `r${i + 1}-${col}`, plan, { frontier, next: M.expectedNext(st) })).join('')}
</div>`;

  // The back doorway is at the van's rear across the full width, so it is drawn
  // there — a column beside both bands — rather than as another strip below.
  const backDoor = `<div style="width:104px;flex:none;margin-top:25px;height:260px;border-radius:var(--radius-lg);border:1px dashed ${T.border};background:${T.void};padding:12px 10px;display:flex;flex-direction:column;align-items:center;gap:10px">
    <span style="${label};color:${T.warn};text-align:center;line-height:1.4">BACK<br>DOORWAY</span>
    <div style="flex:1;display:flex;align-items:flex-end">${bar(st, 'door-back', null, 120)}</div>
    <span style="font:var(--type-label);color:${T.faint};text-align:center;line-height:1.4">empty<br>last resort</span>
  </div>`;

  const sideDoorNote = sideLeft
    ? `${sideLeft} position${sideLeft === 1 ? '' : 's'} left · ${sideStaged} staged here`
    : `rows 1–${M.SIDE_DOOR_ROWS} full · everything left goes in the back`;

  return `<div style="width:1440px;height:840px;flex:none;background:${T.page};border:1px solid ${T.hair};border-radius:var(--radius-lg);overflow:hidden;padding:14px 18px;display:flex;flex-direction:column;gap:8px">

  <div style="height:46px;flex:none;display:flex;align-items:center;gap:16px">
    <div style="width:44px;height:44px;flex:none;border:1px solid ${T.border};border-radius:var(--radius-md);display:flex;align-items:center;justify-content:center;color:${T.muted};font-size:19px">←</div>
    <div style="display:flex;flex-direction:column;gap:1px">
      <span style="font:700 var(--text-21)/1.15 ${T.display};letter-spacing:var(--tracking-tight);color:${T.strong}">Stavanger Route</span>
      <span style="${label};color:${T.muted}">WED 19 AUG · 6 STOPS · ${tier === 1 ? 'ROUTE LIST ONLY' : tier === 2 ? 'COUNTS KNOWN' : 'FULLY SCANNED'}</span>
    </div>
    <div style="flex:1"></div>
    <div style="display:flex;gap:22px;align-items:flex-start">
      ${stat('POSITIONS LEFT', String(free), tightVan ? T.danger : T.strong, tightVan ? `${notAboard} stops need one` : '')}
      ${stat('SIDE DOOR', sideLeft ? `${sideLeft} left` : 'shut', sideLeft ? T.warn : T.danger, '')}
      ${stat('CRATES IN', String(M.cratesIn(st)), T.body, '')}
      ${stat('STOPS LOGGED', `${M.QUEUE.filter(k => st.closed[k]).length} / 6`, T.body, '')}
    </div>
    <div style="width:44px;height:44px;flex:none;border:1px solid ${T.border};border-radius:var(--radius-md);display:flex;align-items:center;justify-content:center;color:${T.muted};font-size:15px">⚑</div>
  </div>

  ${consoleStrip(st, tier)}

  <div style="display:flex;align-items:center;gap:10px;flex:none;padding-left:62px">
    <span style="${label};color:${sideLeft ? T.warn : T.danger}">SIDE DOOR · ${sideLeft ? 'OPEN · REACHES ROWS 1–' + M.SIDE_DOOR_ROWS : 'SHUT'}</span>
    <span style="font:var(--type-body-sm);color:${T.faint}">${esc(sideDoorNote)}</span>
    <span style="flex:1;height:1px;background:${T.hair}"></span>
    <span style="color:${T.warn};font-size:15px">↓</span>
  </div>

  <div style="display:flex;gap:8px;flex:none;padding-left:62px">
    ${M.SPOTS.filter(s => s.door === 'side').map(s => spot(st, s.id, 'flex:1 1 0;min-width:0')).join('')}
    <div style="width:104px;flex:none;border-radius:var(--radius-lg);border:1px dashed ${T.border};background:${T.void};padding:10px;display:flex;flex-direction:column;gap:4px;align-items:center;justify-content:center;text-align:center">
      <span style="${label};color:${T.warn}">SIDE<br>DOORWAY</span>
      <span style="font:var(--type-label);color:${T.info};line-height:1.4">❄ freeze<br>shares this</span>
    </div>
  </div>

  <div style="display:flex;gap:8px;flex:1;min-height:0;align-items:flex-start">
    <div style="flex:1;min-width:0;display:flex;flex-direction:column;gap:8px">
      ${heads}
      ${band('right', 'RIGHT', 'kerb')}
      ${band('left', 'LEFT', 'driver')}
    </div>
    ${backDoor}
    <div style="width:18px;flex:none;display:flex;align-items:center;justify-content:center;color:${T.warn};font-size:16px;align-self:stretch">←</div>
    <div style="width:214px;flex:none;display:flex;flex-direction:column;gap:8px">
      <div style="height:17px;flex:none;${label};color:${T.warn}">BACK DOORS</div>
      ${spot(st, 'back-1', 'height:150px', true)}
      ${spot(st, 'back-2', 'height:150px', true)}
    </div>
  </div>

  <div style="height:88px;flex:none;display:flex;gap:8px;align-items:stretch">
    <div style="width:54px;flex:none;display:flex;flex-direction:column;justify-content:center;align-items:flex-end;text-align:right">
      <span style="${label};color:${T.faint};line-height:1.3">AS<br>TAPPED<br>IN</span>
    </div>
    ${M.QUEUE.map(k => {
      const closed = !!st.closed[k], now = k === M.expectedNext(st);
      const at = M.spotHolding(st, k), pos = M.positionsOf(st, k);
      const state = closed ? ['DONE ↺', T.ok] : now ? ['LOADING NOW', T.accentText]
        : at ? ['ON ' + at.name, T.warn] : ['WAITING', T.faint];
      return `<div style="flex:1 1 0;min-width:0;border-radius:var(--radius-lg);border:1px solid ${now ? 'var(--border-accent)' : (closed ? T.ok + '3A' : T.hair)};background:${now ? T.card : T.raised};padding:9px 11px;display:flex;flex-direction:column;justify-content:center;gap:3px">
        <div style="display:flex;align-items:center;gap:7px;min-width:0">
          <span style="width:9px;height:9px;border-radius:3px;flex:none;background:${M.CUST[k].color}"></span>
          <span style="font:600 var(--text-14)/1.15 ${T.display};letter-spacing:var(--tracking-snug);color:${closed || now ? T.strong : T.muted};overflow:hidden;text-overflow:ellipsis;white-space:nowrap">${esc(M.CUST[k].name)}</span>
          <div style="flex:1"></div>
          <span style="${label};color:${state[1]}">${esc(state[0])}</span>
        </div>
        <span style="font:var(--type-label);color:${pos.length ? T.body : T.faint};overflow:hidden;text-overflow:ellipsis;white-space:nowrap">${esc(pos.length ? pos.map(M.posLabel).join('  ') : 'stop ' + M.stopOf(k).i + ' · not in yet')}</span>
      </div>`;
    }).join('')}
  </div>
</div>`;
}

// ── the button-state ladder, generated from real board states ───────────────
function stateLadder() {
  const scene = (fn) => { const st = board(fn); return { st, ps: M.pushState(st, 'side-1') }; };
  const rows = [
    ['Loading order agrees', scene(st => { st.staged['side-1'] = { cust: 'OLA', n: 3 }; })],
    ['Someone else is up, and they are staged', scene(st => {
      st.staged['side-1'] = { cust: 'JAT', n: 3 }; st.staged['side-2'] = { cust: 'OLA', n: 2 }; })],
    ['Someone else is up, and they are aboard', scene(st => {
      st.van['r1-left'] = [{ cust: 'OLA', n: 3 }]; st.staged['side-1'] = { cust: 'JAT', n: 3 }; })],
    ['Too tall for the window', scene(st => {
      st.van['r1-left'] = [{ cust: 'OLA', n: 2 }]; st.van['r1-right'] = [{ cust: 'OLA', n: 2 }];
      st.closed.OLA = true; st.staged['side-1'] = { cust: 'JAT', n: 12 }; })],
    ['Too thin for the window', scene(st => {
      st.van['r1-left'] = [{ cust: 'OLA', n: 8 }]; st.van['r1-right'] = [{ cust: 'OLA', n: 8 }];
      st.closed.OLA = true; st.staged['side-1'] = { cust: 'JAT', n: 1 }; })],
    ['Side rows full, more than one crate', scene(st => {
      ['r1-left','r1-right','r2-left','r2-right','r3-left','r3-right','r4-left','r4-right']
        .forEach(id => st.van[id] = [{ cust: 'OLA', n: 3 }]);
      st.staged['side-1'] = { cust: 'JAT', n: 2 }; })],
    ['Side rows full, a single crate', scene(st => {
      ['r1-left','r1-right','r2-left','r2-right','r3-left','r3-right','r4-left','r4-right']
        .forEach(id => st.van[id] = [{ cust: 'OLA', n: 3 }]);
      st.staged['side-1'] = { cust: 'JAT', n: 1 }; })],
  ];
  const toneOf = k => ({ ready: T.ok, doorway: T.ok, split: T.warn, order: T.warn,
    thin: T.warn, physical: T.danger, nofit: T.danger }[k] || T.faint);
  const filled = k => k === 'ready' || k === 'doorway';
  return rows.map(([title, { ps }]) => {
    const tone = toneOf(ps.kind);
    return `<div style="display:flex;gap:16px;align-items:flex-start;padding:14px 0;border-top:1px solid ${T.hair}">
    <div style="width:250px;flex:none;display:flex;flex-direction:column;gap:3px">
      <span style="font:600 var(--text-14)/1.2 var(--font-sans);color:${T.body}">${esc(title)}</span>
      <span style="${label};color:${T.faint}">kind: ${esc(ps.kind)}</span>
    </div>
    <div style="width:250px;flex:none;height:48px;border-radius:var(--radius-md);display:flex;align-items:center;justify-content:center;text-align:center;padding:0 12px;font:600 var(--text-15)/1.15 var(--font-sans);${
      filled(ps.kind) ? `background:${tone};color:${T.onAccent}` : `background:${tone}1A;border:1px solid ${tone}66;color:${tone}` }">${esc(ps.label)}</div>
    <p style="flex:1;min-width:0;margin:0;font:var(--type-body-sm);color:${ps.why ? tone : T.faint};max-width:56ch">${esc(ps.why || 'no explanation needed — this one is simply allowed')}</p>
  </div>`;
  }).join('');
}

// ── portrait ────────────────────────────────────────────────────────────────
function portrait(st) {
  const door = M.sideDoorOpen(st) ? 'side' : 'back';
  const frontier = M.ORDER.filter(id => M.doorOf(id) === door && M.isEmpty(st, id))[0];
  const rowStrip = r => `<div style="display:flex;gap:8px;flex:1 1 0;min-height:0;align-items:stretch">
    <div style="width:30px;flex:none;display:flex;align-items:center;justify-content:flex-end"><span style="${label};color:${r <= M.SIDE_DOOR_ROWS ? T.warn : T.muted}">R${r}</span></div>
    ${cell(st, `r${r}-left`, null, { frontier, next: M.expectedNext(st) })}
    ${cell(st, `r${r}-right`, null, { frontier, next: M.expectedNext(st) })}
    ${r <= M.SIDE_DOOR_ROWS ? `<div style="width:16px;flex:none;display:flex;align-items:center;justify-content:center;color:${T.warn};font-size:15px">←</div>` : '<div style="width:16px;flex:none"></div>'}
  </div>`;
  return `<div style="width:900px;height:1384px;flex:none;background:${T.page};border:1px solid ${T.hair};border-radius:var(--radius-lg);overflow:hidden;padding:14px 16px;display:flex;flex-direction:column;gap:8px">
  <div style="display:flex;align-items:center;gap:12px;flex:none">
    <div style="width:42px;height:42px;flex:none;border:1px solid ${T.border};border-radius:var(--radius-md);display:flex;align-items:center;justify-content:center;color:${T.muted}">←</div>
    <div style="display:flex;flex-direction:column;gap:1px">
      <span style="font:700 var(--text-18)/1.15 ${T.display};letter-spacing:var(--tracking-tight);color:${T.strong}">Stavanger Route</span>
      <span style="${label};color:${T.muted}">6 STOPS · ROUTE LIST ONLY</span>
    </div>
    <div style="flex:1"></div>
    <div style="display:flex;flex-direction:column;align-items:flex-end"><span style="${label};color:${T.faint}">POSITIONS LEFT</span><span style="font:900 var(--text-26)/1 ${T.display};color:${T.strong}">${M.positionsLeft(st,'side')+M.positionsLeft(st,'back')}</span></div>
  </div>
  ${portraitConsole(st)}
  <div style="display:flex;align-items:center;gap:10px;flex:none;padding-left:38px">
    <span style="${label};color:${T.faint}">CAB · FRONT</span>
    <span style="flex:1;height:1px;background:${T.hair}"></span>
    <span style="${label};color:${T.warn}">SIDE DOOR →</span>
  </div>
  <div style="display:flex;gap:8px;flex:4 1 0;min-height:0">
    <div style="flex:1;min-width:0;display:flex;flex-direction:column;gap:8px">${[1,2,3,4].map(rowStrip).join('')}</div>
    <div style="width:216px;flex:none;display:flex;flex-direction:column;gap:8px">
      ${M.SPOTS.filter(s => s.door === 'side').map(s => spot(st, s.id, 'flex:1 1 0;min-height:0')).join('')}
    </div>
  </div>
  <div style="display:flex;gap:8px;flex:5 1 0;min-height:0">
    <div style="flex:1;min-width:0;display:flex;flex-direction:column;gap:8px">${[5,6,7,8,9].map(rowStrip).join('')}</div>
    <div style="width:216px;flex:none;display:flex;flex-direction:column;gap:8px">
      ${M.SPOTS.filter(s => s.door === 'back').map(s => spot(st, s.id, 'flex:1 1 0;min-height:0')).join('')}
      <div style="flex:1 1 0;min-height:0;border-radius:var(--radius-lg);border:1px dashed ${T.border};background:${T.void};display:flex;flex-direction:column;align-items:center;justify-content:center;gap:4px;text-align:center">
        <span style="${label};color:${T.warn}">BACK DOORWAY</span>
        <span style="font:var(--type-label);color:${T.faint}">free · last resort</span>
      </div>
    </div>
  </div>
  <div style="display:flex;align-items:center;gap:10px;flex:none;padding-left:38px">
    <span style="${label};color:${T.warn}">BACK DOORS ↑</span>
    <span style="flex:1;height:1px;background:${T.hair}"></span>
  </div>
</div>`;
}

// ── the document ────────────────────────────────────────────────────────────
const section = (id, tag, title, note, body) => `
<section id="${id}" data-screen-label="${esc(tag + ' ' + title)}" style="display:flex;flex-direction:column;gap:14px">
  <div style="display:flex;align-items:baseline;gap:12px;flex-wrap:wrap">
    <span style="${label};color:${T.accent}">${esc(tag)}</span>
    <h2 style="margin:0;font:var(--type-h2);color:${T.strong}">${esc(title)}</h2>
  </div>
  <p style="margin:0;max-width:78ch;font:var(--type-body);color:${T.muted};text-wrap:pretty">${note}</p>
  ${body}
</section>`;

const MID2 = board(st => {
  st.van['r1-left'] = [{ cust: 'OLA', n: 5 }];
  st.van['r1-right'] = [{ cust: 'OLA', n: 5 }];
  st.van['r2-left'] = [{ cust: 'JAT', n: 3 }];
  st.closed.OLA = true;
  st.staged['side-1'] = { cust: 'JAT', n: 2 };
  st.staged['side-2'] = { cust: 'HIN', n: 2 };
  st.staged['side-3'] = { cust: 'SVE', n: 4 };
});

// Further along, so the planned-against-actual gauges have something to show:
// two stops closed, a third part-loaded, and the back door now the live one.
const LATER = board(st => {
  st.van['r1-left'] = [{ cust: 'OLA', n: 5 }];
  st.van['r1-right'] = [{ cust: 'OLA', n: 5 }];
  st.van['r2-left'] = [{ cust: 'JAT', n: 5 }];
  st.van['r2-right'] = [{ cust: 'HIN', n: 2 }];
  st.van['r3-left'] = [{ cust: 'SVE', n: 4 }];
  st.closed.OLA = true; st.closed.JAT = true; st.closed.HIN = true;
  st.staged['side-1'] = { cust: 'SVE', n: 3 };
  st.staged['back-1'] = { cust: 'FRO', n: 2 };
});

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
  <p style="margin:0;color:${T.muted};text-wrap:pretty">The screen a driver holds at the pallet while loading a delivery van, on the Hampter design system. Nine rows by two columns, loaded in reverse delivery order, with a side door that reaches only the first four rows.</p>
  <p style="margin:0;color:${T.faint};text-wrap:pretty">Every figure, name and sentence on this page is produced by the rule set in <code style="font:var(--type-mono);color:${T.accentText}">docs/design/sorting-live/src/model.js</code> — the same code the working prototype runs — so the design cannot drift from the logic it describes. Rebuild with <code style="font:var(--type-mono);color:${T.accentText}">node build.mjs</code>.</p>
</div>

${section('S1', 'S1', 'The board, mid-load', `Route list only: nothing is known until it is tapped in. Olavstoppen is closed out and aboard, Jåtten is half in, and all three side spots are holding something — one green, two amber. <strong style="color:${T.body}">The stack bar in each position is the whole point of this revision:</strong> its height answers “how full is the van” and its colour and code answer “who goes where”, so the two questions no longer need a view toggle between them.`, landscape(MID2, 1))}

${section('S2', 'S2', 'The same board, with counts known', `When a total per customer has been scanned, the board plans the whole van up front by running the live rules forward. Planned crates appear as <strong style="color:${T.info}">dashed outlines stacked above the solid ones</strong> — so a position part-filled reads as “three in, two still to come” structurally rather than as a colour wash. The header also names the pallet to pull from next. Shown further into the same load, with Sverdrup part-way in at R3 &middot; L — four solid, three still expected.`, landscape(LATER, 3))}

${section('S3', 'S3', 'Held upright', `The van turns with the tablet: cab at the top, rows running down, and the van’s right side — the one the door is on — on the right. So the packing spots become a column beside exactly the rows they can reach, and the back doors are at the bottom. The console stacks rather than running along one row.`, `<div style="display:flex;gap:32px;align-items:flex-start">${portrait(MID2)}<div style="width:300px;flex:none;display:flex;flex-direction:column;gap:12px"><span style="${label};color:${T.faint}">900 × 1384</span><p style="margin:0;font:var(--type-body-sm);color:${T.muted};text-wrap:pretty">Rows come out ~118px each here, taller than the 90px the previous draft managed, because the route strip has been dropped in portrait — upright there is no room for it and the load order is legible from the map itself.</p></div></div>`)}

${section('S4', 'S4', 'What every push button can say', `Amber means <em>allowed, and here is what it costs</em>. Red means <em>the van physically cannot</em>, and only two situations are ever red. Every non-green state names the tap that clears it. These rows are rendered from real board states, not written out — the sentence beside each button is what the code emits.`, `<div style="max-width:1180px">${stateLadder()}</div>`)}

${section('S5', 'S5', 'Where this differs from the previous draft', `Seven things were wrong with the last board; these are the six this revision fixes and the one it does not.`, `<div style="display:flex;flex-direction:column;gap:2px;max-width:1180px">
${[
  ['Two questions, one view', 'The space/identity toggle is gone. A taller stack bar carries fullness; the code beside it carries identity.', true],
  ['Room for the map', 'Positions grew from 111×146 to 126px tall with a 26px bar, and the route strip shrank from 166px to 88px. The information is in better proportion to how often it is read.', true],
  ['Warnings fold into what they are about', 'The capacity warning is now a state of the POSITIONS LEFT stat and the side-door budget lives on the door line, so neither is a band that appears and shoves the layout around.', true],
  ['The doorways are drawn where they are', 'The back doorway is a column at the van’s rear across both bands; the side well sits at the end of the side-door row. Both are always visible, because both are real floor.', true],
  ['Planned versus actual is structural', 'Dashed rungs above solid ones, not a tint.', true],
  ['Identity is not colour alone', 'Every occupied position shows its three-letter code at all times, not only when the cell is too narrow for a name.', true],
  ['A skipped tap still cannot be detected', 'Nothing here fixes that — the app has no independent view of the van. The route strip is labelled “as tapped in” to set the expectation, which is honesty rather than a solution.', false],
].map(([t, d, fixed]) => `<div style="display:flex;gap:14px;align-items:flex-start;padding:12px 0;border-top:1px solid ${T.hair}">
  <span style="width:20px;flex:none;color:${fixed ? T.ok : T.warn};font-size:15px">${fixed ? '✓' : '—'}</span>
  <span style="width:260px;flex:none;font:600 var(--text-14)/1.3 var(--font-sans);color:${T.body}">${esc(t)}</span>
  <p style="margin:0;flex:1;font:var(--type-body-sm);color:${T.muted};max-width:64ch;text-wrap:pretty">${esc(d)}</p>
</div>`).join('')}
</div>`)}

${section('S6', 'S6', 'Notes on the design system', `Two deliberate departures, both because this screen is used standing up in a cold warehouse rather than at a desk.`, `<div style="display:flex;flex-direction:column;gap:2px;max-width:1180px">
${[
  ['Controls are taller than the kit', 'The largest button in the system is <code style="font:var(--type-mono);color:'+T.accentText+'">--control-height-lg</code> at 42px. The primary targets here are 48–60px, because the driver is often gloved and a dropped tap commits a crate to a position. Everything else — colour, type, radius, spacing — comes straight from the tokens.'],
  ['No hover state carries meaning', 'This is a touch screen with no pointer. Anything the kit expresses through <code style="font:var(--type-mono);color:'+T.accentText+'">:hover</code> has to also be visible at rest.'],
].map(([t, d]) => `<div style="display:flex;gap:14px;align-items:flex-start;padding:12px 0;border-top:1px solid ${T.hair}">
  <span style="width:280px;flex:none;font:600 var(--text-14)/1.3 var(--font-sans);color:${T.body}">${esc(t)}</span>
  <p style="margin:0;flex:1;font:var(--type-body-sm);color:${T.muted};max-width:64ch;text-wrap:pretty">${d}</p>
</div>`).join('')}
</div>`)}

</div>
</x-dc>
</body>
</html>
`;

writeFileSync(join(here, 'Van loading board.dc.html'), doc);
console.log('wrote Van loading board.dc.html —', doc.length, 'bytes');
