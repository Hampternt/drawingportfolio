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
<title>Sorting · live board demo</title>
${helmet}
<style>
  :root { color-scheme: dark; }
  html, body { margin: 0; background: #07060B; color: #CDC6DD;
               font-family: "Space Grotesk", system-ui, sans-serif; }
  #chrome { display: flex; gap: 10px; align-items: stretch; flex-wrap: wrap;
            padding: 12px 16px; border-bottom: 1px solid #1A1723; background: #0B0910; }
  #chrome h1 { font-family: Archivo, system-ui, sans-serif; font-size: 15px; font-weight: 800;
               letter-spacing: -0.02em; color: #F2EEF8; margin: 0; align-self: center;
               padding-right: 8px; white-space: nowrap; }
  .tier { flex: 1 1 240px; min-width: 0; text-align: left; cursor: pointer;
          background: #0E0C14; border: 1px solid #262232; border-radius: 11px;
          padding: 9px 12px; color: #8D87A0; font: inherit; display: flex;
          flex-direction: column; gap: 2px; }
  .tier b { font-family: Archivo, system-ui, sans-serif; font-size: 14px; color: #CDC6DD; }
  .tier span { font-size: 11.5px; line-height: 1.35; color: #5F5876; }
  .tier.on { background: #17141F; border-color: #B48EF7; }
  .tier.on b { color: #F2EEF8; }
  .tier.on span { color: #8D87A0; }
  #reset { align-self: center; cursor: pointer; background: rgba(242,238,248,.05);
           border: 1px solid #262232; border-radius: 10px; color: #CDC6DD;
           font: inherit; font-size: 13px; padding: 10px 16px; white-space: nowrap; }
  /* Authored at exactly 1440x840 CSS — the Movink Pad Pro in landscape after
     the URL bar. Anything narrower is scaled, never reflowed, so what you tap
     here is what you tap there. */
  #stage { display: flex; justify-content: center; overflow: hidden; }
  #scaler { transform-origin: top center; }
  #board { width: 1440px; height: 840px; }
</style>
</head>
<body>

<div id="chrome">
  <h1>Live board</h1>
${buttons}
  <button id="reset">Start over</button>
</div>

<div id="stage"><div id="scaler"><div id="board"></div></div></div>

<template id="board-template">
${board.markup}
</template>

<template id="settings-template">
${settings.markup}
</template>

<script>
${read('runtime.js')}
${read('model.js')}
${read('board.js')}

COMPONENT = new Component({ tier: 1, accent: '#B48EF7' });

document.querySelectorAll('[data-tier]').forEach(function (b) {
  b.addEventListener('click', function () {
    COMPONENT.props.tier = Number(b.dataset.tier);
    paint();
  });
});
document.getElementById('reset').addEventListener('click', function () {
  var tier = COMPONENT.props.tier;
  COMPONENT = new Component({ tier: tier, accent: '#B48EF7' });
  paint();
});

function fit() {
  var s = Math.min(1, (window.innerWidth - 24) / 1440);
  document.getElementById('scaler').style.transform = 'scale(' + s + ')';
  document.getElementById('stage').style.height = (840 * s) + 'px';
}
window.addEventListener('resize', fit);
fit();
paint();
</script>
</body>
</html>
`;
writeFileSync(join(here, 'demo.html'), page);
console.log('wrote demo.html —', page.length, 'bytes');
