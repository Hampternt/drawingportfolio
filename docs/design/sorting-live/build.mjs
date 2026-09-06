// Rebuilds demo.html from src/. Node only, no dependencies.
//   node docs/design/sorting-live/build.mjs
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const read = f => readFileSync(join(here, 'src', f), 'utf8');

const screen = f => {
  const body = read(f).split('<x-dc>')[1].split('</x-dc>')[0];
  return { helmet: /<helmet>([\s\S]*?)<\/helmet>/.exec(body)[1].trim(), markup: body.split('</helmet>')[1].trim() };
};
const board = screen('board.html');
const settings = screen('settings.html');
const helmet = board.helmet;

const TIERS = [
  [1, 'Customer order only',
   'The route list and nothing else. Every count, every position and every stack height gets decided at the pallet.'],
  [2, 'Order + crate counts',
   'Weighed or scanned totals per customer. The board plans the whole van up front and shows what belongs where before anything is lifted.'],
  [3, 'Everything scanned',
   'Counts plus which pallet each order is buried in, so the board can also say what to pull next and from where.'],
];
const buttons = TIERS.map(([n, t, d]) =>
  `      <button class="tier" data-tier="${n}"><b>${t}</b><span>${d}</span></button>`).join('\n');

const page = `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Van load board</title>
${helmet}
<style>
  :root { color-scheme: dark; }
  html, body { margin: 0; background: #07060B; color: #CDC6DD;
               font-family: "Space Grotesk", system-ui, sans-serif; }
  body { overflow: hidden; }
  /* Authored at exactly 90rem x 52.5rem — 1440 x 840 at the browser's default
     16px, the Movink Pad Pro in landscape after the URL bar. The chrome bar and
     the board are one fixed-aspect drawing measured in that one unit, so the
     layout at any size is the same layout rather than a reflowed one, and fit()
     resizes the lot by setting the root font size. Nothing here is in pixels;
     that is what makes a single number enough. */
  #stage { display: flex; align-items: center; justify-content: center;
           min-height: 100vh; min-height: 100dvh; overflow: hidden; }
  #page { width: 90rem; flex: none; }
  #chrome { display: flex; gap: 0.625rem; align-items: stretch;
            padding: 0.75rem 1rem; border-bottom: 0.0625rem solid #1A1723; background: #0B0910; }
  #chrome h1 { font-family: Archivo, system-ui, sans-serif; font-size: 0.9375rem; font-weight: 800;
               letter-spacing: -0.02em; color: #F2EEF8; margin: 0; align-self: center;
               padding-right: 0.5rem; white-space: nowrap; }
  .tier { flex: 1 1 15rem; min-width: 0; text-align: left; cursor: pointer;
          background: #0E0C14; border: 0.0625rem solid #262232; border-radius: 0.6875rem;
          padding: 0.5625rem 0.75rem; color: #8D87A0; font: inherit; display: flex;
          flex-direction: column; gap: 0.125rem; }
  .tier b { font-family: Archivo, system-ui, sans-serif; font-size: 0.875rem; color: #CDC6DD; }
  .tier span { font-size: 0.71875rem; line-height: 1.35; color: #5F5876; }
  .tier.on { background: #17141F; border-color: #B48EF7; }
  .tier.on b { color: #F2EEF8; }
  .tier.on span { color: #8D87A0; }
  .shell { align-self: center; cursor: pointer; background: rgba(242,238,248,.05);
           border: 0.0625rem solid #262232; border-radius: 0.625rem; color: #CDC6DD;
           font: inherit; font-size: 0.8125rem; padding: 0.625rem 1rem; white-space: nowrap; }
  /* Full screen drops the tier descriptions rather than the tier buttons: the
     bar goes from three lines to one and hands the board the difference, and
     the only way back out of the demo is still on screen. */
  body.big .tier span { display: none; }
  #board { width: 90rem; height: 52.5rem; }
</style>
</head>
<body>

<div id="stage"><div id="page">

  <div id="chrome">
    <h1>Live board</h1>
${buttons}
    <button id="full" class="shell">Full screen</button>
    <button id="reset" class="shell">Start over</button>
  </div>

  <div id="board"></div>

</div></div>

<template id="board-template">
${board.markup}
</template>

<template id="settings-template">
${settings.markup}
</template>

<script>
${read('runtime.js')}
${read('store.js')}
${read('model.js')}
${read('board.js')}

// ── boot ─────────────────────────────────────────────────────────────────────
function startProps(extra) {
  var p = { tier: 1, accent: '#B48EF7', storage: storageWorks() ? 'on' : 'off' };
  Object.keys(extra || {}).forEach(function (k) { p[k] = extra[k]; });
  return p;
}
COMPONENT = new Component(startProps());

// Restore, then raise anything that would have been smaller than the load the
// session starts with — the same invariant the steppers enforce, applied to the
// reload rather than to a tap.
var stored = readSettings(loadStored());
if (stored) {
  var fitted = fitSettings(stored, COMPONENT.state.st);
  Object.keys(fitted.van).forEach(function (k) { COMPONENT.props[k] = fitted.van[k]; });
  COMPONENT.props.rules = fitted.rules;
}

// Written after every paint rather than from each control: that is one place
// instead of a dozen, and it cannot miss a path — including Restore defaults,
// which writes nothing and so removes the key.
var lastWritten = loadStored();
ON_PAINT = function () {
  var payload = writeSettings(COMPONENT.props);
  var bare = !Object.keys(payload.van).length && !Object.keys(payload.rules).length;
  var text = bare ? null : JSON.stringify(payload);
  if (text === lastWritten) return;
  saveStored(text);
  lastWritten = text;
};

document.querySelectorAll('[data-tier]').forEach(function (b) {
  b.addEventListener('click', function () {
    COMPONENT.props.tier = Number(b.dataset.tier);
    paint();
  });
});
// Start over is about the load, not the rules — the van does not change shape
// because the morning did.
document.getElementById('reset').addEventListener('click', function () {
  var keep = writeSettings(COMPONENT.props);
  COMPONENT = new Component(startProps(Object.assign({ tier: COMPONENT.props.tier, rules: keep.rules }, keep.van)));
  paint();
});

// ── how big the drawing is ───────────────────────────────────────────────────
// One number resizes the page, because the page has one unit. The board's own
// box is a constant; the chrome bar's height is not — it depends on the fonts
// that actually loaded — but it is a constant *in rem*, since the bar is 90rem
// wide whatever the viewport is and so always wraps the same way. So measure it
// at the design size, which costs one forced layout and cannot go circular.
var DESIGN = 16, BOARD_W = 90, BOARD_H = 52.5;
var root = document.documentElement, big = false;
function fit() {
  root.style.fontSize = DESIGN + 'px';
  var pageH = BOARD_H + document.getElementById('chrome').getBoundingClientRect().height / DESIGN;
  var s = Math.min((window.innerWidth - 16) / (BOARD_W * DESIGN),
                   (window.innerHeight - 8) / (pageH * DESIGN));
  // Full screen means the whole screen: this is a drawing, not a bitmap, so
  // past its design size it gets bigger rather than blurrier. Everywhere else
  // the design size is still the ceiling.
  root.style.fontSize = (DESIGN * Math.max(0.2, big ? s : Math.min(1, s))) + 'px';
}
window.addEventListener('resize', fit);

// ── full screen ──────────────────────────────────────────────────────────────
// Two things, deliberately: the page's own big layout, and the browser's
// fullscreen if it will give it. Asking is best-effort — inside an iframe
// without the fullscreen permission the request is simply refused, and there
// the slim bar plus the URL bar we cannot remove is still the whole win. So the
// layout never waits on the API, and the API never becomes the state.
var fullBtn = document.getElementById('full');
function fullEl() { return document.fullscreenElement || document.webkitFullscreenElement || null; }
function setBig(v) {
  big = v;
  document.body.classList.toggle('big', v);
  fullBtn.textContent = v ? 'Exit full screen' : 'Full screen';
  fit();
}
function askFull() {
  var req = root.requestFullscreen || root.webkitRequestFullscreen;
  if (!req) return;
  try { var r = req.call(root); if (r && r.catch) r.catch(function () {}); } catch (e) {}
}
function dropFull() {
  var ex = document.exitFullscreen || document.webkitExitFullscreen;
  if (!fullEl() || !ex) return;
  try { var r = ex.call(document); if (r && r.catch) r.catch(function () {}); } catch (e) {}
}
fullBtn.addEventListener('click', function () {
  if (big) { setBig(false); dropFull(); } else { setBig(true); askFull(); }
});
// Escape leaves the browser's fullscreen without telling the button, so follow
// it out. A refused request fires nothing, which is why big survives one.
['fullscreenchange', 'webkitfullscreenchange'].forEach(function (ev) {
  document.addEventListener(ev, function () { if (!fullEl() && big) setBig(false); else fit(); });
});
document.addEventListener('keydown', function (e) {
  if ((e.key !== 'f' && e.key !== 'F') || e.metaKey || e.ctrlKey || e.altKey) return;
  var t = e.target;
  if (t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.isContentEditable)) return;
  e.preventDefault();
  fullBtn.click();
});

fit();
paint();
</script>
</body>
</html>
`;
writeFileSync(join(here, 'demo.html'), page);
console.log('wrote demo.html —', page.length, 'chars');
