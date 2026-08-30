// ── Live mode ────────────────────────────────────────────────────────────────
// No manifest, no generated plan, no crate counts. The only input is the stop
// list. Every rule the generator used to settle up front has to be enforced
// live instead, at the moment of the tap.
// A third length, for cells too narrow to hold a name — nine positions across
// 1440px leaves about seventy pixels for text.
//
// It has to be derived from the actual stop list, never hashed per name: three
// letters of the first word renders "Rema 1000 Hillevåg" and "Rema 1000 Madla"
// identically, and on this board a code is what you read before you carry crates
// into a building. Collisions lengthen until they separate.
function makeCodes(names) {
  var keys = Object.keys(names), out = {}, used = {}, W = {};
  function words(x) {
    return String(x).toUpperCase().replace(/[^A-ZÆØÅ0-9 ]/g, ' ').trim().split(/\s+/).filter(Boolean);
  }
  keys.forEach(function (k) { W[k] = words(names[k]); });

  // Group by the first word, because that is what collides — a street of Rema
  // 1000s, three Coops, two Jåtten somethings.
  var groups = {};
  keys.forEach(function (k) {
    var g = (W[k][0] || '').slice(0, 3);
    (groups[g] = groups[g] || []).push(k);
  });

  keys.forEach(function (k) {
    var w = W[k], first = w[0] || String(k), head = first.slice(0, 3), c = [];
    if ((groups[head] || []).length > 1) {
      // The shared word carries no information, so build the code out of the
      // word that does — skipping bare numbers, which "Rema 1000" is full of.
      var tail = w.slice(1).filter(function (t) { return !/^[0-9]+$/.test(t); });
      var d = tail[0] || w[1] || '';
      c.push(first.slice(0, 1) + d.slice(0, 2));
      c.push(first.slice(0, 1) + d.slice(0, 3));
      c.push(first.slice(0, 2) + d.slice(0, 2));
      c.push(head + d.slice(0, 1));
      if (tail[1]) c.push(first.slice(0, 1) + tail[1].slice(0, 2));
    }
    c.push(head, first.slice(0, 4), head + (w[1] || '').slice(0, 1));

    var pick = null;
    for (var i = 0; i < c.length && !pick; i++) if (c[i] && c[i].length >= 2 && !used[c[i]]) pick = c[i];
    if (!pick) {                       // never collide: number them rather than repeat
      var base = head || 'X', n = 2;
      while (used[base + n]) n++;
      pick = base + n;
    }
    used[pick] = true;
    out[k] = pick;
  });
  return out;
}

var CUST = {
  OLA: { name: 'Olavstoppen',    short: 'Olavstoppen', color: '#6FBF97' },
  JAT: { name: 'Jåtten Skole',   short: 'Jåtten', color: '#FFB570' },
  HIN: { name: 'Hinna',          short: 'Hinna', color: '#F7768E' },
  SVE: { name: 'Sverdrup Steel', short: 'Sverdrup', color: '#7AA2F7' },
  FRO: { name: 'Frøystad',       short: 'Frøystad', color: '#B48EF7' },
  MAR: { name: 'Marlink',        short: 'Marlink', color: '#4FD6A8' }
};

// Delivery order, exactly as the route app lists it. Stop 1 is delivered first.
var STOPS = [
  { i: 1, key: 'MAR' }, { i: 2, key: 'FRO' }, { i: 3, key: 'SVE' },
  { i: 4, key: 'HIN' }, { i: 5, key: 'JAT' }, { i: 6, key: 'OLA' }
];
// Loading runs the other way: the last delivery goes in first and ends up
// deepest, against the cab.
var QUEUE = STOPS.slice().reverse().map(function (s) { return s.key; });
function stopOf(k) { return STOPS.filter(function (s) { return s.key === k; })[0]; }

// ── the van ──────────────────────────────────────────────────────────────────
// None of these are the app's to decide, and there is more than one van in the
// fleet, so they are settings rather than constants.
//
// Nine rows, not the seven the methodology doc assumed — the floor turned out
// longer than it was first paced out. The side door's reach is still the doc's
// four, but that number was written against a seven-row picture, so it is worth
// re-checking against a van that is two positions longer.
//
// Two columns is the one number that is not a setting: the ±3 stability rule is
// defined per column, front to back, and left is never compared to right. That
// shape is the rule, not a measurement.
// Demo fixtures for the three levels of foresight. COUNTS is what a weighed or
// scanned order sheet gives you; PALLETS is what reading the pallet photos adds
// on top of it. Neither exists in a route-only session.
var COUNTS = { OLA: 10, JAT: 5, HIN: 2, SVE: 7, FRO: 4, MAR: 3 };
var PALLETS = { OLA: 'B', JAT: 'C', HIN: 'C', SVE: 'A', FRO: 'A', MAR: 'B' };

var ROWS, CAP, SIDE_DOOR_ROWS, STAB = 3, ORDER = [], SPOTS = [], DOORS = [], ALL_POS = [];

