
// ── the demo state ───────────────────────────────────────────────────────────
// Mid-session on purpose: Olavstoppen is closed out and aboard, Jåtten is half
// in, and all three side spots are holding something — which is the moment the
// ordering guard and the side-door budget both have something to say.
function seed() {
  var st = emptyState();
  st.van['r1-left']  = [{ cust: 'OLA', n: 3 }];
  st.van['r1-right'] = [{ cust: 'OLA', n: 3 }];
  st.van['r2-left']  = [{ cust: 'OLA', n: 2 }];
  st.van['r2-right'] = [{ cust: 'OLA', n: 2 }];
  st.van['r3-left']  = [{ cust: 'JAT', n: 3 }];
  st.closed['OLA'] = true;
  st.staged['side-1'] = { cust: 'JAT', n: 2 };
  st.staged['side-2'] = { cust: 'HIN', n: 2 };
  st.staged['side-3'] = { cust: 'SVE', n: 4 };
  st.open = { n: 0 };
  return st;
}

class Component extends DCLogic {
  constructor(props) {
    super(props);
    this.state = { st: seed(), target: null, mode: 'space', hist: [] };
  }

  // Every mutation goes through here so Undo is a single, honest rule: put the
  // whole board back the way it was one tap ago.
  apply(fn) {
    var before = cloneState(this.state.st);
    before.open = { n: this.state.st.open.n };
    var next = cloneState(this.state.st);
    next.open = { n: this.state.st.open.n };
    fn(next);
    var hist = this.state.hist.slice();
    hist.push(before);
    if (hist.length > 40) hist.shift();
    this.setState({ st: next, hist: hist });
  }
  undo() {
    var hist = this.state.hist.slice();
    if (!hist.length) return;
    this.setState({ st: hist.pop(), hist: hist });
  }

  // The door that is actually live. The rows the side door reaches go in that
  // way; once they are full the side door is shut and the rest is back-door
  // work. A van with no side door at all just starts shut.
  liveDoor(st) { return sideDoorOpen(st) ? 'side' : 'back'; }

  // Commit whatever has been stacked straight into the open position.
  sealOpen(st) {
    var who = expectedNext(st);
    var id = resolveTarget(st, this.liveDoor(st), this.state.target, who);
    if (!id || !who || !st.open.n) return;
    st.van[id].push({ cust: who, n: st.open.n });
    st.open = { n: 0 };
  }

  // ── style helpers ──────────────────────────────────────────────────────────
  btn(kind, h, accent) {
    var base = 'height:' + h + 'px;border-radius:10px;display:flex;align-items:center;justify-content:center;'
      + 'font-weight:600;font-size:15px;white-space:nowrap;flex:none;padding:0 12px;text-align:center;line-height:1.15;';
    if (kind === 'primary')  return base + 'background:' + accent + ';color:#191624;';
    if (kind === 'go')       return base + 'background:#4FD6A8;color:#10221C;';
    if (kind === 'warn')     return base + 'background:rgba(255,181,112,.12);border:1px solid rgba(255,181,112,.45);color:#FFB570;';
    if (kind === 'stop')     return base + 'background:rgba(247,118,142,.10);border:1px solid rgba(247,118,142,.40);color:#F7768E;';
    if (kind === 'quiet')    return base + 'background:rgba(242,238,248,.05);border:1px solid #262232;color:#CDC6DD;';
    return base + 'background:rgba(242,238,248,.03);color:#4A445C;';
  }
  slotStyle(kind, colour) {
    var base = 'flex:0 1 7px;min-height:3px;border-radius:2px;';
    if (kind === 'free')  return base + 'background:rgba(242,238,248,.04);border:1px dashed rgba(242,238,248,.12);';
    if (kind === 'open')  return base + 'background:' + colour + '55;border:1px solid ' + colour + ';';
    return base + 'background:' + colour + ';';
  }

