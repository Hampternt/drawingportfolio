// Last Call loop wiring (Plan E; Pack 1 lc-mobile-play-flow). The F.1
// action bar's `data-lc-post` buttons, Plan C's `lc:arm`/`lc:disarm`
// CustomEvents, and Pack 1's tray/targeting-overlay/ARMED-stack surface
// all funnel through here — one delegated listener per event type, bound
// once on `document.body`, so nothing here needs rebinding when a repaint
// (lcApply, lcApplyTable, lc_screen.html's lcpublic swap) replaces the DOM
// it targets. Task 5 adds flights/hits on top of the same globals.
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
    if (!e.target.closest) return;
    // Pack 1: a tab switch changes which pane is measurable — redraw the
    // table's arrows once the new pane has laid out. No return: the tab
    // buttons carry no other behaviour here.
    if (e.target.closest("[data-lc-tab]")) {
      window.requestAnimationFrame(function () {
        restingMode();
        if (window.lcTableSync) window.lcTableSync();
      });
    }
    // Pack 2: the inspect sheet's PLAY row — stage the card and jump to
    // the TABLE tab's targeting overlay (rAF: the pane must lay out first).
    var toStage = e.target.closest("[data-lc-sheet-tostage]");
    if (toStage) {
      var stageId = toStage.dataset.cardId;
      closeSheet();
      var tableTab = document.querySelector('[data-lc-tab="table"]');
      if (tableTab) tableTab.click();
      window.requestAnimationFrame(function () {
        openTargeting(stageId);
      });
      return;
    }
    if (e.target.closest("[data-lc-sheet-close]")) {
      closeSheet();
      return;
    }
    // Pack 3: the mulligan overlay — open / pick / cancel / confirm.
    if (e.target.closest("[data-lc-mull-open]")) {
      openMulligan();
      return;
    }
    var mullCard = e.target.closest(".lc-mull-card");
    if (mullCard) {
      var at = mullPicks.indexOf(mullCard);
      if (at > -1) mullPicks.splice(at, 1);
      else mullPicks.push(mullCard);
      var overlay = mullCard.closest("[data-lc-mull]");
      if (overlay) mullSync(overlay);
      return;
    }
    if (e.target.closest("[data-lc-mull-cancel]")) {
      closeMulligan();
      return;
    }
    if (e.target.closest("[data-lc-mull-confirm]")) {
      var ids = mullPicks.map(function (card) { return card.dataset.cardId; });
      if (ids.length) {
        post("mulligan", "cards=" + encodeURIComponent(ids.join(",")));
      }
      closeMulligan();
      return;
    }
    // Pack 2: the side-quest drawer's handle toggles it out and back.
    var handle = e.target.closest("[data-lc-tabdrawer]");
    if (handle) {
      var drawer = handle.closest(".lc-tabcard");
      if (drawer) {
        if (drawer.hasAttribute("data-open")) {
          drawer.removeAttribute("data-open");
        } else {
          drawer.setAttribute("data-open", "");
        }
      }
      return;
    }
    // Pack 1: overlay row -> commit; overlay backdrop -> cancel; stack
    // mini -> take-back (locked stacks are committed, no take-back).
    var row = e.target.closest(".lc-tgt-row");
    if (row) {
      chooseTarget(row);
      return;
    }
    if (e.target.closest("[data-lc-overlay]")) {
      closeTargeting();
      return;
    }
    var stackMini = e.target.closest(".lc-stack-mini");
    if (stackMini) {
      if (!stackMini.closest("[data-locked]")) {
        post("disarm", "card_id=" + encodeURIComponent(stackMini.dataset.cardId));
      }
      return;
    }
    var el = e.target.closest("[data-lc-post]");
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

  // Pack 1 (lc-mobile-play-flow) retired the Lock-beat `<select>` target
  // picker and its delegated change listener — targeting now happens in the
  // TABLE tab's overlay below, which posts `arm` then `target` in one
  // gesture.

  // ---- Pack 1: the TABLE tab's tray / targeting overlay / ARMED stack ----
  //
  // Same delegation discipline as everything above: pointer + click
  // listeners bound once on document.body, transient state (the staged
  // card, the live drag) in module scope, and `lcTableSync` re-derives the
  // visual state from whatever DOM the latest repaint left behind.

  var trayState = { staged: null };
  var trayDrag = null;

  function tablePane() {
    return document.querySelector('[data-lc-pane="table"]');
  }

  function overlayEl() {
    var pane = tablePane();
    return pane ? pane.querySelector("[data-lc-overlay]") : null;
  }

  function trayMini(cardId) {
    var pane = tablePane();
    return pane
      ? pane.querySelector('.lc-tray-mini[data-card-id="' + cardId + '"]')
      : null;
  }

  function closeTargeting() {
    trayState.staged = null;
    restingMode();
    var pane = tablePane();
    if (!pane) return;
    pane.querySelectorAll(".lc-tray-mini.is-staged").forEach(function (m) {
      m.classList.remove("is-staged");
    });
    var ov = overlayEl();
    if (ov) ov.hidden = true;
  }

  // Stage a card and open the overlay showing only the rows its `targets`
  // class admits: "one" -> every seat row, "self" -> the viewer's own row,
  // anything else ("all"/"table") -> the EVERYONE row. Reactions never arm
  // (the engine refuses them — they play in the Reveal response window), so
  // they get the note instead of a doomed POST.
  function openTargeting(cardId) {
    var mini = trayMini(cardId);
    var ov = overlayEl();
    if (!mini || !ov) return;
    if (mini.dataset.kind === "reaction") {
      note("REACTIONS PLAY AT THE REVEAL — HOLD ONTO IT");
      return;
    }
    var targets = mini.dataset.targets;
    trayState.staged = cardId;
    tablePane().querySelectorAll(".lc-tray-mini").forEach(function (m) {
      m.classList.toggle("is-staged", m === mini);
    });
    ov.querySelectorAll(".lc-tgt-row").forEach(function (row) {
      var t = row.dataset.target;
      var show = targets === "one" ? t !== "all"
        : targets === "self" ? row.hasAttribute("data-me")
          : t === "all";
      row.hidden = !show;
      row.classList.remove("is-over");
    });
    var pv = ov.querySelector("[data-lc-preview]");
    var src = tablePane().querySelector('[data-preview-for="' + cardId + '"]');
    if (pv) pv.innerHTML = src ? src.innerHTML : "";
    ov.hidden = false;
    setMode("target");
  }

  // The commit: POST arm, then (for a seat-targeted card) POST target with
  // the chosen seat, then fire the decorative arm flash. The tick repaint
  // carries the real armed queue back — nothing here mutates game state
  // client-side.
  function chooseTarget(row) {
    var id = trayState.staged;
    var mini = id && trayMini(id);
    if (!mini || !row) {
      closeTargeting();
      return;
    }
    var targets = mini.dataset.targets;
    var deck = mini.dataset.deck;
    var targetVal = row.dataset.target;
    var nameEl = row.querySelector(".lc-tgt-name");
    var label = targetVal === "all" ? "EVERYONE"
      : row.hasAttribute("data-me") ? "YOURSELF"
        : (nameEl ? nameEl.textContent : "");
    var src = tablePane().querySelector('[data-preview-for="' + id + '"]');
    var previewHTML = src ? src.innerHTML : "";
    post("arm", "card_id=" + encodeURIComponent(id)).then(function (ok) {
      if (!ok) return;
      if (targets === "one" && targetVal !== "all") {
        post(
          "target",
          "card_id=" + encodeURIComponent(id) +
            "&target=" + encodeURIComponent(targetVal)
        );
      }
      if (window.lcArmFlash) {
        window.lcArmFlash({
          deck: deck,
          caption: "YOU → " + label,
          previewHTML: previewHTML,
        });
      }
      // the badge narrates the flash, then falls back to the tab's rest
      setMode("arming", deck);
      window.setTimeout(restingMode, 1600);
    });
    closeTargeting();
  }

  function onTrayPointerDown(e) {
    var mini = e.target.closest ? e.target.closest(".lc-tray-mini") : null;
    if (!mini) return;
    try { mini.setPointerCapture(e.pointerId); } catch (_) {}
    trayDrag = {
      id: mini.dataset.cardId,
      x0: e.clientX,
      y0: e.clientY,
      moved: false,
      ghost: null,
      over: null,
    };
  }

  function onTrayPointerMove(e) {
    var d = trayDrag;
    if (!d) return;
    if (!d.moved && Math.hypot(e.clientX - d.x0, e.clientY - d.y0) < 10) return;
    d.moved = true;
    var mini = trayMini(d.id);
    if (!mini || mini.dataset.kind === "reaction") return;
    if (trayState.staged !== d.id) openTargeting(d.id);
    if (!d.ghost) {
      var layer = document.getElementById("lc-flights");
      if (layer) {
        var g = document.createElement("div");
        g.className = "lc-tray-ghost" +
          (mini.dataset.deck ? " lc-deck-" + mini.dataset.deck : "");
        var cost = document.createElement("span");
        cost.className = "lc-tray-cost";
        cost.textContent = mini.dataset.cost || "";
        var title = document.createElement("span");
        title.className = "lc-tray-title";
        var titleEl = mini.querySelector(".lc-tray-title");
        title.textContent = titleEl ? titleEl.textContent : "";
        g.appendChild(cost);
        g.appendChild(title);
        layer.appendChild(g);
        d.ghost = g;
      }
    }
    if (d.ghost) {
      var lr = d.ghost.parentNode.getBoundingClientRect();
      d.ghost.style.left = (e.clientX - lr.left) + "px";
      d.ghost.style.top = (e.clientY - lr.top) + "px";
    }
    var over = null;
    var ov = overlayEl();
    if (ov) {
      ov.querySelectorAll(".lc-tgt-row:not([hidden])").forEach(function (row) {
        var rr = row.getBoundingClientRect();
        var hit = e.clientX >= rr.left && e.clientX <= rr.right &&
          e.clientY >= rr.top && e.clientY <= rr.bottom;
        row.classList.toggle("is-over", hit);
        if (hit) over = row;
      });
    }
    d.over = over;
  }

  function onTrayPointerUp(e) {
    var d = trayDrag;
    trayDrag = null;
    if (!d) return;
    if (d.ghost) d.ghost.remove();
    // Same rule as the wheel (finding 5): only a genuine pointerup commits;
    // a cancelled gesture cleans up without staging or dropping anything.
    if (e.type !== "pointerup") {
      if (d.moved) closeTargeting();
      return;
    }
    if (d.moved) {
      if (d.over) {
        trayState.staged = d.id;
        chooseTarget(d.over);
      } else {
        closeTargeting();
      }
    } else if (trayState.staged === d.id) {
      closeTargeting(); // second tap un-stages
    } else {
      openTargeting(d.id);
    }
  }

  // Re-derive the play surface's visual state after a repaint (lcApplyTable
  // replaces #lc-table wholesale) or a tab switch: redraw the persistent
  // arrows, and either restore or drop the targeting overlay depending on
  // whether the staged card still sits in the fresh tray.
  window.lcTableSync = function () {
    if (window.lcTableArrows) window.lcTableArrows();
    flashChips();
    if (!trayState.staged) return;
    if (trayMini(trayState.staged)) {
      openTargeting(trayState.staged);
    } else {
      // the staged card left the tray (armed, or the hand changed under us)
      trayState.staged = null;
      var ov = overlayEl();
      if (ov) ov.hidden = true;
    }
  };

  // Pack 2 / D3 (lc-mobile-play-flow): the wheel's tap now dispatches
  // lc:inspect — reading, not arming. The old lc:arm listener (instant-arm,
  // and the Draw-beat tap-to-swap it also carried) retires with the
  // gesture; playing lives in the TABLE tray/overlay, and the swap moves
  // into Pack 3's mulligan overlay. lc:disarm stays: the hand pane's armed
  // column still takes a tap back.
  function onInspect(e) {
    openSheet(e.detail.cardId);
  }

  function onDisarm(e) {
    post("disarm", "card_id=" + encodeURIComponent(e.detail.cardId));
  }

  // ---- Pack 2: inspect sheet, side-quest drawer, mode badge ------------

  function sheetEl() {
    var pane = document.querySelector('[data-lc-pane="hand"]');
    return pane ? pane.querySelector("[data-lc-sheet]") : null;
  }

  // Clone the tapped card's stash entry (expanded face + meta grid +
  // actions, all server-rendered) into the sheet slot and lift the sheet.
  function openSheet(cardId) {
    var sheet = sheetEl();
    if (!sheet) return;
    var src = sheet.querySelector('[data-inspect-for="' + cardId + '"]');
    var slot = sheet.querySelector("[data-lc-sheet-slot]");
    if (!src || !slot) return;
    slot.innerHTML = src.innerHTML;
    sheet.hidden = false;
  }

  function closeSheet() {
    var sheet = sheetEl();
    if (!sheet) return;
    sheet.hidden = true;
    var slot = sheet.querySelector("[data-lc-sheet-slot]");
    if (slot) slot.innerHTML = "";
  }

  // ---- Pack 3: the mulligan overlay --------------------------------------

  var mullPicks = []; // picked .lc-mull-card elements, in pick order

  function mullEl() {
    var pane = document.querySelector('[data-lc-pane="hand"]');
    return pane ? pane.querySelector("[data-lc-mull]") : null;
  }

  function mullSync(overlay) {
    overlay.querySelectorAll(".lc-mull-card").forEach(function (card) {
      var i = mullPicks.indexOf(card);
      card.classList.toggle("is-picked", i > -1);
      var badge = card.querySelector(".lc-mull-badge");
      if (badge) {
        badge.hidden = i === -1;
        badge.textContent = i === -1 ? "" : String(i + 1);
      }
    });
    var count = overlay.querySelector("[data-lc-mull-count]");
    if (count) count.textContent = String(mullPicks.length);
    var confirm = overlay.querySelector("[data-lc-mull-confirm]");
    if (confirm) {
      confirm.disabled = mullPicks.length === 0;
      confirm.textContent = mullPicks.length
        ? "SWAP " + mullPicks.length +
          (mullPicks.length === 1 ? " CARD" : " CARDS")
        : "SWAP CARDS";
    }
  }

  function openMulligan() {
    var overlay = mullEl();
    if (!overlay) return;
    mullPicks = [];
    mullSync(overlay);
    overlay.hidden = false;
    setMode("mulligan");
  }

  function closeMulligan() {
    var overlay = mullEl();
    if (overlay) overlay.hidden = true;
    mullPicks = [];
    restingMode();
  }

  // ---- Pack 3: mini-table chip flashes (hit shake / heal tick) ----------

  var prevChipHp = {};

  // Diffs each seat chip's data-hp against the previous repaint and plays
  // the hit shake / heal flash — the phone-chip analogue of fireHits'
  // plaque path. Called from lcTableSync, so every table repaint diffs
  // exactly once; the class rides off on animationend like fireHits'.
  function flashChips() {
    var next = {};
    document
      .querySelectorAll('.lc-minitable-chip[data-seat]')
      .forEach(function (chip) {
        var seat = chip.dataset.seat;
        var hp = Number(chip.dataset.hp);
        next[seat] = hp;
        var was = prevChipHp[seat];
        if (was === undefined || hp === was) return;
        var cls = hp < was ? "is-hit" : "is-good";
        chip.classList.add(cls);
        chip.addEventListener("animationend", function onEnd(e) {
          if (e.animationName !== "lc-hp-flash" && e.animationName !== "lc-hp-good") return;
          chip.classList.remove(cls);
          chip.removeEventListener("animationend", onEnd);
        });
      });
    prevChipHp = next;
  }

  // The tab row's mode badge: one word for what the player is doing.
  // Deck-tinted for ARMING via the lc-deck-* class (CSS resolves the ink).
  function setMode(mode, deck) {
    var badge = document.getElementById("lc-mode-badge");
    if (!badge) return;
    badge.textContent = mode.toUpperCase();
    badge.dataset.mode = mode;
    badge.className = "lc-mode-badge" + (deck ? " lc-deck-" + deck : "");
  }

  function activeTab() {
    var sel = document.querySelector('[data-lc-tab][aria-selected="true"]');
    return sel ? sel.dataset.lcTab : "hand";
  }

  // The badge's at-rest state follows the active tab.
  function restingMode() {
    var t = activeTab();
    setMode(t === "hand" ? "read" : t === "table" ? "play" : "log");
  }

  // Moves the private hand fetch's <template data-lc-actions> (a sibling of
  // #lc-hand, not a descendant — same reason the setup form's END GAME
  // button lives outside #lc-table) into the shell's persistent
  // .lc-actions, then discards the template. Never no-ops silently on a
  // missing bar: absence just means this fetch carried no template (the
  // route is unreachable without one), so nothing to relocate.
  window.lcLoopApply = function (pane) {
    // Pack 2: the tab row's pull count rides every private repaint —
    // updated BEFORE the template guard below, which returns early.
    var handRoot = pane && pane.querySelector("#lc-hand");
    var pulls = handRoot ? handRoot.dataset.pulls : undefined;
    var pullEl = document.getElementById("lc-mode-pulls");
    if (pullEl && pulls !== undefined) {
      pullEl.textContent = pulls + " PULLS";
      pullEl.hidden = false;
    }
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
    document.body.addEventListener("lc:inspect", onInspect);
    document.body.addEventListener("lc:disarm", onDisarm);
    // Pack 1: the tray's tap/drag surface — delegated like everything else;
    // setPointerCapture keeps the move/up stream flowing through the mini
    // (and bubbling here) even when the finger leaves it.
    document.body.addEventListener("pointerdown", onTrayPointerDown);
    document.body.addEventListener("pointermove", onTrayPointerMove);
    document.body.addEventListener("pointerup", onTrayPointerUp);
    document.body.addEventListener("pointercancel", onTrayPointerUp);
    window.lcLoopPublic();
    restingMode();
    // the initial page carries no <template data-lc-actions>, so this only
    // seeds the pull count off the server-rendered #lc-hand
    window.lcLoopApply(document.querySelector('[data-lc-pane="hand"]'));
    if (window.lcTableSync) window.lcTableSync();
  }
  document.addEventListener("DOMContentLoaded", init);
})();
