// Sorting & Loading Assistant — the session list at /sorting.
//
// The board itself is not in here any more. It is sorting-model.js,
// sorting-board.js and sorting-runtime.js — the same files the demo in
// docs/design/sorting-live and the design document run — booted by
// sorting-app.js. What is left here is the one screen those do not draw: the
// list of routes, and the box a plan is pasted into.
//
// `boot()` is safe to run any number of times: hx-boost swaps body children
// without a page load, so this file is re-executed on every boosted navigation
// back to the list, and DOMContentLoaded fires only on the very first one.

(function () {
  'use strict';

  // ── The session list (/sorting) ─────────────────────────────────────────

  function initIndex() {
    var file = document.getElementById('sort-file');
    if (file && !file.dataset.bound) {
      file.dataset.bound = '1';
      file.addEventListener('change', function () {
        var f = file.files && file.files[0];
        if (!f) return;
        var reader = new FileReader();
        reader.onload = function () {
          var box = document.getElementById('sort-payload');
          if (box) {
            box.value = String(reader.result || '');
            box.focus();
          }
        };
        reader.readAsText(f);
      });
    }

    var buttons = document.querySelectorAll('[data-delete-session]');
    for (var i = 0; i < buttons.length; i++) {
      (function (btn) {
        if (btn.dataset.bound) return;
        btn.dataset.bound = '1';
        btn.addEventListener('click', function () {
          var id = btn.getAttribute('data-delete-session');
          var label = btn.getAttribute('data-route') || 'this session';
          if (!window.confirm('Delete ' + label + '? The plan and its progress go with it.')) return;
          btn.disabled = true;
          fetch('/api/sorting/sessions/' + id, { method: 'DELETE' })
            .then(function (r) {
              if (!r.ok) throw new Error('delete failed');
              var row = document.getElementById('sort-session-' + id);
              if (row) row.remove();
            })
            .catch(function () {
              btn.disabled = false;
              window.alert('Could not delete that session — try again when you have signal.');
            });
        });
      })(buttons[i]);
    }
  }

  // ── Boot ────────────────────────────────────────────────────────────────

  function boot() {
    initIndex();
  }

  if (!window.__sortingBound) {
    window.__sortingBound = true;
    document.addEventListener('htmx:afterSwap', boot);
  }

  // `defer` guarantees the document is parsed, so this covers the first load;
  // the afterSwap listener covers every boosted one.
  boot();
})();
