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

  function init() { ensureLayer(); }
  document.addEventListener("DOMContentLoaded", init);
  document.addEventListener("htmx:afterSwap", function (e) { ensureLayer(e.target); });
})();