function configure(cfg) {
  cfg = cfg || {};
  ROWS = cfg.rows == null ? 9 : cfg.rows;
  CAP = cfg.capacity == null ? 8 : cfg.capacity;
  SIDE_DOOR_ROWS = cfg.sideDoorRows == null ? 4 : Math.min(cfg.sideDoorRows, ROWS);
  var nSide = cfg.sideSpots == null ? 3 : cfg.sideSpots;
  var nBack = cfg.backSpots == null ? 2 : cfg.backSpots;

  // Rows run 1→N because nothing can be put in front of what is already
  // aboard. Within a row LEFT goes before RIGHT because the door is on the
  // van's right: the far column has to be crossed to, and a filled near column
  // blocks that path.
  ORDER = [];
  for (var r = 1; r <= ROWS; r++) { ORDER.push('r' + r + '-left'); ORDER.push('r' + r + '-right'); }

  SPOTS = [];
  for (var i = 1; i <= nSide; i++) SPOTS.push({ id: 'side-' + i, door: 'side', name: 'SIDE ' + i });
  for (var j = 1; j <= nBack; j++) SPOTS.push({ id: 'back-' + j, door: 'back', name: 'BACK ' + j });

  // The doorways themselves. Off the grid — nothing routine goes here and
  // nextPosition() cannot reach them — but they are floor, and when the van
  // runs short they are floor you can stand a stack on.
  DOORS = [];
  if (SIDE_DOOR_ROWS > 0) DOORS.push('door-side');
  DOORS.push('door-back');
  ALL_POS = ORDER.concat(DOORS);
}
function isDoor(id) { return String(id).indexOf('door-') === 0; }
function doorwayOf(door) { return 'door-' + door; }

function rowOf(id) { return isDoor(id) ? ROWS + 1 : parseInt(String(id).slice(1), 10); }
function colOf(id) { return id.indexOf('left') > -1 ? 'left' : 'right'; }
function doorOf(id) { return isDoor(id) ? String(id).slice(5) : (rowOf(id) <= SIDE_DOOR_ROWS ? 'side' : 'back'); }
function zone(door) { return ORDER.filter(function (id) { return doorOf(id) === door; }); }
function posLabel(id) {
  if (id === 'door-side') return 'SIDE DOORWAY';
  if (id === 'door-back') return 'BACK DOORWAY';
  return 'R' + rowOf(id) + ' · ' + (colOf(id) === 'left' ? 'L' : 'R');
}

configure({});
(function () {
  var names = {};
  Object.keys(CUST).forEach(function (k) { names[k] = CUST[k].name; });
  var codes = makeCodes(names);
  Object.keys(CUST).forEach(function (k) { CUST[k].code = codes[k]; });
}());
function spotById(id) { return SPOTS.filter(function (s) { return s.id === id; })[0]; }

var MONO = "'IBM Plex Mono', monospace";

// ── reading the state ────────────────────────────────────────────────────────
// van[id] is a stack, bottom first. n === null means it went in uncounted.
function heightOf(st, id) {
  var n = 0, unknown = false;
  st.van[id].forEach(function (l) { if (l.n == null) unknown = true; else n += l.n; });
  return unknown ? null : n;
}
function isEmpty(st, id) { return st.van[id].length === 0; }

// The innermost position this door can still reach — what the push-in button
// works out so the user never has to.
// A split reserves the next position in the same column for the rest of that
// order. Held is not free — but it is only ever held for the customer whose
// stack it continues, and only while that stack is still open.
function heldFor(st, cust) {
  var keys = Object.keys(st.held || {});
  for (var i = 0; i < keys.length; i++) if (st.held[keys[i]] === cust) return keys[i];
  return null;
}
function isHeldAgainst(st, id, cust) {
  return !!(st.held && st.held[id] && st.held[id] !== cust);
}
function nextPosition(st, door, cust) {
  var z = zone(door);
  for (var i = 0; i < z.length; i++) {
    if (!isEmpty(st, z[i])) continue;
    if (cust !== undefined && isHeldAgainst(st, z[i], cust)) continue;
    return z[i];
  }
  return null;
}
function sideDoorOpen(st) { return nextPosition(st, 'side') !== null; }