  // ── one packing spot ───────────────────────────────────────────────────────
  spotVals(spot, st, accent, wide) {
    var self = this, held = st.staged[spot.id], ps = pushState(st, spot.id, this.state.target);
    // Combining only comes up when something forces it — the ±3 rule, or the
    // floor running out. Two customers on one stack is how the wrong goods get
    // carried into a building, so it is never the quiet default.
    var why = held ? hostReason(st, spot.id) : null;
    var host = why ? stackHost(st, spot.id) : null;
    var lone = held ? singleCrateDoor(st, spot.id) : false;
    var colour = held ? CUST[held.cust].color : accent;
    var live = !!held;
    // The side spots share a row and flex; the back spot hangs off the end of a
    // band, where flexing makes it compete with nine cells and collapse — it
    // came out 126px wide with its text column at zero.
    var bh = wide ? 44 : 40, tone = live ? '#17141F' : '#0E0C14';
    var ring = ps.kind === 'ready' ? '1px solid rgba(79,214,168,.45)'
      : (ps.kind === 'physical' ? '1px solid rgba(247,118,142,.35)'
        : (ps.kind === 'empty' ? '1px solid #201C2B' : '1px solid rgba(255,181,112,.35)'));

    var chosen = this.state.target;
    var pushIntoOwn = function () { self.apply(function (s) { self.sealOpen(s); doPush(s, spot.id, null, chosen); }); };
    var pushSplit = function () {
      self.apply(function (s) {
        self.sealOpen(s);
        doPush(s, spot.id, ps.take, chosen, ps.plan ? ps.plan.cells[1] : null);
      });
    };
    var intoDoorway = function () { self.apply(function (s) { doDoorway(s, spot.id); }); };
    var stackOnHost = function () { self.apply(function (s) { doStack(s, spot.id, host.id); }); };
    var pushKind = 'off', pushLabel = '—', act = function () {};
    var hasAlt = false, altLabel = '', alt = function () {};

    if (ps.kind === 'doorway') {
      pushKind = ps.good ? 'go' : 'warn';
      pushLabel = wide ? ps.label : (lone ? 'Side door' : 'Doorway');
      act = intoDoorway;
    }
    else if (why === 'stability' && lone) {
      // A single crate at the side door settles the ±3 problem without putting
      // two customers on one stack. Mixing is the last resort, not the first.
      pushKind = 'go'; pushLabel = wide ? 'Put it at the side door' : 'Side door'; act = intoDoorway;
      hasAlt = true; altLabel = posLabel(ps.target); alt = pushIntoOwn;
    }
    else if (why === 'stability' && host) {
      // The ±3 rule leaves this stack no room on its own, so combining is the
      // move — amber rather than green, and it says what it costs.
      pushKind = 'warn'; pushLabel = 'Stack on ' + posLabel(host.id); act = stackOnHost;
      hasAlt = true; altLabel = posLabel(ps.target); alt = pushIntoOwn;
    }
    else if (ps.kind === 'ready')    { pushKind = 'go';   pushLabel = wide ? ps.label : 'Push in'; act = pushIntoOwn; }
    else if (ps.kind === 'chosen')   { pushKind = 'warn'; pushLabel = wide ? ps.label : 'Push in'; act = pushIntoOwn; }
    else if (ps.kind === 'split')    { pushKind = 'warn'; pushLabel = ps.label; act = pushSplit; }
    else if (ps.kind === 'nofit')    { pushKind = 'stop'; pushLabel = ps.label; }
    else if (ps.kind === 'order')    { pushKind = 'warn'; pushLabel = wide ? 'Push in anyway' : 'Anyway'; act = pushIntoOwn; }
    else if (ps.kind === 'thin')     { pushKind = 'warn'; pushLabel = wide ? 'Push in anyway' : 'Anyway'; act = pushIntoOwn; }
    else if (ps.kind === 'physical') { pushKind = 'stop'; pushLabel = ps.label; }

    // What sits beside the primary, in the order the van prefers: a lone crate
    // at the side door, then the doorway as an escape, and only then somebody
    // else's stack. Kept out of the chain above — an `if` spliced into an
    // else-if chain silently orphans everything below it, which is exactly the
    // bug this block replaces.
    if (held && ps.target && !isDoor(ps.target) && !hasAlt) {
      if (lone) { hasAlt = true; altLabel = 'Side door'; alt = intoDoorway; }
      else if (spaceIsTight(st) && doorwayFree(st, spot.door)) { hasAlt = true; altLabel = 'Doorway'; alt = intoDoorway; }
      else if (why === 'space' && host) { hasAlt = true; altLabel = 'On ' + posLabel(host.id); alt = stackOnHost; }
    }

    var takeNext = unstagedNext(st);
    var sub, subCol = '#8D87A0';
    if (!held) { sub = takeNext ? 'start ' + CUST[takeNext].short : 'nothing waiting'; subCol = takeNext ? '#CBB0FF' : '#5F5876'; }
    else if (why === 'stability' && lone) {
      sub = 'no room beside ' + posLabel(ps.target) + '’s neighbour — and one crate is easier '
        + 'to reach at the side door anyway';
      subCol = '#FFB570';
    }
    else if (why === 'stability' && host) {
      sub = 'no room beside ' + posLabel(ps.target) + '’s neighbour — but two customers on one stack '
        + 'is how the wrong crate gets carried in';
      subCol = '#FFB570';
    }
    else if (ps.why) { sub = ps.why; subCol = ps.kind === 'physical' || ps.good === false ? '#F7768E' : '#FFB570'; }
    else if (lone) { sub = 'one crate — easier to reach at the side door, and off anybody else’s stack'; subCol = '#FFB570'; }
    else if (why === 'space' && host) { sub = 'floor is short — ' + CUST[host.below].short + '’s stack could take it'; subCol = '#FFB570'; }
    else { sub = 'stop ' + stopOf(held.cust).i + ' of 6 · staged'; }

    return {
      tile: (wide ? 'flex:1 1 0;min-width:0;height:146px;' : 'width:244px;flex:none;height:146px;')
        + 'background:' + tone + ';border:' + ring + ';border-radius:12px;'
        + 'padding:9px 10px;display:flex;flex-direction:column;gap:' + (wide ? 7 : 5) + 'px;',
      tileV: 'flex:1 1 0;min-height:0;background:' + tone + ';border:' + ring + ';border-radius:12px;'
        + 'padding:9px 10px;display:flex;flex-direction:column;gap:7px;',
      name: spot.name,
      nameStyle: 'font-family:' + MONO + ';font-size:11px;letter-spacing:.06em;color:#FFB570;',
      dest: ps.target ? '→ ' + posLabel(ps.target) : (ps.kind === 'physical' ? 'no way in' : '—'),
      destStyle: 'font-family:' + MONO + ';font-size:11px;color:' + (ps.kind === 'physical' ? '#F7768E' : '#5F5876') + ';',
      count: held ? String(held.n) : '+',
      countStyle: 'width:' + (wide ? 66 : 54) + 'px;height:' + (wide ? 54 : 48) + 'px;flex:none;border-radius:11px;display:flex;align-items:center;justify-content:center;'
        + 'font-family:Archivo,system-ui,sans-serif;font-weight:800;font-size:' + (held ? 28 : 24) + 'px;letter-spacing:-0.02em;'
        + (held ? 'background:' + colour + '1F;border:1px solid ' + colour + '55;color:#F2EEF8;'
               : 'background:rgba(242,238,248,.04);border:1px dashed #2E2940;color:#8D87A0;'),
      plus: function () {
        self.apply(function (s) {
          if (s.staged[spot.id]) doBump(s, spot.id, 1);
          else { var k = unstagedNext(s); if (k) { doAssign(s, spot.id, k); doBump(s, spot.id, 1); } }
        });
      },
      head: held ? CUST[held.cust].name : 'Take next',
      headStyle: 'font-family:Archivo,system-ui,sans-serif;font-weight:700;font-size:' + (wide ? 17 : 15) + 'px;'
        + 'line-height:1.15;color:' + (held ? '#F2EEF8' : '#8D87A0') + ';letter-spacing:-0.02em;'
        + 'overflow:hidden;text-overflow:ellipsis;white-space:nowrap;',
      sub: sub,
      subStyle: 'font-size:12px;line-height:1.25;color:' + subCol + ';overflow:hidden;text-overflow:ellipsis;white-space:nowrap;',
      minus: function () { self.apply(function (s) { doBump(s, spot.id, -1); }); },
      minusStyle: 'width:36px;height:36px;flex:none;border-radius:9px;display:flex;align-items:center;justify-content:center;'
        + 'font-size:19px;' + (held && held.n > 0 ? 'background:rgba(242,238,248,.05);border:1px solid #262232;color:#8D87A0;'
                                                  : 'background:rgba(242,238,248,.02);color:#3A3548;'),
      push: act,
      pushStyle: this.btn(pushKind, bh, accent) + 'flex:1 1 0;min-width:0;overflow:hidden;',
      pushLabel: pushLabel,
      hasAlt: hasAlt,
      alt: alt,
      altStyle: this.btn('quiet', bh, accent) + 'width:' + (wide ? 82 : 66) + 'px;font-size:13px;padding:0 6px;',
      altLabel: altLabel,
      close: function () { self.apply(function (s) { doClose(s, spot.id); }); },
      closeStyle: this.btn(held ? 'quiet' : 'off', bh, accent) + 'width:' + (wide ? 74 : 62) + 'px;',
      closeLabel: 'Done'
    };
  }

