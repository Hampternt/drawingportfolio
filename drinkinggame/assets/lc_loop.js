// Last Call loop wiring (Plan E). The F.1 action bar's `data-lc-post`
// buttons, the Lock-beat target picker, Plan C's `lc:arm`/`lc:disarm`
// CustomEvents, and the live beat timer all funnel through here — one
// delegated listener per event type, bound once on `document.body`, so
// nothing here needs rebinding when a repaint (hx-boost, lcApply,
// lc_screen.html's lcpublic swap) replaces the DOM it targets. Task 5 adds
// flights/hits on top of the same globals.
(function () {
  "use strict";

  var NOTE_MS = 2600, URGENT_MS = 5000;
  var urgentTimer = null;

  function post(action, body) {
    return fetch(BP + "/room/" + CODE + "/lastcall/" + action, {
      method: "POST",
      credentials: "same-origin",
      headers: { "Content-Type": "application/x-www-form-urlencoded" },
      body: body || "",
    }).then(function (res) {
      if (!res.ok) {
        res.text().then(function (t) { note(t || "Not now."); });
      }
      return res.ok;
    });
  }

  // The action bar's own inline `#lc-actions-note` — map_lc's "Can't afford
  // …" / "… needs a target." 422 bodies surface here, verbatim, for NOTE_MS.
  function note(text) {
    var el = document.getElementById("lc-actions-note");
    if (!el) return;
    el.textContent = text;
    el.hidden = false;
    window.clearTimeout(el._lcNoteTimer);
    el._lcNoteTimer = window.setTimeout(function () {
      el.hidden = true;
    }, NOTE_MS);
  }

  // One delegated click listener for every `[data-lc-post]` button the
  // action bar ever renders — arm/disarm are their own CustomEvent listeners
  // below (the wheel/armed-column dispatch those, not a data-lc-post
  // button), everything else in the F.1 table (begin/lock/draw/end) posts
  // straight from here.
  document.body.addEventListener("click", function (e) {
    var el = e.target.closest ? e.target.closest("[data-lc-post]") : null;
    if (!el || el.disabled) return;
    var action = el.dataset.lcPost;
    var body = el.dataset.vessel !== undefined
      ? "vessel=" + encodeURIComponent(el.dataset.vessel)
      : "";
    post(action, body);
  });

  // One delegated change listener for the Lock-beat target picker.
  document.body.addEventListener("change", function (e) {
    var sel = e.target.closest ? e.target.closest("select[data-lc-target]") : null;
    if (!sel) return;
    post(
      "target",
      "card_id=" + encodeURIComponent(sel.dataset.cardId) +
        "&target=" + encodeURIComponent(sel.value)
    );
  });

  // Plan C's contract: lc:arm/lc:disarm are dispatched by the wheel/armed
  // column BEFORE the wheel's glide settles — this listener must not assume
  // the wheel is at rest. Delegated once, never rebound.
  document.body.addEventListener("lc:arm", function (e) {
    post("arm", "card_id=" + encodeURIComponent(e.detail.cardId)).then(function (ok) {
      if (!ok || !window.lcFlight) return;
      var face = e.target.querySelector && e.target.querySelector(".lc-cardface");
      window.lcFlight(e.target, window.lcAnchor("armed"), {
        direction: "play",
        scale: "dot",
        deck: face && face.dataset.deck,
      });
    });
  });
  document.body.addEventListener("lc:disarm", function (e) {
    post("disarm", "card_id=" + encodeURIComponent(e.detail.cardId));
  });

  // Moves the private hand fetch's <template data-lc-actions> (a sibling of
  // #lc-hand, not a descendant — same reason the setup form's END GAME
  // button lives outside #lc-table) into the shell's persistent
  // .lc-actions, then discards the template. Never no-ops silently on a
  // missing bar: absence just means this fetch carried no template (the
  // route is unreachable without one), so nothing to relocate.
  window.lcLoopApply = function (pane) {
    var tpl = pane && pane.querySelector("template[data-lc-actions]");
    if (!tpl) return;
    var bar = document.querySelector(".lc-actions");
    if (bar) bar.innerHTML = tpl.innerHTML;
    tpl.remove();
  };

  // Arms (or re-arms) the live beat timer from its own data-deadline-ms —
  // both shells share the one #lc-beat-timer id, so this one function
  // serves the phone banner and the big-screen banner alike.
  window.lcLoopPublic = function () {
    var timer = document.getElementById("lc-beat-timer");
    window.clearTimeout(urgentTimer);
    urgentTimer = null;
    if (!timer || timer.dataset.deadlineMs === undefined) return;
    var deadline = Number(timer.dataset.deadlineMs);
    var remaining = Math.max(0, deadline - Date.now());
    timer.style.setProperty("--lc-beat-ms", remaining + "ms");
    timer.classList.remove("is-urgent");
    if (remaining <= URGENT_MS) {
      timer.classList.add("is-urgent");
    } else {
      urgentTimer = window.setTimeout(function () {
        timer.classList.add("is-urgent");
      }, remaining - URGENT_MS);
    }
  };

  function init() {
    if (window.__lcLoopBound) return;
    window.__lcLoopBound = true;
    window.lcLoopPublic(); // arm the server-rendered banner's timer
  }
  document.addEventListener("DOMContentLoaded", init);
})();