// Everything outward of the innermost free position is free too — we fill in
// order — so any empty position in this door's zone is physically reachable.
// The board defaults to the innermost because that is what the loading order
// wants, but the driver can name a different one, and sometimes should: keeping
// the deep positions clear for a big order still on the pallet is a real call.
function resolveTarget(st, door, chosen, cust) {
  if (chosen && doorOf(chosen) === door && isEmpty(st, chosen)) return chosen;
  // The rest of a split order goes where the split said it would.
  var hold = cust ? heldFor(st, cust) : null;
  if (hold && doorOf(hold) === door && isEmpty(st, hold)) return hold;
  return nextPosition(st, door, cust);
}
function targetIsChosen(st, door, chosen, cust) {
  var auto = nextPosition(st, door, cust);
  return !!(chosen && resolveTarget(st, door, chosen, cust) === chosen && chosen !== auto);
}
function positionsLeft(st, door) {
  return zone(door).filter(function (id) { return isEmpty(st, id); }).length;
}
function stagedAtDoor(st, door) {
  return SPOTS.filter(function (s) { return s.door === door && st.staged[s.id]; }).length;
}
function cratesIn(st) {
  var n = 0;
  ALL_POS.forEach(function (id) { (st.van[id] || []).forEach(function (l) { n += (l.n || 0); }); });
  return n;
}
// Space is limited before it is gone. The doorway gets offered while there is
// still a choice about what goes in it, not once there is none.
function stopsNotAboard(st) {
  return QUEUE.filter(function (k) { return !st.closed[k] && !isAboard(st, k); }).length;
}
function spaceIsTight(st) {
  var free = positionsLeft(st, 'side') + positionsLeft(st, 'back');
  return free === 0 || free < stopsNotAboard(st);
}
// A doorway stack is the most reachable thing in the van and the first thing in
// the way, so it wants the earliest delivery still to be loaded.
function firstPending(st) {
  var open = STOPS.filter(function (s) { return !st.closed[s.key]; });
  return open.length ? open[0].key : null;
}
function doorwayFree(st, door) {
  var id = doorwayOf(door);
  if (DOORS.indexOf(id) < 0) return false;
  return isEmpty(st, id);
}
// Who the loading order says goes in next. Done is what advances this, which is
// why Done has to be a button and not something inferred.
function expectedNext(st) {
  for (var i = 0; i < QUEUE.length; i++) if (!st.closed[QUEUE[i]]) return QUEUE[i];
  return null;
}
// The next customer nobody has picked up yet — what an empty spot offers.
function unstagedNext(st) {
  var held = {};
  SPOTS.forEach(function (s) { if (st.staged[s.id]) held[st.staged[s.id].cust] = true; });
  for (var i = 0; i < QUEUE.length; i++) {
    if (!st.closed[QUEUE[i]] && !held[QUEUE[i]]) return QUEUE[i];
  }
  return null;
}
function spotHolding(st, cust) {
  return SPOTS.filter(function (s) { return st.staged[s.id] && st.staged[s.id].cust === cust; })[0];
}
function isAboard(st, cust) {
  return ALL_POS.some(function (id) {
    return st.van[id].some(function (l) { return l.cust === cust; }); });
}
function positionsOf(st, cust) {
  return ALL_POS.filter(function (id) {
    return st.van[id].some(function (l) { return l.cust === cust; }); });
}

// ── the stability rule, live ─────────────────────────────────────────────────
// Same column only, immediate neighbours only. A position holding nothing is
// exempt, and so is one that went in uncounted — we genuinely do not know.
function stabilityAt(st, id, wouldBe) {
  if (isDoor(id)) return [];                 // a doorway has no neighbours in a column
  var r = rowOf(id), col = colOf(id), out = [];
  [r - 1, r + 1].forEach(function (nr) {
    if (nr < 1 || nr > ROWS) return;
    var nid = 'r' + nr + '-' + col;
    if (isEmpty(st, nid)) return;
    var h = heightOf(st, nid);
    if (h == null || wouldBe == null) return;
    var d = Math.abs(wouldBe - h);
    if (d > STAB) out.push({ neighbour: nid, theirs: h, ours: wouldBe, apart: d });
  });
  return out;
}
// The heights this position may legally finish at, given both neighbours in its
// own column. Both bounds matter: too tall breaks the rule one way, too short
// breaks it the other.
function windowAt(st, id) {
  if (isDoor(id)) return { lo: 0, hi: CAP };
  var r = rowOf(id), col = colOf(id), lo = 0, hi = CAP;
  [r - 1, r + 1].forEach(function (nr) {
    if (nr < 1 || nr > ROWS) return;
    var nid = 'r' + nr + '-' + col;
    if (isEmpty(st, nid)) return;                 // an empty position is exempt
    var h = heightOf(st, nid);
    if (h == null) return;                        // so is one that went in uncounted
    hi = Math.min(hi, h + STAB);
    lo = Math.max(lo, h - STAB);
  });
  return { lo: Math.max(0, lo), hi: hi };
}
function maxAllowedAt(st, id) { return windowAt(st, id).hi; }

