// ── where the settings live between mornings ─────────────────────────────────
// Browser-only, and the only file here that is: model.js and board.js run under
// node in the tests and must stay that way.
//
// localStorage is not always there. A private window, cookies blocked, or a
// browser set to refuse site data can make even the property access throw — so
// every path in here fails to "no stored settings" rather than to a blank
// screen. The board is worth more than the preferences.
var SETTINGS_KEY = 'sorting-live.settings';

function storageWorks() {
  try {
    var probe = SETTINGS_KEY + '.probe';
    window.localStorage.setItem(probe, '1');
    window.localStorage.removeItem(probe);
    return true;
  } catch (e) { return false; }
}
function loadStored() {
  try { return window.localStorage.getItem(SETTINGS_KEY); } catch (e) { return null; }
}
// null removes the key, so a board back on its defaults leaves nothing behind
// and a fresh browser and a reset one are the same browser.
function saveStored(text) {
  try {
    if (text === null) window.localStorage.removeItem(SETTINGS_KEY);
    else window.localStorage.setItem(SETTINGS_KEY, text);
    return true;
  } catch (e) { return false; }
}
