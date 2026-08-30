// ── the demo state ───────────────────────────────────────────────────────────
// Mid-load on purpose: Olavstoppen and Jåtten are aboard and closed out, three
// rows deep, with the side door still open on row 4. Hinna is built on one side
// spot and Sverdrup on the next — which is the moment the push, the top-up and
// the loading-order guard all have something to say at once.
function seed() {
  var st = emptyState();
  st.van['r1-left']  = [{ cust: 'OLA', n: 3 }];
  st.van['r1-right'] = [{ cust: 'OLA', n: 3 }];
  st.van['r2-left']  = [{ cust: 'OLA', n: 2 }];
  st.van['r2-right'] = [{ cust: 'OLA', n: 2 }];
  st.van['r3-left']  = [{ cust: 'JAT', n: 3 }];
  st.van['r3-right'] = [{ cust: 'JAT', n: 2 }];
  st.closed['OLA'] = true;
  st.closed['JAT'] = true;
  st.staged['side-1'] = { cust: 'HIN', n: 2 };
  st.staged['side-2'] = { cust: 'SVE', n: 4 };
  return st;
}

// ── the projection ───────────────────────────────────────────────────────────
// The camera stands at the van's rear-right corner, raised. Both +u (toward the
// right wall) and +v (toward the back doors) come toward it, so the cab-left
// corner is the top of the picture and the corner you are standing at is the
// bottom. That is what puts the left column on the left, the right column on
// the right, and the side door — which only ever serves the deep rows — at the
// far end on the right, with its packing spots outside it.
//
// It is dimetric rather than true isometric, and it has to be: at 30° on both
// axes, nine rows of van is 950px wide and 850 tall before a crate goes in.
// Two axes at different angles fit it, and the eye reads the picture the same.
var VIEW = {
  cx: 120, cy: 50,      // one column, left wall toward right wall
  rx: 42,  ry: 48,      // one row, cab toward back doors
  ch: 11,               // one crate of height
  wall: 8.4,            // how far up the far wall and the bulkhead go
  gutter: 0.62,         // where the row numbers sit, outside the left wall
  padU: 2.34, padW: 1.24, padD: 1.06, padV: 0.30, padPitch: 1.22,  // the side spots
  backW: 1.16, backD: 1.1, backV: 0.5, backPitch: 1.3           // the back spots
};
// The box the whole picture is fitted into. Everything scales to it, so a van
// with seven rows and one that has nine both fill the same frame.
var SCENE = { x: 16, y: 86, w: 1014, h: 596 };
var STAGE_CAP = 8;       // a staged pile draws at true height — it is about to be one

function shade(hex, f, a) {
  var n = parseInt(String(hex).slice(1), 16);
  var c = [n >> 16, (n >> 8) & 255, n & 255]
    .map(function (v) { return Math.round(Math.min(255, v * f)); });
  return a == null ? 'rgb(' + c.join(',') + ')' : 'rgba(' + c.join(',') + ',' + a + ')';
}
function noop() {}
function fx(n) { return (Math.round(n * 1000) / 1000); }

class Component extends DCLogic {
  constructor(props) {
    super(props);
    // focus  — the packing spot the console is driving
    // target — a hand-picked van position for the next push in
    // host   — a hand-picked stack for the next top-up
    this.state = { st: seed(), focus: 'side-1', target: null, host: null, hist: [] };
  }

  // Every mutation goes through here so Undo is a single honest rule: put the
  // whole board back the way it was one tap ago.
  apply(fn) {
    var before = cloneState(this.state.st);
    var next = cloneState(this.state.st);
    fn(next);
    var hist = this.state.hist.slice();
    hist.push(before);
    if (hist.length > 40) hist.shift();
    this.setState({ st: next, hist: hist });
  }
  undo() {
    var hist = this.state.hist.slice();
    if (!hist.length) return;
    this.setState({ st: hist.pop(), hist: hist, target: null, host: null });
  }

  focusSpot(st) {
    var id = this.state.focus;
    if (id && st.staged[id]) return id;
    // A spot that lost its customer stops driving the console; the next one
    // holding something takes over rather than leaving it blank.
    var live = SPOTS.filter(function (s) { return st.staged[s.id]; })[0];
    return live ? live.id : null;
  }

  // Tapping the van picks a position by hand: an empty one this door can still
  // reach becomes the push target, a stack that could legally take crates
  // becomes the top-up host, and anything else clears both.
  pickCell(st, id) {
    var focus = this.focusSpot(st);
    if (!focus) return;
    var door = spotById(focus).door;
    if (isEmpty(st, id)) {
      if (doorOf(id) !== door) return this.setState({ target: null, host: null });
      return this.setState({ target: this.state.target === id ? null : id, host: null });
    }
    var hosts = stackHosts(st, focus, this.topTake(st, focus)).map(function (h) { return h.id; });
    if (hosts.indexOf(id) < 0) return this.setState({ target: null, host: null });
    this.setState({ host: this.state.host === id ? null : id, target: null });
  }

  // A small order goes on top whole; a big one sheds a single crate. Either way
  // the button says the number before it is tapped.
  topTake(st, spotId) {
    var held = st.staged[spotId];
    if (!held) return 0;
    if (!held.n) return 1;
    return held.n <= THIN ? held.n : 1;
  }