// Splitting a big order, solved rather than guessed.
//
// The obvious greedy — fill to the ceiling and spill the rest — is what the
// methodology warns against without naming it: behind a closed 8 it produces
// 8 then 2, a gap of six, manufactured by the mechanism that claims to enforce
// the limit. At push time the quantity is known, so divide it evenly across
// the fewest positions that will hold it, front-most cells taking the remainder.
//
// The spill goes to the next position in FILL order, which is usually the same
// row's other column — and that is free, because ±3 compares front to back
// within one column and never left to right. Ten crates come out 5 + 5 across
// one row rather than 8 + 2 down one, which is also how the generated Stavanger
// plan placed Olavstoppen's ten.
function fillRun(st, start, k) {
  var i = ORDER.indexOf(start);
  if (i < 0) return null;
  var out = [];
  for (var j = 0; j < k; j++) {
    var id = ORDER[i + j];
    if (!id) return null;
    if (j > 0 && !isEmpty(st, id)) return null;
    out.push(id);
  }
  return out;
}
function splitPlan(st, n, start, cap) {
  if (isDoor(start) || !n) return null;
  var ceiling = cap == null ? CAP : Math.min(CAP, cap);
  for (var k = 1; k <= 4; k++) {
    var cells = fillRun(st, start, k);
    if (!cells) continue;
    var base = Math.floor(n / k), rem = n % k, h = [], i;
    for (i = 0; i < k; i++) h.push(base + (i < rem ? 1 : 0));
    if (h[0] > ceiling) continue;

    // Project the whole placement and check every cell against its own column,
    // counting the other cells of this same plan. Checking only the first is
    // how a legal-looking split lands an illegal stack two positions later.
    var proj = {}, ok = true;
    for (i = 0; i < k; i++) proj[cells[i]] = h[i];
    for (i = 0; i < k && ok; i++) {
      var id = cells[i], r = rowOf(id), col = colOf(id);
      [r - 1, r + 1].forEach(function (nr) {
        if (!ok || nr < 1 || nr > ROWS) return;
        var nid = 'r' + nr + '-' + col;
        var nh = proj[nid] !== undefined ? proj[nid]
               : (isEmpty(st, nid) ? null : heightOf(st, nid));
        if (nh == null) return;                     // empty or uncounted is exempt
        if (Math.abs(proj[id] - nh) > STAB) ok = false;
      });
    }
    if (!ok) continue;
    return { cells: cells, heights: h };
  }
  return null;
}

// Run the live rules forward over counts that are already known. Same window,
// same split, same fill order — so a plan drawn up front and a board built one
// crate at a time can never disagree about what should have happened.
function planAhead(counts, from) {
  // Planning from the current board, not from an empty van: once crates are
  // aboard the only useful question is where the REST of them go.
  var st = from ? cloneState(from) : emptyState(), byCust = {}, short = [];
  if (from) { st.closed = {}; SPOTS.forEach(function (sp) { st.staged[sp.id] = null; }); }
  QUEUE.forEach(function (cust) {
    var already = 0;
    if (from) ALL_POS.forEach(function (id) {
      (st.van[id] || []).forEach(function (l) { if (l.cust === cust) already += (l.n || 0); });
    });
    var left = Math.max(0, (counts[cust] || 0) - already), guard = 0;
    while (left > 0 && guard++ < 40) {
      var door = sideDoorOpen(st) ? 'side' : 'back';
      var target = resolveTarget(st, door, null, cust);
      if (!target) break;
      var w = windowAt(st, target), take = left;
      // Because the fill order alternates columns, a stack's neighbour in its
      // own column is the customer two behind it, not the one immediately
      // after. With the counts in hand the planner can see that coming: cap
      // this stack so the next two still have a window to land in, and let the
      // split spread it. Without this, Sverdrup's 7 sits four above Marlink's 3.
      var ahead = QUEUE.slice(QUEUE.indexOf(cust) + 1, QUEUE.indexOf(cust) + 3)
        .map(function (k) { return counts[k] || 0; }).filter(function (v) { return v > 0; });
      var cap = w.hi;
      if (ahead.length) cap = Math.min(cap, Math.min.apply(null, ahead) + STAB);
      if (left > cap && left <= cap * 4) take = cap;
      if (left > cap) {
        // Ask for a split that respects the cap, so the remainder is a share
        // rather than a leftover: 7 under a cap of 6 comes out 4 + 3, not 6 + 1.
        var plan = splitPlan(st, left, target, cap);
        take = plan ? plan.heights[0] : Math.min(left, cap);
        if (plan && plan.cells[1]) st.held[plan.cells[1]] = cust;
      }
      if (take <= 0) break;
      st.van[target].push({ cust: cust, n: take });
      if (st.held[target] === cust) delete st.held[target];
      (byCust[cust] = byCust[cust] || []).push({ id: target, n: take });
      left -= take;
    }
    if (left > 0) short.push({ cust: cust, left: left });
    st.closed[cust] = true;
    Object.keys(st.held).forEach(function (id) { if (st.held[id] === cust) delete st.held[id]; });
  });
  return { van: st.van, byCust: byCust, short: short };
}

