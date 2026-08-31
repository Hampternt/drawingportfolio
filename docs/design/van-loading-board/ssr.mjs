// Renders ../sorting-live/src/board.html to static HTML in node, against the
// value tree ../sorting-live/src/board.js produces.
//
// The point is that this design document draws the REAL board rather than a
// second drawing of it. The demo renders that markup in a browser through
// runtime.js; this renders the same markup, from the same values, with no
// browser at all — so the two cannot disagree about what the screen looks like.
//
// It supports exactly what runtime.js supports and nothing more: {{dotted.path}}
// in text and in attribute values, <sc-for list as>, <sc-if value>, and onClick,
// which a static page drops.

const VOID = new Set(['area', 'base', 'br', 'col', 'embed', 'hr', 'img', 'input',
  'link', 'meta', 'param', 'source', 'track', 'wbr']);
const DROP = new Set(['onclick', 'hint-placeholder-count', 'hint-placeholder-val']);

function parseAttrs(src) {
  const out = {};
  const re = /([^\s=/]+)(?:\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+)))?/g;
  let m;
  while ((m = re.exec(src))) out[m[1].toLowerCase()] = m[2] ?? m[3] ?? m[4] ?? '';
  return out;
}

export function parse(html) {
  const root = { tag: '#root', attrs: {}, kids: [] }, stack = [root];
  // comment | close | open. Attribute values may contain '>' inside quotes, so
  // the attribute run is matched rather than scanning to the first '>'.
  const re = /<!--[\s\S]*?-->|<\/([a-zA-Z][\w-]*)\s*>|<([a-zA-Z][\w-]*)((?:\s+[^\s=/>]+(?:\s*=\s*(?:"[^"]*"|'[^']*'|[^\s>]+))?)*)\s*(\/?)>/g;
  let last = 0, m;
  const text = s => { if (s.trim()) stack[stack.length - 1].kids.push({ tag: '#text', text: s }); };
  while ((m = re.exec(html))) {
    text(html.slice(last, m.index));
    last = re.lastIndex;
    if (m[0].startsWith('<!--')) continue;
    if (m[1]) { if (stack.length > 1) stack.pop(); continue; }
    const node = { tag: m[2].toLowerCase(), attrs: parseAttrs(m[3] || ''), kids: [] };
    stack[stack.length - 1].kids.push(node);
    if (!m[4] && !VOID.has(node.tag)) stack.push(node);
  }
  text(html.slice(last));
  return root;
}

const resolve = (scope, path) =>
  String(path).split('.').reduce((o, k) => (o == null ? undefined : o[k]), scope);
const hole = a => String(a).replace(/\{\{|\}\}/g, '').trim();

// Literal chunks of the template pass through as authored — they carry entities
// like &#8722; that must not be escaped again. Only the interpolated values are
// escaped, which is what document.createTextNode/setAttribute do at runtime.
function interp(str, scope, esc) {
  return String(str).split(/(\{\{[^}]+\}\})/).map(part => {
    if (!part.startsWith('{{')) return part;
    const v = resolve(scope, hole(part));
    return v == null ? '' : esc(String(v));
  }).join('');
}
const escText = s => s.replace(/&(?![a-zA-Z#][a-zA-Z0-9]{1,7};)/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
const escAttr = s => escText(s).replace(/"/g, '&quot;');

export function render(node, scope) {
  if (node.tag === '#text') return interp(node.text, scope, escText);
  if (node.tag === '#root') return node.kids.map(k => render(k, scope)).join('');

  if (node.tag === 'sc-for') {
    const list = resolve(scope, hole(node.attrs.list)) || [];
    return list.map(item => {
      const inner = Object.create(scope);
      inner[node.attrs.as] = item;
      return node.kids.map(k => render(k, inner)).join('');
    }).join('');
  }
  if (node.tag === 'sc-if') {
    return resolve(scope, hole(node.attrs.value)) ? node.kids.map(k => render(k, scope)).join('') : '';
  }

  const attrs = Object.entries(node.attrs)
    .filter(([k]) => !DROP.has(k))
    .map(([k, v]) => ` ${k}="${interp(v, scope, escAttr)}"`).join('');
  const open = `<${node.tag}${attrs}>`;
  if (VOID.has(node.tag)) return open;
  return open + node.kids.map(k => render(k, scope)).join('') + `</${node.tag}>`;
}

// The board itself: everything after the artboard's <helmet>, which is chrome
// for the canvas rather than part of the screen.
export function boardTemplate(markup) {
  const body = markup.split('<x-dc>')[1].split('</x-dc>')[0];
  return parse(body.split('</helmet>')[1]);
}
