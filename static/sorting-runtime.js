// ── a very small template runtime ────────────────────────────────────────────
// Renders the same markup the artboards use — {{dotted.path}} holes, <sc-for>,
// <sc-if>, onClick handlers — so the demo and the design cannot drift. The
// markup lives in an inert <template>, which is what stops the browser trying
// to compile onClick as script.
function resolve(scope, path) {
  return String(path).split('.').reduce(function (o, k) { return o == null ? undefined : o[k]; }, scope);
}
function interp(str, scope) {
  return str.replace(/\{\{([^}]+)\}\}/g, function (_, p) {
    var v = resolve(scope, p.trim());
    return v == null ? '' : String(v);
  });
}
function hole(attr) { return String(attr).replace(/\{\{|\}\}/g, '').trim(); }

function build(node, scope) {
  if (node.nodeType === 3) return [document.createTextNode(interp(node.nodeValue, scope))];
  if (node.nodeType !== 1) return [];
  var tag = node.tagName.toLowerCase(), out = [];

  if (tag === 'sc-for') {
    var list = resolve(scope, hole(node.getAttribute('list'))) || [];
    var as = node.getAttribute('as');
    list.forEach(function (item) {
      var inner = Object.create(scope);
      inner[as] = item;
      Array.prototype.forEach.call(node.childNodes, function (c) { out = out.concat(build(c, inner)); });
    });
    return out;
  }
  if (tag === 'sc-if') {
    if (!resolve(scope, hole(node.getAttribute('value')))) return [];
    Array.prototype.forEach.call(node.childNodes, function (c) { out = out.concat(build(c, scope)); });
    return out;
  }

  var el = document.createElement(tag);
  Array.prototype.forEach.call(node.attributes, function (a) {
    if (a.name.toLowerCase() === 'onclick') {
      var fn = resolve(scope, hole(a.value));
      if (typeof fn === 'function') {
        el.style.cursor = 'pointer';
        el.addEventListener('click', function (e) { e.preventDefault(); e.stopPropagation(); fn(); });
      }
      return;
    }
    el.setAttribute(a.name, interp(a.value, scope));
  });
  Array.prototype.forEach.call(node.childNodes, function (c) {
    build(c, scope).forEach(function (n) { el.appendChild(n); });
  });
  return [el];
}

// ── the shim the board's logic is written against ────────────────────────────
// ON_PAINT is how anything outside the render loop finds out that state changed
// — the demo hangs its settings persistence on it. Left null, nothing happens.
// BOARD_HOST is the element the tree is painted into. The demo has one board
// on an otherwise empty page and calls it `board`; the app's page is a whole
// site around it and names it something that could only be this.
var COMPONENT = null, PENDING = false, ON_PAINT = null, BOARD_HOST = 'board';
class DCLogic {
  constructor(props) { this.props = props || {}; }
  setState(o) { Object.assign(this.state, o); schedule(); }
}
function schedule() {
  if (PENDING) return;
  PENDING = true;
  requestAnimationFrame(function () { PENDING = false; paint(); });
}
function paint() {
  var host = document.getElementById(BOARD_HOST);
  var vals = COMPONENT.renderVals();
  // The board and the settings are two screens against one component, so the
  // values say which markup they are for rather than the caller guessing.
  var tpl = document.getElementById((vals.screen || 'board') + '-template');
  host.textContent = '';
  Array.prototype.forEach.call(tpl.content.childNodes, function (c) {
    build(c, vals).forEach(function (n) { host.appendChild(n); });
  });
  document.querySelectorAll('[data-tier]').forEach(function (b) {
    b.classList.toggle('on', Number(b.dataset.tier) === (COMPONENT.props.tier || 1));
  });
  if (ON_PAINT) ON_PAINT();
}