// ── the push-in guard ────────────────────────────────────────────────────────
// Two kinds of no. PHYSICAL is a hard stop: the van cannot do it. Everything
// else is the user's call — amber, allowed, and the cost spelled out.
function pushState(st, spotId, chosen) {
  var spot = spotById(spotId);
  var held = st.staged[spotId];
  if (!held) return { kind: 'empty', label: '—', why: '' };

  var target = resolveTarget(st, spot.door, chosen, held.cust);
  if (!target) {
    if (spot.door === 'side') {
      // Once rows 1–N are full nothing more can be pushed in this way — it
      // would have to travel past what is already aboard. So it gets carried
      // round to the back, and the well is only the answer when there is no
      // back left to carry it to. A single crate is the exception: it belongs
      // at the side door anyway, and walking it round the van earns nothing.
      if (doorwayFree(st, 'side') && (held.n === 1 || spaceIsTight(st))) return doorwayState(st, spotId);
      return { kind: 'physical', label: 'Round the back',
        why: 'Rows 1–' + SIDE_DOOR_ROWS + ' are full, so nothing more goes in this way — it would have to '
          + 'travel past what is already aboard. Carry this round to the back.' };
    }
    if (doorwayFree(st, 'back')) return doorwayState(st, spotId);
    return { kind: 'physical', label: 'Van full',
      why: 'Every position and both doorways are taken. Nothing left to put it in.' };
  }

  // A hand-picked position that is not the innermost one leaves a gap, and the
  // gap gets filled later by someone delivered earlier — who then sits deeper
  // than this stack. Worth saying out loud before it happens.
  if (targetIsChosen(st, spot.door, chosen, held.cust)) {
    var skipped = nextPosition(st, spot.door, held.cust);
    return { kind: 'chosen', target: target, label: 'Push in → ' + posLabel(target),
      why: 'You picked this one. ' + posLabel(skipped) + ' stays free — whoever fills it ends up deeper.' };
  }

  var want = expectedNext(st);
  // A same-depth pair reads as harmless — two customers in one row, in either
  // order, still satisfies depth-monotone. It is not harmless, because the
  // customer being skipped is by definition unfinished: their next crates go
  // one row further out, and this stack is then deeper than part of them.
  //
  //   OLA (stop 6) -> R1·L,  JAT (stop 5) -> R1·R,  OLA's rest -> R2·L
  //   and at stop 5 you reach past OLA at R2·L to get JAT at R1·R.
  //
  // So the guard stays sequence-strict. It is amber, not a block, and it names
  // the tap that clears it.
  if (want && want !== held.cust) {
    var at = spotHolding(st, want);
    var why;
    if (at) why = CUST[want].name + ' goes in first — they are on ' + at.name + '.';
    else if (isAboard(st, want)) why = 'Tap Done on ' + CUST[want].name + ' first, or push this anyway.';
    else why = CUST[want].name + ' has not been staged — push this and it lands in front of them.';
    return { kind: 'order', target: target, label: 'Push in anyway', why: why };
  }

  var w = held.n ? windowAt(st, target) : { lo: 0, hi: CAP };
  if (held.n > w.hi) {
    var plan = splitPlan(st, held.n, target);
    if (plan && plan.cells.length > 1) {
      return { kind: 'split', target: target, take: plan.heights[0], plan: plan,
        label: 'Push in ' + plan.heights[0] + ' of ' + held.n,
        why: 'All ' + held.n + ' will not stand at ' + posLabel(target) + ' — ' + w.hi
           + ' is its ceiling. Split it ' + plan.heights.join(' + ') + ': '
           + plan.cells.map(posLabel).join(', then ') + '.' };
    }
    return { kind: 'nofit', target: target, label: 'Will not ramp',
      why: held.n + ' cannot be made to step down from ' + posLabel(target)
         + ' — the column behind it has no room. Take some back off, or carry the rest round.' };
  }
  if (held.n && held.n < w.lo) {
    var tall = stabilityAt(st, target, held.n)[0];
    return { kind: 'thin', target: target, label: 'Push in anyway',
      why: 'Only ' + held.n + ' next to ' + posLabel(tall.neighbour) + '’s ' + tall.theirs
         + ' — ' + tall.apart + ' apart, and ' + w.lo + ' is the floor here.' };
  }
  return { kind: 'ready', target: target, label: 'Push in → ' + posLabel(target), why: '' };
}

// Standing a stack in the doorway itself. Always the driver's call and never
// the board's own idea while a numbered position is still free — it blocks the
// door it stands in, so what goes there has to be what comes out first.
function doorwayState(st, spotId) {
  var spot = spotById(spotId), held = st.staged[spotId];
  var first = firstPending(st), mine = stopOf(held.cust), why, good;
  if (held.n === 1 && spot.door === 'side') {
    why = 'One crate — the side door is the easy place to reach it from, and it keeps '
      + CUST[held.cust].name + ' off anybody else\u2019s stack.';
    good = true;
  } else if (!first || first === held.cust) {
    why = 'Comes out at stop ' + mine.i + ', before anything else — the doorway is the right place for it.';
    good = true;
  } else {
    why = CUST[held.cust].name + ' is stop ' + mine.i + '. In the '
      + (spot.door === 'side' ? 'side' : 'back') + ' doorway it is in the way at every stop before that.';
    good = false;
  }
  // The side well is not empty floor by default — the freeze ware goes in
  // there at the end, and it has to still fit.
  if (spot.door === 'side' && st.frozenAtDoor) why += ' The freeze ware shares this space at the end.';
  return { kind: 'doorway', target: doorwayOf(spot.door),
    label: held.n === 1 && spot.door === 'side' ? 'Put it at the side door' : 'Stand it in the doorway',
    why: why, good: good };
}

