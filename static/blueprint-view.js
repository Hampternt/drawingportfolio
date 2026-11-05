// Blueprint planner: the view. Draws a plan into the spike's viewport and
// keeps the drawing in step with the app's state.
//
// One stage element carries the pan and zoom as a single CSS transform. Inside
// it, nodes are <button>s placed in rem (1rem = 1u), and wires are paths in an
// <svg> whose viewBox is one unit square drawn 1rem wide, so an SVG user unit
// is a grid unit too. Nothing here converts to pixels: the transform does.
//
// A drag redraws only the moved nodes and the wires that touch them.
(function (root) {
  'use strict';

  var SVG = 'http://www.w3.org/2000/svg';
  var STUB = 2.5; // a dangling wire's stub, in u

  function el(tag, cls, text) {
    var e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text != null) e.textContent = text;
    return e;
  }

  function svgEl(tag, attrs) {
    var e = document.createElementNS(SVG, tag);
    Object.keys(attrs || {}).forEach(function (k) { e.setAttribute(k, attrs[k]); });
    return e;
  }

  function place(e, x, y) { e.style.transform = 'translate(' + x + 'rem,' + y + 'rem)'; }

  function plural(n, one, many) { return n + ' ' + (n === 1 ? one : many); }

  // The layers icon from the design canvas, for a blueprint node's title.
  function layersIcon() {
    var s = svgEl('svg', { viewBox: '0 0 24 24', 'aria-hidden': 'true', class: 'bp-node__icon' });
    ['m12 2 10 5-10 5L2 7z', 'm2 17 10 5 10-5', 'm2 12 10 5 10-5'].forEach(function (d) {
      s.appendChild(svgEl('path', { d: d }));
    });
    return s;
  }

  function create(viewport, M) {
    var stage = viewport.querySelector('.bp-stage');
    var wireSvg = stage.querySelector('.bp-wires');
    var nodeLayer = stage.querySelector('.bp-nodes');
    var ghostSvg = stage.querySelector('.bp-ghost');
    var box = viewport.querySelector('.bp-box');

    var doc = null, g = 'main', geo = {};
    var nodeEls = {}, stackEls = {}, portEls = {}, wires = [], byNode = {};
    var ghostPath = svgEl('path', { class: 'bp-ghost__line' });
    var ghostEnd = svgEl('circle', { class: 'bp-ghost__end', r: 0.3125 });
    ghostSvg.appendChild(ghostPath);
    ghostSvg.appendChild(ghostEnd);
    var marked = { focus: null, target: null, from: null };

    function portKey(node, side, port) { return node + '\u0000' + side + '\u0000' + port; }

    function nodeLabel(name, q) {
      return name + ', ' + (q.n.kind || 'machine') + ', ' + (q.n.status || 'no status') + '. '
        + plural(q.m.outs.length, 'output', 'outputs') + ', ' + plural(q.m.ins.length, 'input', 'inputs') + '.';
    }

    function buildNode(name, q, wired, bad) {
      var n = q.n, m = q.m;
      var b = el('button', 'bp-node bp-node--' + (n.status || 'planned') + (n.kind === 'blueprint' ? ' bp-node--blueprint' : ''));
      b.type = 'button';
      b.tabIndex = -1;
      b.dataset.node = name;
      b.setAttribute('aria-label', nodeLabel(name, q));
      b.style.width = m.w + 'rem';
      b.style.height = m.h + 'rem';
      place(b, q.x, q.y);

      var head = el('span', 'bp-node__head');
      if (n.kind === 'blueprint') head.appendChild(layersIcon());
      head.appendChild(el('span', 'bp-node__title', name));
      if (n.status) {
        var badge = el('span', 'bp-node__badge');
        badge.appendChild(el('span', 'bp-node__badge-dot'));
        badge.appendChild(document.createTextNode(n.status));
        head.appendChild(badge);
      }
      b.appendChild(head);

      var body = el('span', 'bp-node__body');
      M.portList(m).forEach(function (p) {
        var ref = name + '.' + p.port;
        var row = el('span', 'bp-port bp-port--' + p.side);
        if (wired[p.side + ':' + ref]) row.classList.add('is-wired');
        if (p.side === 'in' && bad[ref]) row.classList.add('is-bad');
        row.dataset.side = p.side;
        row.dataset.port = p.port;
        row.style.setProperty('--bp-port', M.portColor(p.type));
        row.appendChild(el('span', 'bp-port__dot'));
        row.appendChild(el('span', 'bp-port__name', p.port));
        row.appendChild(el('span', 'bp-port__type', String(p.type)));
        body.appendChild(row);
        portEls[portKey(name, p.side, p.port)] = row;
      });
      b.appendChild(body);

      m.noteLines.forEach(function (line) { b.appendChild(el('span', 'bp-node__note', line)); });

      if (n.kind === 'blueprint') {
        var foot = el('span', 'bp-node__foot');
        foot.appendChild(el('span', null, m.foot));
        var later = el('span', 'bp-node__later', 'open · later');
        later.title = 'Entering a blueprint is a later slice';
        foot.appendChild(later);
        b.appendChild(foot);
      }
      return b;
    }

    // ── Wires ────────────────────────────────────────────────────────────────
    function wireGeom(w) {
      var A = w.fromOk ? M.portXY(geo, w.an, 'out', w.ap) : null;
      var B = w.toOk ? M.portXY(geo, w.bn, 'in', w.bp) : null;
      if (A && B) {
        var cv = M.wireCurve(A[0], A[1], B[0], B[1]);
        return { d: M.pathD(cv), label: w.mismatch ? M.curveMid(cv) : null };
      }
      if (A) return { d: 'M' + A[0] + ' ' + A[1] + 'H' + (A[0] + STUB), label: [A[0] + STUB + 0.375, A[1]] };
      if (B) return { d: 'M' + (B[0] - STUB) + ' ' + B[1] + 'H' + B[0], label: [B[0] - STUB - 0.375, B[1]] };
      return null;
    }

    function buildWire(w) {
      var gEl = svgEl('g', { class: 'bp-wire' + (w.mismatch ? ' is-mismatch' : '') + (w.dangling ? ' is-dangling' : ''), 'data-w': w.idx });
      var hit = svgEl('path', { class: 'bp-wire__hit' });
      var line = svgEl('path', { class: 'bp-wire__line' });
      if (!w.mismatch && !w.dangling) line.style.stroke = M.portColor(w.ft);
      gEl.appendChild(hit);
      gEl.appendChild(line);
      var tag = null;
      if (w.mismatch || w.dangling) {
        tag = el('span', 'bp-tag ' + (w.mismatch ? 'bp-tag--mismatch' : 'bp-tag--dangling' + (w.fromOk ? '' : ' bp-tag--left')));
        tag.textContent = w.mismatch ? w.ft + ' ≠ ' + w.tt : w.fromOk ? '→ ' + w.b + '  ?' : '?  ' + w.a + ' →';
      }
      return { w: w, g: gEl, hit: hit, line: line, tag: tag };
    }

    function drawWire(rec) {
      var gm = wireGeom(rec.w);
      if (!gm) { rec.g.style.display = 'none'; if (rec.tag) rec.tag.hidden = true; return; }
      rec.hit.setAttribute('d', gm.d);
      rec.line.setAttribute('d', gm.d);
      if (rec.tag && gm.label) place(rec.tag, gm.label[0], gm.label[1]);
    }

    // ── Whole redraw ─────────────────────────────────────────────────────────
    // Any change to what a plan *is* (a node, a port, a wire) redraws it all.
    // Moving only redraws what moved: see moved().
    function render(nextDoc, nextG) {
      doc = nextDoc;
      g = nextG;
      geo = M.geometry(doc, g);
      var checks = M.checkGraph(doc, g);
      var wired = {}, bad = {};
      checks.forEach(function (w) {
        if (w.fromOk) wired['out:' + w.a] = true;
        if (w.toOk) wired['in:' + w.b] = true;
        if (w.mismatch) bad[w.b] = true;
      });

      nodeEls = {}; stackEls = {}; portEls = {}; wires = []; byNode = {};
      marked = { focus: null, target: null, from: null };
      var nodeFrag = document.createDocumentFragment();
      var wireFrag = document.createDocumentFragment();
      var tagFrag = document.createDocumentFragment();

      M.readingOrder(geo).forEach(function (name) {
        var q = geo[name];
        if (q.n.kind === 'blueprint') {
          var st = el('span', 'bp-stack bp-stack--' + (q.n.status || 'planned'));
          st.setAttribute('aria-hidden', 'true');
          st.style.width = q.m.w + 'rem';
          st.style.height = q.m.h + 'rem';
          place(st, q.x + 0.375, q.y + 0.375);
          stackEls[name] = st;
          nodeFrag.appendChild(st);
        }
        var b = buildNode(name, q, wired, bad);
        nodeEls[name] = b;
        nodeFrag.appendChild(b);
      });

      checks.forEach(function (w) {
        var rec = buildWire(w);
        wires.push(rec);
        [w.an, w.bn].forEach(function (n) { (byNode[n] = byNode[n] || []).push(rec); });
        drawWire(rec);
        wireFrag.appendChild(rec.g);
        if (rec.tag) tagFrag.appendChild(rec.tag);
      });

      wireSvg.replaceChildren(wireFrag);
      nodeLayer.replaceChildren(nodeFrag, tagFrag);
    }

    // Redraw the named nodes at their layout positions, and every wire that
    // touches one of them, once each.
    function moved(names) {
      var seen = new Set();
      names.forEach(function (name) {
        var q = geo[name], pos = doc.layout[M.layoutKey(g, name)];
        if (!q || !pos) return;
        q.x = pos[0]; q.y = pos[1];
        place(nodeEls[name], q.x, q.y);
        if (stackEls[name]) place(stackEls[name], q.x + 0.375, q.y + 0.375);
        (byNode[name] || []).forEach(function (rec) {
          if (seen.has(rec)) return;
          seen.add(rec);
          drawWire(rec);
        });
      });
    }

    function setView(view, unitPx) {
      stage.style.transform = 'translate(' + view.x + 'px,' + view.y + 'px) scale(' + view.z + ')';
      viewport.style.setProperty('--bp-cell', unitPx * view.z + 'px');
      viewport.style.setProperty('--bp-ox', view.x + 'px');
      viewport.style.setProperty('--bp-oy', view.y + 'px');
      viewport.classList.toggle('is-far', view.z < 0.5);
    }

    function setSelection(sel, selWire) {
      Object.keys(nodeEls).forEach(function (name) {
        var on = sel.has(name);
        nodeEls[name].classList.toggle('is-selected', on);
        nodeEls[name].setAttribute('aria-pressed', on ? 'true' : 'false');
      });
      wires.forEach(function (rec) { rec.g.classList.toggle('is-selected', rec.w.idx === selWire); });
    }

    function mark(kind, p) {
      if (marked[kind]) marked[kind].classList.remove('is-' + kind);
      marked[kind] = p ? portEls[portKey(p.node, p.side, p.port)] || null : null;
      if (marked[kind]) marked[kind].classList.add('is-' + kind);
    }

    function setGhost(from, to) {
      if (!from || !to) { ghostSvg.classList.remove('is-on'); return; }
      // Drawn from the output end, so the curve bows the right way.
      var cv = from.side === 'out' ? M.wireCurve(from.x, from.y, to[0], to[1]) : M.wireCurve(to[0], to[1], from.x, from.y);
      ghostPath.setAttribute('d', M.pathD(cv));
      ghostEnd.setAttribute('cx', to[0]);
      ghostEnd.setAttribute('cy', to[1]);
      ghostSvg.classList.add('is-on');
    }

    function setBox(r) {
      if (!r) { box.hidden = true; return; }
      box.hidden = false;
      box.style.left = Math.min(r.x1, r.x2) + 'px';
      box.style.top = Math.min(r.y1, r.y2) + 'px';
      box.style.width = Math.abs(r.x2 - r.x1) + 'px';
      box.style.height = Math.abs(r.y2 - r.y1) + 'px';
    }

    return {
      render: render,
      moved: moved,
      setView: setView,
      setSelection: setSelection,
      mark: mark,
      setGhost: setGhost,
      setBox: setBox,
      geo: function () { return geo; },
      nodeEl: function (name) { return nodeEls[name] || null; },
    };
  }

  root.BP = root.BP || {};
  root.BP.view = { create: create };
  if (root.BP.tryBoot) root.BP.tryBoot();
})(window);