  btn(kind, h, accent) {
    var base = 'height:' + h + 'px;border-radius:13px;display:flex;flex-direction:column;align-items:center;'
      + 'justify-content:center;gap:2px;flex:none;padding:0 12px;text-align:center;';
    if (kind === 'go')      return base + 'background:#4FD6A8;color:#0B2119;';
    if (kind === 'primary') return base + 'background:' + accent + ';color:#191624;';
    if (kind === 'warn')    return base + 'background:rgba(255,181,112,.12);border:1px solid rgba(255,181,112,.45);color:#FFB570;';
    if (kind === 'stop')    return base + 'background:rgba(247,118,142,.10);border:1px solid rgba(247,118,142,.42);color:#F7768E;';
    if (kind === 'quiet')   return base + 'background:rgba(242,238,248,.05);border:1px solid #2A2438;color:#CDC6DD;';
    return base + 'background:rgba(242,238,248,.025);color:#3F3A52;';
  }

  // ── the picture ────────────────────────────────────────────────────────────
  scene(st, accent, plan) {
    var self = this, V = VIEW, parts = [];
    var sideSpots = SPOTS.filter(function (s) { return s.door === 'side'; });
    var backSpots = SPOTS.filter(function (s) { return s.door === 'back'; });
    var top = Math.max(CAP, V.wall);

    // Fit. The projection is linear in the scale, so the picture's bounding box
    // is too — one division rather than a search.
    var pts = [[-V.gutter, 0, 0], [-V.gutter, ROWS, 0], [0, 0, top], [2, 0, top],
               [0, ROWS, 0], [2, ROWS, 0], [2, 0, 0], [0, 0, 0]];
    sideSpots.forEach(function (s, i) {
      var v0 = V.padV + i * V.padPitch;
      pts.push([V.padU, v0, 0], [V.padU + V.padW, v0, STAGE_CAP],
               [V.padU + V.padW + 0.62, v0 + V.padD, 0], [V.padU, v0 + V.padD, 0]);
    });
    backSpots.forEach(function (s, i) {
      var u0 = 0.06 + i * V.backPitch, v0 = ROWS + V.backV;
      pts.push([u0, v0, 0], [u0 + V.backW, v0, STAGE_CAP],
               [u0 + V.backW, v0 + V.backD + 0.42, 0], [u0, v0 + V.backD + 0.42, 0]);
    });
    var xs = pts.map(function (p) { return p[0] * V.cx - p[1] * V.rx; });
    var ys = pts.map(function (p) { return p[0] * V.cy + p[1] * V.ry - p[2] * V.ch; });
    var minX = Math.min.apply(null, xs), maxX = Math.max.apply(null, xs);
    var minY = Math.min.apply(null, ys), maxY = Math.max.apply(null, ys);
    var k = Math.min(1, SCENE.w / (maxX - minX), SCENE.h / (maxY - minY));
    var cx = V.cx * k, cy = V.cy * k, rx = V.rx * k, ry = V.ry * k, ch = V.ch * k;
    var ox = SCENE.x - minX * k + (SCENE.w - (maxX - minX) * k) / 2;
    var oy = SCENE.y - minY * k + (SCENE.h - (maxY - minY) * k) / 2;
    function P(u, v, w) { return [u * cx - v * rx, u * cy + v * ry - (w || 0) * ch]; }
    var COL = [cx, cy], ROW = [-rx, ry];
    function mul(a, n) { return [a[0] * n, a[1] * n]; }

    // A parallelogram spanned by two screen vectors. Sheared, so nothing with
    // words in it is ever drawn this way.
    function quad(o, a, b, style, tap) {
      parts.push({ kids: [], text: '',
        tap: tap || noop,
        style: 'position:absolute;left:0;top:0;width:100px;height:100px;transform-origin:0 0;transform:matrix('
          + [fx(a[0] / 100), fx(a[1] / 100), fx(b[0] / 100), fx(b[1] / 100), fx(o[0]), fx(o[1])].join(',')
          + ');' + style });
    }
    // Upright, centred on a projected point. Everything readable is one of these.
    function chip(p, text, style, tap) {
      parts.push({ kids: [], text: text, tap: tap || noop,
        style: 'position:absolute;left:' + fx(p[0]) + 'px;top:' + fx(p[1]) + 'px;'
          + 'transform:translate(-50%,-50%);white-space:nowrap;pointer-events:none;' + style });
    }
    var mono = "font-family:'IBM Plex Mono',monospace;";

    // ── the shell ────────────────────────────────────────────────────────────
    // Only the two walls that stand behind the load are drawn. The right wall
    // is between the camera and everything it holds, so it is cut down to a
    // sill — and the side door is the stretch of that sill it opens through.
    quad(P(0, 0), mul(ROW, ROWS), [0, -V.wall * ch],
      'background:linear-gradient(to bottom,#1B1626,#0E0B15);box-shadow:inset 0 0 0 1px #342D48;');
    quad(P(0, 0), mul(COL, 2), [0, -V.wall * ch],
      'background:linear-gradient(to bottom,#221C31,#14101F);box-shadow:inset 0 0 0 1px #3A3252;');
    quad(P(0, 0, V.wall), mul(ROW, ROWS), [0, 4], 'background:#584F76;');
    quad(P(0, 0, V.wall), mul(COL, 2), [0, 4], 'background:#6B6188;');
    chip(P(1, 0.16, V.wall * 0.52), 'CAB', mono + 'font-size:' + fx(11 * k + 1) + 'px;letter-spacing:.16em;color:#6B6386;');
    quad(P(0, 0), mul(COL, 2), mul(ROW, ROWS), 'background:#131020;box-shadow:inset 0 0 0 1px #3A3252;');

    // ── the state the picture has to answer ──────────────────────────────────
    var focus = this.focusSpot(st), fspot = focus ? spotById(focus) : null;
    var held = focus ? st.staged[focus] : null;
    var door = fspot ? fspot.door : (sideDoorOpen(st) ? 'side' : 'back');
    var frontier = held ? resolveTarget(st, door, this.state.target, held.cust) : null;
    var picked = held && targetIsChosen(st, door, this.state.target, held.cust);
    var hosts = held ? stackHosts(st, focus, this.topTake(st, focus)) : [];
    var hostIds = hosts.map(function (h) { return h.id; });
    var hostNow = (this.state.host && hostIds.indexOf(this.state.host) > -1) ? this.state.host
      : (hosts[0] ? hosts[0].id : null);
    var hostTake = held ? this.topTake(st, focus) : 0;
    var shut = !sideDoorOpen(st);

    // the sill, broken where the door opens
    for (var r = 0; r < ROWS; r++) {
      var isDoorRow = r < SIDE_DOOR_ROWS;
      quad(P(2, r), ROW, [0, -(isDoorRow ? 0.55 : 1.9) * ch],
        isDoorRow ? (shut ? 'background:rgba(247,118,142,.16);box-shadow:inset 0 0 0 1px rgba(247,118,142,.5);'
                          : 'background:rgba(255,181,112,.20);box-shadow:inset 0 0 0 1px rgba(255,181,112,.62);')
                  : 'background:#171320;box-shadow:inset 0 0 0 1px #2A2438;');
    }
    if (SIDE_DOOR_ROWS > 0) {
      chip(P(2.02, 0.06, 1.1),
        shut ? 'SIDE DOOR · SHUT' : 'SIDE DOOR · ROWS 1–' + SIDE_DOOR_ROWS,
        mono + 'font-size:' + fx(10 * k + 1) + 'px;font-weight:600;letter-spacing:.12em;transform:translate(1%,-50%);'
          + 'color:' + (shut ? '#F7768E' : '#FFB570') + ';');
    }
    for (var rr = 0; rr < ROWS; rr++) {
      chip(P(-V.gutter, rr + 0.5), 'R' + (rr + 1),
        mono + 'font-size:' + fx(11 * k + 1) + 'px;color:' + (rr < SIDE_DOOR_ROWS && !shut ? '#7A6E58' : '#57506E') + ';');
    }

    // ── the load, back to front ──────────────────────────────────────────────
    // Emission order is depth order: a nearer stack is drawn later and paints
    // over what it stands in front of, which is what it does in the van.
    function stripes(layers, f) {
      return layers.slice().reverse().map(function (l) {
        return { style: 'flex:' + (l.n || 1) + ' 0 0;min-height:0;background:' + shade(CUST[l.cust].color, f) + ';'
          + 'background-image:repeating-linear-gradient(to bottom,transparent 0,transparent ' + fx(ch - 1)
          + 'px,rgba(0,0,0,.42) ' + fx(ch - 1) + 'px,rgba(0,0,0,.42) ' + fx(ch) + 'px);'
          + 'box-shadow:inset 0 -1px 0 rgba(0,0,0,.5);' };
      });
    }
    // Three faces per stack, not per crate: the one facing the back doors, the
    // one facing the right wall, and the top. The crate lines are a gradient
    // inside each customer's band, so an eight-high stack is still four divs.
    function drawStack(u, v, layers, n, tap) {
      var h = n * ch;
      var o1 = P(u, v + 1, n);
      parts.push({ kids: stripes(layers, 0.70), text: '', tap: tap || noop,
        style: 'position:absolute;left:0;top:0;width:' + fx(cx) + 'px;height:' + fx(h) + 'px;transform-origin:0 0;'
          + 'transform:matrix(1,' + fx(cy / cx) + ',0,1,' + fx(o1[0]) + ',' + fx(o1[1]) + ');display:flex;flex-direction:column;' });
      var o2 = P(u + 1, v, n);
      parts.push({ kids: stripes(layers, 0.46), text: '', tap: tap || noop,
        style: 'position:absolute;left:0;top:0;width:' + fx(rx) + 'px;height:' + fx(h) + 'px;transform-origin:0 0;'
          + 'transform:matrix(-1,' + fx(ry / rx) + ',0,1,' + fx(o2[0]) + ',' + fx(o2[1]) + ');display:flex;flex-direction:column;' });
      quad(P(u, v, n), COL, ROW, 'background:' + shade(CUST[layers[layers.length - 1].cust].color, 1)
        + ';box-shadow:inset 0 0 0 1px rgba(0,0,0,.38);', tap);
    }

    for (var row = 0; row < ROWS; row++) {
      for (var col = 0; col < 2; col++) {
        (function (row, col) {
          var id = 'r' + (row + 1) + '-' + (col ? 'right' : 'left');
          var layers = st.van[id], n = heightOf(st, id), unknown = n == null && layers.length;
          var isNext = id === frontier, isHost = id === hostNow;
          var reach = doorOf(id) === 'side' && shut && !layers.length;
          var tap = function () { self.pickCell(self.state.st, id); };
          var ghost = (!layers.length && plan && plan.van[id] && plan.van[id].length) ? plan.van[id] : null;

          quad(P(col, row), COL, ROW,
            (isNext ? 'background:' + accent + '2E;box-shadow:inset 0 0 0 2px ' + (picked ? '#FFB570' : accent) + ';'
              : (layers.length ? 'background:rgba(0,0,0,.28);box-shadow:inset 0 0 0 1px rgba(203,176,255,.10);'
                : (reach ? 'background:rgba(247,118,142,.05);box-shadow:inset 0 0 0 1px rgba(247,118,142,.16);'
                  : 'background:rgba(203,176,255,.022);box-shadow:inset 0 0 0 1px rgba(203,176,255,.13);')))
            + 'cursor:pointer;', tap);

          var gn = ghost ? ghost.reduce(function (a, l) { return a + l.n; }, 0) : 0;
          if (ghost) {
            // Drawn as a volume, not a floating rectangle: a face down to the
            // floor is what stops a planned stack reading as a lid hanging in
            // mid-air over the position two rows behind it.
            var oF = P(col, row + 1, gn);
            parts.push({ kids: [], text: '', tap: tap,
              style: 'position:absolute;left:0;top:0;width:' + fx(cx) + 'px;height:' + fx(gn * ch)
                + 'px;transform-origin:0 0;transform:matrix(1,' + fx(cy / cx) + ',0,1,' + fx(oF[0]) + ',' + fx(oF[1])
                + ');background:rgba(122,162,247,.07);border-left:1px dashed rgba(122,162,247,.4);'
                + 'border-right:1px dashed rgba(122,162,247,.4);' });
            quad(P(col, row, gn), COL, ROW,
              'background:rgba(122,162,247,.10);box-shadow:inset 0 0 0 1px rgba(122,162,247,.55);', tap);
          }
          if (layers.length) {
            var draw = unknown ? [{ cust: layers[0].cust, n: 1 }] : layers;
            drawStack(col, row, draw, unknown ? 1 : n, tap);
            var names = [];
            layers.forEach(function (l) { if (names.indexOf(l.cust) < 0) names.push(l.cust); });
            chip(P(col + 0.5, row + 0.26, unknown ? 1 : n),
              names.map(function (c) { return CUST[c].code; }).join('+') + ' ' + (unknown ? '?' : n),
              'font-family:Archivo,system-ui,sans-serif;font-weight:700;font-size:' + fx(12 * k + 1) + 'px;'
              + 'color:#0B0910;text-shadow:0 1px 0 rgba(255,255,255,.28);');
            if (isHost && hostTake) {
              // What the top-up would do, drawn where it would land: the crates
              // themselves, in the customer's colour, standing on the host.
              var base = unknown ? 1 : n, oG = P(col, row + 1, base + hostTake);
              parts.push({ kids: [], text: '', tap: tap,
                style: 'position:absolute;left:0;top:0;width:' + fx(cx) + 'px;height:' + fx(hostTake * ch)
                  + 'px;transform-origin:0 0;transform:matrix(1,' + fx(cy / cx) + ',0,1,' + fx(oG[0]) + ',' + fx(oG[1])
                  + ');background:' + shade(CUST[held.cust].color, 0.7, 0.5) + ';border:1px dashed #FFB570;' });
              quad(P(col, row, base + hostTake), COL, ROW,
                'background:' + shade(CUST[held.cust].color, 1, 0.55) + ';box-shadow:inset 0 0 0 2px #FFB570;', tap);
              chip(P(col + 0.5, row + 0.24, base + hostTake), '+' + hostTake,
                'font-family:Archivo,system-ui,sans-serif;font-weight:800;font-size:' + fx(12 * k + 1) + 'px;color:#FFF2E2;');
            }
          } else if (isNext || ghost) {
            // One chip per position. The next position with a plan on it has two
            // things to say and they are the same sentence, not two labels
            // stacked on top of each other.
            var planned = ghost ? ghost.map(function (l) { return CUST[l.cust].code; }).join('+') + ' ' + gn : '';
            chip(P(col + 0.5, row + (ghost ? 0.26 : 0.5), gn),
              isNext ? (picked ? 'PICKED' : 'NEXT') + (planned ? ' · ' + planned : ' IN') : planned,
              (ghost && !isNext ? "font-family:Archivo,system-ui,sans-serif;font-weight:700;" : mono + 'font-weight:600;letter-spacing:.09em;')
              + 'font-size:' + fx((ghost && !isNext ? 11 : 10) * k + 1) + 'px;'
              + 'color:' + (picked ? '#FFB570' : (isNext ? '#CBB0FF' : '#7AA2F7')) + ';');
          }
        }(row, col));
      }
    }

    // ── the ground outside ───────────────────────────────────────────────────
    // Pavement, not van floor — so the pads are drawn flat and unwalled, and the
    // pile you have built on one stands on it exactly the way it will stand in
    // the van a moment later.
    function drawPad(spot, u0, v0, w, d, outward) {
      var on = st.staged[spot.id], isFocus = spot.id === focus;
      var col = on ? CUST[on.cust].color : '#5F5876';
      quad(P(u0, v0), mul(COL, w), mul(ROW, d),
        'background:' + (on ? 'rgba(242,238,248,.05)' : 'rgba(242,238,248,.022)')
        + ';box-shadow:inset 0 0 0 ' + (isFocus ? '2px ' + accent : '1px ' + (on ? col + '66' : '#2A2438')) + ';cursor:pointer;',
        function () { self.setState({ focus: spot.id, target: null, host: null }); });
      if (on && on.n) {
        // Inset, so the pad stays readable as ground with a pile standing on it.
        var m = 0.1, iu = u0 + m, iv = v0 + m, iw = w - 2 * m, id = d - 2 * m;
        var n = Math.min(on.n, STAGE_CAP);
        var h = n * ch, oA = P(iu, iv + id, n), oB = P(iu + iw, iv, n);
        parts.push({ kids: stripes([{ cust: on.cust, n: n }], 0.70), text: '', tap: noop,
          style: 'position:absolute;left:0;top:0;width:' + fx(cx * iw) + 'px;height:' + fx(h) + 'px;transform-origin:0 0;'
            + 'transform:matrix(1,' + fx(cy / cx) + ',0,1,' + fx(oA[0]) + ',' + fx(oA[1]) + ');display:flex;flex-direction:column;' });
        parts.push({ kids: stripes([{ cust: on.cust, n: n }], 0.46), text: '', tap: noop,
          style: 'position:absolute;left:0;top:0;width:' + fx(rx * id) + 'px;height:' + fx(h) + 'px;transform-origin:0 0;'
            + 'transform:matrix(-1,' + fx(ry / rx) + ',0,1,' + fx(oB[0]) + ',' + fx(oB[1]) + ');display:flex;flex-direction:column;' });
        quad(P(iu, iv, n), mul(COL, iw), mul(ROW, id),
          'background:' + shade(col, 1) + ';box-shadow:inset 0 0 0 1px rgba(0,0,0,.38);');
        chip(P(u0 + w / 2, v0 + d / 2, n), CUST[on.cust].code + ' ' + on.n,
          'font-family:Archivo,system-ui,sans-serif;font-weight:800;font-size:' + fx(13 * k + 1) + 'px;'
          + 'letter-spacing:-.01em;color:#0B0910;text-shadow:0 1px 0 rgba(255,255,255,.26);');
      } else if (on) {
        chip(P(u0 + w / 2, v0 + d / 2), CUST[on.cust].code + ' ·',
          'font-family:Archivo,system-ui,sans-serif;font-weight:800;font-size:' + fx(13 * k + 1) + 'px;color:' + col + ';');
      } else {
        chip(P(u0 + w / 2, v0 + d / 2), 'free',
          "font-family:'Space Grotesk',sans-serif;font-size:" + fx(11 * k + 1) + 'px;color:#3F3A52;');
      }
      // The spot's own name always sits on the pavement in front of it, so a
      // pile standing on the pad never hides which pad it is.
      chip(outward ? P(u0 + w + 0.26, v0 + d / 2) : P(u0 + w / 2, v0 + d + 0.28), spot.name,
        mono + 'font-size:' + fx(10 * k + 1) + 'px;font-weight:600;letter-spacing:.09em;'
        + 'color:' + (isFocus ? '#CBB0FF' : (on ? '#8D87A0' : '#4A445C')) + ';');
    }
    sideSpots.forEach(function (s, i) { drawPad(s, V.padU, V.padV + i * V.padPitch, V.padW, V.padD, true); });
    backSpots.forEach(function (s, i) { drawPad(s, 0.06 + i * V.backPitch, ROWS + V.backV, V.backW, V.backD, false); });

    // ── the doorways ─────────────────────────────────────────────────────────
    // Off the grid and off the floor plan, so drawn on the threshold itself —
    // and only once something stands there or the floor has run short enough
    // that it is about to.
    DOORS.forEach(function (id) {
      var stack = st.van[id], on = stack.length ? stack[stack.length - 1] : null;
      if (!on && !spaceIsTight(st)) return;
      var isSide = doorOf(id) === 'side';
      var u0 = isSide ? 2.02 : 0.4, v0 = isSide ? SIDE_DOOR_ROWS - 1.0 : ROWS - 0.02;
      var w = isSide ? 0.42 : 1.2, d = isSide ? 1.0 : 0.4;
      quad(P(u0, v0), mul(COL, w), mul(ROW, d),
        'background:' + (on ? 'rgba(255,181,112,.16)' : 'rgba(255,181,112,.05)')
        + ';box-shadow:inset 0 0 0 1px rgba(255,181,112,' + (on ? '.55' : '.24') + ');');
      chip(P(u0 + w / 2, v0 + d / 2, on ? 1.2 : 0),
        on ? CUST[on.cust].code + (on.n == null ? ' ?' : ' ' + on.n) : 'well',
        mono + 'font-size:' + fx(10 * k + 1) + 'px;font-weight:600;letter-spacing:.06em;color:#FFB570;');
    });

    return { box: 'position:absolute;left:' + fx(ox) + 'px;top:' + fx(oy) + 'px;width:0;height:0;', parts: parts,
             focus: focus, held: held, door: door, frontier: frontier, picked: picked,
             hosts: hosts, hostNow: hostNow, shut: shut };
  }