// Combining. Whoever is delivered EARLIER goes on top, so they come off without
// disturbing the one underneath.
function canStackOn(st, id, cust) {
  if (isEmpty(st, id)) return { ok: false, why: 'nothing there to stack on' };
  var below = st.van[id][st.van[id].length - 1].cust;
  if (stopOf(cust).i > stopOf(below).i) {
    return { ok: false, why: CUST[cust].name + ' is delivered after ' + CUST[below].name
      + ' — it goes underneath, not on top.' };
  }
  return { ok: true, below: below };
}
// Combining is a remedy, not a preference. Two customers on one stack is how
// the wrong goods get carried into a building, so a thin stack takes a position
// of its own by default and this only gets offered when something forces it:
// the ±3 rule, or genuinely running out of floor. hostReason() below decides
// which, and null means do not offer it at all.
//
// The host has to be legal three ways: the customer underneath is delivered
// later, the roof still clears, and the taller result does not break the host's
// own neighbours.
//
// Of the legal hosts, take the OUTERMOST row. Reaching deep to top up a stack
// is the awkward move, and burying a customer further forward than they need to
// be is the one mistake this rule exists to avoid. Ties go to the shorter stack
// so the column evens out.
var THIN = 3;
function hostReason(st, spotId) {
  var held = st.staged[spotId];
  if (!held || !held.n) return null;
  if (!stackHost(st, spotId)) return null;
  var spot = spotById(spotId);
  var target = nextPosition(st, spot.door);
  if (target && stabilityAt(st, target, held.n).length) return 'stability';
  if (spaceIsTight(st)) return 'space';
  return null;
}
// A customer with a single crate is better off at the side door than buried in
// a position of its own — easy to reach, and it keeps them off somebody else's
// stack. Offered, never forced.
function singleCrateDoor(st, spotId) {
  var held = st.staged[spotId];
  return !!(held && held.n === 1 && spotById(spotId).door === 'side' && doorwayFree(st, 'side'));
}
// How many customers a position is carrying. More than one is the thing worth
// seeing from across the van at delivery time.
function custCount(st, id) {
  var seen = [];
  (st.van[id] || []).forEach(function (l) { if (seen.indexOf(l.cust) < 0) seen.push(l.cust); });
  return seen.length;
}

function stackHost(st, spotId) {
  var spot = spotById(spotId), held = st.staged[spotId];
  if (!held || !held.n || held.n > THIN) return null;
  var best = null;
  zone(spot.door).forEach(function (id) {
    if (isEmpty(st, id)) return;
    if (!canStackOn(st, id, held.cust).ok) return;
    var h = heightOf(st, id);
    if (h == null || h + held.n > CAP) return;
    if (stabilityAt(st, id, h + held.n).length) return;
    var cand = { id: id, h: h, r: rowOf(id), below: st.van[id][st.van[id].length - 1].cust };
    if (!best || cand.r > best.r || (cand.r === best.r && cand.h < best.h)) best = cand;
  });
  return best;
}

