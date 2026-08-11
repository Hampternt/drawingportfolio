// Last Call hand wheel. Drag/snap/notch the private hand carousel, keep the
// cost rail in sync with the focused card, and turn a tap on the focused
// card (or an armed mini) into an arm/disarm intent for the caller to wire
// up (spec: "nothing in this plan listens to either event" — Plan D/E's
// job, not this file's).
(function () {
  "use strict";

  var STEP = 21, RADIUS = 470, SENS = 0.28, SNAP_MS = 220, NOTCH_MS = 200;

  function reduced() {
    return window.matchMedia &&
           window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  }

  // One persisted camera angle for the phone's single live wheel (decision
  // 8): saved on every angle change by the wheel whose stage sits inside
  // #lc-hand, restored (re-wrapped to the new N) when lcWheelInit rebuilds
  // after a repaint. Preview wheels are gallery demos and do not persist —
  // they are never inside #lc-hand, so they never touch this var.
  var savedAngle = 0;

  function wrapToN(a, n) {
    var span = n * STEP;
    a = a % span;
    if (a > span / 2) a -= span;
    if (a < -span / 2) a += span;
    return a;
  }

  function layout(cards, angle, dragging) {
    var N = cards.length;
    for (var i = 0; i < N; i++) {
      var d = i - angle / STEP;
      while (d > N / 2) d -= N;
      while (d < -N / 2) d += N;
      var ad = Math.abs(d);
      var el = cards[i];
      el.style.transform = "rotateX(" + (-d * STEP) + "deg) translateZ(" + RADIUS + "px)";
      el.style.opacity = String(Math.max(0, 1 - 0.48 * ad));
      el.style.visibility = ad > 2.05 ? "hidden" : "visible";
      el.style.zIndex = String(100 - Math.round(ad * 10));
      el.classList.toggle("is-focused", ad < 0.5);
      el.classList.toggle("is-dragging", !!dragging);
      el.classList.toggle("is-far", ad > 1.6);
    }
    return ((Math.round(angle / STEP) % N) + N) % N; // focused index
  }

  // Suppression scope for arm/disarm: "no .lc-armed[data-locked] exists in
  // the same #lc-hand" (decision 7). Preview fixtures sit outside any
  // #lc-hand — scope to the nearest .lc-handgroup (a wheel + its armed
  // column) or, failing that, the nearest .lc-armed itself (a standalone
  // armed-column swatch is its own scope, so a locked standalone sample
  // still self-suppresses) before falling back to a document-wide check.
  // Without this, one always-locked preview sample (group 8 row 3) would
  // gag every other group's dispatch page-wide.
  function locked(el) {
    var scope = el.closest("#lc-hand") ||
      el.closest(".lc-handgroup") ||
      el.closest(".lc-armed") ||
      document;
    if (scope.nodeType === 1 && scope.matches(".lc-armed[data-locked]")) {
      return true;
    }
    return !!scope.querySelector(".lc-armed[data-locked]");
  }

  function syncRail(stage, focusedIdx) {
    var group = stage.closest(".lc-handgroup");
    if (!group) return;
    var above = group.querySelector(".lc-costrail-above");
    if (above) above.textContent = String(focusedIdx + 1).padStart(2, "0");
    group.querySelectorAll(".lc-costrail-group").forEach(function (g) {
      g.classList.toggle("is-active", Number(g.dataset.idx) === focusedIdx);
    });
  }

  // Double-injection guard on the stage itself: hx-boost / the hand pane's
  // manual innerHTML repaint both re-run init, and must not double-bind.
  function initWheelStage(stage) {
    if (stage.dataset.lcWheelBound) return;

    var track = stage.querySelector(".lc-wheel-track");
    var cards = track ? Array.from(track.querySelectorAll(".lc-wheel-card")) : [];
    if (!cards.length) return;
    stage.dataset.lcWheelBound = "1";

    var N = cards.length;
    var persist = !!stage.closest("#lc-hand");
    var angle = persist ? wrapToN(savedAngle, N) : 0;
    var dragging = false;
    var raf = null;
    var y0 = 0, x0 = 0, a0 = 0, t0 = 0, downTarget = null;

    function relayout(isDragging) {
      var focusedIdx = layout(cards, angle, isDragging);
      syncRail(stage, focusedIdx);
      if (persist) savedAngle = angle;
    }

    function snap(a) {
      return Math.round(a / STEP) * STEP;
    }

    function glide(to, ms) {
      if (raf) {
        cancelAnimationFrame(raf);
        raf = null;
      }
      if (reduced()) {
        angle = to;
        relayout(false);
        return;
      }
      var from = angle;
      var start = null;
      function step(now) {
        if (start === null) start = now;
        var p = Math.min(1, (now - start) / ms);
        var eased = 1 - Math.pow(1 - p, 3);
        angle = from + (to - from) * eased;
        // finding 3 (fix wave): relayout as if dragging for the duration
        // of the glide — `.is-dragging` disables the 280ms CSS transform
        // transition (lastcall.css:428), so the rAF loop's own cubic
        // ease-out is the only easing in effect. Without this the CSS
        // transition chases every per-frame write and the release snap
        // settles in ~SNAP_MS + 280ms of exponential lag, not ~220ms.
        relayout(true);
        if (p < 1) {
          raf = requestAnimationFrame(step);
        } else {
          raf = null;
          relayout(dragging);
        }
      }
      raf = requestAnimationFrame(step);
    }

    function dispatchArm(cardEl) {
      if (locked(cardEl)) return;
      cardEl.dispatchEvent(new CustomEvent("lc:arm", {
        bubbles: true,
        detail: { cardId: cardEl.dataset.cardId },
      }));
    }

    stage.addEventListener("pointerdown", function (e) {
      if (raf) {
        cancelAnimationFrame(raf);
        raf = null;
      }
      try { stage.setPointerCapture(e.pointerId); } catch (_) {}
      y0 = e.clientY;
      x0 = e.clientX;
      a0 = angle;
      t0 = Date.now();
      downTarget = e.target;
      dragging = true;
    });

    stage.addEventListener("pointermove", function (e) {
      if (!dragging) return;
      angle = a0 - (e.clientY - y0) * SENS;
      relayout(true);
    });

    function release(e) {
      if (!dragging) return;
      dragging = false;
      try { stage.releasePointerCapture(e.pointerId); } catch (_) {}
      // finding 6 (fix wave): travel is the larger of the two axes, not
      // vertical-only — a quick horizontal wobble with little vertical
      // component must not read as a tap on the focused card.
      var travel = Math.max(Math.abs(e.clientY - y0), Math.abs(e.clientX - x0));
      var elapsed = Date.now() - t0;
      // finding 5 (fix wave): only a genuine pointerup may qualify as a
      // tap. `release` is also bound to pointercancel (so a drag that gets
      // cancelled mid-gesture still stops cleanly), but an aborted touch
      // must never arm a card — Plan D/E will POST on lc:arm.
      if (e.type === "pointerup" && travel < 6 && elapsed < 250) {
        var cardEl = downTarget && downTarget.closest ?
          downTarget.closest(".lc-wheel-card") : null;
        if (cardEl && cardEl.classList.contains("is-focused")) {
          angle = a0;
          dispatchArm(cardEl);
          // finding 7 (fix wave): a tap that interrupted a mid-flight
          // glide restores `a0`, which may itself be unsnapped — glide to
          // the snapped angle instead of a bare relayout so the wheel is
          // never left off-notch after a tap.
          glide(snap(angle), SNAP_MS);
          return;
        }
      }
      glide(snap(angle), SNAP_MS);
    }
    stage.addEventListener("pointerup", release);
    stage.addEventListener("pointercancel", release);

    stage.addEventListener("wheel", function (e) {
      e.preventDefault();
      glide(snap(angle + Math.sign(e.deltaY) * STEP), NOTCH_MS);
    }, { passive: false });

    // finding 4 (fix wave): glide to the congruent target nearest the
    // current angle, not the absolute `idx * STEP`. `angle` is unbounded
    // (only wrapped on re-init), so after a few full drag revolutions an
    // absolute target would animate back through every revolution in
    // SNAP_MS. Used by the rail's tap-to-jump (decision 2).
    function glideToIndex(idx, ms) {
      var target = idx * STEP;
      var revs = Math.round((angle - target) / (N * STEP));
      glide(target + revs * N * STEP, ms);
    }

    stage.lcWheelApi = { glide: glide, glideToIndex: glideToIndex };

    relayout(false);
  }

  // Rail scrubbing (decision 2): the whole column is one pointer surface —
  // a tap maps its y to the nearest cost group and glides the sibling
  // wheel there. Own guard, since the rail root is a sibling of the stage,
  // not the stage itself.
  function initRail(rail) {
    if (rail.dataset.lcRailBound) return;
    rail.dataset.lcRailBound = "1";

    rail.addEventListener("pointerdown", function (e) {
      var group = rail.closest(".lc-handgroup");
      if (!group) return;
      var stage = group.querySelector("[data-lc-wheel]");
      if (!stage || !stage.lcWheelApi) return;
      var groups = Array.from(group.querySelectorAll(".lc-costrail-group"));
      if (!groups.length) return;
      var nearest = null, best = Infinity;
      groups.forEach(function (g) {
        var r = g.getBoundingClientRect();
        var mid = r.top + r.height / 2;
        var dist = Math.abs(e.clientY - mid);
        if (dist < best) {
          best = dist;
          nearest = g;
        }
      });
      if (!nearest) return;
      var idx = Number(nearest.dataset.idx);
      stage.lcWheelApi.glideToIndex(idx, SNAP_MS);
    });
  }

  // Armed column: a tap on an armed mini disarms it (locked-suppressed,
  // decision 7). Own guard, same reasoning as initRail.
  function initArmed(armed) {
    if (armed.dataset.lcArmedBound) return;
    armed.dataset.lcArmedBound = "1";

    armed.addEventListener("click", function (e) {
      var mini = e.target.closest ? e.target.closest(".lc-mini[data-card-id]") : null;
      if (!mini || !armed.contains(mini)) return;
      if (locked(mini)) return;
      mini.dispatchEvent(new CustomEvent("lc:disarm", {
        bubbles: true,
        detail: { cardId: mini.dataset.cardId },
      }));
    });
  }

  window.lcWheelInit = function (root) {
    root = root || document;
    root.querySelectorAll("[data-lc-wheel]").forEach(initWheelStage);
    root.querySelectorAll(".lc-costrail").forEach(initRail);
    root.querySelectorAll(".lc-armed").forEach(initArmed);
  };

  document.addEventListener("DOMContentLoaded", function () { window.lcWheelInit(); });
  document.addEventListener("htmx:afterSwap", function (e) { window.lcWheelInit(e.target); });
})();
