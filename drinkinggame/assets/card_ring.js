// Ring of Fire: tap a face-down card on the phone's table to draw.
//
// With 52 cards round a phone-sized ring the cards overlap, so a tap
// picks the card whose centre is nearest the finger rather than whichever
// one the browser hit-tests on top. The request is the DRAW button's,
// plus that card's slot: POST <data-draw-url> with slot=N. The slot only
// decides which card leaves the ring; the server still deals the next
// card in the deck.
//
// The card goes at once (is-taken) so the tap feels immediate; the next
// game frame redraws the ring from the server either way. Listener on the
// document, so a fresh ring from an SSE swap needs no re-binding.
(function () {
  if (window.cardRingBound) return; // loaded twice: keep the first binding
  window.cardRingBound = true;
  const REACH = 40; // px: a tap further than this from every card misses

  function nearestCard(ring, x, y) {
    let best = null, bestD = REACH;
    ring.querySelectorAll(".ring-card:not(.is-taken)").forEach(c => {
      const r = c.getBoundingClientRect();
      const d = Math.hypot(r.left + r.width / 2 - x, r.top + r.height / 2 - y);
      if (d < bestD) { best = c; bestD = d; }
    });
    return best;
  }

  function showError(text) {
    const el = document.getElementById("game-error");
    if (!el) return;
    el.innerHTML = text;
    setTimeout(() => { el.innerHTML = ""; }, 4000);
  }

  document.addEventListener("click", ev => {
    if (!ev.target.closest(".ring-card")) return;
    const ring = ev.target.closest("[data-card-ring][data-draw-url]");
    if (!ring) return;
    const card = nearestCard(ring, ev.clientX, ev.clientY);
    if (!card) return;
    card.classList.add("is-taken");
    fetch(ring.dataset.drawUrl, {
      method: "POST",
      body: new URLSearchParams({ slot: card.dataset.slot }),
      credentials: "same-origin",
    })
      .then(res => {
        if (res.ok) return;
        card.classList.remove("is-taken");
        return res.text().then(showError);
      })
      .catch(() => card.classList.remove("is-taken"));
  });
})();