// ── beginning a customer ─────────────────────────────────────────────────────
// The queue is the way in now: pick a stop, say which door you are packing it
// at, and that claims a packing spot. Everything after that — push in, on top,
// done — happens against the spot, so the door is chosen once rather than
// re-derived on every tap.
function freeSpotAt(st, door) {
  return SPOTS.filter(function (s) { return s.door === door && !st.staged[s.id]; })[0] || null;
}
// Amber is the default answer here. Almost nothing about starting a customer at
// a door is genuinely impossible — it is just sometimes a worse idea than the
// alternative, and the board's job is to say which and let the driver decide.
function beginState(st, cust, door) {
  if (st.closed[cust]) {
    return { kind: 'closed', label: 'Reopen', spot: null,
      why: CUST[cust].name + ' is closed out. Reopen it and their crates can go in again.' };
  }
  var mine = spotHolding(st, cust);
  if (mine && mine.door === door) {
    return { kind: 'packing', label: 'On ' + mine.name, spot: mine.id, why: '' };
  }
  if (mine) {
    // They are on the other door's spot. Tapping this one means carry it round,
    // which is a real thing the driver does the moment a door shuts on them.
    var landing = freeSpotAt(st, door);
    if (!landing) {
      return { kind: 'nospot', label: 'No spot', spot: mine.id,
        why: 'Every ' + door + ' spot is holding somebody, so there is nowhere to carry it to.' };
    }
    return { kind: 'move', label: 'Carry to ' + landing.name, spot: landing.id,
      why: 'Carry ' + CUST[cust].name + '’s stack from ' + mine.name + ' round to ' + landing.name + '.' };
  }
  var spot = freeSpotAt(st, door);
  if (!spot) {
    return { kind: 'nospot', label: 'No spot', spot: null,
      why: 'All ' + SPOTS.filter(function (s) { return s.door === door; }).length + ' '
        + door + ' spots are holding somebody. Finish one first.' };
  }
  if (door === 'side' && !sideDoorOpen(st)) {
    // Rows 1–4 are full, so nothing pushes in this way any more. The well itself
    // is still floor, and it is where a single crate wants to be — so this is a
    // warning about the door, not a refusal of the spot.
    if (!doorwayFree(st, 'side')) {
      return { kind: 'shut', label: 'Side shut', spot: null,
        why: 'Rows 1–' + SIDE_DOOR_ROWS + ' are full and the side well is taken. This one goes in the back.' };
    }
    return { kind: 'well', label: 'Side well', spot: spot.id,
      why: 'Rows 1–' + SIDE_DOOR_ROWS + ' are full, so nothing pushes in past them — but the side well '
        + 'still takes a stack, and one crate belongs there anyway.' };
  }
  var want = expectedNext(st);
  if (want && want !== cust) {
    return { kind: 'order', label: 'Start anyway', spot: spot.id,
      why: CUST[want].name + ' loads first — start this one and it lands in front of them.' };
  }
  return { kind: 'ready', label: 'Pack ' + (door === 'side' ? 'at the side' : 'at the back'), spot: spot.id, why: '' };
}
function doBegin(st, cust, door) {
  var mine = spotHolding(st, cust);
  if (mine && mine.door === door) return mine.id;
  if (mine) return doMoveSpot(st, mine.id, door);
  var spot = freeSpotAt(st, door);
  if (!spot) return null;
  doAssign(st, spot.id, cust);
  return spot.id;
}
// Carrying a part-built stack round to the other door. It is the answer when a
// door shuts mid-order — which happens to whoever is loading when rows 1–4 fill
// — and without it the board can say "round the back" and offer no way to do it.
function doMoveSpot(st, spotId, door) {
  var h = st.staged[spotId];
  if (!h) return null;
  var to = freeSpotAt(st, door);
  if (!to) return null;
  st.staged[to.id] = { cust: h.cust, n: h.n };
  st.staged[spotId] = null;
  // Whatever a split reserved on the old door is not where the rest is going now.
  Object.keys(st.held || {}).forEach(function (id) { if (st.held[id] === h.cust) delete st.held[id]; });
  return to.id;
}

// What a push should commit, when the driver has not counted.
//
// Only ever derived from something real: the stack this one has to ramp off in
// its own column, clamped into the window. With nothing behind it there is
// nothing to infer from, and the honest answer is null — the push goes in
// uncounted and says so, rather than the board inventing a number that the van
// diagram will then repeat back as fact.
function suggestAt(st, id) {
  if (isDoor(id)) return null;
  var r = rowOf(id), col = colOf(id);
  var prev = r > 1 ? 'r' + (r - 1) + '-' + col : null;
  if (!prev || isEmpty(st, prev)) return null;
  var base = heightOf(st, prev);
  if (base == null) return null;
  var w = windowAt(st, id);
  return Math.max(1, Math.min(CAP, w.hi, Math.max(w.lo, base)));
}

// ── one crate on top of somebody else's stack ────────────────────────────────
// The small-order move, and the one the methodology calls a technique rather
// than a fallback: a stop with two crates does not need a position of its own
// next to a stack of six. It does need the right host — delivered later, so it
// stays underneath — and the outermost one, so nobody is buried deeper than
// they have to be.
function stackHosts(st, spotId, n) {
  var spot = spotById(spotId), held = st.staged[spotId];
  if (!spot || !held) return [];
  var take = n == null ? held.n : n;
  if (!take) return [];
  var out = [];
  zone(spot.door).forEach(function (id) {
    if (isEmpty(st, id)) return;
    if (!canStackOn(st, id, held.cust).ok) return;
    var h = heightOf(st, id);
    if (h == null || h + take > CAP) return;
    if (stabilityAt(st, id, h + take).length) return;
    out.push({ id: id, h: h, r: rowOf(id), below: st.van[id][st.van[id].length - 1].cust });
  });
  // Outermost first, and among equals the shorter stack, so the column evens out.
  out.sort(function (a, b) { return b.r - a.r || a.h - b.h; });
  return out;
}
function topUpState(st, spotId, chosen, n) {
  var held = st.staged[spotId];
  if (!held) return { kind: 'empty', label: '—', why: '' };
  var take = n == null ? (held.n || 1) : n;
  var hosts = stackHosts(st, spotId, take);
  if (!hosts.length) {
    // Say which of the three reasons it is, because they call for different moves.
    var any = zone(spotById(spotId).door).filter(function (id) { return !isEmpty(st, id); });
    var why = !any.length ? 'Nothing is aboard on this side yet — there is no stack to put it on.'
      : (any.every(function (id) { return !canStackOn(st, id, held.cust).ok; })
          ? CUST[held.cust].name + ' is delivered before everything aboard on this side, so they would '
            + 'have to go underneath. Give them their own position.'
          : 'Every stack it could go on is either at the roof or would break the ±3 ramp.');
    return { kind: 'nohost', label: 'No stack for it', why: why };
  }
  var host = (chosen && hosts.filter(function (h) { return h.id === chosen; })[0]) || hosts[0];
  var picked = !!(chosen && host.id === chosen && hosts[0].id !== chosen);
  return {
    kind: picked ? 'chosen' : 'ready', host: host, take: take,
    label: '+' + take + ' on top', target: host.id,
    why: 'On ' + CUST[host.below].name + '’s ' + host.h + ' at ' + posLabel(host.id)
      + ' — they are delivered later, so this comes off first.'
      + (picked ? ' You picked this one.' : '')
  };
}

