// Art feed keyboard layer — `/` search, `Esc` leave, `J` / `K` walk the cards.
//
// Inert on every page without an #art-search field or .hm-post cards, which is
// why it can live in both shells.
//
// hx-boost replaces body children without a reload, so DOMContentLoaded fires
// once per real page load only. This binds on htmx:afterSwap too and guards
// against a second listener, the pattern static/palette.js uses.

function artfeedCards() {
  // Queried fresh every keypress: HTMX replaces #feed's contents on every
  // search, so a cached list would point at detached nodes.
  return Array.prototype.slice.call(document.querySelectorAll('#feed .hm-post'));
}

function artfeedStep(delta) {
  const cards = artfeedCards();
  if (!cards.length) return;

  const active = document.activeElement;
  const current = active && active.closest ? active.closest('.hm-post') : null;
  let idx = current ? cards.indexOf(current) : -1;

  if (idx < 0) {
    // Nothing focused yet: J starts at the top, K at the bottom.
    idx = delta > 0 ? 0 : cards.length - 1;
  } else {
    idx = Math.min(cards.length - 1, Math.max(0, idx + delta));
  }

  const card = cards[idx];
  card.focus({ preventScroll: true });
  const reduce = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  card.scrollIntoView({ block: 'nearest', behavior: reduce ? 'auto' : 'smooth' });
}

function artfeedIsTyping(el) {
  if (!el) return false;
  const tag = el.tagName;
  return tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT' || el.isContentEditable;
}

function artfeedInit() {
  // Already bound (normal page load or a previous boost navigation)
  if (window.artfeedBound) return;
  window.artfeedBound = true;

  document.addEventListener('keydown', function (e) {
    // Leave chords alone so Ctrl+K still reaches the command palette.
    if (e.ctrlKey || e.metaKey || e.altKey) return;

    const search = document.getElementById('art-search');
    const active = document.activeElement;

    // Esc is handled before the typing guard on purpose: leaving the field is
    // the one thing that has to work while the field has focus.
    if (e.key === 'Escape' && search && active === search) {
      search.blur();
      return;
    }

    if (artfeedIsTyping(active)) return;

    if (e.key === '/' && search) {
      // Without preventDefault the slash lands in the field it just focused.
      e.preventDefault();
      search.focus();
      search.select();
      return;
    }

    if (e.key === 'j' || e.key === 'J') {
      e.preventDefault();
      artfeedStep(1);
    } else if (e.key === 'k' || e.key === 'K') {
      e.preventDefault();
      artfeedStep(-1);
    } else if (e.key === 'v' || e.key === 'V') {
      // Admin-only in effect rather than by a check: a visitor's page never
      // renders this link, so the lookup returns null and the key does nothing.
      // The same element id serves both directions — entering the preview and
      // leaving it — so this one branch toggles.
      const toggle = document.getElementById('art-visitor-toggle');
      if (toggle) {
        e.preventDefault();
        toggle.click();
      }
    }
  });
}

// Initial page load
document.addEventListener('DOMContentLoaded', artfeedInit);
// hx-boost replaces <body> content without a full page reload — re-run after each swap
document.addEventListener('htmx:afterSwap', artfeedInit);
