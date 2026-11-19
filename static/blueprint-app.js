// Blueprints: the canvas app. Input, undo and boot.
//
// The page boots from the plan in #bp-doc. #bp-app's data-mode says what the
// canvas may do:
//
//   demo  the visitor demo: look, pan, zoom, select, follow wires
//   read  an owner's plan, read-only (slice S2)
//   edit  an owner's plan, edited and saved (slice S3)
//
// Read-only is not hidden buttons: in demo and read mode the handlers that
// move, add, delete or wire are never reached, and nothing is ever sent.
// The input model follows docs/design/blueprint-planner/canvas/Input-model.dc.html.
//
// hx-boost: this file can run more than once (every boosted visit re-executes
// it) and the three scripts can arrive in any order. Each one calls
// BP.tryBoot() when it lands, and boot() starts only once all three are here
// and the page has a canvas that is not already running. The only listeners
// outside the canvas are bound once, and they look the app up when they fire.
(function (root) {
  'use strict';

  var BP = root.BP = root.BP || {};

  function unitPx() {
    return parseFloat(getComputedStyle(document.documentElement).fontSize) || 16;
  }

  function isField(t) {
    return t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.tagName === 'SELECT' || t.isContentEditable);
  }

  function start(rootEl, M, V) {
    var editable = rootEl.dataset.mode === 'edit';
    var viewport = rootEl.querySelector('.bp-viewport');
    var countsEl = rootEl.querySelector('#bp-counts');
    var zoomEl = rootEl.querySelector('#bp-zoom');
    var liveEl = rootEl.querySelector('#bp-live');

    var U = unitPx();
    var doc = JSON.parse(rootEl.querySelector('#bp-doc').textContent);
    // v1 draws the root graph only; entering a blueprint is a later slice.
    var G = doc.root;
    var view = { x: 0, y: 0, z: 1 };
    var sel = new Set(), primary = null, selWire = null;
    var focusPort = null;   // the port ] and [ are on: { node, side, port }
    var kbWire = null;      // a wire started with W, waiting for Enter
    var undo = [], redo = [];
    var arrowRunAt = 0;     // a run of arrow presses is one undo step
    var spaceHeld = false;
    var drag = null, frame = 0, lastPt = null;
    var v = V.create(viewport, M);

    // Edit mode only. `rev` is the plan's version as this tab last saw it; it
    // goes out as If-Match on every save.
    var rev = Number(rootEl.dataset.rev) || 0;
    var saveUrl = rootEl.dataset.save || '', outlineUrl = rootEl.dataset.outline || '';
    var saveEl = rootEl.querySelector('#bp-save'), saveTextEl = rootEl.querySelector('#bp-save-text');
    var emptyEl = rootEl.querySelector('#bp-empty');
    var dirty = false, inFlight = false, saveTimer = 0, failures = 0, paused = false;
    var insp = null;   // the inspector, created at the end of start()
    var pop = null;    // the add-node search box, likewise

    // ── Drawing ──────────────────────────────────────────────────────────────
    // The whole plan, every graph: the same counts the server answers a save
    // with and the outline's headline shows.
    function updateCounts() {
      var s = M.summarizeDoc(doc);
      countsEl.textContent = s.nodes + (s.nodes === 1 ? ' machine' : ' machines') + ' · '
        + s.wires + (s.wires === 1 ? ' wire' : ' wires') + ' · ' + s.dangling + ' dangling · '
        + s.mismatch + ' mismatch';
      countsEl.classList.toggle('has-problems', !!(s.dangling || s.mismatch));
    }

    function applyView() {
      v.setView(view, U);
      zoomEl.textContent = Math.round(view.z * 100) + '% · 1u = ' + Math.round(U * view.z * 10) / 10 + 'px';
    }

    function updateSel() {
      v.setSelection(sel, selWire);
      v.mark('focus', focusPort);
      v.mark('from', kbWire);
      if (insp) insp.show(primary);
    }

    // A structural change redraws everything. The nodes are new elements
    // afterwards, so focus that was on one moves to its replacement.
    function rerender() {
      var hadFocus = viewport.contains(document.activeElement);
      var geoNodes = (doc.graphs[G] || { nodes: {} }).nodes;
      sel.forEach(function (n) { if (!geoNodes[n]) sel.delete(n); });
      if (primary && !geoNodes[primary]) primary = null;
      if (focusPort && !geoNodes[focusPort.node]) focusPort = null;
      if (kbWire && !geoNodes[kbWire.node]) kbWire = null;
      if (selWire !== null && selWire >= doc.graphs[G].wires.length) selWire = null;
      v.render(doc, G);
      if (insp) insp.refresh();
      updateSel();
      syncGhost();
      updateCounts();
      if (emptyEl) emptyEl.hidden = !editable || Object.keys(doc.graphs[G].nodes).length > 0;
      if (hadFocus) {
        var e = primary && v.nodeEl(primary);
        (e || viewport).focus({ preventScroll: true });
      }
    }

    // The keyboard wire's ghost is redrawn from wherever its port is now, and
    // hidden once there is no keyboard wire (a mouse drag draws its own).
    function syncGhost() {
      if (drag && drag.mode === 'wire') return;
      if (!kbWire) { v.setGhost(null); return; }
      var a = M.portXY(v.geo(), kbWire.node, kbWire.side, kbWire.port);
      if (!a) { kbWire = null; v.setGhost(null); return; }
      var to = focusPort && focusPort.node !== kbWire.node && M.portXY(v.geo(), focusPort.node, focusPort.side, focusPort.port);
      v.setGhost({ side: kbWire.side, x: a[0], y: a[1] }, to || [a[0] + (kbWire.side === 'out' ? 3 : -3), a[1]]);
    }

    function announce(msg) {
      // Cleared first, so the same sentence twice is still spoken twice.
      liveEl.textContent = '';
      requestAnimationFrame(function () { liveEl.textContent = msg; });
    }

    // ── Undo: a snapshot per action, as the sorting board does ───────────────
    function snapshot() {
      arrowRunAt = 0;
      undo.push(JSON.stringify(doc));
      if (undo.length > 200) undo.shift();
      redo = [];
    }

    function step(from, to, word) {
      if (!editable) return;
      if (!from.length) { announce('Nothing to ' + word + '.'); return; }
      to.push(JSON.stringify(doc));
      doc = JSON.parse(from.pop());
      arrowRunAt = 0;
      selWire = null; // a position in the wire list, which undo can reshuffle
      rerender();
      changed();
      announce(word === 'undo' ? 'Undone.' : 'Redone.');
    }

    // ── View ─────────────────────────────────────────────────────────────────
    function centre() { return [viewport.clientWidth / 2, viewport.clientHeight / 2]; }

    // Zoom waits while the pointer is dragging: every drag measures from where
    // it started on screen, and a zoom under it would move what it is holding.
    function zoomBy(factor, at) {
      if (drag) return;
      var p = at || centre();
      view = M.zoomAt(view, U, p[0], p[1], view.z * factor);
      applyView();
    }

    function zoomTo(z) {
      if (drag) return;
      var p = centre();
      view = M.zoomAt(view, U, p[0], p[1], z);
      applyView();
    }

    function fit() {
      if (drag) return;
      view = M.fitView(M.bounds(v.geo(), M.checkGraph(doc, G)), U, viewport.clientWidth, viewport.clientHeight);
      applyView();
    }

    // Pans just enough to bring a node fully into view, with a 1u margin.
    function reveal(name) {
      var q = v.geo()[name];
      if (!q) return;
      var a = M.toScreen(view, U, q.x - 1, q.y - 1), b = M.toScreen(view, U, q.x + q.m.w + 1, q.y + q.m.h + 1);
      var W = viewport.clientWidth, H = viewport.clientHeight, dx = 0, dy = 0;
      if (b[0] - a[0] > W || a[0] < 0) dx = -a[0]; else if (b[0] > W) dx = W - b[0];
      if (b[1] - a[1] > H || a[1] < 0) dy = -a[1]; else if (b[1] > H) dy = H - b[1];
      if (dx || dy) { view = { x: view.x + dx, y: view.y + dy, z: view.z }; applyView(); }
    }

    // ── Selection ────────────────────────────────────────────────────────────
    function focusNode(name) {
      var e = name && v.nodeEl(name);
      if (e) e.focus({ preventScroll: true });
    }

    function selectOnly(name, add) {
      if (!add) sel.clear();
      sel.add(name);
      primary = name;
      selWire = null;
      if (!focusPort || focusPort.node !== name) focusPort = null;
      updateSel();
      reveal(name);
      focusNode(name);
      var q = v.geo()[name];
      announce(name + ', ' + (q.n.kind || 'machine') + ', ' + (q.n.status || 'no status') + '. '
        + q.m.outs.length + ' out, ' + q.m.ins.length + ' in.' + (add ? ' ' + sel.size + ' selected.' : ''));
    }

    function clearSelection() {
      sel.clear();
      primary = null;
      selWire = null;
      focusPort = null;
      updateSel();
    }

    // ── Edits ────────────────────────────────────────────────────────────────
    function wireSentence(pair) {
      var w = M.checkGraph(doc, G).filter(function (c) { return c.a === pair[0] && c.b === pair[1]; })[0];
      return 'Wired ' + pair[0] + ' to ' + pair[1] + ', type ' + w.ft + (w.mismatch ? '. Type mismatch: ' + w.tt + '.' : '.');
    }

    function tryConnect(p, q) {
      if (!editable) return false;
      var before = JSON.stringify(doc);
      var pair = M.connect(doc, G, p, q);
      if (!pair) {
        announce(p.side === q.side ? 'Both ends are ' + (p.side === 'out' ? 'outputs' : 'inputs') + '.'
          : p.node === q.node ? 'A machine is not wired to itself.' : 'That wire is already there.');
        return false;
      }
      undo.push(before);
      redo = [];
      arrowRunAt = 0;
      rerender();
      changed();
      announce(wireSentence(pair));
      return true;
    }

    function addMachineAt(w) {
      if (!editable) return;
      snapshot();
      var name = M.addNode(doc, G, 'machine', [w[0] - 4.5, w[1] - 2], {
        kind: 'machine', status: 'idea', in: { in: 'Item' }, out: { out: 'Item' },
      });
      rerender();
      changed();
      selectOnly(name);
      announce('Added ' + name + '.');
    }

    function deleteSelection() {
      if (!editable) return;
      if (selWire !== null) {
        var w = doc.graphs[G].wires[selWire];
        snapshot();
        M.removeWire(doc, G, selWire);
        selWire = null;
        rerender();
        changed();
        announce('Deleted wire ' + w[0] + ' to ' + w[1] + '. Ctrl + Z to undo.');
        return;
      }
      if (!sel.size) return;
      var names = Array.from(sel);
      snapshot();
      M.removeNodes(doc, G, names);
      clearSelection();
      rerender();
      changed();
      viewport.focus({ preventScroll: true });
      announce('Deleted ' + (names.length === 1 ? names[0] : names.length + ' machines') + '. Ctrl + Z to undo.');
    }

    function moveSelection(dx, dy) {
      if (!editable || !sel.size) return;
      var now = performance.now();
      if (now - arrowRunAt > 700) snapshot();
      arrowRunAt = now;
      var names = Array.from(sel);
      M.moveNodes(doc, G, names, dx, dy);
      v.moved(names);
      syncGhost();
      if (primary) reveal(primary);
      changed();
    }

    // ── Pointer ──────────────────────────────────────────────────────────────
    var rect = null;
    // Measured from the padding box, where the stage and the box overlay sit,
    // not from the viewport's border.
    function local(e) { return [e.clientX - rect.left - viewport.clientLeft, e.clientY - rect.top - viewport.clientTop]; }

    function beginDrag(e, d) {
      drag = d;
      viewport.setPointerCapture(e.pointerId);
      drag.pointerId = e.pointerId;
    }

    function onDown(e) {
      if (drag) return;
      rect = viewport.getBoundingClientRect();
      var p = local(e);
      // Keys follow the pointer into the canvas, whatever had focus before.
      if (!viewport.contains(document.activeElement)) viewport.focus({ preventScroll: true });
      // Shift + right-click is the browser's own menu.
      if (e.button === 2 && e.shiftKey) return;
      if (e.button === 2) { beginDrag(e, { mode: 'right', s: p, v0: view }); return; }
      if (e.button === 1 || (e.button === 0 && spaceHeld)) {
        e.preventDefault(); // no middle-click autoscroll
        beginDrag(e, { mode: 'pan', s: p, v0: view });
        viewport.classList.add('is-panning');
        return;
      }
      if (e.button !== 0) return;

      var nodeEl = e.target.closest('.bp-node');
      var portEl = e.target.closest('.bp-port');
      var wireEl = e.target.closest('.bp-wire');
      if (nodeEl && portEl && editable) {
        e.preventDefault();
        var from = { node: nodeEl.dataset.node, side: portEl.dataset.side, port: portEl.dataset.port };
        var xy = M.portXY(v.geo(), from.node, from.side, from.port);
        from.x = xy[0]; from.y = xy[1];
        kbWire = null;
        v.mark('from', from);
        v.setGhost(from, xy);
        beginDrag(e, { mode: 'wire', s: p, from: from, target: null });
        return;
      }
      if (nodeEl) {
        var name = nodeEl.dataset.node, wasIn = sel.has(name);
        if (e.shiftKey) { if (!wasIn) sel.add(name); }
        else if (!wasIn) sel.clear();
        sel.add(name);
        primary = name;
        selWire = null;
        if (focusPort && focusPort.node !== name) focusPort = null;
        updateSel();
        focusNode(name);
        // Read-only, the press still selects; the drag simply never moves
        // anything (see tick()).
        beginDrag(e, {
          mode: 'move', s: p, name: name, names: Array.from(sel), applied: [0, 0],
          toggleOff: e.shiftKey && wasIn, narrow: !e.shiftKey && wasIn && sel.size > 1, snapped: false,
        });
        return;
      }
      if (wireEl) {
        sel.clear();
        primary = null;
        focusPort = null;
        selWire = Number(wireEl.dataset.w);
        updateSel();
        viewport.focus({ preventScroll: true });
        var w = doc.graphs[G].wires[selWire];
        announce('Wire ' + w[0] + ' to ' + w[1] + ' selected.' + (editable ? ' Delete removes it.' : ''));
        return;
      }
      viewport.focus({ preventScroll: true });
      beginDrag(e, { mode: 'box', s: p, base: e.shiftKey ? new Set(sel) : new Set(), moved: false });
    }

    function onMove(e) {
      if (!drag || e.pointerId !== drag.pointerId) return;
      lastPt = local(e);
      if (!frame) frame = requestAnimationFrame(tick);
    }

    // Pointer moves are coalesced to one update per frame.
    function tick() {
      frame = 0;
      if (!drag || !lastPt) return;
      var p = lastPt;

      if (drag.mode === 'right') {
        // Under 0.25u it is a click (the menu, in S3). Past that it pans.
        if (Math.hypot(p[0] - drag.s[0], p[1] - drag.s[1]) <= 0.25 * U) return;
        drag.mode = 'pan';
        viewport.classList.add('is-panning');
      }
      if (drag.mode === 'pan') {
        view = { x: drag.v0.x + p[0] - drag.s[0], y: drag.v0.y + p[1] - drag.s[1], z: view.z };
        applyView();
        return;
      }
      if (drag.mode === 'move') {
        if (!editable) return;
        var k = U * view.z;
        var du = Math.round((p[0] - drag.s[0]) / k), dv = Math.round((p[1] - drag.s[1]) / k);
        if (du === drag.applied[0] && dv === drag.applied[1]) return;
        if (!drag.snapped) { snapshot(); drag.snapped = true; }
        M.moveNodes(doc, G, drag.names, du - drag.applied[0], dv - drag.applied[1]);
        drag.applied = [du, dv];
        v.moved(drag.names);
        syncGhost();
        return;
      }
      if (drag.mode === 'wire') {
        var w = M.toWorld(view, U, p[0], p[1]);
        var want = drag.from.side === 'out' ? 'in' : 'out';
        drag.target = M.nearestPort(v.geo(), w[0], w[1], view.z, want, drag.from.node);
        v.mark('target', drag.target);
        v.setGhost(drag.from, drag.target ? M.portXY(v.geo(), drag.target.node, drag.target.side, drag.target.port) : w);
        return;
      }
      if (drag.mode === 'box') {
        if (!drag.moved && Math.hypot(p[0] - drag.s[0], p[1] - drag.s[1]) <= 0.25 * U) return;
        drag.moved = true;
        v.setBox({ x1: drag.s[0], y1: drag.s[1], x2: p[0], y2: p[1] });
        var a = M.toWorld(view, U, drag.s[0], drag.s[1]), b = M.toWorld(view, U, p[0], p[1]);
        sel = new Set(drag.base);
        M.nodesInRect(v.geo(), a[0], a[1], b[0], b[1]).forEach(function (n) { sel.add(n); });
        primary = null;
        selWire = null;
        updateSel();
      }
    }

    function endDrag(e, cancelled) {
      // A frame still pending holds the last move: apply it before letting go.
      if (frame) {
        cancelAnimationFrame(frame);
        frame = 0;
        if (!cancelled) tick();
      }
      var d = drag;
      drag = null;
      if (e && viewport.hasPointerCapture(d.pointerId)) viewport.releasePointerCapture(d.pointerId);
      viewport.classList.remove('is-panning');
      if (d.mode === 'wire') {
        // The drop is judged where the pointer let go, not at the last frame.
        if (!cancelled && e) {
          var w = M.toWorld(view, U, local(e)[0], local(e)[1]);
          d.target = M.nearestPort(v.geo(), w[0], w[1], view.z, d.from.side === 'out' ? 'in' : 'out', d.from.node);
        }
        v.setGhost(null);
        v.mark('target', null);
        v.mark('from', null);
        if (!cancelled && d.target) tryConnect(d.from, d.target);
        else if (!cancelled && e && pop) {
          // Dropped on empty grid: offer the machine at the wire's other end.
          var at = local(e);
          pop.open(d.from, at, M.toWorld(view, U, at[0], at[1]));
        }
        return;
      }
      if (d.mode === 'move') {
        if (!d.snapped && !cancelled) {
          if (d.toggleOff) { sel.delete(d.name); primary = null; updateSel(); }
          else if (d.narrow) selectOnly(d.name);
          else selectOnly(d.name, sel.size > 1);
        } else if (d.snapped) {
          changed();
          announce('Moved ' + (d.names.length === 1 ? d.names[0] : d.names.length + ' machines') + '.');
        }
        return;
      }
      if (d.mode === 'box') {
        v.setBox(null);
        if (!d.moved && !cancelled && !d.base.size) clearSelection();
        else if (d.moved) announce(sel.size + ' selected.');
      }
    }

    function onUp(e) {
      if (!drag || e.pointerId !== drag.pointerId) return;
      endDrag(e, false);
      lastPt = null;
    }

    function onCancel(e) {
      if (!drag || e.pointerId !== drag.pointerId) return;
      endDrag(e, true);
      lastPt = null;
    }

    function onWheel(e) {
      e.preventDefault();
      rect = viewport.getBoundingClientRect();
      var dy = e.deltaY * (e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? 400 : 1);
      zoomBy(Math.exp(-dy * 0.0015), local(e));
    }

    function onDouble(e) {
      if (!editable || e.target.closest('.bp-node') || e.target.closest('.bp-wire')) return;
      rect = viewport.getBoundingClientRect();
      var p = local(e);
      addMachineAt(M.toWorld(view, U, p[0], p[1]));
    }

    // ── Keyboard ─────────────────────────────────────────────────────────────
    var DIRS = { ArrowLeft: 'left', ArrowRight: 'right', ArrowUp: 'up', ArrowDown: 'down' };

    function walkPorts(delta) {
      if (!primary) { announce('Select a machine first.'); return; }
      var list = M.portList(v.geo()[primary].m);
      if (!list.length) { announce(primary + ' has no ports.'); return; }
      var i = -1;
      if (focusPort && focusPort.node === primary) {
        list.forEach(function (p, k) { if (p.side === focusPort.side && p.port === focusPort.port) i = k; });
      }
      var next = list[(i + delta + list.length * 2) % list.length];
      if (i < 0 && delta < 0) next = list[list.length - 1];
      if (i < 0 && delta > 0) next = list[0];
      focusPort = { node: primary, side: next.side, port: next.port };
      v.mark('focus', focusPort);
      if (kbWire) {
        var a = M.portXY(v.geo(), kbWire.node, kbWire.side, kbWire.port);
        v.setGhost({ side: kbWire.side, x: a[0], y: a[1] }, M.portXY(v.geo(), primary, next.side, next.port));
      }
      announce(primary + '.' + next.port + ', ' + (next.side === 'out' ? 'output' : 'input') + ', type ' + next.type + '.');
    }

    function startKeyboardWire() {
      if (!focusPort) { announce('Pick a port with ] first.'); return; }
      kbWire = { node: focusPort.node, side: focusPort.side, port: focusPort.port };
      var a = M.portXY(v.geo(), kbWire.node, kbWire.side, kbWire.port);
      v.mark('from', kbWire);
      v.setGhost({ side: kbWire.side, x: a[0], y: a[1] }, [a[0] + (kbWire.side === 'out' ? 3 : -3), a[1]]);
      announce('Wiring from ' + kbWire.node + '.' + kbWire.port + '. Go to another machine, pick a port with ] and press Enter. Esc cancels.');
    }

    function cancelKeyboardWire() {
      kbWire = null;
      v.mark('from', null);
      v.setGhost(null);
      announce('Wire cancelled.');
    }

    // > and < follow a wire downstream or upstream, from the focused port if
    // there is one, else from the machine. The first wire wins; the chooser
    // for several is a later slice.
    function follow(down) {
      if (!primary) return;
      var hit = M.checkGraph(doc, G).filter(function (w) {
        if (w.dangling) return false;
        if (down) return w.an === primary && (!focusPort || focusPort.side !== 'out' || w.ap === focusPort.port);
        return w.bn === primary && (!focusPort || focusPort.side !== 'in' || w.bp === focusPort.port);
      })[0];
      if (!hit) { announce('No wire ' + (down ? 'downstream' : 'upstream') + '.'); return; }
      var to = down ? hit.bn : hit.an;
      selectOnly(to);
      focusPort = { node: to, side: down ? 'in' : 'out', port: down ? hit.bp : hit.ap };
      v.mark('focus', focusPort);
    }

    function onKey(e) {
      if (isField(e.target)) return;
      var k = e.key, ctrl = e.ctrlKey || e.metaKey;

      if (k === 'Escape') {
        if (drag && drag.mode === 'wire') { endDrag(null, true); announce('Wire cancelled.'); }
        else if (kbWire) cancelKeyboardWire();
        else if (sel.size || selWire !== null || focusPort) { clearSelection(); viewport.focus({ preventScroll: true }); }
        else { var first = rootEl.querySelector('.bp-toolbar button'); if (first) first.focus(); }
        e.preventDefault();
        return;
      }
      if (drag && drag.mode !== 'pan' && drag.mode !== 'right') {
        // Mid-drag, an edit would pull the plan out from under the pointer.
        e.preventDefault();
        return;
      }
      if (k === ' ') {
        // Held, Space pans with the pointer. It never toggles the selection,
        // and the preventDefault keeps it from clicking a focused node.
        spaceHeld = true;
        viewport.classList.add('is-grab');
        e.preventDefault();
        return;
      }
      if (ctrl) {
        var lower = k.toLowerCase();
        if (editable && lower === 'z' && !e.shiftKey) { step(undo, redo, 'undo'); e.preventDefault(); return; }
        if (editable && ((lower === 'z' && e.shiftKey) || lower === 'y')) { step(redo, undo, 'redo'); e.preventDefault(); return; }
        if (lower === 'a') {
          sel = new Set(Object.keys(v.geo()));
          selWire = null;
          updateSel();
          announce(sel.size + ' selected.');
          e.preventDefault();
          return;
        }
      }
      if ((ctrl || e.altKey) && DIRS[k]) {
        var from = primary || M.readingOrder(v.geo())[0];
        var to = from && (from === primary ? M.nearestInDirection(v.geo(), from, DIRS[k]) : from);
        if (to) selectOnly(to, e.shiftKey);
        else announce('Nothing ' + DIRS[k] + ' of ' + from + '.');
        e.preventDefault();
        return;
      }
      if (ctrl || e.altKey) return; // everything else with a modifier is the browser's or the site's

      if (DIRS[k] && editable) {
        var n = e.shiftKey ? 4 : 1;
        moveSelection(k === 'ArrowLeft' ? -n : k === 'ArrowRight' ? n : 0, k === 'ArrowUp' ? -n : k === 'ArrowDown' ? n : 0);
        e.preventDefault();
        return;
      }
      if (k === 'Tab') {
        var order = M.readingOrder(v.geo());
        if (!order.length) return; // an empty plan lets Tab leave
        var i = order.indexOf(primary);
        var next = i < 0 ? (e.shiftKey ? order.length - 1 : 0) : (i + (e.shiftKey ? -1 : 1) + order.length) % order.length;
        selectOnly(order[next]);
        e.preventDefault();
        return;
      }
      if (k === 'Enter' && editable) {
        if (kbWire && focusPort && focusPort.node !== kbWire.node) {
          var a = kbWire;
          kbWire = null;
          v.setGhost(null);
          v.mark('from', null);
          tryConnect(a, focusPort);
        } else if (kbWire) announce('Pick a port on another machine first.');
        e.preventDefault();
        return;
      }
      switch (k) {
        case '+': case '=': zoomBy(1.25); break;
        case '-': case '_': case '−': zoomBy(0.8); break;
        case '0': zoomTo(1); break;
        case 'f': case 'F': fit(); break;
        case ']': walkPorts(1); break;
        case '[': walkPorts(-1); break;
        case '>': follow(true); break;
        case '<': follow(false); break;
        case 'w': case 'W': if (!editable) return; startKeyboardWire(); break;
        case 'a': case 'A': if (!editable) return; addMachineAt(M.toWorld(view, U, centre()[0], centre()[1])); break;
        case 'Delete': case 'Backspace': if (!editable) return; deleteSelection(); break;
        default: return;
      }
      e.preventDefault();
    }

    function onKeyUp(e) {
      if (e.key === ' ') { spaceHeld = false; viewport.classList.remove('is-grab'); e.preventDefault(); }
    }

    viewport.addEventListener('pointerdown', onDown);
    viewport.addEventListener('pointermove', onMove);
    viewport.addEventListener('pointerup', onUp);
    viewport.addEventListener('pointercancel', onCancel);
    viewport.addEventListener('lostpointercapture', onCancel);
    viewport.addEventListener('wheel', onWheel, { passive: false });
    viewport.addEventListener('dblclick', onDouble);
    viewport.addEventListener('keydown', onKey);
    viewport.addEventListener('keyup', onKeyUp);
    viewport.addEventListener('focusout', function (e) {
      if (!viewport.contains(e.relatedTarget)) { spaceHeld = false; viewport.classList.remove('is-grab'); }
    });
    viewport.addEventListener('contextmenu', function (e) { if (!e.shiftKey) e.preventDefault(); });

    // ── Saving ───────────────────────────────────────────────────────────────
    // Every edit marks the plan dirty and schedules a save. One save is in the
    // air at a time: the next waits for this one's answer, because it has to
    // send the rev that answer carries. Two saves sent from the same rev would
    // see the second refused, and the owner would get the "changed elsewhere"
    // banner for their own edit.
    function setSave(state, text) {
      if (!saveEl) return;
      saveEl.classList.remove('is-saved', 'is-pending', 'is-failed');
      saveEl.classList.add('is-' + state);
      saveTextEl.textContent = text;
    }

    function changed() {
      if (!saveUrl || paused) return;
      dirty = true;
      setSave('pending', 'unsaved changes');
      clearTimeout(saveTimer);
      saveTimer = setTimeout(save, 700);
    }

    function retryLater() {
      failures++;
      clearTimeout(saveTimer);
      saveTimer = setTimeout(save, Math.min(30000, 1000 * Math.pow(2, failures)));
    }

    function save() {
      saveTimer = 0;
      if (!dirty || inFlight || paused) return;
      // The server refuses what this refuses; there is no point sending it.
      var errs = M.validate(doc);
      if (errs.length) { setSave('failed', 'not saved: ' + errs[0]); return; }
      var body = JSON.stringify(doc);
      dirty = false;
      inFlight = true;
      setSave('pending', 'saving…');
      fetch(saveUrl, {
        method: 'PUT',
        credentials: 'same-origin',
        headers: { 'Content-Type': 'application/json', 'If-Match': '"' + rev + '"' },
        body: body,
      }).then(function (res) {
        return res.json().catch(function () { return null; }).then(function (j) { return { res: res, j: j }; });
      }).then(function (r) {
        inFlight = false;
        var res = r.res, j = r.j;
        if (res.ok && j && typeof j.rev === 'number') {
          rev = j.rev;
          rootEl.dataset.rev = String(rev);
          failures = 0;
          var railRev = document.querySelector('.bp-rail__plan[aria-current="page"] .bp-rail__rev');
          if (railRev) railRev.textContent = 'rev ' + rev;
          refreshOutline();
          if (dirty) save();
          else setSave('saved', 'saved · rev ' + rev);
          return;
        }
        dirty = true;
        if (res.status === 412) { pause(j && j.rev); return; }
        if (res.status === 400 || res.status === 413) {
          setSave('failed', 'not saved: ' + ((j && j.errors && j.errors[0]) || 'the server refused the plan'));
          return;
        }
        if (res.redirected || res.status === 401 || res.status === 404) {
          // Signed out (the save was sent to the login page) or the plan is
          // gone. Retrying would not help; saying so does.
          setSave('failed', 'not saved: ' + (res.status === 404 ? 'this plan no longer exists' : 'signed out — sign in again in another tab'));
          retryLater();
          return;
        }
        setSave('failed', 'not saved · retrying');
        retryLater();
      }).catch(function () {
        inFlight = false;
        dirty = true;
        setSave('failed', 'not saved · offline, retrying');
        retryLater();
      });
    }

    // A 412: someone (the AI, another tab) saved first. This tab stops editing
    // and saving; the banner's one action is Reload.
    function pause(current) {
      paused = true;
      editable = false;
      clearTimeout(saveTimer);
      if (pop) pop.close(true);
      setSave('failed', 'not saved · editing paused');
      var banner = rootEl.querySelector('#bp-banner');
      if (banner) {
        var detail = rootEl.querySelector('#bp-banner-detail');
        if (detail && current) {
          detail.textContent = 'Rev ' + current + ' was saved somewhere else, after this tab loaded rev ' + rev
            + '. Your last change was refused, so this tab has stopped saving. Reloading discards what it had not saved.';
        }
        banner.hidden = false;
      }
      viewport.classList.add('is-paused');
      if (insp) insp.refresh();
      announce('This plan changed elsewhere. Editing is paused; reload to continue.');
    }

    // The outline beside the canvas is the server's rendering, so it is
    // fetched again after each save rather than drawn a second time here.
    function refreshOutline() {
      var cur = rootEl.querySelector('#bp-outline');
      if (!outlineUrl || !cur) return;
      fetch(outlineUrl, { credentials: 'same-origin' }).then(function (res) {
        return res.ok ? res.text() : null;
      }).then(function (html) {
        var now = rootEl.querySelector('#bp-outline');
        if (!html || !now) return;
        var t = document.createElement('template');
        t.innerHTML = html.trim();
        var fresh = t.content.querySelector('#bp-outline');
        if (fresh) now.replaceWith(fresh);
      }).catch(function () {});
    }

    // Sends what is not saved yet, without waiting for an answer: the page is
    // going away. keepalive lets the request outlive it (up to 64 KB; a larger
    // plan relies on the autosave having run, 0.7 s after the last edit).
    function flush() {
      if (!dirty || inFlight || paused || !saveUrl) return;
      if (M.validate(doc).length) return;
      try {
        fetch(saveUrl, {
          method: 'PUT', credentials: 'same-origin', keepalive: true,
          headers: { 'Content-Type': 'application/json', 'If-Match': '"' + rev + '"' },
          body: JSON.stringify(doc),
        });
        dirty = false;
      } catch (err) { /* nothing left to tell */ }
    }

    // ── The editor's API, for the inspector and the add-node box ─────────────
    // One edit, one undo step: `fn` changes the document and returns '' or an
    // error sentence. An error puts the document back exactly as it was.
    function edit(fn) {
      if (!editable) return 'Editing is paused.';
      var before = JSON.stringify(doc);
      var err = fn(doc);
      if (err) { doc = JSON.parse(before); return err; }
      if (JSON.stringify(doc) === before) return '';
      undo.push(before);
      if (undo.length > 200) undo.shift();
      redo = [];
      arrowRunAt = 0;
      rerender();
      changed();
      return '';
    }

    var api = {
      doc: function () { return doc; },
      graph: function () { return G; },
      editable: function () { return editable; },
      rev: function () { return rev; },
      edit: edit,
      // Selects a machine without moving focus: the inspector keeps it.
      select: function (name) {
        if (!name || !doc.graphs[G].nodes[name]) return;
        sel = new Set([name]);
        primary = name;
        selWire = null;
        focusPort = null;
        updateSel();
        reveal(name);
      },
      focusCanvas: function () { viewport.focus({ preventScroll: true }); },
      deleteNode: function (name) {
        if (!editable || !doc.graphs[G].nodes[name]) return;
        sel = new Set([name]);
        primary = name;
        selWire = null;
        deleteSelection();
      },
      announce: announce,
    };

    // ── Toolbar ──────────────────────────────────────────────────────────────
    var actions = {
      fit: fit,
      'zoom-in': function () { zoomBy(1.25); },
      'zoom-out': function () { zoomBy(0.8); },
    };
    if (editable) {
      actions.undo = function () { step(undo, redo, 'undo'); };
      actions.redo = function () { step(redo, undo, 'redo'); };
      actions.add = function () { addMachineAt(M.toWorld(view, U, centre()[0], centre()[1])); };
    }
    rootEl.querySelectorAll('[data-bp-action]').forEach(function (b) {
      var act = actions[b.dataset.bpAction];
      if (!act) return;
      // Focus goes back to the canvas, so the next key (F after a pan, Ctrl
      // + A after Fit) reaches it instead of the button.
      b.addEventListener('click', function () {
        act();
        viewport.focus({ preventScroll: true });
      });
    });

    if (editable && BP.inspector) {
      insp = BP.inspector.create(rootEl, M, api);
      pop = BP.inspector.createPop(rootEl, M, api);
    }
    var reloadBtn = rootEl.querySelector('#bp-reload');
    if (reloadBtn) reloadBtn.addEventListener('click', function () { location.reload(); });

    rerender();
    fit();

    return {
      // Called on a body-targeted htmx:beforeSwap: the canvas is about to be
      // replaced, and everything bound to it goes with it. An edit not saved
      // yet is sent on its way first.
      destroy: function () {
        if (frame) cancelAnimationFrame(frame);
        drag = null;
        clearTimeout(saveTimer);
        flush();
      },
      unsaved: function () { return (dirty || inFlight) && !paused; },
      flush: flush,
    };
  }

  function boot() {
    var rootEl = document.getElementById('bp-app');
    if (!rootEl || rootEl.dataset.booted || !BP.model || !BP.view) return;
    // The editor needs its inspector too; the read-only pages do not wait for it.
    if (rootEl.dataset.mode === 'edit' && !BP.inspector) return;
    rootEl.dataset.booted = '1';
    BP.app = start(rootEl, BP.model, BP.view);
  }

  BP.tryBoot = boot;

  if (!BP.bound) {
    BP.bound = true;
    document.addEventListener('htmx:afterSwap', function () { if (BP.tryBoot) BP.tryBoot(); });
    document.addEventListener('htmx:beforeSwap', function (e) {
      if (e.target !== document.body || !BP.app) return;
      BP.app.destroy();
      BP.app = null;
    });
    // A full navigation (the address bar, a reload, a link htmx does not
    // boost) sends what is not saved, and asks first if a save is still out.
    window.addEventListener('pagehide', function () { if (BP.app) BP.app.flush(); });
    window.addEventListener('beforeunload', function (e) {
      if (!BP.app || !BP.app.unsaved()) return;
      BP.app.flush();
      if (BP.app.unsaved()) { e.preventDefault(); e.returnValue = ''; }
    });
  }

  boot();
})(window);
