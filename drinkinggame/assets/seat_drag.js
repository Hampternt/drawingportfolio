// Drag a seat on the phone's table to move that player there.
//
// Pointer events, not HTML5 drag-and-drop, which phones don't fire on
// touch. The drop sends the same request as the ROOM tab's seat dropdown:
// POST <data-reorder-url> with player_id + 1-based position.
//
// Every `room` frame replaces the table's markup, which would destroy the
// chip mid-drag. So while a drag is live, room.html parks the newest frame
// in `seatDrag.pending` and this script applies it when the drag ends.
// Listeners sit on the document, so a fresh table needs no re-binding.
(function () {
  if (window.seatDrag) return; // loaded twice: keep the first binding
  const drag = (window.seatDrag = { active: false, pending: null });
  const THRESHOLD = 6; // px of travel before a press becomes a drag
  const DROP_RADIUS = 56; // px from a seat's centre that counts as on it
  let press = null; // { seat, id, url, x, y, pointerId }
  let ghost = null;
  let target = null;

  function liveSeats(url) {
    return Array.from(
      document.querySelectorAll('[data-seat-table][data-reorder-url] .felt-seat')
    ).filter(s => s.closest("[data-seat-table]").dataset.reorderUrl === url);
  }

  // Nearest seat to the pointer, resolved against the live DOM every move.
  function seatAt(x, y) {
    let best = null, bestD = DROP_RADIUS;
    for (const s of liveSeats(press.url)) {
      if (s.dataset.playerId === press.id) continue;
      const r = s.getBoundingClientRect();
      const d = Math.hypot(r.left + r.width / 2 - x, r.top + r.height / 2 - y);
      if (d < bestD) { best = s; bestD = d; }
    }
    return best;
  }

  function setTarget(s) {
    if (target === s) return;
    if (target) target.classList.remove("is-drop-target");
    target = s;
    if (target) target.classList.add("is-drop-target");
  }

  function begin() {
    drag.active = true;
    press.seat.classList.add("is-dragging");
    ghost = document.createElement("div");
    ghost.className = "felt-ghost";
    ghost.innerHTML = press.seat.innerHTML;
    document.body.appendChild(ghost);
  }

  function finish() {
    if (ghost) ghost.remove();
    ghost = null;
    setTarget(null);
    if (press && press.seat.isConnected) press.seat.classList.remove("is-dragging");
    press = null;
    drag.active = false;
    if (drag.pending !== null && window.applyRoomFrame) {
      const html = drag.pending;
      drag.pending = null;
      window.applyRoomFrame(html);
    }
  }

  function send(url, playerId, position) {
    const body = new URLSearchParams({ player_id: playerId, position: position });
    fetch(url, { method: "POST", body: body, credentials: "same-origin" })
      .then(res => {
        if (res.ok) return;
        return res.text().then(t => {
          const el = document.getElementById("game-error");
          if (!el) return;
          el.innerHTML = t;
          setTimeout(() => { el.innerHTML = ""; }, 4000);
        });
      })
      .catch(() => {});
  }

  document.addEventListener("pointerdown", ev => {
    const seat = ev.target.closest(".felt-seat[data-drag]");
    if (!seat || press) return;
    const table = seat.closest("[data-seat-table][data-reorder-url]");
    if (!table) return;
    press = {
      seat: seat,
      id: seat.dataset.playerId,
      url: table.dataset.reorderUrl,
      x: ev.clientX,
      y: ev.clientY,
      pointerId: ev.pointerId,
    };
    try { seat.setPointerCapture(ev.pointerId); } catch (_) { /* detached */ }
  });

  document.addEventListener("pointermove", ev => {
    if (!press || ev.pointerId !== press.pointerId) return;
    if (!drag.active) {
      if (Math.hypot(ev.clientX - press.x, ev.clientY - press.y) < THRESHOLD) return;
      begin();
    }
    ev.preventDefault();
    ghost.style.left = ev.clientX + "px";
    ghost.style.top = ev.clientY + "px";
    setTarget(seatAt(ev.clientX, ev.clientY));
  });

  document.addEventListener("pointerup", ev => {
    if (!press || ev.pointerId !== press.pointerId) return;
    if (drag.active && target) send(press.url, press.id, target.dataset.position);
    finish();
  });

  document.addEventListener("pointercancel", ev => {
    if (!press || ev.pointerId !== press.pointerId) return;
    finish();
  });
})();