  // ── the console ────────────────────────────────────────────────────────────
  consoleVals(st, accent, plan, S) {
    var self = this, focus = S.focus, held = S.held;
    var box = 'position:absolute;left:' + SCENE.x + 'px;top:698px;width:1014px;display:flex;flex-direction:column;gap:7px;';
    var off = this.btn('off', 76, accent), quiet = this.btn('quiet', 76, accent);
    var big = 'font:700 19px/1 Archivo,system-ui,sans-serif;letter-spacing:-.02em;';
    var sub = "font:500 11px/1 'IBM Plex Mono',monospace;letter-spacing:.06em;opacity:.74;";
    var dead = {
      box: box, eyebrow: '', eyebrowStyle: '', note: '', noteStyle: '',
      minus: noop, minusStyle: off + 'width:0;padding:0;border:0;',
      plus: noop, plusStyle: off + 'width:0;padding:0;border:0;', plusLabel: '', plusNote: '', plusBig: big, plusSub: sub,
      push: noop, pushStyle: off + 'width:0;padding:0;border:0;', pushLabel: '', pushNote: '', pushBig: big, pushSub: sub,
      hasTop: false, top: noop, topStyle: off + 'width:0;padding:0;border:0;', topLabel: '', topNote: '', topBig: big, topSub: sub,
      done: noop, doneStyle: off + 'width:0;padding:0;border:0;', doneLabel: '', doneBig: big,
      showWhy: false, why: '', whyStyle: ''
    };

    if (!focus) {
      var waiting = expectedNext(st);
      return Object.assign({}, dead, {
        eyebrow: waiting ? 'NOTHING ON A PACKING SPOT' : 'EVERY STOP CLOSED OUT',
        eyebrowStyle: "font:600 11px/1 'IBM Plex Mono',monospace;letter-spacing:.10em;color:"
          + (waiting ? '#8D87A0' : '#4FD6A8') + ';',
        note: waiting ? 'Pick ' + CUST[waiting].name + ' on the right and say which door you are packing them at.'
          : cratesIn(st) + ' crates aboard. Nothing left to load.',
        noteStyle: "font:400 14px/1.3 'Space Grotesk',sans-serif;color:#8D87A0;",
        done: function () { self.undo(); },
        doneStyle: this.btn(this.state.hist.length ? 'quiet' : 'off', 76, accent) + 'width:120px;',
        doneLabel: 'Undo', doneBig: big
      });
    }

    var spot = spotById(focus);
    var ps = pushState(st, focus, this.state.target);
    var take = this.topTake(st, focus);
    var tu = topUpState(st, focus, this.state.host, take);
    // Combining is a remedy, never a preference: two customers on one stack is
    // how the wrong goods get carried into a building. It only goes amber when
    // the ±3 rule or the floor has taken the alternative away.
    var forced = hostReason(st, focus);
    var suggest = ps.target ? suggestAt(st, ps.target) : null;

    var kind = 'go', label = ps.label, act;
    var note = ps.target ? posLabel(ps.target) + (held.n ? '' : ' · not counted') : '';
    var chosen = this.state.target;
    act = function () {
      self.apply(function (s) {
        if (ps.kind === 'doorway') doDoorway(s, focus);
        else if (ps.kind === 'split') doPush(s, focus, ps.take, chosen, ps.plan ? ps.plan.cells[1] : null);
        else doPush(s, focus, null, chosen);
      });
      self.setState({ target: null, host: null });
    };
    if (ps.kind === 'ready')         { kind = held.n ? 'go' : 'quiet'; label = 'Push in' + (held.n ? ' ' + held.n : ''); }
    else if (ps.kind === 'chosen')   { kind = 'warn'; label = 'Push in' + (held.n ? ' ' + held.n : ''); }
    else if (ps.kind === 'split')    { kind = 'warn'; label = 'Push in ' + ps.take + ' of ' + held.n; }
    else if (ps.kind === 'order')    { kind = 'warn'; label = 'Push in anyway'; }
    else if (ps.kind === 'thin')     { kind = 'warn'; label = 'Push in anyway'; }
    else if (ps.kind === 'doorway')  { kind = ps.good ? 'go' : 'warn'; label = ps.label; note = posLabel(ps.target); }
    else if (ps.kind === 'nofit')    { kind = 'stop'; label = ps.label; act = noop; }
    else if (ps.kind === 'physical') {
      kind = 'stop'; label = ps.label; act = noop;
      // "Round the back" is an instruction, so it has to be a button. Without
      // one the board tells the driver what to do and gives them no way to
      // record having done it — and the order is stranded on a shut door.
      var other = spot.door === 'side' ? 'back' : 'side';
      var landing = freeSpotAt(st, other);
      if (landing) {
        kind = 'warn'; label = 'Carry round the back'; note = 'to ' + landing.name;
        act = function () {
          self.apply(function (s) { doMoveSpot(s, focus, other); });
          self.setState({ focus: landing.id, target: null, host: null });
        };
      }
    }

    var topKind = tu.kind === 'nohost' ? 'off' : (forced ? 'warn' : 'quiet');
    return {
      box: box,
      eyebrow: spot.name + ' · ' + CUST[held.cust].name.toUpperCase(),
      eyebrowStyle: "font:600 11px/1 'IBM Plex Mono',monospace;letter-spacing:.10em;color:"
        + (S.picked ? '#FFB570' : '#CBB0FF') + ';',
      note: (this.props.tier >= 3 && PALLETS[held.cust] ? 'pallet ' + PALLETS[held.cust] + ' · ' : '')
        + 'stop ' + stopOf(held.cust).i + ' of ' + STOPS.length
        + (this.props.tier >= 2 && COUNTS[held.cust] ? ' · ' + COUNTS[held.cust] + ' expected' : ''),
      noteStyle: "font:400 12px/1 'Space Grotesk',sans-serif;color:#5F5876;",

      minus: function () { self.apply(function (s) { doBump(s, focus, -1); }); },
      minusStyle: this.btn(held.n ? 'quiet' : 'off', 76, accent) + 'width:56px;font-size:22px;',
      plus: function () { self.apply(function (s) { doBump(s, focus, 1); }); },
      plusStyle: this.btn('quiet', 76, accent) + 'width:132px;',
      plusLabel: held.n ? '+ 1  (' + held.n + ')' : '+ 1 crate',
      plusNote: held.n ? 'on the spot' : (suggest ? 'or push ' + suggest + ' blind' : 'uncounted'),
      plusBig: big, plusSub: sub,

      push: act,
      pushStyle: this.btn(kind, 76, accent) + 'width:216px;',
      pushLabel: label, pushNote: note, pushBig: big, pushSub: sub,

      // The slot stays whether or not there is a stack to use, because a button
      // that disappears moves every button beside it under a hand that is
      // already reaching for one of them.
      hasTop: true,
      top: tu.kind === 'nohost' ? noop : function () {
        self.apply(function (s) { doStack(s, focus, tu.host.id, take); });
        self.setState({ host: null });
      },
      topStyle: this.btn(topKind, 76, accent) + 'width:150px;',
      topLabel: '+' + take + ' on top',
      topNote: tu.host ? posLabel(tu.host.id) : 'no stack for it', topBig: big, topSub: sub,

      done: function () { self.apply(function (s) { doClose(s, focus); }); self.setState({ focus: null, target: null, host: null }); },
      doneStyle: this.btn('quiet', 76, accent) + 'width:104px;', doneLabel: 'Done', doneBig: big,

      // When the ±3 rule blocks the position AND a stack could take the crates
      // instead, both facts are load-bearing: one says why the ordinary move is
      // amber, the other says what the remedy costs. Showing only the first is
      // how the driver ends up mixing a stack without being told what mixing is.
      showWhy: !!(ps.why || (forced && tu.kind !== 'nohost')),
      why: [ps.why, forced && tu.host
        ? (forced === 'space' ? 'The floor is running short. ' : '')
          + CUST[tu.host.below].name + '’s stack at ' + posLabel(tu.host.id) + ' would take them — '
          + 'but two customers on one stack is how the wrong crate gets carried into a building.'
        : ''].filter(function (x) { return x; }).join('  '),
      whyStyle: 'font:400 13px/1.35 "Space Grotesk",sans-serif;color:'
        + (kind === 'stop' ? '#F7768E' : '#FFB570') + ';max-width:590px;'
    };
  }

