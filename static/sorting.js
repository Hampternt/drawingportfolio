// Sorting & Loading Assistant — the board that gets worked from.
//
// Two screens live in here: the session list at /sorting and the board at
// /sorting/{id}. Both boot from `boot()`, which is safe to run any number of
// times — hx-boost swaps body children without a page load, so this file is
// re-executed on every boosted navigation back to either screen, and
// DOMContentLoaded fires only on the very first one.
//
// ── Why the board renders here rather than in Rust ────────────────────────
//
// A tick has to land instantly with a glove on. Server-rendering the checklist
// would put a round trip between the tap and the ink, which in a warehouse
// basement is the difference between a tool and a nuisance. So the whole
// document is handed over in a <script type="application/json"> block, the
// board renders from it locally, and the server is *told* about each tick
// afterwards. If that telling fails — no signal, tab asleep, server
// restarting — the tick still happened: it goes into a queue in localStorage
// and is replayed when the network comes back. The board is usable start to
// finish with the radio off.

(function () {
  'use strict';

  var QUEUE_KEY = 'sorting-queue-v1';

  // ── Small helpers ───────────────────────────────────────────────────────

  function el(tag, cls, text) {
    var n = document.createElement(tag);
    if (cls) n.className = cls;
    if (text !== undefined && text !== null) n.textContent = String(text);
    return n;
  }

  function svgEl(tag, attrs) {
    var n = document.createElementNS('http://www.w3.org/2000/svg', tag);
    for (var k in attrs) {
      if (Object.prototype.hasOwnProperty.call(attrs, k)) n.setAttribute(k, attrs[k]);
    }
    return n;
  }

  function readJson(id, fallback) {
    var node = document.getElementById(id);
    if (!node) return fallback;
    try {
      return JSON.parse(node.textContent);
    } catch (e) {
      return fallback;
    }
  }

  // A stable colour for a customer the plan gave none for. Hashing the name
  // rather than cycling a palette means the same customer keeps the same
  // colour across every route, which is the whole point of colouring them.
  function fallbackColor(name) {
    var h = 0;
    for (var i = 0; i < name.length; i++) h = (h * 31 + name.charCodeAt(i)) % 360;
    return 'hsl(' + h + ' 58% 62%)';
  }

  function titleCase(s) {
    return s ? s.charAt(0).toUpperCase() + s.slice(1) : s;
  }

  // "Pallet A", "Standby side-1", "Van · row 3 · right" — one phrasing, used
  // by the focus card and the step list alike so they can't drift apart.
  function endpointLabel(e) {
    if (!e || !e.type) return '—';
    if (e.type === 'pallet') return e.stackId ? 'Pallet ' + e.stackId : 'Pallet';
    if (e.type === 'standby') return 'Standby ' + (e.slot || '');
    if (e.type === 'van') {
      var bits = ['Van'];
      if (e.row !== undefined && e.row !== null) bits.push('row ' + e.row);
      if (e.column) bits.push(e.column);
      return bits.join(' · ');
    }
    return titleCase(e.type);
  }

  // ── The offline queue ───────────────────────────────────────────────────
  //
  // One entry per tick that the server has not acknowledged. Ordered, and
  // replayed in order, so a tick-then-untick cannot land the wrong way round.

  function loadQueue() {
    try {
      var raw = window.localStorage.getItem(QUEUE_KEY);
      var parsed = raw ? JSON.parse(raw) : [];
      return Array.isArray(parsed) ? parsed : [];
    } catch (e) {
      // Private mode, cleared storage, a browser that refuses the accessor
      // outright — none of which should stop the board rendering.
      return [];
    }
  }

  function saveQueue(q) {
    try {
      window.localStorage.setItem(QUEUE_KEY, JSON.stringify(q));
    } catch (e) {
      /* the board still works; only the replay-after-reload is lost */
    }
  }

  // ── The session list (/sorting) ─────────────────────────────────────────

  function initIndex() {
    var file = document.getElementById('sort-file');
    if (file && !file.dataset.bound) {
      file.dataset.bound = '1';
      file.addEventListener('change', function () {
        var f = file.files && file.files[0];
        if (!f) return;
        var reader = new FileReader();
        reader.onload = function () {
          var box = document.getElementById('sort-payload');
          if (box) {
            box.value = String(reader.result || '');
            box.focus();
          }
        };
        reader.readAsText(f);
      });
    }

    var buttons = document.querySelectorAll('[data-delete-session]');
    for (var i = 0; i < buttons.length; i++) {
      (function (btn) {
        if (btn.dataset.bound) return;
        btn.dataset.bound = '1';
        btn.addEventListener('click', function () {
          var id = btn.getAttribute('data-delete-session');
          var label = btn.getAttribute('data-route') || 'this session';
          if (!window.confirm('Delete ' + label + '? The plan and its progress go with it.')) return;
          btn.disabled = true;
          fetch('/api/sorting/sessions/' + id, { method: 'DELETE' })
            .then(function (r) {
              if (!r.ok) throw new Error('delete failed');
              var row = document.getElementById('sort-session-' + id);
              if (row) row.remove();
            })
            .catch(function () {
              btn.disabled = false;
              window.alert('Could not delete that session — try again when you have signal.');
            });
        });
      })(buttons[i]);
    }
  }

  // ── The board (/sorting/{id}) ───────────────────────────────────────────

  function initBoard() {
    var root = document.querySelector('.sorting-board');
    if (!root || root.dataset.bound) return;
    root.dataset.bound = '1';

    var sessionId = root.getAttribute('data-session-id');
    var plan = readJson('sorting-payload', {});
    var serverDone = readJson('sorting-progress', []);

    var steps = (plan.pickSequence || []).slice().sort(function (a, b) {
      return (a.step || 0) - (b.step || 0);
    });

    // Server truth, then whatever this tablet has ticked since and not yet
    // managed to report. Replaying the queue over the server's answer is what
    // makes a reload mid-shift come back to the right place.
    var done = {};
    serverDone.forEach(function (s) {
      done[s] = true;
    });
    var queue = loadQueue();
    queue.forEach(function (op) {
      if (String(op.sessionId) !== String(sessionId)) return;
      if (op.completed) done[op.step] = true;
      else delete done[op.step];
    });

    var focusStep = null; // null = "the first thing not done"
    var showDone = false;

    var colors = buildColorMap(plan);

    // ── Progress ──

    function doneCount() {
      var n = 0;
      steps.forEach(function (s) {
        if (done[s.step]) n++;
      });
      return n;
    }

    function cratesLoaded() {
      var n = 0;
      steps.forEach(function (s) {
        if (done[s.step] && s.to && s.to.type === 'van') n += Math.max(0, s.quantity || 0);
      });
      return n;
    }

    function cratesToLoad() {
      var n = 0;
      steps.forEach(function (s) {
        if (s.to && s.to.type === 'van') n += Math.max(0, s.quantity || 0);
      });
      return n;
    }

    function currentStep() {
      if (focusStep !== null) {
        for (var i = 0; i < steps.length; i++) {
          if (steps[i].step === focusStep) return steps[i];
        }
      }
      for (var j = 0; j < steps.length; j++) {
        if (!done[steps[j].step]) return steps[j];
      }
      return null;
    }

    function nextAfter(step) {
      var seen = false;
      for (var i = 0; i < steps.length; i++) {
        if (seen && !done[steps[i].step]) return steps[i];
        if (steps[i] === step) seen = true;
      }
      return null;
    }

    // ── Talking to the server ──

    function syncIndicator() {
      var node = document.getElementById('sort-sync');
      if (!node) return;
      var pending = loadQueue().filter(function (op) {
        return String(op.sessionId) === String(sessionId);
      }).length;
      if (pending === 0) {
        node.hidden = true;
        node.textContent = '';
      } else {
        node.hidden = false;
        node.textContent = pending + ' unsynced';
      }
    }

    var flushing = false;
    function flush() {
      if (flushing) return;
      var q = loadQueue();
      if (q.length === 0) {
        syncIndicator();
        return;
      }
      flushing = true;
      var op = q[0];
      fetch('/api/sorting/sessions/' + op.sessionId + '/steps/' + op.step, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ completed: !!op.completed })
      })
        .then(function (r) {
          if (!r.ok && r.status !== 404) throw new Error('sync failed');
          // A 404 means the session is gone from under us. Dropping the op is
          // the only way out — retrying it forever would wedge the queue and
          // every later tick behind it.
          var cur = loadQueue();
          cur.shift();
          saveQueue(cur);
          flushing = false;
          syncIndicator();
          if (cur.length) flush();
        })
        .catch(function () {
          flushing = false;
          syncIndicator();
        });
    }

    function enqueue(step, completed) {
      var q = loadQueue();
      q.push({ sessionId: sessionId, step: step, completed: completed });
      saveQueue(q);
      syncIndicator();
      flush();
    }

    function setDone(step, value) {
      if (value) done[step] = true;
      else delete done[step];
      enqueue(step, value);
      render();
    }

    // ── Rendering ──

    function render() {
      renderProgress();
      renderFocus();
      renderSteps();
      renderVan();
      syncIndicator();
    }

    function renderProgress() {
      var total = steps.length;
      var n = doneCount();
      var text = document.getElementById('sort-progress-text');
      var fill = document.getElementById('sort-progress-fill');
      if (text) {
        text.textContent = total
          ? n + ' / ' + total + ' moves · ' + cratesLoaded() + ' of ' + cratesToLoad() + ' crates in'
          : 'No moves in this plan';
      }
      if (fill) fill.style.width = (total ? Math.round((n / total) * 100) : 0) + '%';
    }

    function renderFocus() {
      var host = document.getElementById('sort-focus');
      if (!host) return;
      host.textContent = '';

      var step = currentStep();
      if (!step) {
        var doneCard = el('div', 'sort-focus__done');
        doneCard.appendChild(el('div', 'sort-focus__donemark', '✓'));
        doneCard.appendChild(
          el('strong', null, steps.length ? 'Every move is ticked.' : 'This plan has no pick sequence.')
        );
        doneCard.appendChild(
          el(
            'p',
            null,
            steps.length
              ? 'The van matches the loading plan. Check the Van tab before you close the doors.'
              : 'The Van and Checks tabs still work.'
          )
        );
        host.appendChild(doneCard);
        return;
      }

      var card = el('div', 'sort-focus__card');
      if (focusStep !== null) card.classList.add('is-pinned');

      var top = el('div', 'sort-focus__top');
      top.appendChild(el('span', 'sort-focus__step', 'Step ' + step.step));
      if (focusStep !== null) {
        var back = el('button', 'sort-focus__unpin', 'Back to next');
        back.type = 'button';
        back.addEventListener('click', function () {
          focusStep = null;
          render();
        });
        top.appendChild(back);
      }
      card.appendChild(top);

      var who = el('div', 'sort-focus__who');
      var dot = el('span', 'sort-focus__dot');
      dot.style.background = colorFor(step.customer);
      who.appendChild(dot);
      who.appendChild(el('span', 'sort-focus__name', step.customer || 'Unnamed'));
      card.appendChild(who);

      var move = el('div', 'sort-focus__move');
      move.appendChild(el('span', 'sort-focus__qty', '×' + (step.quantity || 0)));
      var route = el('div', 'sort-focus__route');
      route.appendChild(el('span', 'sort-focus__from', endpointLabel(step.from)));
      route.appendChild(el('span', 'sort-focus__arrow', '→'));
      var toNode = el('span', 'sort-focus__to', endpointLabel(step.to));
      if (step.to && step.to.type === 'standby') toNode.classList.add('is-standby');
      route.appendChild(toNode);
      move.appendChild(route);
      card.appendChild(move);

      var doneBtn = el('button', 'sort-focus__done-btn', 'Done');
      doneBtn.type = 'button';
      doneBtn.addEventListener('click', function () {
        setDone(step.step, true);
        focusStep = null;
      });
      card.appendChild(doneBtn);

      var after = nextAfter(step);
      var footer = el('div', 'sort-focus__next');
      if (after) {
        footer.appendChild(el('span', 'sort-focus__nextlabel', 'Then'));
        footer.appendChild(
          el(
            'span',
            'sort-focus__nexttext',
            '×' + (after.quantity || 0) + ' ' + (after.customer || '') + ' → ' + endpointLabel(after.to)
          )
        );
      } else {
        footer.appendChild(el('span', 'sort-focus__nextlabel', 'Last move'));
      }
      card.appendChild(footer);

      host.appendChild(card);
    }

    function renderSteps() {
      var list = document.getElementById('sort-steps');
      if (!list) return;
      list.textContent = '';
      var cur = currentStep();

      steps.forEach(function (s) {
        var isDone = !!done[s.step];
        if (isDone && !showDone) return;

        var li = el('li', 'sort-step');
        if (isDone) li.classList.add('is-done');
        if (cur && s.step === cur.step) li.classList.add('is-current');

        var body = el('button', 'sort-step__body');
        body.type = 'button';
        body.appendChild(el('span', 'sort-step__num', s.step));

        var mid = el('span', 'sort-step__mid');
        var line1 = el('span', 'sort-step__line1');
        var dot = el('span', 'sort-step__dot');
        dot.style.background = colorFor(s.customer);
        line1.appendChild(dot);
        line1.appendChild(el('span', 'sort-step__name', s.customer || 'Unnamed'));
        line1.appendChild(el('span', 'sort-step__qty', '×' + (s.quantity || 0)));
        mid.appendChild(line1);
        mid.appendChild(
          el('span', 'sort-step__line2', endpointLabel(s.from) + '  →  ' + endpointLabel(s.to))
        );
        body.appendChild(mid);
        body.addEventListener('click', function () {
          focusStep = s.step;
          switchTab('sort');
          render();
          var focus = document.getElementById('sort-focus');
          if (focus && focus.scrollIntoView) focus.scrollIntoView({ block: 'nearest' });
        });
        li.appendChild(body);

        var tick = el('button', 'sort-step__tick', isDone ? '✓' : '');
        tick.type = 'button';
        tick.setAttribute('aria-pressed', isDone ? 'true' : 'false');
        tick.setAttribute(
          'aria-label',
          (isDone ? 'Untick' : 'Tick') + ' step ' + s.step + ', ' + (s.customer || '')
        );
        tick.addEventListener('click', function () {
          setDone(s.step, !done[s.step]);
        });
        li.appendChild(tick);

        list.appendChild(li);
      });

      if (!list.children.length) {
        var empty = el('li', 'sort-steps__empty', showDone
          ? 'No moves in this plan.'
          : 'Nothing left — every move is ticked. Use “Show done” to review them.');
        list.appendChild(empty);
      }
    }

    function colorFor(customer) {
      return colors[customer] || fallbackColor(customer || '');
    }

    // ── The van ──
    //
    // Two things are drawn on top of the loading plan, and both come from the
    // ticks rather than the document: which crates are already in (solid, not
    // outlined), and what is currently sitting in the standby slots. That is
    // the difference between a diagram of the plan and a picture of the van as
    // it stands right now.

    function renderVan() {
      var host = document.getElementById('sort-van');
      if (!host) return;
      host.textContent = '';

      var cfg = plan.vanConfig || {};
      var totalRows = cfg.totalRows || 7;
      var maxHeight = cfg.maxHeight || 8;
      var rows = (plan.loadingPlan && plan.loadingPlan.rows) || [];

      if (!rows.length) {
        host.appendChild(el('p', 'sort-empty', 'This plan has no loading diagram.'));
        host.appendChild(renderStandby());
        return;
      }

      var cells = buildCells(rows, totalRows);
      applyPlaced(cells);

      var GUT = 30, CW = 104, CH = 92, GAP = 8, PAD = 8;
      var width = GUT + CW * 2 + GAP + PAD * 2;
      var height = PAD * 2 + 26 + totalRows * (CH + GAP) + 26;

      var svg = svgEl('svg', {
        viewBox: '0 0 ' + width + ' ' + height,
        class: 'sort-vansvg',
        role: 'img',
        'aria-label': 'Van loading diagram, ' + totalRows + ' rows, left and right columns'
      });

      // Centred on the diagram's real middle, and kept short: an <svg> root
      // clips at its own viewBox, so a caption wider than the drawing silently
      // loses its last word.
      var mid = width / 2;
      svg.appendChild(svgText(mid, PAD + 14, 'CAB · LOADED FIRST', 'sort-vansvg__cap'));

      var sideRows = {};
      ((cfg.sideDoor && cfg.sideDoor.rows) || []).forEach(function (r) {
        sideRows[r] = true;
      });

      var cur = currentStep();

      for (var r = 1; r <= totalRows; r++) {
        var y = PAD + 26 + (r - 1) * (CH + GAP);
        svg.appendChild(svgText(PAD + 10, y + CH / 2 + 4, String(r), 'sort-vansvg__rownum'));

        if (sideRows[r]) {
          svg.appendChild(
            svgEl('rect', {
              x: PAD + GUT - 5,
              y: y,
              width: 3,
              height: CH,
              rx: 1.5,
              class: 'sort-vansvg__door'
            })
          );
        }

        ['left', 'right'].forEach(function (col, ci) {
          var x = PAD + GUT + ci * (CW + GAP);
          svg.appendChild(
            svgEl('rect', {
              x: x,
              y: y,
              width: CW,
              height: CH,
              rx: 4,
              class: 'sort-vansvg__cell'
            })
          );

          var stack = (cells[r] && cells[r][col]) || [];
          var unit = CH / maxHeight;
          stack.forEach(function (crate, idx) {
            var cy = y + CH - (idx + 1) * unit;
            var rect = svgEl('rect', {
              x: x + 3,
              y: cy + 1,
              width: CW - 6,
              height: Math.max(2, unit - 2),
              rx: 2,
              class: 'sort-vansvg__crate' + (crate.placed ? ' is-placed' : '')
            });
            rect.setAttribute('fill', colorFor(crate.customer));
            if (!crate.placed) rect.setAttribute('fill-opacity', '0.18');
            if (crate.uncertain) rect.setAttribute('stroke-dasharray', '3 2');
            var t = svgEl('title', {});
            t.textContent =
              crate.customer + ' · row ' + r + ' ' + col + ' · ' + (crate.placed ? 'loaded' : 'not yet');
            rect.appendChild(t);
            svg.appendChild(rect);
          });

          // A position often holds two customers — a small order stacked on a
          // larger one, the later delivery on top so it comes off first. Colour
          // alone leaves you counting bands against a legend to see where one
          // ends, so each customer's run gets a rule under it and, where there
          // is room, its own tag. This is the thing the diagram is for.
          runsOf(stack).forEach(function (run) {
            var top = y + CH - (run.end + 1) * unit;
            var tall = run.count * unit;
            if (run.start > 0) {
              svg.appendChild(
                svgEl('line', {
                  x1: x + 3,
                  x2: x + CW - 3,
                  y1: y + CH - run.start * unit,
                  y2: y + CH - run.start * unit,
                  class: 'sort-vansvg__split'
                })
              );
            }
            if (tall >= 18) {
              svg.appendChild(
                svgText(
                  x + CW / 2,
                  top + tall / 2 + 4,
                  abbreviate(run.customer),
                  'sort-vansvg__tag' + (run.placed ? '' : ' is-pending')
                )
              );
            }
          });

          if (cur && cur.to && cur.to.type === 'van' && cur.to.row === r && cur.to.column === col) {
            svg.appendChild(
              svgEl('rect', {
                x: x - 2,
                y: y - 2,
                width: CW + 4,
                height: CH + 4,
                rx: 6,
                class: 'sort-vansvg__target'
              })
            );
          }

          if (stack.length) {
            svg.appendChild(
              svgText(x + CW - 8, y + 13, String(stack.length), 'sort-vansvg__count')
            );
          }
        });
      }

      svg.appendChild(
        svgText(mid, height - PAD - 6, 'DOORS · DROPPED FIRST', 'sort-vansvg__cap')
      );

      host.appendChild(svg);
      host.appendChild(renderStandby());
      host.appendChild(renderLegend(cells));
    }

    // Consecutive crates in one position belonging to the same customer. The
    // plan lists a position's entries in load order, so a run's start index is
    // also its height off the floor of that stack.
    function runsOf(stack) {
      var runs = [];
      for (var i = 0; i < stack.length; i++) {
        var last = runs[runs.length - 1];
        if (last && last.customer === stack[i].customer) {
          last.end = i;
          last.count++;
          if (!stack[i].placed) last.placed = false;
        } else {
          runs.push({
            customer: stack[i].customer,
            start: i,
            end: i,
            count: 1,
            placed: !!stack[i].placed
          });
        }
      }
      return runs;
    }

    // Three letters of the customer's first word — enough to tell the two
    // halves of a shared stack apart at a glance without a trip to the legend.
    function abbreviate(name) {
      var word = String(name || '').replace(/[^\p{L}\p{N} ]/gu, ' ').trim().split(/\s+/)[0] || '';
      return word.slice(0, 3).toUpperCase();
    }

    function svgText(x, y, text, cls) {
      var t = svgEl('text', { x: x, y: y, class: cls, 'text-anchor': 'middle' });
      t.textContent = text;
      return t;
    }

    // Column contents, bottom of the stack first — which is the order the plan
    // lists its entries in, because the first entry into a row is the one
    // everything else goes on top of.
    function buildCells(rows, totalRows) {
      var cells = {};
      for (var r = 1; r <= totalRows; r++) cells[r] = { left: [], right: [] };
      rows.forEach(function (row) {
        var bucket = cells[row.row];
        if (!bucket) return;
        (row.entries || []).forEach(function (e) {
          for (var i = 0; i < Math.max(0, e.left || 0); i++) {
            bucket.left.push({ customer: e.customer, uncertain: !!e.uncertain, placed: false });
          }
          for (var j = 0; j < Math.max(0, e.right || 0); j++) {
            bucket.right.push({ customer: e.customer, uncertain: !!e.uncertain, placed: false });
          }
        });
      });
      return cells;
    }

    // Marks crates as in the van, using only the ticked steps. A step says
    // "3 of X into row 2 right", so the first three unplaced X crates in that
    // cell become placed — the plan never says *which* three, and physically
    // it cannot matter.
    function applyPlaced(cells) {
      steps.forEach(function (s) {
        if (!done[s.step]) return;
        if (!s.to || s.to.type !== 'van') return;
        var bucket = cells[s.to.row];
        if (!bucket) return;
        var stack = bucket[s.to.column];
        if (!stack) return;
        var left = Math.max(0, s.quantity || 0);
        for (var i = 0; i < stack.length && left > 0; i++) {
          if (!stack[i].placed && stack[i].customer === s.customer) {
            stack[i].placed = true;
            left--;
          }
        }
      });
    }

    function renderStandby() {
      var wrap = el('div', 'sort-standby');
      var cfg = (plan.vanConfig && plan.vanConfig.standby) || {};
      var sideSlots = cfg.sideSlots === undefined ? 3 : cfg.sideSlots;
      var backSlots = cfg.backSlots === undefined ? 2 : cfg.backSlots;

      // A standby spot holds a small stack, not a set: crates go on top and
      // come back off the top. Tracking it as an ordered pile rather than a
      // tally is what lets the panel say which one you can actually reach —
      // and a plan that tries to pull from underneath shows up as a pile that
      // does not drain in the order it was built.
      var piles = {};
      steps.forEach(function (s) {
        if (!done[s.step]) return;
        var qty = Math.max(0, s.quantity || 0);
        if (s.from && s.from.type === 'standby' && s.from.slot) {
          var pile = piles[s.from.slot] || (piles[s.from.slot] = []);
          var owed = qty;
          while (owed > 0 && pile.length) {
            var top = pile[pile.length - 1];
            var take = Math.min(owed, top.count);
            top.count -= take;
            owed -= take;
            if (top.count <= 0) pile.pop();
          }
        }
        if (s.to && s.to.type === 'standby' && s.to.slot) {
          var onto = piles[s.to.slot] || (piles[s.to.slot] = []);
          var last = onto[onto.length - 1];
          if (last && last.customer === s.customer) last.count += qty;
          else onto.push({ customer: s.customer, count: qty });
        }
      });

      var inUse = Object.keys(piles).filter(function (k) { return piles[k].length; }).length;
      wrap.appendChild(
        el(
          'h3',
          'sort-standby__title',
          inUse ? 'Standby — ' + inUse + ' of 5 in use' : 'Standby — all clear'
        )
      );
      var grid = el('div', 'sort-standby__grid');
      var names = [];
      for (var i = 1; i <= sideSlots; i++) names.push('side-' + i);
      for (var j = 1; j <= backSlots; j++) names.push('back-' + j);

      names.forEach(function (slot) {
        var box = el('div', 'sort-slot');
        box.appendChild(el('span', 'sort-slot__name', slot));
        var pile = piles[slot] || [];
        if (!pile.length) {
          box.appendChild(el('span', 'sort-slot__empty', 'empty'));
        } else {
          box.classList.add('is-occupied');
          // Top of the pile first — the same order the pallet reference uses,
          // and the order you can actually take them off in.
          pile
            .slice()
            .reverse()
            .forEach(function (layer, i) {
              var chip = el('span', 'sort-slot__chip', layer.count + '× ' + layer.customer);
              chip.style.borderColor = colorFor(layer.customer);
              if (i === 0 && pile.length > 1) chip.classList.add('is-top');
              box.appendChild(chip);
            });
        }
        grid.appendChild(box);
      });
      wrap.appendChild(grid);
      return wrap;
    }

    function renderLegend(cells) {
      var totals = {};
      Object.keys(cells).forEach(function (r) {
        ['left', 'right'].forEach(function (col) {
          cells[r][col].forEach(function (crate) {
            var t = totals[crate.customer] || { total: 0, placed: 0 };
            t.total++;
            if (crate.placed) t.placed++;
            totals[crate.customer] = t;
          });
        });
      });

      var wrap = el('div', 'sort-legend');
      wrap.appendChild(el('h3', 'sort-legend__title', 'By customer'));
      var names = Object.keys(totals).sort();
      names.forEach(function (name) {
        var t = totals[name];
        var row = el('div', 'sort-legend__row');
        if (t.placed >= t.total) row.classList.add('is-complete');
        var sw = el('span', 'sort-legend__swatch');
        sw.style.background = colorFor(name);
        row.appendChild(sw);
        row.appendChild(el('span', 'sort-legend__name', name));
        row.appendChild(el('span', 'sort-legend__count', t.placed + ' / ' + t.total));
        wrap.appendChild(row);
      });
      if (!names.length) wrap.appendChild(el('p', 'sort-empty', 'Nothing placed in this plan.'));
      return wrap;
    }

    // ── Tabs ──

    function switchTab(name) {
      var tabs = root.querySelectorAll('.sort-tab');
      for (var i = 0; i < tabs.length; i++) {
        var on = tabs[i].getAttribute('data-tab') === name;
        tabs[i].classList.toggle('is-active', on);
        tabs[i].setAttribute('aria-selected', on ? 'true' : 'false');
      }
      var panes = root.querySelectorAll('.sort-pane');
      for (var j = 0; j < panes.length; j++) {
        panes[j].classList.toggle('is-active', panes[j].getAttribute('data-pane') === name);
      }
      root.setAttribute('data-tab', name);
    }

    // Above 1000px the van has its own column and its tab is hidden, so
    // "van" stops being a selectable pane. Rotating a tablet into landscape
    // with it selected is the one way to land there anyway — which would show
    // the van twice and the checklist not at all.
    var wide = window.matchMedia('(min-width: 1000px)');
    function ensureUsablePane() {
      if (wide.matches && root.getAttribute('data-tab') === 'van') switchTab('sort');
    }
    if (wide.addEventListener) wide.addEventListener('change', ensureUsablePane);

    var tabButtons = root.querySelectorAll('.sort-tab');
    for (var t = 0; t < tabButtons.length; t++) {
      (function (btn) {
        btn.addEventListener('click', function () {
          switchTab(btn.getAttribute('data-tab'));
        });
      })(tabButtons[t]);
    }
    switchTab('sort');

    var toggle = document.getElementById('sort-toggle-done');
    if (toggle) {
      toggle.addEventListener('click', function () {
        showDone = !showDone;
        toggle.setAttribute('aria-pressed', showDone ? 'true' : 'false');
        toggle.textContent = showDone ? 'Hide done' : 'Show done';
        renderSteps();
      });
    }

    render();
    flush();
    acquireWakeLock();

    if (!window.__sortingNetBound) {
      window.__sortingNetBound = true;
      window.addEventListener('online', function () {
        flush();
      });
    }
  }

  // ── Colours ─────────────────────────────────────────────────────────────
  //
  // The loading plan assigns each customer a colour; the checklist, the van
  // and the legend all read it from here so a customer is the same colour
  // everywhere on the board.

  function buildColorMap(plan) {
    var map = {};
    var rows = (plan.loadingPlan && plan.loadingPlan.rows) || [];
    rows.forEach(function (row) {
      (row.entries || []).forEach(function (e) {
        if (e.customer && e.color && !map[e.customer]) map[e.customer] = e.color;
      });
    });
    return map;
  }

  // ── Keeping the screen on ───────────────────────────────────────────────
  //
  // A tablet that sleeps between crates is a tablet that gets left on the
  // bench. The lock is a live resource, so it is released on a boosted
  // navigation away — which never fires unload — and re-taken when the tab
  // comes back to the foreground, because the browser drops it on its own
  // whenever the page is hidden.

  function acquireWakeLock() {
    if (!('wakeLock' in navigator)) return;
    releaseWakeLock();
    try {
      navigator.wakeLock
        .request('screen')
        .then(function (lock) {
          window.__sortingWakeLock = lock;
        })
        .catch(function () {
          /* denied, low battery, or not allowed in this context */
        });
    } catch (e) {
      /* older browsers throw rather than reject */
    }
  }

  function releaseWakeLock() {
    var lock = window.__sortingWakeLock;
    window.__sortingWakeLock = null;
    if (lock && typeof lock.release === 'function') {
      try {
        lock.release();
      } catch (e) {
        /* already gone */
      }
    }
  }

  // The script's own globals reset when it re-executes after a boosted
  // navigation, while the previous lock is still live — so drop it here, at
  // top level, before anything reassigns it. Same reasoning as barcode.js's
  // camera stream.
  releaseWakeLock();

  // ── Boot ────────────────────────────────────────────────────────────────

  function boot() {
    initIndex();
    initBoard();
  }

  if (!window.__sortingBound) {
    window.__sortingBound = true;

    document.addEventListener('htmx:afterSwap', boot);

    // hx-boost replaces the body's children without ever firing unload, so
    // this is the only notice the board gets that it is being navigated away
    // from.
    document.addEventListener('htmx:beforeSwap', function (e) {
      if (e.target === document.body) releaseWakeLock();
    });

    document.addEventListener('visibilitychange', function () {
      if (document.visibilityState === 'visible' && document.querySelector('.sorting-board')) {
        acquireWakeLock();
      }
    });
  }

  // `defer` guarantees the document is parsed, so this covers the first load;
  // the afterSwap listener covers every boosted one.
  boot();
})();
