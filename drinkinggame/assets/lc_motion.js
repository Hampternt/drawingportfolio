// Last Call motion helper. The contract is one class plus CSS custom
// properties (spec §7.7) — no animation logic lives in feature code.
(function () {
  "use strict";
  var LAYER_ID = "lc-flights";

  function reduced() {
    return window.matchMedia &&
           window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  }

  // Double-injection guard: hx-boost swaps body children without a reload, so
  // this runs again on every navigation and must not stack a second layer.
  function ensureLayer(root) {
    var host = (root || document).querySelector("[data-lc-scene]") || document.body;
    var layer = host.querySelector("#" + LAYER_ID);
    if (layer) return layer;                     // <- the guard
    layer = document.createElement("div");
    layer.id = LAYER_ID;
    host.appendChild(layer);
    return layer;
  }

  window.lcAnchor = function (name, root) {
    return (root || document)
      .querySelector('[data-flight-anchor="' + name + '"]');
  };

  function centre(el, originRect) {
    var r = el.getBoundingClientRect();
    return { x: r.left + r.width / 2 - originRect.left,
             y: r.top + r.height / 2 - originRect.top };
  }

  window.lcFlight = function (fromEl, toEl, opts) {
    opts = opts || {};
    var arrive = opts.onArrive || function () {};
    // Reduced motion: no node at all, but the arrival still fires. The
    // README's rule is that arrival must tick the destination's counter —
    // "the number and the animation are one event, never two" — so skipping
    // the animation must never skip the count.
    if (reduced() || !fromEl || !toEl) { arrive(); return; }

    var layer = ensureLayer();
    var origin = layer.getBoundingClientRect();
    var a = centre(fromEl, origin), b = centre(toEl, origin);

    var node = document.createElement("div");
    node.className = "lc-flight" + (opts.deck ? " lc-deck-" + opts.deck : "");
    node.setAttribute("data-flight", opts.direction || "draw");
    node.setAttribute("data-scale", opts.scale === "dot" ? "dot" : "card");
    node.style.left = a.x + "px";
    node.style.top = a.y + "px";
    node.style.setProperty("--dx", (b.x - a.x) + "px");
    node.style.setProperty("--dy", (b.y - a.y) + "px");
    if (opts.delay) node.style.animationDelay = opts.delay + "ms";

    // Fires once, then removes itself. No timers: animationend is the only
    // signal that stays correct when the tab is backgrounded and rAF throttles.
    node.addEventListener("animationend", function () {
      node.remove();
      arrive();
    }, { once: true });
    layer.appendChild(node);
  };

  // ---- Pack 1 (lc-mobile-play-flow) --------------------------------------

  // The design handoff's curved arrow: a quadratic bezier whose control
  // point bows toward the felt centre — of the two perpendicular offsets
  // from the chord's midpoint, the one nearer (W/2, H/2) wins.
  function curve(x1, y1, x2, y2, W, H) {
    var mx = (x1 + x2) / 2, my = (y1 + y2) / 2;
    var dx = x2 - x1, dy = y2 - y1, len = Math.hypot(dx, dy) || 1;
    var nx = -dy / len, ny = dx / len, bend = len * 0.18;
    var ax = mx + nx * bend, ay = my + ny * bend;
    var bx = mx - nx * bend, by = my - ny * bend;
    var qx = ax, qy = ay;
    if (Math.hypot(bx - W / 2, by - H / 2) < Math.hypot(ax - W / 2, ay - H / 2)) {
      qx = bx; qy = by;
    }
    return "M" + x1.toFixed(1) + " " + y1.toFixed(1) +
      " Q" + qx.toFixed(1) + " " + qy.toFixed(1) +
      " " + x2.toFixed(1) + " " + y2.toFixed(1);
  }

  var SVG_NS = "http://www.w3.org/2000/svg";

  // Redraws the ARMED stack's persistent arrows into the table scene's
  // [data-lc-arrows] layer: one dotted curve per targeted stack mini, or
  // one staggered wave pair when anything in the stack is AOE. Reads the
  // fresh DOM every call (the table repaints wholesale), so callers just
  // invoke it after any repaint or tab switch. CSS owns colour (deck
  // classes) and all motion; a hidden pane measures 0x0 and clears the
  // layer instead of drawing garbage geometry.
  window.lcTableArrows = function () {
    var scene = document.querySelector("[data-lc-scene-table]");
    var svg = scene && scene.querySelector("[data-lc-arrows]");
    if (!svg) return;
    while (svg.firstChild) svg.removeChild(svg.firstChild);
    var r = scene.getBoundingClientRect();
    if (!r.width || !r.height) return;
    var W = r.width, H = r.height;
    svg.setAttribute("viewBox", "0 0 " + W + " " + H);
    var waveDeck = null;
    scene.querySelectorAll(".lc-stack-mini").forEach(function (mini) {
      var deck = mini.dataset.deck;
      if (mini.hasAttribute("data-aoe")) {
        if (waveDeck === null) waveDeck = deck || "";
        return; // one wave pair regardless of how many AOE plays are armed
      }
      var seat = mini.dataset.arrow;
      if (seat === undefined) return; // "one" card still awaiting a target
      var chip = scene.querySelector('[data-flight-anchor="seat-' + seat + '"]');
      if (!chip) return;
      var mr = mini.getBoundingClientRect();
      var cr = chip.getBoundingClientRect();
      var sx = mr.right - r.left, sy = mr.top + mr.height / 2 - r.top;
      var ex = cr.left + cr.width / 2 - r.left;
      var ey = cr.top + cr.height / 2 - r.top;
      var dx = ex - sx, dy = ey - sy, len = Math.hypot(dx, dy) || 1;
      ex -= dx / len * 36; ey -= dy / len * 36; // stop short of the chip
      var path = document.createElementNS(SVG_NS, "path");
      path.setAttribute("d", curve(sx, sy, ex, ey, W, H));
      if (deck) path.setAttribute("class", "lc-deck-" + deck);
      svg.appendChild(path);
    });
    if (waveDeck !== null) {
      for (var i = 0; i < 2; i++) {
        var el = document.createElementNS(SVG_NS, "ellipse");
        el.setAttribute("cx", W / 2);
        el.setAttribute("cy", H / 2);
        el.setAttribute("rx", Math.max(0, W / 2 - 16));
        el.setAttribute("ry", Math.max(0, H / 2 - 14));
        if (waveDeck) el.setAttribute("class", "lc-deck-" + waveDeck);
        if (i) el.style.animationDelay = "1s";
        svg.appendChild(el);
      }
    }
  };

  // The arm flash: the staged card's preview, large at the felt centre
  // under a "YOU → TGT" caption, then a 450ms fly into the ARMED stack.
  // Decorative only — the armed queue's real state arrives via the tick
  // repaint regardless — so reduced motion (and a hidden TABLE pane) skip
  // it outright rather than degrading it. Lives in the #lc-flights layer,
  // which no table repaint ever replaces.
  window.lcArmFlash = function (opts) {
    opts = opts || {};
    if (reduced()) return;
    var scene = document.querySelector("[data-lc-scene-table]");
    var felt = scene && scene.querySelector('[data-flight-anchor="felt"]');
    if (!felt || felt.offsetParent === null) return;
    var layer = ensureLayer();
    var origin = layer.getBoundingClientRect();
    var c = centre(felt, origin);

    var node = document.createElement("div");
    node.className = "lc-armflash" + (opts.deck ? " lc-deck-" + opts.deck : "");
    var cap = document.createElement("div");
    cap.className = "lc-armflash-cap";
    cap.textContent = opts.caption || "";
    node.appendChild(cap);
    if (opts.previewHTML) {
      var body = document.createElement("div");
      body.innerHTML = opts.previewHTML; // server-rendered, already escaped
      node.appendChild(body);
    }
    node.style.left = c.x + "px";
    node.style.top = c.y + "px";
    layer.appendChild(node);

    window.setTimeout(function () {
      var stack = window.lcAnchor("stack");
      var to = stack && stack.offsetParent !== null
        ? centre(stack, origin)
        : { x: c.x - 120, y: c.y }; // repaint not landed yet: fly left
      node.style.transition =
        "transform 450ms cubic-bezier(.2,.8,.3,1), opacity 450ms cubic-bezier(.2,.8,.3,1)";
      node.style.transform = "translate(calc(-50% + " + (to.x - c.x) +
        "px), calc(-50% + " + (to.y - c.y) + "px)) scale(.26)";
      node.style.opacity = "0.3";
      window.setTimeout(function () { node.remove(); }, 520);
    }, 700);
  };

  function init() { ensureLayer(); }
  document.addEventListener("DOMContentLoaded", init);
  document.addEventListener("htmx:afterSwap", function (e) { ensureLayer(e.target); });
})();
