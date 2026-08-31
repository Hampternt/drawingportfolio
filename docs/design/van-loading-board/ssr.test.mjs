// Checks that ssr.mjs renders the same DOM the browser builds from the same
// markup and the same values. This is the whole basis of the design document
// being the screen rather than a drawing of it, so it is worth a test rather
// than a claim.
//
// Unlike ../sorting-live/src/*.test.js this one needs a browser, because the
// thing it compares against is a browser.
//
//   node ssr.test.mjs                       # uses the bundled Chromium
//   PW=/path/to/playwright node ssr.test.mjs
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { tmpdir } from 'node:os';
import { boardTemplate, render } from './ssr.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const LIVE = join(here, '..', 'sorting-live', 'src');
const read = f => readFileSync(join(LIVE, f), 'utf8');

class DCLogic { constructor(p) { this.props = p || {}; } setState(o) { Object.assign(this.state, o); } }
globalThis.DCLogic = DCLogic;
const Component = eval(read('model.js') + read('board.js') + '\n;Component');
const TPL = boardTemplate(read('board.html'));
const SET = boardTemplate(read('settings.html'));

let fails = 0;
const ok = (c, m) => { if (!c) { fails++; console.log('  FAIL  ' + m); } };

// ── what it does without a browser at all ───────────────────────────────────
const html = render(TPL, new Component({ accent: '#B48EF7' }).renderVals());
ok(!/\{\{/.test(html), 'no {{hole}} survives into the output');
ok(!/undefined/.test(html), 'and nothing resolves to the word undefined');
ok(!/<sc-(for|if)/.test(html), 'sc-for and sc-if are expanded, not emitted');
ok(!/onclick/i.test(html), 'onClick is dropped — a static page has no handlers');
ok(/&#8722;/.test(html), 'entities authored in the markup pass through unescaped');
ok(/font:400 13px\/1\.35 &quot;Space Grotesk&quot;/.test(html) || !/13px\/1\.35 "Space/.test(html),
  'a quote inside a style value is escaped, or the value does not contain one');

const path = join(tmpdir(), 'ssr-check.html');
writeFileSync(path, `<!doctype html><meta charset="utf-8"><style>html,body{margin:0}</style>${html}`);

// ── and whether that is what the browser would have built ───────────────────
const PW = process.env.PW || '/opt/node22/lib/node_modules/playwright/index.mjs';
let chromium;
try { ({ chromium } = await import(PW)); }
catch { console.log('playwright not found at ' + PW + ' — skipping the browser comparison'); process.exit(fails ? 1 : 0); }

const b = await chromium.launch({ proxy: process.env.HTTPS_PROXY ? { server: process.env.HTTPS_PROXY } : undefined,
                                  args: ['--ignore-certificate-errors'] });
const p = await b.newPage({ viewport: { width: 1500, height: 1000 } });
const shape = () => [...document.body.querySelectorAll('*')]
  .map(e => e.tagName + '|' + (e.getAttribute('style') || '') + '|' + (e.children.length ? '' : e.textContent));

await p.goto('file://' + join(here, '..', 'sorting-live', 'demo.html'));
await p.waitForTimeout(500);
const live = await p.evaluate(`(${shape})()`.replace('document.body', 'document.getElementById("board")'));
await p.goto('file://' + path);
await p.waitForTimeout(200);
const ssr = await p.evaluate(`(${shape})()`);

// …and the settings screen, through the same machinery
const c2 = new Component({ accent: '#B48EF7' });
c2.state.screen = 'settings';
const setHtml = render(SET, c2.renderVals());
const setPath = join(tmpdir(), 'ssr-check-settings.html');
writeFileSync(setPath, `<!doctype html><meta charset="utf-8"><style>html,body{margin:0}</style>${setHtml}`);
await p.goto('file://' + join(here, '..', 'sorting-live', 'demo.html'));
await p.waitForTimeout(400);
await p.evaluate(() => { COMPONENT.setState({ screen: 'settings' }); });
await p.waitForTimeout(200);
const liveSet = await p.evaluate(`(${shape})()`.replace('document.body', 'document.getElementById("board")'));
await p.goto('file://' + setPath);
await p.waitForTimeout(200);
const ssrSet = await p.evaluate(`(${shape})()`);
await b.close();

ok(liveSet.length > 100, 'the settings screen rendered something too  ' + liveSet.length + ' nodes');
let sdiff = 0;
for (let i = 0; i < Math.max(liveSet.length, ssrSet.length); i++) if (liveSet[i] !== ssrSet[i]) sdiff++;
ok(sdiff === 0, 'the settings screen matches as well  ' + sdiff + ' of ' + liveSet.length + ' differ');

ok(live.length > 150, 'the live board rendered something to compare against  ' + live.length + ' nodes');
ok(live.length === ssr.length, 'the same number of nodes  live ' + live.length + ' ssr ' + ssr.length);
let diff = 0;
for (let i = 0; i < Math.max(live.length, ssr.length); i++) {
  if (live[i] !== ssr[i]) {
    if (diff < 3) console.log('  #' + i + '\n   live: ' + String(live[i]).slice(0, 160) + '\n   ssr : ' + String(ssr[i]).slice(0, 160));
    diff++;
  }
}
ok(diff === 0, diff + ' of ' + live.length + ' nodes differ');
console.log(fails ? `\n  ${fails} failed`
  : `passed — the static render is the live screen: board ${live.length} nodes, settings ${liveSet.length} nodes, all identical`);
process.exit(fails ? 1 : 0);
