// Last Call loop wiring (Plan E). The F.1 action bar's `data-lc-post`
// buttons, the Lock-beat target picker, Plan C's `lc:arm`/`lc:disarm`
// CustomEvents, and the live beat timer all funnel through here — one
// delegated listener per event type, bound once on `document.body`, so
// nothing here needs rebinding when a repaint (lcApply, lc_screen.html's
// lcpublic swap) replaces the DOM it targets. Task 5 adds flights/hits on
// top of the same globals.
(function () {
  "use strict";

  var NOTE_MS = 2600, REVEAL_STAGGER_MS = 220;

  // The motion pass's own memory of "what the DOM said last time" — beat,
  // per-seat HP, per-seat draw count. Module-level (not read off the DOM
  // fresh on both sides of a diff) because the diff itself needs a BEFORE
  // and an AFTER, and the DOM only ever holds the AFTER by the time
  // lcLoopPublic runs. Starts empty: the very first call can only ever see
  // increases/decreases against "nothing recorded yet", which the `!==
  // undefined` guards below treat as "no change to report", not as a fake
  // 0 -> n hit/draw.
  var prev = { beat: null, hp: {}, draws: {} };

  // Every seat-keyed number this pass cares about, read off whichever
  // surface is actually in the DOM — the big screen's `.lc-plaque` carries
  // both `data-hp` and `data-draws` (Plan A/E5); the phone's mini table has
  // no plaque at all, so on that page this simply finds nothing and the
  // maps stay empty, which is correct: hits and draw flights are plaque-only
  // effects (see the Hits comment below).
  function seatNumbers(attr) {
    var out = {};
    document.querySelectorAll(".lc-plaque[data-seat]").forEach(function (el) {
      var v = el.dataset[attr];
      if (v !== undefined) out[el.dataset.seat] = Number(v);
    });
    return out;
  }

  function snapshot() {
    var banner = document.getElementById("lc-banner");
    return {
      beat: banner ? banner.dataset.beat : null,
      hp: seatNumbers("hp"),
      draws: seatNumbers("draws"),
    };
  }

  // Both anchors must resolve to an on-screen element: `lcAnchor` finds the
  // first DOM match regardless of visibility, and the phone's TABLE pane is
  // usually `hidden` (decision E17) — a flight between a real rect and a
  // zero-rect anchor is garbage, not merely invisible, so this is a hard
  // skip, not a degrade.
  function visible(el) {
    return !!el && el.offsetParent !== null;
  }

  function fireRevealFlights() {
    var felt = window.lcAnchor && window.lcAnchor("felt");
    document.querySelectorAll(".lc-centre-play").forEach(function (el, i) {
      var from = window.lcAnchor("seat-" + el.dataset.seat);
      if (!visible(from) || !visible(felt)) return;
      var mini = el.querySelector(".lc-mini");
      window.lcFlight(from, felt, {
        direction: "play",
        deck: mini && mini.dataset.deck,
        delay: i * REVEAL_STAGGER_MS,
      });
    });
  }

  function fireDrawFlights(next) {
    var firstDeck = document.querySelector(".lc-deckstack[data-deck]");
    var from = firstDeck && window.lcAnchor("deck-" + firstDeck.dataset.deck);
    if (!visible(from)) return;
    Object.keys(next.draws).forEach(function (seat) {
      if (prev.draws[seat] === undefined || next.draws[seat] <= prev.draws[seat]) return;
      var to = window.lcAnchor("seat-" + seat);
      if (!visible(to)) return;
      window.lcFlight(from, to, { direction: "draw" });
    });
  }

  // Mini-table chips carry no `.lc-plaque`/`is-hit` rule at all (only the HP
  // number itself repaints there) — `seatNumbers` already scopes the hp
  // read to `.lc-plaque`, so this only ever finds a match, and therefore
  // only ever shakes, a big-screen plaque.
  function fireHits(next) {
    Object.keys(next.hp).forEach(function (seat) {
      if (prev.hp[seat] === undefined || next.hp[seat] >= prev.hp[seat]) return;
      var plaque = document.querySelector('.lc-plaque[data-seat="' + seat + '"]');
      if (!plaque) return;
      plaque.classList.add("is-hit");
      // `.is-hit` drives two animations at once — `lc-shake` on the plaque
      // itself (190ms) and `lc-hp-flash` on the nested `.lc-hp` (560ms,
      // lastcall.css's own reduced-motion block groups both). animationend
      // bubbles from whichever finishes; filtering on the LONGER one is
      // what keeps the class (and therefore the flash) alive for its full
      // 560ms instead of the shake's earlier end yanking it off both at
      // 190ms. Under reduced motion neither animation ever plays, so this
      // listener simply never fires and the class sits inert — harmless,
      // since no other rule selects on `.is-hit`.
      plaque.addEventListener("animationend", function onEnd(e) {
        if (e.animationName !== "lc-hp-flash") return;
        plaque.classList.remove("is-hit");
        plaque.removeEventListener("animationend", onEnd);
      });
    });
  }

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
  // …" / "… needs a target." 422 bodies are plain text and surface here
  // verbatim; every other refusal (403/409, the far more common mid-game
  // case) comes from `GameError::into_response` (error.rs), whose body is
  // markup: `<p class="error">…</p>`. Parsing the text into a detached
  // element and reading its `textContent` strips that markup down to the
  // message either way — inert (never inserted into the document, so no
  // script executes) and works for both plain-text and HTML bodies alike.
  function note(text) {
    var el = document.getElementById("lc-actions-note");
    if (!el) return;
    var parsed = document.createElement("div");
    parsed.innerHTML = text;
    el.textContent = parsed.textContent || text;
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
  function onClick(e) {
    var el = e.target.closest ? e.target.closest("[data-lc-post]") : null;
    if (!el || el.disabled) return;
    var action = el.dataset.lcPost;
    var body;
    if (el.dataset.lcBody !== undefined) {
      // Plan G, Task 3: pact buttons carry a pre-encoded `key=int` body —
      // server-rendered seat numbers, nothing here needs encoding.
      body = el.dataset.lcBody;
    } else {
      // Plan I, Task 5: the generic collector — draw's `data-vessel`,
      // react's `data-card-id`/`data-play`, haunt's `data-play` alone. Any
      // button lacking a given attribute just contributes nothing for it.
      var parts = [];
      if (el.dataset.vessel) parts.push("vessel=" + el.dataset.vessel);
      if (el.dataset.cardId) parts.push("card_id=" + encodeURIComponent(el.dataset.cardId));
      if (el.dataset.play) parts.push("play=" + el.dataset.play);
      body = parts.join("&");
    }
    post(action, body);
  }

  // One delegated change listener for the Lock-beat target picker.
  function onChange(e) {
    var sel = e.target.closest ? e.target.closest("select[data-lc-target]") : null;
    if (!sel) return;
    post(
      "target",
      "card_id=" + encodeURIComponent(sel.dataset.cardId) +
        "&target=" + encodeURIComponent(sel.value)
    );
  }

  // Plan C's contract: lc:arm/lc:disarm are dispatched by the wheel/armed
  // column BEFORE the wheel's glide settles — this listener must not assume
  // the wheel is at rest. Delegated once, never rebound.
  function onArm(e) {
    post("arm", "card_id=" + encodeURIComponent(e.detail.cardId)).then(function (ok) {
      if (!ok || !window.lcFlight) return;
      var face = e.target.querySelector && e.target.querySelector(".lc-cardface");
      window.lcFlight(e.target, window.lcAnchor("armed"), {
        direction: "play",
        scale: "dot",
        deck: face && face.dataset.deck,
      });
    });
  }

  function onDisarm(e) {
    post("disarm", "card_id=" + encodeURIComponent(e.detail.cardId));
  }

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

  // The public-frame motion pass (the live beat timer this function once
  // armed died with the beat clock, 2026-08-13): a beat flip to "reveal"
  // fires the E.1 flights from every locking seat to the felt, a
  // `data-draws` increase fires a deck-to-seat flight, and a `data-hp`
  // decrease shakes that seat's plaque — all diffed against `prev`, the
  // previous call's own snapshot.
  window.lcLoopPublic = function () {
    var next = snapshot();
    if (prev.beat !== "reveal" && next.beat === "reveal") fireRevealFlights();
    fireDrawFlights(next);
    fireHits(next);
    prev = next;
  };

  // Double-injection guard: binds the four delegated listeners exactly once
  // (they never need rebinding — see the file banner) and arms the
  // server-rendered banner's timer on first load.
  function init() {
    if (window.__lcLoopBound) return;
    window.__lcLoopBound = true;
    document.body.addEventListener("click", onClick);
    document.body.addEventListener("change", onChange);
    document.body.addEventListener("lc:arm", onArm);
    document.body.addEventListener("lc:disarm", onDisarm);
    window.lcLoopPublic();
  }
  document.addEventListener("DOMContentLoaded", init);
})();
