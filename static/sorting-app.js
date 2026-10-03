// Sorting & Loading Assistant — the live board, wired to the route it is loading.
//
// The board itself is sorting-model.js and sorting-board.js: the same files the
// demo and the design document run, and the same 432 checks cover all three.
// Nothing about the loading rules is in here. What is in here is everything the
// board does not know about — which route it is loading, where the driver's van
// settings live, and how a tap survives a tablet with no signal.
//
// ── Why the log is the board ──────────────────────────────────────────────
//
// There is no fixed list of moves to tick off. The driver says which door a
// customer is being packed at, pushes crates in a few at a time, tops up
// somebody else's stack, closes an order out — and what that produces is a van.
// So every action appends a row carrying the whole board after it, and reload
// replays the last one.
//
// Storing the state rather than the verb is deliberate. The alternative is a
// semantic log the server replays, which means the loading rules in Rust as
// well as here: a second copy of the rule set, which is the one thing this port
// exists to avoid. What the server keeps is the property that actually matters
// — appends are ordered and (session, seq) makes a retry a no-op — so two taps
// a second apart both land and a replayed queue changes nothing.

(function () {
  'use strict';

  var QUEUE_KEY = 'sorting-live-queue-v1';

  // ── Small helpers ───────────────────────────────────────────────────────

  function readJson(id, fallback) {
    var el = document.getElementById(id);
    if (!el) return fallback;
    try {
      var v = JSON.parse(el.textContent || 'null');
      return v == null ? fallback : v;
    } catch (e) {
      return fallback;
    }
  }

  // localStorage can throw on the property access itself in a private window or
  // with site data blocked, so every touch of it is wrapped and every failure
  // degrades to "nothing queued" rather than to a dead board.
  function loadQueue() {
    try {
      var raw = window.localStorage.getItem(QUEUE_KEY);
      var q = raw ? JSON.parse(raw) : [];
      return Array.isArray(q) ? q : [];
    } catch (e) {
      return [];
    }
  }
  function saveQueue(q) {
    try {
      if (q.length) window.localStorage.setItem(QUEUE_KEY, JSON.stringify(q));
      else window.localStorage.removeItem(QUEUE_KEY);
    } catch (e) {
      /* a queue we cannot persist is still flushed in this tab */
    }
  }

  // ── Talking to the server ───────────────────────────────────────────────

  var queue = [];
  var flushing = false;

  function url(op) {
    var base = '/api/sorting/sessions/' + encodeURIComponent(op.sessionId) + '/actions';
    return op.op === 'undo' ? base + '/' + encodeURIComponent(op.seq) : base;
  }

  function send(op) {
    if (op.op === 'undo') return fetch(url(op), { method: 'DELETE' });
    return fetch(url(op), {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(op.body)
    });
  }

  // Strictly in order, and it stops at the first failure rather than skipping
  // past it: an undo that overtook the push it undoes would leave the server
  // holding a board the driver has already walked back.
  function flush() {
    if (flushing || !queue.length) return;
    flushing = true;
    var op = queue[0];
    send(op)
      .then(function (r) {
        // A 4xx is the server's final answer — retrying it forever would wedge
        // the queue behind one bad row. Drop it and carry on; the board on
        // this tablet is still right, and a reload is what reconciles.
        if (r.ok || (r.status >= 400 && r.status < 500)) queue.shift();
        saveQueue(queue);
      })
      .catch(function () {
        /* no signal: leave it at the head and try again later */
      })
      .then(function () {
        flushing = false;
        showSync();
        if (queue.length && navigator.onLine !== false) {
          window.setTimeout(flush, 1200);
        }
      });
  }

  function enqueue(op) {
    queue.push(op);
    saveQueue(queue);
    showSync();
    flush();
  }

  function showSync() {
    var el = document.getElementById('sort-sync');
    if (!el) return;
    var n = queue.length;
    el.hidden = n === 0;
    el.textContent = n === 1 ? '1 move not sent yet' : n + ' moves not sent yet';
  }

  // ── Keeping the screen on ───────────────────────────────────────────────
  //
  // A tablet that sleeps between crates is a tablet that gets left on the
  // bench. The lock is a live resource: released on a boosted navigation away,
  // which never fires unload, and re-taken when the tab comes back, because the
  // browser drops it on its own whenever the page is hidden.

  var wakeLock = null;

  function acquireWakeLock() {
    if (!('wakeLock' in navigator) || wakeLock) return;
    try {
      navigator.wakeLock
        .request('screen')
        .then(function (l) {
          wakeLock = l;
          l.addEventListener('release', function () { wakeLock = null; });
        })
        .catch(function () { /* refused, or the tab is not visible */ });
    } catch (e) {
      /* not available */
    }
  }

  function releaseWakeLock() {
    if (!wakeLock) return;
    try { wakeLock.release(); } catch (e) { /* already gone */ }
    wakeLock = null;
  }

  // ── Full screen ─────────────────────────────────────────────────────────
  //
  // Two things at once, and deliberately not one: the page's own big layout —
  // site chrome gone, the board given the room it leaves — and the browser's
  // fullscreen on top of it. The API is allowed to refuse (an iframe without
  // the permission, an iPad that never had it), and on a tablet the layout half
  // is most of the win, so the button never waits on the request and the
  // request never becomes the state.

  function fullScreenEl() {
    return document.fullscreenElement || document.webkitFullscreenElement || null;
  }

  function askFullScreen() {
    var el = document.documentElement;
    var req = el.requestFullscreen || el.webkitRequestFullscreen;
    if (!req) return;
    try {
      var r = req.call(el);
      if (r && r.catch) r.catch(function () {});
    } catch (e) {
      /* refused: the big layout stands on its own */
    }
  }

  function dropFullScreen() {
    var ex = document.exitFullscreen || document.webkitExitFullscreen;
    if (!fullScreenEl() || !ex) return;
    try {
      var r = ex.call(document);
      if (r && r.catch) r.catch(function () {});
    } catch (e) {
      /* already out */
    }
  }

  // Looked up rather than closed over: this is called from document-level
  // listeners that outlive any one board.
  function setBoardFull(on) {
    var board = document.querySelector('.sorting-board');
    var btn = document.getElementById('sort-full');
    if (board) board.classList.toggle('is-full', on);
    if (btn) {
      btn.setAttribute('aria-pressed', on ? 'true' : 'false');
      btn.textContent = on ? 'Exit full screen' : 'Full screen';
    }
    fit();
  }

  // ── Fitting the drawing ─────────────────────────────────────────────────
  //
  // The board is a fixed 90rem x 52.5rem picture. It is resized with `zoom`
  // rather than the root font size the demo uses: this page has a site header
  // above it that must not shrink with the van, and hx-boost swaps body
  // children without restoring `<html>`'s style, so a root font size set here
  // would follow the driver onto the next page. `zoom` is layout, not a
  // transform, so the picture stays a drawing rather than becoming a bitmap of
  // one — and where it is unsupported the board simply renders at its design
  // size, which is a scrollbar rather than a broken screen.

  var DESIGN_W = 90 * 16, DESIGN_H = 52.5 * 16;

  function fit() {
    var wrap = document.getElementById('sort-live');
    if (!wrap) return;
    var board = document.querySelector('.sorting-board');
    var full = board && board.classList.contains('is-full');
    var top = wrap.getBoundingClientRect().top;
    // Room below the bar, less the gutter the page keeps at the bottom. In full
    // screen the chrome is gone and the whole viewport is the board's.
    var room = window.innerHeight - top - (full ? 12 : 28);
    var byWidth = wrap.clientWidth / DESIGN_W;
    var byHeight = room / DESIGN_H;
    // Never below a quarter: past that the board is unreadable anyway and a
    // page that has to be scrolled is the better failure.
    var z = Math.max(0.25, Math.min(full ? 3 : 1, byWidth, byHeight));
    wrap.style.setProperty('--sort-zoom', String(z));
  }

  // ── Boot ────────────────────────────────────────────────────────────────

  var RECORD = null;   // set per board; the prototype wrappers call through it
  var VERB = '';

  // Every verb that changes the van, wrapped once so `apply()` can label what
  // it just did without board.js having to tell it. The log reads as a morning
  // rather than as a hundred rows of "change".
  function wrapVerbs() {
    if (window.__sortingVerbsWrapped) return;
    window.__sortingVerbsWrapped = true;
    ['doAssign', 'doBump', 'doPush', 'doStack', 'doClose', 'doBegin',
     'doMoveSpot', 'doDoorway', 'doClearDoorway', 'doReopen'].forEach(function (name) {
      var fn = window[name];
      if (typeof fn !== 'function') return;
      window[name] = function () {
        VERB = name.slice(2).toLowerCase();
        return fn.apply(this, arguments);
      };
    });
  }

  // The component's own mutation and undo paths, wrapped once. Both are on the
  // prototype, so this survives a boosted navigation building a new component.
  function wrapComponent() {
    if (typeof Component !== 'function' || Component.__liveWrapped) return;
    Component.__liveWrapped = true;
    var baseApply = Component.prototype.apply;
    Component.prototype.apply = function (fn) {
      VERB = '';
      baseApply.call(this, fn);
      if (RECORD && this === COMPONENT) RECORD.append(VERB || 'change');
    };
    var baseUndo = Component.prototype.undo;
    Component.prototype.undo = function () {
      var had = this.state.hist.length;
      baseUndo.call(this);
      if (RECORD && this === COMPONENT && had) RECORD.undo();
    };
  }

  function boot() {
    var root = document.querySelector('.sorting-board');
    if (!root || root.dataset.liveBound) return;
    if (typeof Component !== 'function' || !document.getElementById('board-template')) return;
    root.dataset.liveBound = '1';

    var sessionId = root.getAttribute('data-session-id');
    var route = readJson('sorting-route', []);
    var van = readJson('sorting-van', {});
    var actions = readJson('sorting-actions', []);
    var storedRules = readJson('sorting-rules', null);

    // The route is not a prop — it is the world the rules are about — so it is
    // installed once, before anything reads a customer.
    configureRoute(route);

    var props = {
      tier: Number(root.getAttribute('data-foresight')) || 1,
      accent: '#B48EF7',
      storage: 'on',
      routeName: root.getAttribute('data-route-name') || '',
      routeDate: (root.getAttribute('data-route-date') || '').toUpperCase(),
      // The board draws a back arrow; here it has somewhere to go.
      onBack: function () { window.location.href = '/sorting'; }
    };
    ['rows', 'capacity', 'sideDoorRows', 'sideSpots', 'backSpots'].forEach(function (k) {
      if (van[k] != null) props[k] = van[k];
    });
    props.rules = van.stability != null ? { stability: van.stability } : {};

    // Shape the van before replaying into it: normalize() and everything that
    // walks ORDER read positions that only exist once configure() has run.
    configure(props);
    BOARD_HOST = 'sort-live-board';
    COMPONENT = new Component(props);

    // Replay is the last row, not the whole log: each action carries the board
    // after it, so the newest one *is* the van. The older rows are history —
    // what undo walks back through, and what a person reads to find out what
    // happened this morning.
    var last = actions.length ? actions[actions.length - 1] : null;
    var seq = last ? last.seq : 0;
    if (last) {
      try {
        var replayed = JSON.parse(last.payload);
        if (replayed && replayed.van) COMPONENT.state.st = normalize(replayed);
      } catch (e) {
        /* an unreadable row leaves an empty van, which the log still explains */
      }
    } else {
      COMPONENT.state.st = emptyState();
    }

    // Undo across a reload. The demo's history is a stack of whole states in
    // memory and dies with the tab, which was fine when the tab was the only
    // place the board existed. Here the log *is* that stack — each row is the
    // board after one action, so the state before action N is row N-1 — and a
    // driver whose tablet went to sleep mid-load should still be able to take
    // back the push they just made.
    //
    // Capped at the same 200 the board caps its own history at: a sixty-crate
    // load is roughly eighty actions, and nobody walks back further than that.
    var HIST_CAP = 200;
    var history = [emptyState()];
    for (var i = 0; i < actions.length - 1; i++) {
      try {
        var earlier = JSON.parse(actions[i].payload);
        if (earlier && earlier.van) history.push(normalize(earlier));
      } catch (e) {
        /* a row we cannot read is a step undo will not walk back through */
      }
    }
    COMPONENT.state.hist = history.slice(-HIST_CAP);

    // The driver's saved van and rules, raised to fit whatever is already
    // aboard — a shape that would drop a loaded position is not offered.
    var saved = readSettings(storedRules ? JSON.stringify(storedRules) : null);
    if (saved) {
      var fitted = fitSettings(saved, COMPONENT.state.st);
      Object.keys(fitted.van).forEach(function (k) { COMPONENT.props[k] = fitted.van[k]; });
      COMPONENT.props.rules = fitted.rules;
    }

    RECORD = {
      append: function (kind) {
        seq += 1;
        var st = COMPONENT.state.st;
        enqueue({
          op: 'append', sessionId: sessionId, seq: seq,
          body: { seq: seq, kind: kind, state: st, crates: cratesIn(st), positions: positionsIn(st) }
        });
      },
      undo: function () {
        if (seq < 1) return;
        enqueue({ op: 'undo', sessionId: sessionId, seq: seq });
        seq -= 1;
      }
    };

    // Written after every paint rather than from each control: one place
    // instead of a dozen, and it cannot miss a path — including Restore
    // defaults, which writes nothing and so deletes the row.
    var lastRules = null;
    ON_PAINT = function () {
      progress();
      var payload = writeSettings(COMPONENT.props, van);
      var bare = !Object.keys(payload.van).length && !Object.keys(payload.rules).length;
      var text = bare ? 'null' : JSON.stringify(payload);
      if (lastRules === null) { lastRules = text; return; }   // the boot paint is not a change
      if (text === lastRules) return;
      lastRules = text;
      fetch('/api/sorting/rules', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ settings: bare ? null : payload })
      }).catch(function () { /* the board is still right; a reload reconciles */ });
    };

    function progress() {
      var text = document.getElementById('sort-progress-text');
      var fill = document.getElementById('sort-progress-fill');
      if (!text) return;
      var st = COMPONENT.state.st;
      var have = cratesIn(st);
      var total = Object.keys(COUNTS).reduce(function (a, k) { return a + COUNTS[k]; }, 0);
      var unknown = uncountedIn(st);
      // Uncounted stacks are always said out loud. A board reading "0 / 24
       // crates in" with two stacks aboard is not a progress bar, it is a lie
       // the driver has to spot.
      text.textContent = (total ? have + ' / ' + total + ' crates in'
                                : have + (have === 1 ? ' crate in' : ' crates in'))
        + (unknown ? ' · ' + unknown + ' uncounted' : '');
      if (fill) fill.style.width = total ? Math.min(100, Math.round(have * 100 / total)) + '%' : '0%';
    }

    wrapVerbs();
    wrapComponent();

    queue = loadQueue().filter(function (op) { return String(op.sessionId) === String(sessionId); });
    showSync();

    var fullBtn = document.getElementById('sort-full');
    if (fullBtn) {
      fullBtn.addEventListener('click', function () {
        var on = !root.classList.contains('is-full');
        setBoardFull(on);
        if (on) askFullScreen();
        else dropFullScreen();
      });
    }

    paint();
    fit();
    flush();
    acquireWakeLock();
  }

  if (!window.__sortingLiveBound) {
    window.__sortingLiveBound = true;

    document.addEventListener('htmx:afterSwap', boot);

    // hx-boost replaces the body's children without ever firing unload, so this
    // is the only notice the board gets that it is being navigated away from.
    // The fullscreen goes with it — the class leaves with the swap, but the
    // browser's fullscreen would not, and it would strand the driver on the
    // next page with no button and, on a tablet, no Escape key.
    document.addEventListener('htmx:beforeSwap', function (e) {
      if (e.target !== document.body) return;
      releaseWakeLock();
      dropFullScreen();
      RECORD = null;
      ON_PAINT = null;
    });

    document.addEventListener('visibilitychange', function () {
      if (document.visibilityState !== 'visible') return;
      if (!document.querySelector('.sorting-board')) return;
      acquireWakeLock();
      flush();
    });

    window.addEventListener('online', flush);
    window.addEventListener('resize', fit);

    // Escape, or the system gesture, leaves the browser's fullscreen without
    // telling the page — so follow it out and put the label back. A refused
    // request fires nothing at all, which is why the big layout survives one.
    ['fullscreenchange', 'webkitfullscreenchange'].forEach(function (ev) {
      document.addEventListener(ev, function () {
        if (!fullScreenEl()) setBoardFull(false);
        else fit();
      });
    });
  }

  // `defer` guarantees the document is parsed, so this covers the first load;
  // the afterSwap listener covers every boosted one.
  boot();
})();