  // ── one van position ───────────────────────────────────────────────────────
  // `tight` says the cell is too narrow to hold a name — nine positions across
  // 1440px leaves about 70px for text, so the colour identifies and a three
  // letter code names.
  cellVals(id, st, accent, cap, tight, plan) {
    var self = this;
    var door = this.liveDoor(st);
    var whoNow = expectedNext(st);
    var frontier = resolveTarget(st, door, this.state.target, whoNow);
    var picked = targetIsChosen(st, door, this.state.target, whoNow) && id === frontier;
    var openHere = id === frontier && st.open.n > 0;
    var who = expectedNext(st);
    var layers = st.van[id].slice();
    if (openHere) layers.push({ cust: who, n: st.open.n, open: true });

    var total = 0, unknown = false;
    layers.forEach(function (l) { if (l.n == null) unknown = true; else total += l.n; });

    var isNext = id === frontier;
    var state = layers.length ? (openHere ? 'open' : 'in') : (isNext ? 'next' : 'empty');
    var reach = doorOf(id) === 'side' && !sideDoorOpen(st) && !layers.length;
    // Only an empty position this door can still reach is worth tapping; tap
    // anything else and the board goes back to choosing for you.
    var pickable = !layers.length && doorOf(id) === door;

    var byHeight = [];
    layers.forEach(function (l) { for (var i = 0; i < (l.n || 1); i++) byHeight.push(l); });
    var slots = [];
    for (var i = 0; i < cap; i++) {
      var h = cap - i, l = byHeight[h - 1];
      slots.push({ style: self.slotStyle(l ? (l.open ? 'open' : 'full') : 'free', l ? CUST[l.cust].color : accent) });
    }
    // Nine rows down a portrait screen leaves a cell too short for eight fixed
    // 7px slots, so they shrink instead of overflowing, and stack from the
    // bottom the way a real one does.
    // Eighteen positions each drawing eight dashed slots is a lot of grid for
    // a van that is mostly empty, and the head already says "8 free". Draw the
    // column only where there is something to show.
    var showSlots = layers.length > 0 || isNext || !!ghost;
    var slotCol = showSlots
      ? 'display:flex;flex-direction:column;justify-content:flex-end;gap:2px;width:18px;flex:none;'
        + 'align-self:stretch;min-height:0;'
      : 'display:none;';

    var names = [];
    layers.forEach(function (l) { if (names.indexOf(l.cust) < 0) names.push(l.cust); });
    // Two customers on one stack is the thing worth spotting from across the
    // van, so it takes the pill rather than hiding in a sub-line.
    var mixed = custCount(st, id) > 1;
    // With counts in hand the board can say what belongs here before anything
    // is lifted. Drawn as a ghost, never as a fact.
    var ghost = (!layers.length && plan && plan.van[id] && plan.van[id].length) ? plan.van[id] : null;
    var pill = state === 'open' ? 'OPEN' : (state === 'in' ? (mixed ? 'MIXED' : 'IN')
      : (isNext ? (picked ? 'PICKED' : 'NEXT') : (reach ? 'SHUT' : (ghost ? 'PLANNED' : 'EMPTY'))));
    var pillCol = mixed && state === 'in' ? ['rgba(255,181,112,.20)', '#FFB570']
      : (state === 'open' ? ['rgba(255,181,112,.16)', '#FFB570']
      : (state === 'in' ? ['rgba(79,214,168,.14)', '#4FD6A8']
        : (state === 'next' ? [accent + '26', '#CBB0FF']
          : (reach ? ['rgba(247,118,142,.12)', '#F7768E']
            : (ghost ? ['rgba(122,162,247,.12)', '#7AA2F7'] : ['rgba(242,238,248,.05)', '#5F5876'])))));

    return {
      tile: 'flex:1 1 0;min-width:0;height:146px;padding:' + (isNext ? '7px 8px' : '8px 9px') + ';border-radius:12px;'
        + 'display:flex;flex-direction:column;gap:5px;'
        + 'background:' + (layers.length ? '#17141F' : '#0E0C14') + ';'
        + 'border:' + (isNext ? '2px solid ' + (picked ? '#FFB570' : accent + '80')
          : '1px solid ' + (layers.length ? '#241F30' : '#1A1723')) + ';',
      tileV: 'flex:1 1 0;min-width:0;height:100%;padding:' + (isNext ? '8px 10px' : '9px 11px') + ';border-radius:12px;'
        + 'display:flex;flex-direction:column;gap:5px;'
        + 'background:' + (layers.length ? '#17141F' : '#0E0C14') + ';'
        + 'border:' + (isNext ? '2px solid ' + (picked ? '#FFB570' : accent + '80')
          : '1px solid ' + (layers.length ? '#241F30' : '#1A1723')) + ';',
      pick: function () { self.setState({ target: pickable && id !== frontier ? id : null }); },
      // Nine across leaves no room for "R4 · R" beside a pill, and the row
      // number is already in the header and the column in the band label.
      pos: tight ? (colOf(id) === 'left' ? 'L' : 'R') : posLabel(id),
      pillText: pill,
      pillStyle: 'font-family:' + MONO + ';font-size:' + (tight ? 9 : 10) + 'px;letter-spacing:.05em;'
        + 'padding:2px ' + (tight ? 5 : 6) + 'px;border-radius:999px;white-space:nowrap;'
        + 'background:' + pillCol[0] + ';color:' + pillCol[1] + ';',
      slots: slots,
      slotCol: slotCol,
      head: layers.length || !ghost
        ? (this.state.mode === 'space'
            ? (unknown ? '?' : String(Math.max(0, cap - total)) + ' free')
            : (names.length ? names.map(function (k) { return tight ? CUST[k].code : CUST[k].short; }).join(' + ') : '—'))
        : ghost.map(function (l) { return tight ? CUST[l.cust].code : CUST[l.cust].short; }).join(' + '),
      headStyle: 'font-family:Archivo,system-ui,sans-serif;font-weight:700;'
        + 'font-size:' + (!layers.length && ghost ? 14 : (this.state.mode === 'space' ? (tight ? 17 : 21) : 14)) + 'px;line-height:1.1;'
        + 'color:' + (layers.length || isNext ? '#F2EEF8' : (ghost ? '#7AA2F7' : '#4A445C')) + ';letter-spacing:-0.02em;'
        + 'overflow:hidden;text-overflow:ellipsis;white-space:nowrap;',
      sub: layers.length ? ((unknown ? 'uncounted' : total + (total === 1 ? ' crate' : ' crates'))
          + (mixed ? ' · two customers' : ''))
        : (isNext ? (picked ? 'you picked this' : 'next in')
          : (reach ? 'side door shut'
            : (ghost ? ghost.reduce(function (a, l) { return a + l.n; }, 0) + ' planned'
              : (pickable ? 'send here' : 'empty')))),
      subStyle: 'font-size:12px;line-height:1.2;color:'
        + (reach ? '#F7768E' : (picked ? '#FFB570' : (isNext ? '#CBB0FF' : (ghost ? '#7AA2F7' : '#5F5876')))) + ';'
        + 'overflow:hidden;text-overflow:ellipsis;white-space:nowrap;'
    };
  }

