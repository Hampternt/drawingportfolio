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

function artfeedPaletteOpen() {
  // palette.js's own overlay — its keydown listener does not stopPropagation
  // on Escape, so without this guard one Esc press would close the palette
  // AND a card popover at once instead of one layer per press.
  const overlay = document.getElementById('palette-overlay');
  return !!overlay && !overlay.hidden;
}

function artfeedInit() {
  // Already bound (normal page load or a previous boost navigation)
  if (window.artfeedBound) return;
  window.artfeedBound = true;

  document.addEventListener('click', function (e) {
    const trigger = e.target.closest('[data-art-pop]');
    const openPop = document.querySelector('.art-pop:not([hidden])');

    if (trigger) {
      const card = trigger.closest('.hm-post');
      const pop = card && card.querySelector('.art-pop');
      if (!pop) return;
      const kind = trigger.getAttribute('data-art-pop');
      // A different trigger on the SAME card's already-open popover is a
      // switch, not a close: the hx-get already refills the shared
      // container with the new fragment, so toggling `hidden` here would
      // just hide it again and cost the click a second press to reopen.
      const switching = pop === openPop && !pop.hidden && pop.dataset.current !== kind;
      const wasOpen = pop === openPop && !pop.hidden && !switching;
      // Only one popover open at a time: close whichever other one is open
      // before (possibly) opening this card's own.
      if (openPop && openPop !== pop) openPop.hidden = true;
      pop.hidden = wasOpen; // toggle: was open (same trigger) -> hide, switching or closed -> show
      pop.dataset.current = kind;
      return;
    }

    // Click outside any popover and outside a trigger button closes the
    // open popover, if there is one.
    if (openPop && !e.target.closest('.art-pop')) {
      openPop.hidden = true;
    }
  });

  document.addEventListener('keydown', function (e) {
    // Leave chords alone so Ctrl+K still reaches the command palette.
    if (e.ctrlKey || e.metaKey || e.altKey) return;

    const search = document.getElementById('art-search');
    const active = document.activeElement;

    // Popover Esc is handled before the search-field Esc branch: one popover
    // open at a time, so one Esc closes it and the next leaves the search
    // field. This runs before artfeedIsTyping too, so Esc works while focus
    // is inside the popover's own textarea. Guarded on the palette NOT being
    // open, or a single Esc press would close the palette and a popover at
    // once — see artfeedPaletteOpen.
    const openPop = document.querySelector('.art-pop:not([hidden])');
    if (e.key === 'Escape' && openPop && !artfeedPaletteOpen()) {
      openPop.hidden = true;
      return;
    }

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
