// Blueprint planner: the model. Pure functions over a plan document, with no
// DOM, so `tests/js/blueprint-model.test.js` runs this served file on plain
// node.
//
// Every length here is in grid units (u). 1u is 1rem on screen at zoom 1, so
// nothing in this file knows how big a pixel is: the view multiplies by the
// root font size and the zoom. Geometry follows the reviewed design canvas,
// docs/design/blueprint-planner/canvas/Guide.dc.html.
//
// Port layout is one port per row, outputs first (the canvas's proposal, still
// open for the owner). All of it lives in measure() and portAt().
(function (root) {
  'use strict';

  // ── Geometry, in u ─────────────────────────────────────────────────────────
  var HEAD = 2;          // title row
  var PAD = 0.5;         // body padding, top and bottom
  var ROW = 1.5;         // one port row
  var NOTE_LH = 1;       // one note line
  var NOTE_GAP = 0.375;  // space under the last note line
  var FOOT = 1.75;       // a blueprint's footer
  var BORDER = 0.0625;   // 1px at 16px
  var MIN_W = 9;
  // Monospace advance, 0.6em per character, at each size the node uses.
  var CH = 0.45;         // ports and types, 12px
  var CH_TITLE = 0.4875; // node title, 13px
  var CH_NOTE = 0.4125;  // notes and the footer, 11px
  var CH_BADGE = 0.4125; // status badge, 10px caps at 0.06em letter-spacing
  var EPS = 1e-9;

  var ZOOM_MIN = 0.2;
  var ZOOM_MAX = 3;
  // A drop lands on a port within this many u *on screen*, whatever the zoom:
  // half of the 2.75u touch target.
  var HIT_SCREEN = 1.375;

  var STATUSES = ['idea', 'planned', 'building', 'done'];
  var KINDS = ['machine', 'blueprint'];

  // ── Names and refs ─────────────────────────────────────────────────────────
  // A node, port or graph name. It starts with a letter or an underscore: a
  // key that looks like a number is moved to the front of a JavaScript object,
  // and a port's place among its keys is its row on the node. No '.' (it
  // splits a wire into node and port) and no '/' (it joins a layout key). The
  // server checks the same rule, in src/blueprint_doc.rs.
  //
  // The names every plain object already answers to are refused as well:
  // `nodes.constructor` is a function before anyone adds a node of that name,
  // and assigning `nodes.__proto__` replaces the prototype instead of adding a
  // key. The list is Object.prototype's own names that fit the pattern; a
  // test checks it against the running engine.
  var NAME_RE = /^[A-Za-z_][A-Za-z0-9_-]{0,63}$/;
  var RESERVED = ['__defineGetter__', '__defineSetter__', '__lookupGetter__', '__lookupSetter__', '__proto__',
    'constructor', 'hasOwnProperty', 'isPrototypeOf', 'propertyIsEnumerable', 'toLocaleString', 'toString', 'valueOf'];
  function isName(s) { return typeof s === 'string' && NAME_RE.test(s) && RESERVED.indexOf(s) < 0; }

  function split(ref) {
    var i = String(ref).indexOf('.');
    return i < 0 ? [String(ref), ''] : [ref.slice(0, i), ref.slice(i + 1)];
  }

  function layoutKey(g, name) { return g + '/' + name; }

  function uniqueName(nodes, base) {
    var has = function (k) { return Object.prototype.hasOwnProperty.call(nodes, k); };
    if (!has(base)) return base;
    for (var i = 2; ; i++) if (!has(base + '_' + i)) return base + '_' + i;
  }

  // ── Types and colour ───────────────────────────────────────────────────────
  // Case and whitespace are folded before compare (the recommended answer to
  // open issue 6), and the hue is hashed after the same folding, so two labels
  // that count as one type always share a colour.
  //
  // The fold is the server's, code point by code point (src/blueprint_doc.rs
  // type_key): drop Unicode White_Space, which is Rust's char::is_whitespace
  // and not quite JS's \s (that has U+FEFF and lacks U+0085), and lowercase
  // each code point alone, so no context rule such as the Greek final sigma
  // applies. Otherwise a header count and the server's outline could differ.
  var WHITE = /[\t-\r \u0085\u00a0\u1680\u2000-\u200a\u2028\u2029\u202f\u205f\u3000]/;
  function typeKey(t) {
    return Array.from(String(t)).filter(function (c) { return !WHITE.test(c); })
      .map(function (c) { return c.toLowerCase(); }).join('');
  }

  function fnv1a(s) {
    var h = 0x811c9dc5;
    for (var i = 0; i < s.length; i++) {
      h ^= s.charCodeAt(i);
      h = Math.imul(h, 0x01000193) >>> 0;
    }
    return h;
  }

  // 30..329: the rose band either side of 0 is kept for mismatches.
  function hueOf(t) { return 30 + (fnv1a(typeKey(t)) % 300); }
  function portColor(t) { return 'hsl(' + hueOf(t) + ' 70% 70%)'; }

  // ── Node size ──────────────────────────────────────────────────────────────
  function ceilU(x) { return Math.ceil(x - EPS); }

  function wrapWords(text, max) {
    var lines = [], cur = '';
    String(text || '').split(/\s+/).filter(Boolean).forEach(function (w) {
      if (!cur) cur = w;
      else if ((cur + ' ' + w).length <= max) cur += ' ' + w;
      else { lines.push(cur); cur = w; }
    });
    if (cur) lines.push(cur);
    return lines.map(function (l) { return l.length > max ? l.slice(0, max - 1) + '…' : l; });
  }

  function footText(doc, n) {
    var sub = doc && doc.graphs ? doc.graphs[n.graph] : null;
    return sub
      ? 'graph ' + n.graph + ' · ' + Object.keys(sub.nodes || {}).length + ' nodes'
      : 'graph ' + (n.graph || '?') + ' · missing';
  }

  // Width and height round up to whole units, so a node's edges sit on grid
  // lines. Each term counts the real borders, paddings and gaps of the markup
  // the view draws, so the box is never narrower than what it holds.
  function measure(name, n, doc) {
    var outs = Object.keys(n.out || {}).map(function (k) { return [k, n.out[k]]; });
    var ins = Object.keys(n.in || {}).map(function (k) { return [k, n.in[k]]; });
    var longest = 0;
    outs.concat(ins).forEach(function (p) { longest = Math.max(longest, p[0].length + 1 + String(p[1]).length); });
    var portW = 1.875 + longest * CH;
    var status = n.status || '';
    var badgeW = status ? 1.4375 + status.length * CH_BADGE : 0;
    var iconW = n.kind && n.kind !== 'machine' ? 1.375 : 0;
    var titleW = 1.5 + iconW + name.length * CH_TITLE + (badgeW ? 0.5 + badgeW : 0);
    var footW = 0, foot = '';
    if (n.kind === 'blueprint') {
      foot = footText(doc, n);
      footW = 2.75 + (foot.length + 12) * CH_NOTE;
    }
    var w = Math.max(MIN_W, ceilU(Math.max(portW, titleW, footW)));
    var perLine = Math.floor((w - 1.5) / CH_NOTE + EPS);
    var noteLines = n.note ? wrapWords(n.note, perLine) : [];
    var rows = Math.max(outs.length + ins.length, 1);
    var raw = 2 * BORDER + HEAD + PAD + rows * ROW + PAD
      + (noteLines.length ? noteLines.length * NOTE_LH + NOTE_GAP : 0)
      + (n.kind === 'blueprint' ? FOOT : 0);
    return { w: w, h: ceilU(raw), outs: outs, ins: ins, noteLines: noteLines, foot: foot };
  }

  // Where a port's dot sits, relative to its node's top-left corner. Outputs
  // are the first rows and sit on the right edge; inputs follow on the left.
  function portAt(m, side, port) {
    var row = -1;
    if (side === 'out') {
      for (var i = 0; i < m.outs.length; i++) if (m.outs[i][0] === port) row = i;
    } else {
      for (var j = 0; j < m.ins.length; j++) if (m.ins[j][0] === port) row = m.outs.length + j;
    }
    if (row < 0) return null;
    return [side === 'out' ? m.w : 0, BORDER + HEAD + PAD + row * ROW + ROW / 2];
  }

  // Every node of a graph, measured and placed: { name: { n, m, x, y } }.
  function geometry(doc, g) {
    var graph = doc.graphs[g] || { nodes: {}, wires: [] };
    var geo = {};
    Object.keys(graph.nodes).forEach(function (name) {
      var pos = (doc.layout && doc.layout[layoutKey(g, name)]) || [0, 0];
      geo[name] = { n: graph.nodes[name], m: measure(name, graph.nodes[name], doc), x: pos[0], y: pos[1] };
    });
    return geo;
  }

  function portXY(geo, node, side, port) {
    var q = geo[node];
    if (!q) return null;
    var p = portAt(q.m, side, port);
    return p ? [q.x + p[0], q.y + p[1]] : null;
  }

  // The box around every node, its blueprint stack and, when `checks` is
  // given, each dangling wire's stub and tag, so Fit shows all of them.
  function bounds(geo, checks) {
    var b = null;
    function grow(x1, y1, x2, y2) {
      if (!b) b = { x1: x1, y1: y1, x2: x2, y2: y2 };
      else {
        b.x1 = Math.min(b.x1, x1); b.y1 = Math.min(b.y1, y1);
        b.x2 = Math.max(b.x2, x2); b.y2 = Math.max(b.y2, y2);
      }
    }
    Object.keys(geo).forEach(function (k) {
      var q = geo[k], extra = q.n.kind === 'blueprint' ? 0.375 : 0;
      grow(q.x, q.y, q.x + q.m.w + extra, q.y + q.m.h + extra);
    });
    (checks || []).forEach(function (w) {
      if (!w.dangling) return;
      var tag = danglingTag(w);
      if (w.fromOk) {
        var A = portXY(geo, w.an, 'out', w.ap);
        if (A) grow(A[0], A[1] - 0.625, A[0] + STUB + 0.375 + tag.w, A[1] + 0.625);
      } else if (w.toOk) {
        var B = portXY(geo, w.bn, 'in', w.bp);
        if (B) grow(B[0] - STUB - 0.375 - tag.w, B[1] - 0.625, B[0], B[1] + 0.625);
      }
    });
    return b;
  }

  // A dangling wire is drawn as a short stub off the port that still exists,
  // with a tag naming the end that does not.
  var STUB = 2.5;
  function danglingTag(w) {
    var text = w.fromOk ? '→ ' + w.b + '  ?' : '?  ' + w.a + ' →';
    // 11px mono, 0.375u padding and a 1px border each side.
    return { text: text, w: text.length * CH_NOTE + 0.875 };
  }

  // ── Wires ──────────────────────────────────────────────────────────────────
  // Cubic bezier with both tangents horizontal: a wire always leaves an output
  // to the right and enters an input from the left. Going backwards it loops
  // out of the node instead of cutting through it.
  function wireCurve(x1, y1, x2, y2) {
    var dx = x2 - x1;
    var c = dx >= 0 ? Math.max(3, dx / 2) : Math.max(6, -dx / 2 + 3);
    return { x1: x1, y1: y1, c1x: x1 + c, c1y: y1, c2x: x2 - c, c2y: y2, x2: x2, y2: y2 };
  }

  function r3(n) { return Math.round(n * 1000) / 1000; }

  function pathD(cv) {
    return 'M' + r3(cv.x1) + ' ' + r3(cv.y1) + 'C' + r3(cv.c1x) + ' ' + r3(cv.c1y) + ' '
      + r3(cv.c2x) + ' ' + r3(cv.c2y) + ' ' + r3(cv.x2) + ' ' + r3(cv.y2);
  }

  function curveMid(cv) {
    return [(cv.x1 + 3 * cv.c1x + 3 * cv.c2x + cv.x2) / 8, (cv.y1 + 3 * cv.c1y + 3 * cv.c2y + cv.y2) / 8];
  }

  // One entry per wire: what it joins and what is wrong with it. A dangling
  // wire stays in the file and is counted, never dropped.
  function checkGraph(doc, g) {
    var graph = doc.graphs[g] || { nodes: {}, wires: [] };
    return (graph.wires || []).map(function (wire, idx) {
      var a = wire[0], b = wire[1];
      var A = split(a), B = split(b);
      var na = graph.nodes[A[0]], nb = graph.nodes[B[0]];
      var ft = na && na.out ? na.out[A[1]] : undefined;
      var tt = nb && nb.in ? nb.in[B[1]] : undefined;
      var why = '';
      if (!na) why = 'no node "' + A[0] + '"';
      else if (ft === undefined) why = 'no out port "' + a + '"';
      else if (!nb) why = 'no node "' + B[0] + '"';
      else if (tt === undefined) why = 'no in port "' + b + '"';
      var dangling = !!why;
      return {
        idx: idx, a: a, b: b, an: A[0], ap: A[1], bn: B[0], bp: B[1], ft: ft, tt: tt, why: why,
        dangling: dangling, mismatch: !dangling && typeKey(ft) !== typeKey(tt),
        fromOk: ft !== undefined, toOk: tt !== undefined,
      };
    });
  }

  function summarize(doc, g) {
    var s = { nodes: Object.keys((doc.graphs[g] || { nodes: {} }).nodes).length, wires: 0, dangling: 0, mismatch: 0 };
    checkGraph(doc, g).forEach(function (w) {
      s.wires++;
      if (w.dangling) s.dangling++;
      if (w.mismatch) s.mismatch++;
    });
    return s;
  }

  // The root first, then the other graphs in document order: the order the
  // outline lists them in.
  function graphOrder(doc) {
    return [doc.root].concat(Object.keys(doc.graphs).filter(function (g) { return g !== doc.root; }));
  }

  // The whole plan's counts, every graph added up. This is the header's line
  // and what the server answers a save with.
  function summarizeDoc(doc) {
    var s = { nodes: 0, wires: 0, dangling: 0, mismatch: 0 };
    graphOrder(doc).forEach(function (g) {
      var t = summarize(doc, g);
      s.nodes += t.nodes; s.wires += t.wires; s.dangling += t.dangling; s.mismatch += t.mismatch;
    });
    return s;
  }

  // ── Validation ─────────────────────────────────────────────────────────────
  // What makes a document unreadable, as opposed to merely unfinished. A
  // dangling wire and a type mismatch are neither: they are counted, never
  // refused. Returns one sentence per problem; an empty list means the server
  // will take it. src/blueprint_doc.rs refuses exactly the same things, and
  // the tests pin both to the same fixtures.
  function isObj(x) { return !!x && typeof x === 'object' && !Array.isArray(x); }

  function validate(doc) {
    var errs = [];
    if (!isObj(doc)) return ['the plan is not a JSON object'];
    if (doc.v !== 1) errs.push('"v" must be 1');
    if (!isObj(doc.graphs)) return errs.concat(['"graphs" must be an object']);
    if (typeof doc.root !== 'string' || !Object.prototype.hasOwnProperty.call(doc.graphs, doc.root)) {
      errs.push('"root" must name a graph');
    }
    Object.keys(doc.graphs).forEach(function (g) {
      var graph = doc.graphs[g];
      if (!isName(g)) errs.push('graph "' + g + '": not a valid name');
      if (!isObj(graph)) { errs.push('graph "' + g + '" must be an object'); return; }
      if (!isObj(graph.nodes)) errs.push('graph "' + g + '": "nodes" must be an object');
      else Object.keys(graph.nodes).forEach(function (name) {
        var n = graph.nodes[name], at = g + '/' + name;
        if (!isName(name)) errs.push('node ' + at + ': not a valid name');
        if (!isObj(n)) { errs.push('node ' + at + ' must be an object'); return; }
        if (KINDS.indexOf(n.kind) < 0) errs.push('node ' + at + ': unknown kind ' + JSON.stringify(n.kind));
        if (n.status !== undefined && STATUSES.indexOf(n.status) < 0) errs.push('node ' + at + ': unknown status ' + JSON.stringify(n.status));
        if (n.note !== undefined && typeof n.note !== 'string') errs.push('node ' + at + ': "note" must be text');
        ['in', 'out'].forEach(function (side) {
          if (n[side] === undefined) return;
          if (!isObj(n[side])) { errs.push('node ' + at + ': "' + side + '" must be an object'); return; }
          Object.keys(n[side]).forEach(function (p) {
            if (!isName(p)) errs.push('node ' + at + ': ' + side + ' port "' + p + '" is not a valid name');
            if (typeof n[side][p] !== 'string') errs.push('node ' + at + ': ' + side + ' port "' + p + '" needs a text type');
          });
        });
        if (n.kind === 'blueprint' && (typeof n.graph !== 'string' || !Object.prototype.hasOwnProperty.call(doc.graphs, n.graph))) {
          errs.push('node ' + at + ': blueprint names a missing graph ' + JSON.stringify(n.graph));
        }
      });
      if (!Array.isArray(graph.wires)) errs.push('graph "' + g + '": "wires" must be a list');
      else graph.wires.forEach(function (w, i) {
        var good = Array.isArray(w) && w.length === 2 && w.every(function (ref) {
          if (typeof ref !== 'string') return false;
          var s = split(ref);
          return ref.indexOf('.') > 0 && isName(s[0]) && isName(s[1]);
        });
        if (!good) errs.push('graph "' + g + '": wire ' + i + ' is not a pair of "node.port"');
      });
    });
    if (doc.layout !== undefined) {
      if (!isObj(doc.layout)) errs.push('"layout" must be an object');
      else Object.keys(doc.layout).forEach(function (k) {
        var p = doc.layout[k];
        if (!Array.isArray(p) || p.length !== 2 || !p.every(function (x) { return typeof x === 'number' && isFinite(x); })) {
          errs.push('layout "' + k + '" must be [x, y]');
        }
      });
    }
    return errs;
  }

  // ── The view transform ─────────────────────────────────────────────────────
  // view = { x, y, z }: screen px = view.x + world u · unitPx · view.z, where
  // unitPx is the root font size in px.
  function toScreen(view, unitPx, wx, wy) {
    return [view.x + wx * unitPx * view.z, view.y + wy * unitPx * view.z];
  }

  function toWorld(view, unitPx, sx, sy) {
    return [(sx - view.x) / (unitPx * view.z), (sy - view.y) / (unitPx * view.z)];
  }

  function clampZoom(z) { return Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, z)); }

  // Zoom to z, keeping the world point under (sx, sy) where it is on screen.
  function zoomAt(view, unitPx, sx, sy, z) {
    var nz = clampZoom(z);
    var w = toWorld(view, unitPx, sx, sy);
    return { x: sx - w[0] * unitPx * nz, y: sy - w[1] * unitPx * nz, z: nz };
  }

  // The zoom and offset that fit a box of world units into a viewport, with a
  // margin of 2u (on screen at zoom 1) all round. Fit never zooms past 100%.
  function fitView(b, unitPx, vw, vh) {
    if (!b) return { x: 2 * unitPx, y: 2 * unitPx, z: 1 };
    var m = 2 * unitPx;
    var bw = Math.max(1, b.x2 - b.x1) * unitPx, bh = Math.max(1, b.y2 - b.y1) * unitPx;
    var z = clampZoom(Math.min(1, (vw - 2 * m) / bw, (vh - 2 * m) / bh));
    return {
      x: (vw - bw * z) / 2 - b.x1 * unitPx * z,
      y: (vh - bh * z) / 2 - b.y1 * unitPx * z,
      z: z,
    };
  }

  // ── Hit testing ────────────────────────────────────────────────────────────
  // The nearest port to a world point within the screen hit radius, on the
  // side a wire could end on. `want` is 'in' or 'out'; `skip` is the node the
  // wire starts from (a machine is never wired to itself).
  function nearestPort(geo, wx, wy, zoom, want, skip) {
    var r = HIT_SCREEN / zoom, best = null;
    Object.keys(geo).forEach(function (name) {
      if (name === skip) return;
      var q = geo[name];
      // Cheap reject: the point is nowhere near this node's box.
      if (wx < q.x - r || wx > q.x + q.m.w + r || wy < q.y - r || wy > q.y + q.m.h + r) return;
      var list = want === 'out' ? q.m.outs : q.m.ins;
      list.forEach(function (p) {
        var at = portAt(q.m, want, p[0]);
        var dx = q.x + at[0] - wx, dy = q.y + at[1] - wy, d = Math.sqrt(dx * dx + dy * dy);
        if (d <= r && (!best || d < best.d)) best = { node: name, side: want, port: p[0], d: d };
      });
    });
    return best;
  }

  // Nodes whose box overlaps a world rectangle, for box select.
  function nodesInRect(geo, x1, y1, x2, y2) {
    var lx = Math.min(x1, x2), hx = Math.max(x1, x2), ly = Math.min(y1, y2), hy = Math.max(y1, y2);
    return Object.keys(geo).filter(function (name) {
      var q = geo[name];
      return q.x < hx && q.x + q.m.w > lx && q.y < hy && q.y + q.m.h > ly;
    });
  }

  // ── Keyboard navigation ────────────────────────────────────────────────────
  // Reading order for Tab: by x, then y, then name. It comes from the layout,
  // so it matches what is on screen.
  function readingOrder(geo) {
    return Object.keys(geo).sort(function (a, b) {
      var A = geo[a], B = geo[b];
      return A.x - B.x || A.y - B.y || (a < b ? -1 : a > b ? 1 : 0);
    });
  }

  // The nearest node centre in a direction ('left', 'right', 'up', 'down'),
  // measured centre to centre. Only nodes inside the 90° cone count.
  function nearestInDirection(geo, from, dir) {
    var q = geo[from];
    if (!q) return null;
    var cx = q.x + q.m.w / 2, cy = q.y + q.m.h / 2, best = null, bestD = Infinity;
    Object.keys(geo).forEach(function (name) {
      if (name === from) return;
      var o = geo[name], dx = o.x + o.m.w / 2 - cx, dy = o.y + o.m.h / 2 - cy;
      var along = dir === 'right' ? dx : dir === 'left' ? -dx : dir === 'down' ? dy : -dy;
      var across = dir === 'right' || dir === 'left' ? Math.abs(dy) : Math.abs(dx);
      if (along <= 0 || across > along) return;
      var d = dx * dx + dy * dy;
      if (d < bestD) { bestD = d; best = name; }
    });
    return best;
  }

  // A node's ports in drawn order: outputs first, then inputs.
  function portList(m) {
    return m.outs.map(function (p) { return { side: 'out', port: p[0], type: p[1] }; })
      .concat(m.ins.map(function (p) { return { side: 'in', port: p[0], type: p[1] }; }));
  }

  // ── Edits ──────────────────────────────────────────────────────────────────
  // Each edit takes a document and changes it in place. Undo is a snapshot
  // per action, taken by the caller, as the sorting board does.
  function clone(doc) { return JSON.parse(JSON.stringify(doc)); }

  function moveNodes(doc, g, names, dx, dy) {
    names.forEach(function (name) {
      if (!doc.graphs[g].nodes[name]) return;
      var k = layoutKey(g, name), p = doc.layout[k] || [0, 0];
      doc.layout[k] = [p[0] + dx, p[1] + dy];
    });
  }

  function addNode(doc, g, base, at, node) {
    var nodes = doc.graphs[g].nodes, name = uniqueName(nodes, base);
    nodes[name] = node;
    doc.layout[layoutKey(g, name)] = [Math.round(at[0]), Math.round(at[1])];
    return name;
  }

  // Deleting a machine from the canvas takes its wires with it. (A dangling
  // wire is what an outside edit leaves behind; the canvas never makes one.)
  function removeNodes(doc, g, names) {
    var graph = doc.graphs[g], gone = {};
    names.forEach(function (name) {
      gone[name] = true;
      delete graph.nodes[name];
      delete doc.layout[layoutKey(g, name)];
    });
    graph.wires = graph.wires.filter(function (w) { return !gone[split(w[0])[0]] && !gone[split(w[1])[0]]; });
  }

  function removeWire(doc, g, idx) { doc.graphs[g].wires.splice(idx, 1); }

  // Joins two ports, whichever end the drag started from. Returns the stored
  // [out, in] pair, or null for a pair that cannot be a wire: two outputs, two
  // inputs, a machine to itself, or a wire that already exists. Fan-in and
  // type mismatches are allowed.
  function connect(doc, g, p, q) {
    if (!p || !q || p.side === q.side || p.node === q.node) return null;
    var nodes = doc.graphs[g].nodes;
    if (!nodes[p.node] || !nodes[q.node]) return null;
    var from = p.side === 'out' ? p : q, to = p.side === 'out' ? q : p;
    var a = from.node + '.' + from.port, b = to.node + '.' + to.port;
    var wires = doc.graphs[g].wires;
    for (var i = 0; i < wires.length; i++) if (wires[i][0] === a && wires[i][1] === b) return null;
    wires.push([a, b]);
    return [a, b];
  }

  // ── Renames and the inspector's edits ──────────────────────────────────────
  // Everything is referenced by name, so a rename has to rewrite every wire
  // that names the old one, or it would leave them dangling. Each returns an
  // error sentence, or '' when the edit was made. Key order is kept: a node's
  // place in `nodes` is the outline's order, and a port's is its row.
  function renameKey(obj, from, to) {
    var out = {};
    Object.keys(obj).forEach(function (k) { out[k === from ? to : k] = obj[k]; });
    return out;
  }

  // `what` carries its article: 'a node', 'an output'.
  function nameProblem(name, taken, what) {
    if (RESERVED.indexOf(name) >= 0) return name + ' is reserved: every JavaScript object already has it.';
    if (!isName(name)) return 'A name starts with a letter or _, then up to 63 letters, digits, _ or -.';
    if (taken) return 'There is already ' + what + ' called ' + name + '.';
    return '';
  }

  function renameNode(doc, g, from, to) {
    var graph = doc.graphs[g];
    if (!graph || !graph.nodes[from]) return 'There is no node called ' + from + '.';
    if (from === to) return '';
    var err = nameProblem(to, Object.prototype.hasOwnProperty.call(graph.nodes, to), 'a node');
    if (err) return err;
    graph.nodes = renameKey(graph.nodes, from, to);
    graph.wires = graph.wires.map(function (w) {
      return w.map(function (ref) { var s = split(ref); return s[0] === from ? to + '.' + s[1] : ref; });
    });
    var kf = layoutKey(g, from);
    if (doc.layout && doc.layout[kf]) { doc.layout[layoutKey(g, to)] = doc.layout[kf]; delete doc.layout[kf]; }
    return '';
  }

  // A port is renamed on one side only. A node may have an input and an output
  // of the same name, and only the wires on the renamed side follow.
  function renamePort(doc, g, node, side, from, to) {
    var n = doc.graphs[g] && doc.graphs[g].nodes[node];
    if (!n || !n[side] || !Object.prototype.hasOwnProperty.call(n[side], from)) return 'There is no ' + side + ' port called ' + from + '.';
    if (from === to) return '';
    var err = nameProblem(to, Object.prototype.hasOwnProperty.call(n[side], to), side === 'out' ? 'an output' : 'an input');
    if (err) return err;
    n[side] = renameKey(n[side], from, to);
    var end = side === 'out' ? 0 : 1, old = node + '.' + from;
    doc.graphs[g].wires.forEach(function (w) { if (w[end] === old) w[end] = node + '.' + to; });
    return '';
  }

  function setPortType(doc, g, node, side, port, type) {
    var n = doc.graphs[g] && doc.graphs[g].nodes[node];
    if (!n || !n[side] || !Object.prototype.hasOwnProperty.call(n[side], port)) return 'There is no ' + side + ' port called ' + port + '.';
    n[side][port] = String(type);
    return '';
  }

  // Adds a port at the end of its side, under a free name. Returns the name.
  function addPort(doc, g, node, side, base, type) {
    var n = doc.graphs[g].nodes[node];
    n[side] = n[side] || {};
    var name = uniqueName(n[side], base || side);
    n[side][name] = String(type == null ? 'Item' : type);
    return name;
  }

  // Removing a port takes its wires with it, as removing a node does.
  function removePort(doc, g, node, side, port) {
    var graph = doc.graphs[g], n = graph && graph.nodes[node];
    if (!n || !n[side] || !Object.prototype.hasOwnProperty.call(n[side], port)) return 'There is no ' + side + ' port called ' + port + '.';
    delete n[side][port];
    var end = side === 'out' ? 0 : 1, ref = node + '.' + port;
    graph.wires = graph.wires.filter(function (w) { return w[end] !== ref; });
    return '';
  }

  function setStatus(doc, g, node, status) {
    var n = doc.graphs[g] && doc.graphs[g].nodes[node];
    if (!n) return 'There is no node called ' + node + '.';
    if (STATUSES.indexOf(status) < 0) return 'A status is one of ' + STATUSES.join(', ') + '.';
    n.status = status;
    return '';
  }

  // An empty note is removed rather than stored as "".
  function setNote(doc, g, node, note) {
    var n = doc.graphs[g] && doc.graphs[g].nodes[node];
    if (!n) return 'There is no node called ' + node + '.';
    var text = String(note == null ? '' : note).trim();
    if (text) n.note = text; else delete n.note;
    return '';
  }

  // ── A load test ────────────────────────────────────────────────────────────
  // A deterministic plan of `count` machines and `wireCount` wires on a grid,
  // for the spike's "drags smoothly" check. Every machine has two outputs and
  // two inputs; wires run left to right with a fixed-seed generator, so a
  // reload draws the same plan. Each wire joins x to a or y to b, and those
  // pairs share a type, so the plan has no mismatches to dash.
  function stressDoc(count, wireCount) {
    var types = ['Request', 'Row', 'Vec<Row>', 'Html'];
    var doc = { v: 1, root: 'main', graphs: { main: { nodes: {}, wires: [] } }, layout: {} };
    var nodes = doc.graphs.main.nodes, cols = Math.ceil(Math.sqrt(count * 1.6));
    var seed = 7;
    function rnd(n) { seed = (Math.imul(seed, 1103515245) + 12345) >>> 0; return (seed >>> 8) % n; }
    var names = [];
    for (var i = 0; i < count; i++) {
      var name = 'm' + String(i).padStart(3, '0');
      names.push(name);
      nodes[name] = {
        kind: 'machine', status: STATUSES[i % 4],
        in: { a: types[0], b: types[3] },
        out: { x: types[0], y: types[3] },
      };
      doc.layout['main/' + name] = [(i % cols) * 14, Math.floor(i / cols) * 10];
    }
    var made = 0, tries = 0;
    while (made < wireCount && tries < wireCount * 20) {
      tries++;
      var s = rnd(count), t = rnd(count);
      if (s === t) continue;
      if (s > t) { var tmp = s; s = t; t = tmp; }
      var pair = rnd(2);
      var w = connect(doc, 'main',
        { node: names[s], side: 'out', port: pair ? 'x' : 'y' },
        { node: names[t], side: 'in', port: pair ? 'a' : 'b' });
      if (w) made++;
    }
    return doc;
  }

  var api = {
    U: { HEAD: HEAD, PAD: PAD, ROW: ROW, NOTE_LH: NOTE_LH, FOOT: FOOT, BORDER: BORDER, MIN_W: MIN_W },
    ZOOM_MIN: ZOOM_MIN, ZOOM_MAX: ZOOM_MAX, HIT_SCREEN: HIT_SCREEN, STATUSES: STATUSES, KINDS: KINDS,
    NAME_RE: NAME_RE, RESERVED: RESERVED, isName: isName, validate: validate,
    graphOrder: graphOrder, summarizeDoc: summarizeDoc,
    renameNode: renameNode, renamePort: renamePort, setPortType: setPortType, addPort: addPort,
    removePort: removePort, setStatus: setStatus, setNote: setNote,
    split: split, layoutKey: layoutKey, uniqueName: uniqueName,
    typeKey: typeKey, fnv1a: fnv1a, hueOf: hueOf, portColor: portColor,
    wrapWords: wrapWords, measure: measure, portAt: portAt, geometry: geometry, portXY: portXY, bounds: bounds,
    STUB: STUB, danglingTag: danglingTag,
    wireCurve: wireCurve, pathD: pathD, curveMid: curveMid, checkGraph: checkGraph, summarize: summarize,
    toScreen: toScreen, toWorld: toWorld, clampZoom: clampZoom, zoomAt: zoomAt, fitView: fitView,
    nearestPort: nearestPort, nodesInRect: nodesInRect,
    readingOrder: readingOrder, nearestInDirection: nearestInDirection, portList: portList,
    clone: clone, moveNodes: moveNodes, addNode: addNode, removeNodes: removeNodes, removeWire: removeWire,
    connect: connect, stressDoc: stressDoc,
  };

  if (typeof module !== 'undefined' && module.exports) module.exports = api;
  else {
    root.BP = root.BP || {};
    root.BP.model = api;
    if (root.BP.tryBoot) root.BP.tryBoot();
  }
})(typeof window !== 'undefined' ? window : this);
