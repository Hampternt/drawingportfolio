// Drag a token, or the start marker, round the phone's table. It slides
// along the rim under your finger and stays where you let go: the drop
// posts just its new angle (data-seat-url with player_id, or
// data-start-url for the marker), and the server re-derives seat order —
// clockwise from the marker — and broadcasts the new numbers.
//
// Pointer events, not HTML5 drag-and-drop, which phones don't fire on
// touch. Angles are degrees clockwise from the bottom of the table, the
// same convention and the same rim ellipse as seat_ring.rs.
//
// Every `room` frame replaces the table's markup, which would destroy the
// token mid-drag. So while a drag is live, room.html parks the newest frame
// in `seatDrag.pending` and this script applies it when the drag ends.
// Listeners sit on the document, so a fresh table needs no re-binding.
(function () {
  if (window.seatDrag) return; // loaded twice: keep the first binding
  const drag = (window.seatDrag = { active: false, pending: null });
  const THRESHOLD = 6; // px of travel before a press becomes a drag
  // Half-axes in % of the table box — seat_ring.rs's RIM and MARKER_RIM.
  const RIM = [43.0, 41.1];
  const MARKER_RIM = [47.0, 45.5];
  let press = null; // { el, table, isMarker, x, y, pointerId, angle }

  // The pointer's angle round the table's centre, corrected for the
  // ellipse so it matches where the server will put the token.
  function angleAt(table, x, y) {
    const r = table.getBoundingClientRect();
    const dx = ((x - r.left) / r.width) * 100 - 50;
    const dy = ((y - r.top) / r.height) * 100 - 50;
    const deg = (Math.atan2(-dx / RIM[0], dy / RIM[1]) * 180) / Math.PI;
    return ((deg % 360) + 360) % 360;
  }

  function placeOnRim(el, deg, axes) {
    const t = (deg * Math.PI) / 180;
    el.style.left = (50 - axes[0] * Math.sin(t)).toFixed(1) + "%";
    el.style.top = (50 + axes[1] * Math.cos(t)).toFixed(1) + "%";
  }

  function finish() {
    if (press && press.el.isConnected) press.el.classList.remove("is-dragging");
    press = null;
    drag.active = false;
    if (drag.pending !== null && window.applyRoomFrame) {
      const html = drag.pending;
      drag.pending = null;
      window.applyRoomFrame(html);
    }
  }

  function send(url, fields) {
    fetch(url, { method: "POST", body: new URLSearchParams(fields), credentials: "same-origin" })
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
    const el = ev.target.closest(".felt-seat[data-drag], .felt-marker[data-drag]");
    if (!el || press) return;
    const table = el.closest("[data-seat-table][data-seat-url]");
    if (!table) return;
    press = {
      el: el,
      table: table,
      isMarker: el.matches(".felt-marker"),
      x: ev.clientX,
      y: ev.clientY,
      pointerId: ev.pointerId,
      angle: null,
    };
    try { el.setPointerCapture(ev.pointerId); } catch (_) { /* detached */ }
  });

  document.addEventListener("pointermove", ev => {
    if (!press || ev.pointerId !== press.pointerId) return;
    if (!drag.active) {
      if (Math.hypot(ev.clientX - press.x, ev.clientY - press.y) < THRESHOLD) return;
      drag.active = true;
      press.el.classList.add("is-dragging");
    }
    ev.preventDefault();
    press.angle = angleAt(press.table, ev.clientX, ev.clientY);
    placeOnRim(press.el, press.angle, press.isMarker ? MARKER_RIM : RIM);
  });

  document.addEventListener("pointerup", ev => {
    if (!press || ev.pointerId !== press.pointerId) return;
    if (drag.active && press.angle !== null) {
      const angle = press.angle.toFixed(1);
      if (press.isMarker) send(press.table.dataset.startUrl, { angle: angle });
      else send(press.table.dataset.seatUrl, { player_id: press.el.dataset.playerId, angle: angle });
    }
    finish();
  });

  document.addEventListener("pointercancel", ev => {
    if (!press || ev.pointerId !== press.pointerId) return;
    finish();
  });
})();