  renderVals() {
    var self = this;
    // The van is a setting, and there is more than one van. Reshape first, then
    // make the board's state fit the shape it just got.
    configure(this.props);
    var st = normalize(this.state.st);
    var tier = this.props.tier == null ? 1 : this.props.tier;
    var plan = tier >= 2 ? planAhead(COUNTS, st) : null;
    var accent = this.props.accent == null ? '#B48EF7' : this.props.accent;
    var S = this.scene(st, accent, plan);

    var free = positionsLeft(st, 'side') + positionsLeft(st, 'back');
    var sideLeft = positionsLeft(st, 'side');
    var doneStops = QUEUE.filter(function (k) { return st.closed[k]; }).length;
    var big = 'font:800 22px/1.15 Archivo,system-ui,sans-serif;letter-spacing:-.02em;color:';
    var stats = [
      { label: 'POSITIONS LEFT', value: positionsHeld(st) ? free + ' · ' + positionsHeld(st) + ' held' : String(free),
        style: big + (free ? '#F2EEF8' : '#F7768E') + ';' },
      { label: 'SIDE DOOR', value: sideLeft ? sideLeft + ' left' : 'shut', style: big + (sideLeft ? '#FFB570' : '#F7768E') + ';' },
      { label: 'CRATES IN', value: String(cratesIn(st)), style: big + '#CDC6DD;' },
      { label: 'STOPS', value: doneStops + ' / ' + QUEUE.length, style: big + '#CDC6DD;' }
    ];

    // ── the route, in loading order ──────────────────────────────────────────
    var doorBtn = 'width:62px;height:46px;border-radius:10px;display:flex;align-items:center;justify-content:center;'
      + "font:700 13px/1 'Space Grotesk',sans-serif;flex:none;";
    var queue = QUEUE.map(function (k) {
      var closed = !!st.closed[k], at = spotHolding(st, k), pos = positionsOf(st, k);
      var isFocus = at && at.id === S.focus;
      var state, col;
      var mine = 0;
      pos.forEach(function (id) { st.van[id].forEach(function (l) { if (l.cust === k) mine += (l.n || 0); }); });
      if (closed) {
        state = pos.length ? 'DONE · ' + mine + ' in ' + pos.length + (pos.length === 1 ? ' position' : ' positions')
          : 'DONE · nothing aboard';
        col = '#4FD6A8';
      }
      else if (isFocus) { state = 'PACKING · ' + at.name; col = '#CBB0FF'; }
      else if (at) { state = 'ON ' + at.name; col = '#FFB570'; }
      else if (pos.length) { state = 'PART IN · ' + pos.map(posLabel).join(' '); col = '#8D87A0'; }
      else { state = 'STOP ' + stopOf(k).i + ' · WAITING'; col = '#5F5876'; }

      function door(d) {
        var bs = beginState(st, k, d);
        var tone = bs.kind === 'ready' ? (d === 'side' ? 'rgba(255,181,112,.10);border:1px solid rgba(255,181,112,.45);color:#FFB570'
                                                       : 'rgba(203,176,255,.10);border:1px solid rgba(180,142,247,.5);color:#CBB0FF')
          : bs.kind === 'packing' ? 'rgba(242,238,248,.05);border:1px solid ' + accent + '80;color:#F2EEF8'
          : bs.kind === 'move' ? 'rgba(203,176,255,.05);border:1px dashed rgba(180,142,247,.5);color:#CBB0FF'
          : bs.kind === 'well' || bs.kind === 'order' ? 'rgba(255,181,112,.07);border:1px dashed rgba(255,181,112,.45);color:#C09263'
          : 'rgba(242,238,248,.02);border:1px solid #201C2B;color:#3F3A52';
        return {
          style: doorBtn + 'background:' + tone + ';',
          label: bs.kind === 'move' ? 'move' : (d === 'side' ? 'side' : 'rear'),
          tap: (bs.kind === 'nospot' || bs.kind === 'shut') ? noop : function () {
            var landed = null;
            self.apply(function (s) { landed = doBegin(s, k, d); });
            self.setState({ focus: landed, target: null, host: null });
          }
        };
      }
      var sideB = door('side'), rearB = door('back');
      return {
        tap: function () { if (at) self.setState({ focus: at.id, target: null, host: null }); },
        tile: 'display:flex;align-items:center;gap:9px;padding:8px 9px;border-radius:11px;cursor:pointer;'
          + 'flex:1 1 0;min-height:62px;max-height:96px;'
          + 'background:' + (isFocus ? '#191524' : '#0E0C14') + ';border:1px solid '
          + (isFocus ? accent + '80' : (closed ? 'rgba(79,214,168,.22)' : '#1B1826')) + ';',
        barStyle: 'width:8px;align-self:stretch;border-radius:3px;flex:none;background:' + CUST[k].color + (closed ? '55' : ''),
        name: CUST[k].name,
        nameStyle: 'font:700 15px/1.15 Archivo,system-ui,sans-serif;letter-spacing:-.02em;overflow:hidden;'
          + 'text-overflow:ellipsis;white-space:nowrap;color:' + (closed || at ? '#F2EEF8' : '#8D87A0') + ';',
        state: state,
        stateStyle: "font:500 11px/1.2 'IBM Plex Mono',monospace;letter-spacing:.05em;color:" + col + ';'
          + 'overflow:hidden;text-overflow:ellipsis;white-space:nowrap;',
        hasDoors: !closed, hasReopen: closed,
        side: sideB.tap, sideStyle: sideB.style, sideLabel: sideB.label,
        rear: rearB.tap, rearStyle: rearB.style, rearLabel: rearB.label,
        reopen: function () { self.apply(function (s) { doReopen(s, k); }); },
        reopenStyle: 'width:129px;height:46px;border-radius:10px;display:flex;align-items:center;justify-content:center;'
          + "font:600 13px/1 'Space Grotesk',sans-serif;flex:none;background:rgba(242,238,248,.03);color:#4A445C;"
      };
    });

    // ── the two ways this goes wrong before anybody notices ──────────────────
    // Both are counts crossing, and both are only fixable while there is still
    // a choice — so they are said the moment they cross, not when they bite.
    var lines = [];
    // The side door shutting on stacks that are still standing at it. Naming the
    // shortfall is not the same as naming who it lands on: the stacks go in in
    // loading order, so the ones past the position count are the ones stranded.
    var queued = SPOTS.filter(function (sp) { return sp.door === 'side' && st.staged[sp.id]; })
      .sort(function (a, b) {
        return QUEUE.indexOf(st.staged[a.id].cust) - QUEUE.indexOf(st.staged[b.id].cust);
      });
    var stranded = queued.slice(sideLeft);
    if (stranded.length) {
      lines.push(sideLeft + ' position' + (sideLeft === 1 ? '' : 's') + ' left through the side door and '
        + queued.length + ' stacks standing at it — '
        + stranded.map(function (sp) { return sp.name + ' (' + CUST[st.staged[sp.id].cust].short + ')'; }).join(' and ')
        + ' will have to be carried round.');
    }
    var notInYet = QUEUE.filter(function (x) { return !st.closed[x] && !isAboard(st, x); }).length;
    if (free < notInYet) {
      lines.push(free + ' position' + (free === 1 ? '' : 's') + ' left and ' + notInYet
        + ' stops with nothing aboard — some will have to share a stack.');
    }
    var warn = {
      show: lines.length > 0,
      text: lines.join('  '),
      style: 'padding:9px 12px;border-radius:11px;flex:none;font:400 12.5px/1.35 "Space Grotesk",sans-serif;'
        + 'background:rgba(247,118,142,.09);border:1px solid rgba(247,118,142,.35);color:#F7768E;'
    };

    var toolBig = "font:700 15px/1 'Space Grotesk',sans-serif;";
    var toolSub = "font:400 10px/1 'IBM Plex Mono',monospace;letter-spacing:.07em;opacity:.7;";
    var tools = [
      { label: 'Undo', sub: this.state.hist.length ? this.state.hist.length + ' back' : 'nothing yet',
        tap: function () { self.undo(); },
        style: this.btn(this.state.hist.length ? 'quiet' : 'off', 58, accent) + 'width:118px;',
        bigStyle: toolBig, subStyle: toolSub },
      { label: '⚑ Odd crate', sub: st.flags ? st.flags + ' flagged' : 'off route',
        tap: function () { self.apply(function (s) { s.flags = (s.flags || 0) + 1; }); },
        style: this.btn(st.flags ? 'warn' : 'quiet', 58, accent) + 'width:134px;',
        bigStyle: toolBig, subStyle: toolSub },
      { label: '❄ Freeze', sub: st.frozenAtDoor ? 'side well' : 'none today',
        tap: function () { self.apply(function (s) { s.frozenAtDoor = !s.frozenAtDoor; }); },
        style: this.btn(st.frozenAtDoor ? 'quiet' : 'off', 58, accent) + 'width:112px;',
        bigStyle: toolBig, subStyle: toolSub }
    ];

    return {
      head: { title: 'Stavanger Route',
              sub: 'WED 19 AUG · ' + STOPS.length + ' STOPS · '
                + (tier === 1 ? 'ROUTE ONLY' : (tier === 2 ? 'COUNTS KNOWN' : 'FULLY SCANNED')) },
      stats: stats,
      scene: { box: S.box, parts: S.parts },
      con: this.consoleVals(st, accent, plan, S),
      queue: queue, warn: warn, tools: tools
    };
  }
}
