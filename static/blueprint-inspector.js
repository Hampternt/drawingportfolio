// Blueprints: the editor's side of the page. The inspector beside the canvas
// (name, ports, type labels, status, note) and the add-node box a wire dropped
// on empty grid opens. Edit mode only.
//
// Neither touches the document itself. Every change goes through api.edit(fn)
// in blueprint-app.js, which makes it one undo step, redraws the canvas and
// schedules the save, or puts the document back if the model refuses it. A
// field commits on `change` (Enter, or leaving it), never per keystroke, so
// one rename is one undo step.
//
// Like the other blueprint scripts, this file can run more than once under
// hx-boost; it only defines functions, and blueprint-app.js calls them.
(function (root) {
  'use strict';

  function el(tag, cls, text) {
    var e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text != null) e.textContent = text;
    return e;
  }

  function plural(n, one, many) { return n + ' ' + (n === 1 ? one : many); }

  // ── The inspector ──────────────────────────────────────────────────────────
  function create(rootEl, M, api) {
    var box = rootEl.querySelector('#bp-inspector');
    var outlineWrap = rootEl.querySelector('#bp-outline-wrap');
    var tabs = Array.prototype.slice.call(rootEl.querySelectorAll('[data-bp-tab]'));
    var current = null;   // the machine shown
    var error = '';       // the last refusal, until the next edit succeeds
    var focusNext = null; // a field to focus after the next build, by data-key
    var pending = false;

    // Builds wait one turn. A field commits on `change`, which fires before
    // Tab moves focus on; building in the same turn would remove the field
    // focus is about to move to. Afterwards the build puts focus back on the
    // field of the same key.
    function schedule() {
      if (pending) return;
      pending = true;
      setTimeout(function () { pending = false; build(); }, 0);
    }

    function setTab(t) {
      box.hidden = t !== 'inspect';
      outlineWrap.hidden = t !== 'outline';
      tabs.forEach(function (b) { b.setAttribute('aria-pressed', b.dataset.bpTab === t ? 'true' : 'false'); });
    }
    tabs.forEach(function (b) { b.addEventListener('click', function () { setTab(b.dataset.bpTab); }); });

    function nodes() { return api.doc().graphs[api.graph()].nodes; }

    // Every edit from here: on success the error line clears, on a refusal it
    // says why and the field goes back to what the document holds.
    function commit(fn) {
      var err = api.edit(fn);
      error = err;
      if (err) { schedule(); api.announce(err); }
      return err;
    }

    function field(key, tag, cls) {
      var f = el(tag, cls);
      f.dataset.key = key;
      return f;
    }

    function label(text, forId) {
      var l = el('label', 'bp-insp__label', text);
      if (forId) l.htmlFor = forId;
      return l;
    }

    function portMeta(checks, side, ref) {
      var ws = checks.filter(function (w) { return (side === 'out' ? w.a : w.b) === ref; });
      var bad = ws.filter(function (w) { return w.mismatch; })[0];
      var dang = ws.filter(function (w) { return w.dangling; })[0];
      if (dang) return { text: 'dangling · ' + dang.why, cls: 'is-dangling', wired: true };
      if (bad) return { text: 'mismatch · gets ' + bad.ft + ' from ' + bad.a, cls: 'is-mismatch', wired: true };
      if (!ws.length) return { text: 'unwired', cls: '', wired: false };
      return {
        text: plural(ws.length, 'wire', 'wires') + ' · ' + ws.map(function (w) { return side === 'out' ? w.b : w.a; }).join(', '),
        cls: '', wired: true,
      };
    }

    function portGroup(form, n, side, checks) {
      var name = current;
      var group = el('div', 'bp-insp__group');
      var head = el('div', 'bp-insp__group-head');
      head.appendChild(el('span', 'bp-insp__label', side === 'out' ? 'Outputs' : 'Inputs'));
      head.appendChild(el('span', 'bp-insp__edge', side === 'out' ? 'right edge · listed first' : 'left edge'));
      group.appendChild(head);
      Object.keys(n[side] || {}).forEach(function (port, i) {
        var type = n[side][port], ref = name + '.' + port, meta = portMeta(checks, side, ref);
        var row = el('div', 'bp-insp__port');
        var dot = el('span', 'bp-insp__dot' + (meta.wired ? ' is-wired' : ''));
        dot.setAttribute('aria-hidden', 'true');
        dot.style.setProperty('--bp-port', M.portColor(type));
        row.appendChild(dot);

        var pn = field('port-' + side + '-' + i + '-name', 'input', 'bp-insp__input bp-insp__input--port');
        pn.value = port;
        pn.spellcheck = false;
        pn.setAttribute('aria-label', (side === 'out' ? 'Output' : 'Input') + ' ' + (i + 1) + ' name');
        pn.addEventListener('change', function () {
          var to = pn.value.trim();
          if (to === port) return;
          commit(function (d) { return M.renamePort(d, api.graph(), name, side, port, to); });
        });
        row.appendChild(pn);

        var pt = field('port-' + side + '-' + i + '-type', 'input', 'bp-insp__input bp-insp__input--type');
        pt.value = type;
        pt.spellcheck = false;
        pt.style.setProperty('--bp-port', M.portColor(type));
        pt.setAttribute('aria-label', port + ' type');
        pt.addEventListener('change', function () {
          if (pt.value === type) return;
          commit(function (d) { return M.setPortType(d, api.graph(), name, side, port, pt.value.trim() || 'Item'); });
        });
        row.appendChild(pt);

        var rm = field('port-' + side + '-' + i + '-remove', 'button', 'bp-insp__remove');
        rm.type = 'button';
        rm.textContent = '×';
        rm.setAttribute('aria-label', 'Remove ' + port + (meta.wired ? ' and its wires' : ''));
        rm.addEventListener('click', function () {
          focusNext = 'add-' + side;
          commit(function (d) { return M.removePort(d, api.graph(), name, side, port); });
        });
        row.appendChild(rm);

        var m = el('span', 'bp-insp__meta ' + meta.cls, meta.text);
        row.appendChild(m);
        group.appendChild(row);
      });
      var add = field('add-' + side, 'button', 'bp-insp__add');
      add.type = 'button';
      add.textContent = side === 'out' ? '+ Add output' : '+ Add input';
      add.addEventListener('click', function () {
        var count = Object.keys((nodes()[name] || {})[side] || {}).length;
        focusNext = 'port-' + side + '-' + count + '-name';
        commit(function (d) { M.addPort(d, api.graph(), name, side, side, 'Item'); return ''; });
      });
      group.appendChild(add);
      form.appendChild(group);
    }

    function build() {
      var had = box.contains(document.activeElement) ? document.activeElement.dataset.key : null;
      var want = focusNext || had;
      focusNext = null;
      box.replaceChildren();
      var n = current && nodes()[current];
      if (!n) {
        box.appendChild(el('p', 'bp-inspector__none', 'Select a machine on the canvas to edit its name, ports, status and note.'));
        return;
      }
      var name = current;
      var checks = M.checkGraph(api.doc(), api.graph());
      var refs = checks.filter(function (w) { return w.an === name || w.bn === name; }).length;
      // A fieldset, so a paused editor disables every control in one place.
      var form = el('fieldset', 'bp-insp');
      form.disabled = !api.editable();
      form.appendChild(el('legend', 'bp-sr', 'Machine ' + name));

      if (error) {
        var e = el('p', 'bp-insp__error', error);
        e.setAttribute('role', 'alert');
        form.appendChild(e);
      }

      var nameRow = el('div', 'bp-insp__row');
      nameRow.appendChild(label('Name', 'bp-insp-name'));
      var ni = field('name', 'input', 'bp-insp__input');
      ni.id = 'bp-insp-name';
      ni.value = name;
      ni.spellcheck = false;
      ni.autocomplete = 'off';
      ni.addEventListener('change', function () {
        var to = ni.value.trim();
        if (to === name) return;
        // Shown under its new name while the canvas redraws; put back if the
        // model refuses it.
        current = to;
        var err = commit(function (d) { return M.renameNode(d, api.graph(), name, to); });
        if (err) { current = name; schedule(); return; }
        api.select(to);
        api.announce('Renamed ' + name + ' to ' + to + '. ' + plural(refs, 'wire follows', 'wires follow') + '.');
      });
      nameRow.appendChild(ni);
      nameRow.appendChild(el('p', 'bp-insp__hint', 'Renaming ' + name + ' rewrites the ' + plural(refs, 'wire', 'wires') + ' that name it.'));
      form.appendChild(nameRow);

      var kindRow = el('div', 'bp-insp__row');
      kindRow.appendChild(el('span', 'bp-insp__label', 'Kind'));
      kindRow.appendChild(el('p', 'bp-insp__value', n.kind === 'blueprint'
        ? 'blueprint → ' + n.graph + ' · opening it is a later slice'
        : 'machine'));
      form.appendChild(kindRow);

      var statusRow = el('div', 'bp-insp__row');
      var sl = el('span', 'bp-insp__label', 'Status');
      sl.id = 'bp-insp-status';
      statusRow.appendChild(sl);
      var seg = el('div', 'bp-insp__seg');
      seg.setAttribute('role', 'group');
      seg.setAttribute('aria-labelledby', 'bp-insp-status');
      M.STATUSES.forEach(function (st) {
        var b = field('status-' + st, 'button', 'bp-insp__segbtn');
        b.type = 'button';
        b.textContent = st;
        b.setAttribute('aria-pressed', n.status === st ? 'true' : 'false');
        b.addEventListener('click', function () {
          commit(function (d) { return M.setStatus(d, api.graph(), name, st); });
        });
        seg.appendChild(b);
      });
      statusRow.appendChild(seg);
      form.appendChild(statusRow);

      portGroup(form, n, 'out', checks);
      portGroup(form, n, 'in', checks);

      var noteRow = el('div', 'bp-insp__row');
      noteRow.appendChild(label('Note', 'bp-insp-note'));
      var ta = field('note', 'textarea', 'bp-insp__input bp-insp__note');
      ta.id = 'bp-insp-note';
      ta.rows = 3;
      ta.value = n.note || '';
      ta.addEventListener('change', function () {
        commit(function (d) { return M.setNote(d, api.graph(), name, ta.value); });
      });
      noteRow.appendChild(ta);
      form.appendChild(noteRow);

      var foot = el('div', 'bp-insp__foot');
      foot.appendChild(el('span', 'bp-insp__rev', 'saves as you go · rev ' + api.rev()));
      var del = field('delete', 'button', 'hm-btn hm-btn--sm hm-btn--danger');
      del.type = 'button';
      del.textContent = 'Delete machine';
      del.addEventListener('click', function () { api.deleteNode(name); });
      foot.appendChild(del);
      form.appendChild(foot);

      box.appendChild(form);
      if (want) {
        var f = box.querySelector('[data-key="' + want + '"]');
        if (f) f.focus({ preventScroll: true });
      }
    }

    // Esc in a field goes back to the canvas, as it steps back everywhere else.
    box.addEventListener('keydown', function (e) {
      if (e.key === 'Escape') { e.preventDefault(); api.focusCanvas(); }
    });

    return {
      // The selection changed. A machine opens the Inspect tab.
      show: function (name) {
        if (name === current) return;
        current = name;
        error = '';
        if (name) setTab('inspect');
        schedule();
      },
      // The document changed: draw the same machine again from it.
      refresh: function () {
        if (current && !nodes()[current]) current = null;
        schedule();
      },
    };
  }

  // ── The add-node box ───────────────────────────────────────────────────────
  // A wire dropped on empty grid. One choice is live, a machine with the
  // dragged port's partner already wired; a blueprint is listed but is the
  // nesting slice, which is later.
  function createPop(rootEl, M, api) {
    var box = rootEl.querySelector('#bp-pop');
    var wrap = box.parentElement;
    var input = box.querySelector('#bp-pop-name');
    var labelEl = box.querySelector('#bp-pop-label');
    var list = box.querySelector('#bp-pop-items');
    var errEl = box.querySelector('#bp-pop-error');
    var from = null, world = null;

    function item(text, sub, enabled) {
      var li = el('li');
      var b = el('button', 'bp-pop__item');
      b.type = 'button';
      b.setAttribute('role', 'option');
      b.disabled = !enabled;
      b.setAttribute('aria-selected', enabled ? 'true' : 'false');
      b.appendChild(el('span', 'bp-pop__item-label', text));
      b.appendChild(el('span', 'bp-pop__item-sub', sub));
      if (enabled) b.addEventListener('click', make);
      li.appendChild(b);
      return li;
    }

    function make() {
      if (!from) return;
      var name = input.value.trim();
      var src = from, at = world;
      var err = api.edit(function (d) {
        var r = M.addWiredNode(d, api.graph(), src, at, name);
        return r.error || '';
      });
      if (err) { errEl.textContent = err; input.focus(); return; }
      close(true);
      api.select(name);
      api.focusCanvas();
      api.announce('Added ' + name + ', wired to ' + src.node + '.' + src.port + '.');
    }

    function open(f, screen, w) {
      if (!api.editable()) return;
      var n = api.doc().graphs[api.graph()].nodes[f.node];
      var type = n && n[f.side] ? n[f.side][f.port] : '?';
      from = { node: f.node, side: f.side, port: f.port };
      world = w;
      labelEl.textContent = 'wired to ' + f.node + '.' + f.port + ' · ' + type;
      input.value = M.uniqueName(api.doc().graphs[api.graph()].nodes, 'machine');
      errEl.textContent = '';
      list.replaceChildren(
        item('Machine', 'a new machine with ' + (f.side === 'out' ? 'an input ' : 'an output ') + f.port + ': ' + type + ', wired', true),
        item('Blueprint', 'collapsing machines into one is a later slice', false)
      );
      box.hidden = false;
      // Beside the drop point, kept inside the canvas.
      var W = wrap.clientWidth, H = wrap.clientHeight, bw = box.offsetWidth, bh = box.offsetHeight;
      box.style.left = Math.max(8, Math.min(screen[0] + 12, W - bw - 8)) + 'px';
      box.style.top = Math.max(8, Math.min(screen[1] - 20, H - bh - 8)) + 'px';
      input.focus();
      input.select();
      api.announce('Dropped on empty grid. Name the new machine and press Enter, or Escape to cancel.');
    }

    function close(silent) {
      if (box.hidden) return;
      box.hidden = true;
      from = null;
      if (!silent) api.focusCanvas();
    }

    box.addEventListener('keydown', function (e) {
      if (e.key === 'Escape') { e.preventDefault(); close(); api.announce('No machine added.'); }
      else if (e.key === 'Enter') { e.preventDefault(); make(); }
    });
    // Clicking anywhere else closes it, as if it were Escape.
    box.addEventListener('focusout', function (e) {
      if (!box.hidden && !box.contains(e.relatedTarget)) close(true);
    });

    return { open: open, close: close };
  }

  root.BP = root.BP || {};
  root.BP.inspector = { create: create, createPop: createPop };
  if (root.BP.tryBoot) root.BP.tryBoot();
})(window);
