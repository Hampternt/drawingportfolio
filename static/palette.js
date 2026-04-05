// Command palette — Ctrl+K / Cmd+K to open.
// To add a command: push one object to COMMANDS.
// adminOnly commands are hidden when IS_ADMIN is false.

const COMMANDS = [
  {
    label: 'Upload new drawing',
    keywords: ['upload', 'post', 'new', 'image', 'add'],
    adminOnly: true,
    action() {
      const composer = document.getElementById('composer');
      if (composer) {
        composer.hidden = false;
        document.getElementById('new-post-btn')?.setAttribute('aria-expanded', 'true');
      } else {
        location.href = '/artportfolio';
      }
    },
  },
  {
    label: 'Go to Art Portfolio',
    keywords: ['feed', 'gallery', 'art', 'drawings', 'portfolio'],
    action() { location.href = '/artportfolio'; },
  },
  {
    label: 'Go to Hub',
    keywords: ['home', 'hub', 'index', 'start', 'main'],
    action() { location.href = '/'; },
  },
  {
    label: 'Admin panel',
    keywords: ['admin', 'settings', 'manage'],
    adminOnly: true,
    action() { location.href = '/admin'; },
  },
];

// ── Engine ────────────────────────────────────────

let paletteResults = [];
let selectedIdx = 0;

function paletteAvailable() {
  return COMMANDS.filter(c => !c.adminOnly || (typeof IS_ADMIN !== 'undefined' && IS_ADMIN));
}

function paletteFilter(query) {
  const q = query.toLowerCase().trim();
  if (!q) return paletteAvailable();
  return paletteAvailable().filter(c =>
    c.label.toLowerCase().includes(q) ||
    c.keywords.some(k => k.includes(q))
  );
}

function paletteRender() {
  const list = document.getElementById('palette-results');
  if (!list) return;
  while (list.firstChild) list.removeChild(list.firstChild);

  paletteResults.forEach((cmd, i) => {
    const el = document.createElement('div');
    el.className = 'palette-item' + (i === selectedIdx ? ' palette-selected' : '');
    el.dataset.i = String(i);
    el.textContent = cmd.label;           // textContent — never innerHTML for dynamic data
    el.addEventListener('mousedown', e => {
      e.preventDefault();                  // prevent input blur before action fires
      selectedIdx = i;
      paletteExecute();
    });
    list.appendChild(el);
  });
}

function paletteExecute() {
  const cmd = paletteResults[selectedIdx];
  if (!cmd) return;
  paletteClose();
  cmd.action();
}

function paletteOpen() {
  const overlay = document.getElementById('palette-overlay');
  if (!overlay) return;
  overlay.hidden = false;
  const input = document.getElementById('palette-input');
  input.value = '';
  selectedIdx = 0;
  paletteResults = paletteAvailable();
  paletteRender();
  input.focus();
}

function paletteClose() {
  const overlay = document.getElementById('palette-overlay');
  if (overlay) overlay.hidden = true;
}

// ── Keyboard ──────────────────────────────────────

document.addEventListener('keydown', e => {
  const overlay = document.getElementById('palette-overlay');
  const isOpen = overlay && !overlay.hidden;

  if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
    e.preventDefault();
    isOpen ? paletteClose() : paletteOpen();
    return;
  }
  if (!isOpen) return;

  if (e.key === 'Escape')    { e.preventDefault(); paletteClose(); return; }
  if (e.key === 'ArrowDown') { e.preventDefault(); selectedIdx = Math.min(selectedIdx + 1, paletteResults.length - 1); paletteRender(); return; }
  if (e.key === 'ArrowUp')   { e.preventDefault(); selectedIdx = Math.max(selectedIdx - 1, 0); paletteRender(); return; }
  if (e.key === 'Enter')     { e.preventDefault(); paletteExecute(); return; }
});

// ── DOM injection ─────────────────────────────────
// Build the overlay with DOM methods to avoid innerHTML with dynamic content.

document.addEventListener('DOMContentLoaded', () => {
  const overlay = document.createElement('div');
  overlay.id = 'palette-overlay';
  overlay.hidden = true;
  overlay.setAttribute('role', 'dialog');
  overlay.setAttribute('aria-modal', 'true');
  overlay.setAttribute('aria-label', 'Command palette');

  const box = document.createElement('div');
  box.id = 'palette-box';

  const input = document.createElement('input');
  input.id = 'palette-input';
  input.type = 'text';
  input.placeholder = 'Search commands\u2026';
  input.setAttribute('autocomplete', 'off');
  input.setAttribute('spellcheck', 'false');

  const results = document.createElement('div');
  results.id = 'palette-results';

  box.appendChild(input);
  box.appendChild(results);
  overlay.appendChild(box);
  document.body.appendChild(overlay);

  overlay.addEventListener('mousedown', e => {
    if (e.target === overlay) paletteClose();
  });

  input.addEventListener('input', e => {
    selectedIdx = 0;
    paletteResults = paletteFilter(e.target.value);
    paletteRender();
  });
});
