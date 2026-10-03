// Backroom's private-pane client.
//
// The room's SSE stream carries only public state, so a player's role, the
// tiles they are holding and their own controls are FETCHED, not pushed:
// every `game` frame is a signal to re-fetch this viewer's own fragment from
// a session-authenticated route.
//
// Three things here are deliberate and easy to get wrong on the next edit:
//
//  * `#sh-private` is a SIBLING of `#game-panel`, never a child. room.html's
//    swapPanel() does `el.innerHTML = html` on #game-panel, so a pane nested
//    inside it would be destroyed and re-fetched on every single frame —
//    a blank role card flashing on every action in the room.
//
//  * There is no `htmx:afterSwap` binding. room.html's only htmx requests
//    are hx-swap="none", and #game-panel is repainted by swapPanel(), which
//    is plain innerHTML plus htmx.process — not an htmx swap. That event
//    never fires on this page.
//
//  * The boot call lives in this file's own DOMContentLoaded handler. An
//    inline `window.shAfterGameSwap()` in room.html's script would run
//    during parsing, before this deferred file has executed, and be
//    silently undefined.
//
// ES5, no build step; `node --check` covers it via scripts/check.sh.
(function () {
  "use strict";

  // The highest seq we have seen on a public panel. A private fetch that
  // answers with anything older lost a race and is dropped.
  var seqFloor = 0;
  var timer = null;

  function slot() {
    return document.getElementById("sh-private");
  }

  function panel() {
    var g = document.getElementById("game-panel");
    return g ? g.querySelector("[data-sh-panel]") : null;
  }

  // Leaving the game (it ended, or another game started) clears the pane so
  // a stale role card cannot outlive the round it belonged to.
  function reset() {
    seqFloor = 0;
    var s = slot();
    if (s && s.innerHTML !== "") {
      s.innerHTML = "";
      s.setAttribute("data-sh-seq", "0");
    }
  }

  function fetchPrivate() {
    var s = slot();
    if (!s) return;
    var body = document.body;
    var bp = body.getAttribute("data-base-path") || "";
    var code = body.getAttribute("data-code") || "";
    if (!code) return;

    fetch(bp + "/room/" + code + "/sh/private", { credentials: "same-origin" })
      .then(function (r) {
        return r.ok ? r.text() : null;
      })
      .then(function (html) {
        if (html === null) return;
        var box = document.createElement("div");
        box.innerHTML = html;
        var frag = box.querySelector("#sh-private");
        // An expired session is answered with a REDIRECT to the landing
        // page, which fetch follows transparently — so `r.ok` is true and
        // the body is a full HTML page. No #sh-private in it means this is
        // not our fragment; drop it rather than blanking the pane.
        if (!frag) return;
        var seq = Number(frag.getAttribute("data-sh-seq") || 0);
        if (seq < seqFloor) return;
        var cur = slot();
        if (!cur) return;
        cur.parentNode.replaceChild(frag, cur);
        // The pane carries hx-post forms; htmx only binds what it has been
        // shown. swapPanel() does the same for the public panel.
        if (window.htmx) window.htmx.process(frag);
      })
      .catch(function () {
        /* offline or navigating away — the next frame re-fetches */
      });
  }

  // One fetch per burst: a vote closing publishes several frames back to
  // back, and each phone needs the last one, not all of them.
  function schedule() {
    if (timer) clearTimeout(timer);
    timer = setTimeout(function () {
      timer = null;
      fetchPrivate();
    }, 60);
  }

  // Called by room.html's `game` SSE listener after the public panel swaps,
  // and once at boot. Detection is structural — a marker attribute on the
  // swapped DOM, never a substring test on the frame text, because player
  // names reach the frame escaped and must not be able to spoof it.
  window.shAfterGameSwap = function () {
    var p = panel();
    if (!p) {
      reset();
      return;
    }
    var seq = Number(p.getAttribute("data-sh-seq") || 0);
    if (seq > seqFloor) seqFloor = seq;
    schedule();
  };

  document.addEventListener("DOMContentLoaded", function () {
    window.shAfterGameSwap();
  });
})();