// ── actions ──────────────────────────────────────────────────────────────────
function emptyState() {
  var van = {}; ALL_POS.forEach(function (id) { van[id] = []; });
  var staged = {}; SPOTS.forEach(function (s) { staged[s.id] = null; });
  return { van: van, staged: staged, closed: {}, flags: 0, frozenAtDoor: true, held: {} };
}
function cloneState(st) {
  var van = {}; ALL_POS.forEach(function (id) { van[id] = (st.van[id] || []).map(function (l) { return { cust: l.cust, n: l.n }; }); });
  var staged = {}; SPOTS.forEach(function (s) { staged[s.id] = st.staged[s.id] ? { cust: st.staged[s.id].cust, n: st.staged[s.id].n } : null; });
  var closed = {}; Object.keys(st.closed).forEach(function (k) { closed[k] = true; });
  var held = {}; Object.keys(st.held || {}).forEach(function (k) { held[k] = st.held[k]; });
  return { van: van, staged: staged, closed: closed, flags: st.flags || 0,
           frozenAtDoor: st.frozenAtDoor !== false, held: held };
}
// Change the van under a session in progress and the board has to keep what
// still fits: positions that survived the reshape hold their stacks, ones that
// no longer exist give theirs back to the spot they came from is a lie we will
// not tell — they are dropped, and the count says so.
function normalize(st) {
  var kept = {};
  ALL_POS.forEach(function (id) { kept[id] = st.van[id] || []; });
  st.van = kept;
  var staged = {};
  SPOTS.forEach(function (s) { staged[s.id] = st.staged[s.id] || null; });
  st.staged = staged;
  return st;
}

// Done is an assertion, not a fact — the rest of that customer's crates can
// still surface two pallets later. Reopening has to be one tap, not a rewind
// through everything done since.
function doReopen(st, cust) { delete st.closed[cust]; }
function doAssign(st, spotId, cust) { st.staged[spotId] = { cust: cust, n: 0 }; }
function doBump(st, spotId, d) {
  var h = st.staged[spotId]; if (!h) return;
  h.n = Math.max(0, (h.n || 0) + d);
}
function doPush(st, spotId, take, chosen, spill) {
  var spot = spotById(spotId), h = st.staged[spotId];
  if (!h) return null;
  var target = resolveTarget(st, spot.door, chosen, h.cust); if (!target) return null;
  var n = take == null ? h.n : take;
  st.van[target].push({ cust: h.cust, n: n === 0 ? null : n });
  st.held = st.held || {};
  if (st.held[target] === h.cust) delete st.held[target];      // the hold was just used
  if (take != null && take < h.n) {
    h.n = h.n - take;
    if (spill) st.held[spill] = h.cust;                        // the rest goes here, and nowhere else
  } else {
    st.staged[spotId] = { cust: h.cust, n: 0 };
  }
  return target;
}
function positionsHeld(st) {
  return Object.keys(st.held || {}).filter(function (id) { return isEmpty(st, id); }).length;
}
function doDoorway(st, spotId) {
  var spot = spotById(spotId), h = st.staged[spotId];
  if (!h) return null;
  var id = doorwayOf(spot.door);
  st.van[id].push({ cust: h.cust, n: h.n === 0 ? null : h.n });
  st.staged[spotId] = { cust: h.cust, n: 0 };
  return id;
}
function doClearDoorway(st, door) { st.van[doorwayOf(door)] = []; }

function doStack(st, spotId, id, take) {
  var h = st.staged[spotId]; if (!h) return null;
  var n = take == null ? h.n : Math.min(take, h.n || take);
  st.van[id].push({ cust: h.cust, n: n === 0 ? null : n });
  // A top-up moves part of the pile; the rest stays staged for the next move.
  if (take != null && h.n && take < h.n) h.n = h.n - take;
  else st.staged[spotId] = { cust: h.cust, n: 0 };
  return id;
}
function doClose(st, spotId) {
  var h = st.staged[spotId]; if (!h) return;
  st.closed[h.cust] = true; st.staged[spotId] = null;
  // A closed customer holds no floor.
  Object.keys(st.held || {}).forEach(function (id) { if (st.held[id] === h.cust) delete st.held[id]; });
}