  renderVals() {
    var self = this;
    // The van is a setting, and there is more than one van. Reshape first, then
    // make the board's state fit the shape it just got.
    configure(this.props);
    var st = normalize(this.state.st);
    // How much was scanned before the doors opened. 1 = the route list only;
    // 2 adds a count per customer; 3 adds which pallet each one is buried in.
    var tier = this.props.tier == null ? 1 : this.props.tier;
    var plan = tier >= 2 ? planAhead(COUNTS, st) : null;
    var accent = this.props.accent == null ? '#B48EF7' : this.props.accent;
    var cap = CAP;
    var door = this.liveDoor(st);
    var who = expectedNext(st);
    var frontier = resolveTarget(st, door, this.state.target, who);
    var picked = targetIsChosen(st, door, this.state.target, who);
    var sideLeft = positionsLeft(st, 'side');
    var sideStaged = stagedAtDoor(st, 'side');
    var free = positionsLeft(st, 'side') + positionsLeft(st, 'back');
    var doneStops = QUEUE.filter(function (k) { return st.closed[k]; }).length;

    // ── header ───────────────────────────────────────────────────────────────
    var big = 'font-family:Archivo,system-ui,sans-serif;font-weight:800;font-size:23px;line-height:1;letter-spacing:-0.02em;color:';
    var stats = [
      { label: 'POSITIONS LEFT', value: positionsHeld(st) ? free + ' · ' + positionsHeld(st) + ' held' : String(free),
        style: big + '#F2EEF8;' },
      { label: 'SIDE DOOR', value: sideLeft ? String(sideLeft) + ' left' : 'shut', style: big + (sideLeft ? '#FFB570;' : '#F7768E;') },
      { label: 'CRATES IN', value: String(cratesIn(st) + st.open.n), style: big + '#CDC6DD;' },
      { label: 'STOPS', value: doneStops + ' / ' + QUEUE.length, style: big + '#CDC6DD;' }
    ];
    function modeBtn(key, label) {
      var on = self.state.mode === key;
      return {
        label: label,
        pick: function () { self.setState({ mode: key }); },
        style: 'padding:8px 13px;border-radius:7px;font-size:13px;font-weight:600;white-space:nowrap;'
          + (on ? 'background:' + accent + ';color:#191624;' : 'color:#8D87A0;')
      };
    }
    var modes = [modeBtn('space', 'Space left'), modeBtn('who', 'Who goes where')];

    // The doc says to flag an off-route or unlabelled crate rather than guess
    // at it. Live, that is a tap: the board cannot tell what is on the label,
    // but it can carry the count to whoever asks at the depot.
    var flag = {
      label: st.flags ? '\u2691 ' + st.flags + ' odd' : '\u2691 Odd crate',
      tap: function () { self.apply(function (s) { s.flags = (s.flags || 0) + 1; }); },
      style: 'height:40px;padding:0 13px;border-radius:10px;display:flex;align-items:center;justify-content:center;'
        + 'font-size:13px;font-weight:600;white-space:nowrap;flex:none;'
        + (st.flags ? 'background:rgba(247,118,142,.12);border:1px solid rgba(247,118,142,.45);color:#F7768E;'
                    : 'background:rgba(242,238,248,.04);border:1px solid #262232;color:#8D87A0;')
    };

    // ── the console ──────────────────────────────────────────────────────────
    var bar;
    if (!who || !frontier) {
      bar = {
        box: 'display:flex;align-items:center;gap:10px;background:rgba(79,214,168,.08);border:1px solid rgba(79,214,168,.35);border-radius:14px;padding:11px 14px;height:76px;flex:none;',
        boxV: 'display:flex;flex-direction:column;justify-content:center;gap:8px;background:rgba(79,214,168,.08);border:1px solid rgba(79,214,168,.35);border-radius:14px;padding:12px 14px;height:140px;flex:none;',
        eyebrow: !who ? 'EVERY STOP CLOSED OUT' : 'VAN FULL',
        eyebrowStyle: 'font-family:' + MONO + ';font-size:12px;letter-spacing:.10em;color:#4FD6A8;',
        title: !who ? 'Loaded' : 'No positions left',
        titleStyle: 'font-family:Archivo,system-ui,sans-serif;font-weight:700;font-size:22px;color:#F2EEF8;letter-spacing:-0.02em;',
        sub: cratesIn(st) + ' crates across ' + (14 - free) + ' positions',
        minus: function () {}, minusStyle: this.btn('off', 56, accent) + 'width:0;padding:0;border:0;',
        plus: function () {}, plusStyle: this.btn('off', 56, accent) + 'width:0;padding:0;border:0;', plusLabel: '',
        seal: function () {}, sealStyle: this.btn('off', 56, accent) + 'width:0;padding:0;border:0;', sealLabel: '',
        close: function () {}, closeStyle: this.btn('off', 56, accent) + 'width:0;padding:0;border:0;', closeLabel: '',
        undo: function () { self.undo(); }, undoStyle: this.btn('quiet', 56, accent) + 'width:92px;'
      };
    } else {
      var openN = st.open.n;
      bar = {
        box: 'display:flex;align-items:center;gap:8px;background:#13101B;border:1px solid '
          + (picked ? 'rgba(255,181,112,.55)' : accent + '55') + ';border-radius:14px;padding:10px 12px;height:76px;flex:none;',
        // Upright the console stacks — what is going in on top, the hands
        // underneath — so it needs a column box, not the landscape row.
        boxV: 'display:flex;flex-direction:column;gap:8px;background:#13101B;border:1px solid '
          + (picked ? 'rgba(255,181,112,.55)' : accent + '55') + ';border-radius:14px;padding:12px 14px;height:140px;flex:none;',
        eyebrow: (tier >= 3 && PALLETS[who] ? 'PALLET ' + PALLETS[who] + ' → ' : 'STRAIGHT OFF THE PALLET → ')
          + posLabel(frontier)
          + (picked ? ' · YOUR PICK' : ' · ' + (door === 'side' ? 'SIDE DOOR' : 'BACK DOORS')),
        eyebrowStyle: 'font-family:' + MONO + ';font-size:12px;letter-spacing:.09em;color:'
          + (picked ? '#FFB570' : '#CBB0FF') + ';',
        title: CUST[who].name,
        titleStyle: 'font-family:Archivo,system-ui,sans-serif;font-weight:700;font-size:24px;color:#F2EEF8;letter-spacing:-0.025em;white-space:nowrap;',
        sub: 'stop ' + stopOf(who).i + ' of 6 · ' + (openN ? openN + ' stacked here' : 'nothing in yet')
          + (tier >= 2 && COUNTS[who] ? ' · ' + COUNTS[who] + ' expected' : ''),
        minus: function () { self.apply(function (s) { s.open.n = Math.max(0, s.open.n - 1); }); },
        minusStyle: this.btn(openN ? 'quiet' : 'off', 56, accent) + 'width:56px;font-size:22px;',
        plus: function () { self.apply(function (s) { s.open.n = s.open.n + 1; }); },
        plusStyle: this.btn('primary', 56, accent) + 'width:212px;font-size:18px;font-weight:700;',
        plusLabel: '+ 1 crate in',
        seal: function () { self.apply(function (s) { self.sealOpen(s); }); },
        sealStyle: this.btn(openN ? 'quiet' : 'off', 56, accent) + 'width:186px;',
        sealLabel: openN ? 'Full · next position' : 'Full · next',
        close: function () { self.apply(function (s) { self.sealOpen(s); s.closed[who] = true; }); },
        closeStyle: this.btn('go', 56, accent) + 'width:176px;',
        closeLabel: 'Done · ' + CUST[who].short,
        undo: function () { self.undo(); },
        undoStyle: this.btn(this.state.hist.length ? 'quiet' : 'off', 56, accent) + 'width:92px;'
      };
    }

    // ── running out of van before running out of route ───────────────────────
    // The generator settles this before a crate is lifted. Live, nobody knows
    // until it happens — so the board watches the two numbers and says the
    // moment they cross, while combining is still an option.
    var notInYet = QUEUE.filter(function (k) { return !st.closed[k] && !isAboard(st, k); }).length;
    var warn = { show: free < notInYet, text: '', style: '' };
    warn.text = free + ' position' + (free === 1 ? '' : 's') + ' left and ' + notInYet
      + ' stops with nothing aboard — some of them will have to share a stack.';
    warn.style = 'display:flex;align-items:center;gap:10px;padding:8px 14px;border-radius:11px;flex:none;'
      + 'background:rgba(247,118,142,.09);border:1px solid rgba(247,118,142,.35);'
      + 'font-size:13px;color:#F7768E;';

    // ── the side door, and its budget ────────────────────────────────────────
    // The failure this heads off: three stacks standing at the side door and
    // only two positions left that the side door can still reach.
    var sideDoor;
    if (!sideLeft) {
      sideDoor = {
        label: 'SIDE DOOR · SHUT',
        labelStyle: 'font-family:' + MONO + ';font-size:12px;letter-spacing:.10em;color:#F7768E;',
        note: SIDE_DOOR_ROWS ? 'Rows 1–' + SIDE_DOOR_ROWS + ' are full. Everything left goes in through the back.'
          : 'No side door on this van — everything goes in through the back.',
        noteStyle: 'font-size:13px;color:#8D87A0;'
      };
    } else {
      // Naming the shortfall is not the same as naming who it lands on. The
      // stacks go in in loading order, so the ones past the position count are
      // the ones that will still be standing at the door when it shuts.
      var queued = SPOTS.filter(function (sp) { return sp.door === 'side' && st.staged[sp.id]; })
        .sort(function (a, b) {
          return QUEUE.indexOf(st.staged[a.id].cust) - QUEUE.indexOf(st.staged[b.id].cust);
        });
      var stranded = queued.slice(sideLeft);
      var tight = stranded.length > 0;
      sideDoor = {
        label: 'SIDE DOOR · OPEN · REACHES ROWS 1–' + SIDE_DOOR_ROWS,
        labelStyle: 'font-family:' + MONO + ';font-size:12px;letter-spacing:.10em;color:#FFB570;',
        note: sideLeft + ' position' + (sideLeft === 1 ? '' : 's') + ' left · ' + sideStaged + ' staged here'
          + (tight ? ' — ' + stranded.map(function (sp) {
              return sp.name + ' (' + CUST[st.staged[sp.id].cust].short + ')';
            }).join(' and ') + ' will still be standing here when it shuts'
            : ''),
        noteStyle: 'font-size:13px;color:' + (tight ? '#FFB570' : '#5F5876') + ';'
      };
    }

    // ── the map ──────────────────────────────────────────────────────────────
    var heads = [];
    for (var r = 1; r <= ROWS; r++) {
      heads.push({
        label: 'R' + r,
        style: 'flex:1 1 0;min-width:0;font-family:' + MONO + ';font-size:11px;letter-spacing:.08em;'
          + 'color:' + (r <= SIDE_DOOR_ROWS ? (sideLeft ? '#FFB570' : '#5F5876') : '#8D87A0') + ';'
      });
    }
    // One back spot hangs off the end of each band. With only one fitted, the
    // driver-side band gets a dead tile rather than a band of a different shape.
    var backList = SPOTS.filter(function (s) { return s.door === 'back'; });
    var bands = [
      { col: 'right', label: 'RIGHT', sub: 'kerb',   spot: backList[0] },
      { col: 'left',  label: 'LEFT',  sub: 'driver', spot: backList[1] || backList[0] }
    ].map(function (b) {
      var cells = [];
      for (var r = 1; r <= ROWS; r++) cells.push(self.cellVals('r' + r + '-' + b.col, st, accent, cap, ROWS >= 8, plan));
      return { label: b.label, sub: b.sub, cells: cells, spot: self.spotVals(b.spot, st, accent, false) };
    });

    var sideSpots = SPOTS.filter(function (s) { return s.door === 'side'; })
      .map(function (s) { return self.spotVals(s, st, accent, true); });

    // Portrait turns the same map through ninety degrees: the cab goes to the
    // top and the rows run down the screen, which puts the van's right side —
    // and so the side door — on the right. Same cells, grouped by row instead
    // of by column, and split at the door boundary so the side spots can sit
    // beside exactly the rows they can reach.
    function rowVals(r) {
      return {
        label: 'R' + r,
        labelStyle: 'font-family:' + MONO + ';font-size:12px;letter-spacing:.08em;'
          + 'color:' + (r <= SIDE_DOOR_ROWS ? (sideLeft ? '#FFB570' : '#5F5876') : '#8D87A0') + ';',
        cells: [self.cellVals('r' + r + '-left', st, accent, cap, false, plan),
                self.cellVals('r' + r + '-right', st, accent, cap, false, plan)]
      };
    }
    var rowsA = [], rowsB = [];
    for (var rr = 1; rr <= ROWS; rr++) (rr <= SIDE_DOOR_ROWS ? rowsA : rowsB).push(rowVals(rr));
    var backSpots = backList.map(function (s) { return self.spotVals(s, st, accent, true); });
    // The two portrait zones are split at the door boundary, so they have to
    // take their share of the height from how many rows each actually holds.
    var zoneA = { style: 'display:flex;gap:8px;flex:' + Math.max(rowsA.length, 1) + ' 1 0;min-height:0;' };
    var zoneB = { style: 'display:flex;gap:8px;flex:' + Math.max(rowsB.length, 1) + ' 1 0;min-height:0;' };

    // ── the doorways ─────────────────────────────────────────────────────────
    // Off the grid, so drawn off the map — and only once they are either in use
    // or the only floor left worth talking about.
    var inUse = DOORS.filter(function (id) { return !isEmpty(st, id); });
    var offering = SPOTS.some(function (sp) { return singleCrateDoor(st, sp.id); });
    var doorways = { show: inUse.length > 0 || spaceIsTight(st) || offering, tiles: [] };
    doorways.tiles = DOORS.map(function (id) {
      var stack = st.van[id], held = stack.length ? stack[stack.length - 1] : null;
      var door = doorOf(id), isSide = door === 'side';
      var mine = held ? stopOf(held.cust) : null;
      // Fine here means it is not in anybody's way: either it comes out first,
      // or it is the single crate this space is best used for.
      var fine = held && (mine.i === 1 || held.n === 1);
      var frozen = isSide && st.frozenAtDoor;
      return {
        tile: 'flex:1 1 0;min-width:0;border-radius:11px;padding:8px 12px;display:flex;align-items:center;gap:12px;'
          + 'background:' + (held ? '#17141F' : '#0A080E') + ';'
          + 'border:1px solid ' + (held ? (fine ? 'rgba(79,214,168,.35)' : 'rgba(247,118,142,.40)')
                                       : 'rgba(255,181,112,.28)') + ';',
        name: posLabel(id),
        nameStyle: 'font-family:' + MONO + ';font-size:11px;letter-spacing:.07em;flex:none;'
          + 'color:' + (held ? (fine ? '#4FD6A8' : '#F7768E') : '#FFB570') + ';',
        head: held ? CUST[held.cust].name : 'free',
        headStyle: 'font-family:Archivo,system-ui,sans-serif;font-weight:700;font-size:15px;letter-spacing:-0.02em;'
          + 'color:' + (held ? '#F2EEF8' : '#5F5876') + ';overflow:hidden;text-overflow:ellipsis;white-space:nowrap;',
        sub: held
          ? (held.n == null ? 'uncounted' : held.n + (held.n === 1 ? ' crate' : ' crates'))
            + ' · stop ' + mine.i
            + (held.n === 1 ? ' · one crate, easy to reach'
                            : (mine.i === 1 ? ' · out first, so it is never in the way'
                                            : ' · in the way until then'))
          : (isSide
              ? 'no pushing in past rows 1–' + SIDE_DOOR_ROWS + ' — but a crate can stand here'
              : 'floor of last resort — it blocks the door'),
        subStyle: 'font-size:12px;color:' + (held && !fine ? '#F7768E' : '#5F5876') + ';'
          + 'overflow:hidden;text-overflow:ellipsis;white-space:nowrap;',
        // The freeze ware goes in here at the end and has to still fit, so the
        // board carries the fact rather than pretending the space is empty.
        hasFreeze: isSide,
        freezeLabel: frozen ? '❄ freeze ware' : '❄ none today',
        freezeStyle: 'height:38px;padding:0 11px;border-radius:9px;display:flex;align-items:center;'
          + 'justify-content:center;font-size:12px;font-weight:600;white-space:nowrap;flex:none;'
          + (frozen ? 'background:rgba(122,162,247,.14);border:1px solid rgba(122,162,247,.45);color:#7AA2F7;'
                    : 'background:rgba(242,238,248,.04);border:1px solid #262232;color:#5F5876;'),
        freezeTap: function () { self.apply(function (x) { x.frozenAtDoor = !x.frozenAtDoor; }); },
        clear: function () { if (held) self.apply(function (x) { doClearDoorway(x, door); }); },
        clearStyle: this.btn(held ? 'quiet' : 'off', 38, accent) + 'width:104px;font-size:13px;',
        clearLabel: held ? 'Take it out' : '—'
      };
    }, this);

    // ── the route, in loading order ──────────────────────────────────────────
    var queue = QUEUE.map(function (k) {
      var at = spotHolding(st, k), pos = positionsOf(st, k);
      var closed = !!st.closed[k], isNow = k === who;
      var state, col;
      if (closed) { state = 'DONE  \u21BA'; col = '#4FD6A8'; }
      else if (isNow) { state = 'LOADING NOW'; col = '#CBB0FF'; }
      else if (at) { state = 'ON ' + at.name; col = '#FFB570'; }
      else { state = 'WAITING'; col = '#5F5876'; }
      return {
        pick: function () { if (closed) self.apply(function (s) { doReopen(s, k); }); },
        tile: 'flex:1 1 0;min-width:0;border-radius:11px;padding:8px 10px;display:flex;flex-direction:column;gap:2px;'
          + 'background:' + (isNow ? '#17141F' : '#0E0C14') + ';'
          + 'border:1px solid ' + (isNow ? accent + '70' : (closed ? 'rgba(79,214,168,.22)' : '#1A1723')) + ';',
        tileV: 'flex:1 1 0;min-height:0;border-radius:11px;padding:7px 10px;display:flex;flex-direction:column;'
          + 'justify-content:center;gap:1px;'
          + 'background:' + (isNow ? '#17141F' : '#0E0C14') + ';'
          + 'border:1px solid ' + (isNow ? accent + '70' : (closed ? 'rgba(79,214,168,.22)' : '#1A1723')) + ';',
        dot: 'width:9px;height:9px;border-radius:3px;flex:none;background:' + CUST[k].color + (closed ? '' : '99') + ';',
        name: CUST[k].name,
        nameStyle: 'font-family:Archivo,system-ui,sans-serif;font-weight:700;font-size:14px;letter-spacing:-0.01em;'
          + 'color:' + (closed || isNow ? '#F2EEF8' : '#8D87A0') + ';overflow:hidden;text-overflow:ellipsis;white-space:nowrap;',
        where: pos.length ? pos.map(posLabel).join('  ') : 'stop ' + stopOf(k).i + ' · not in yet',
        whereStyle: 'font-family:' + MONO + ';font-size:11px;color:' + (pos.length ? '#CDC6DD' : '#5F5876') + ';'
          + 'overflow:hidden;text-overflow:ellipsis;white-space:nowrap;',
        state: state,
        stateStyle: 'font-family:' + MONO + ';font-size:10px;letter-spacing:.08em;color:' + col + ';'
      };
    });

    return { stats: stats, modes: modes, flag: flag, bar: bar, sideDoor: sideDoor, warn: warn,
             sideSpots: sideSpots, heads: heads, bands: bands, queue: queue,
             rowsA: rowsA, rowsB: rowsB, backSpots: backSpots, zoneA: zoneA, zoneB: zoneB,
             doorways: doorways };
  }
}
