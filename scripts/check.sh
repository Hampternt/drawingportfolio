#!/usr/bin/env bash
# The fast ITEM gate for this workspace (seconds, not minutes).
#
# The workflow gates in two tiers:
#   item gate  — this script: does it compile, is the JS syntactically valid.
#                Run after every item, before its commit.
#   pack gate  — ./scripts/verify.sh: fmt + clippy + full workspace test suite
#                + JS. Run before a pack's review/walkthrough; nothing merges
#                without it.
#
# This script deliberately skips fmt, clippy and the test suite — they are
# pack-gate business. Two things it can NOT vouch for:
#   * logic — if the item touches logic (db functions, game rules, auth/
#     visibility, nutrition math), additionally run that area's targeted
#     tests, e.g.:  SQLX_OFFLINE=true cargo test -p drinkinggame rooms::
#   * drinkinggame SQL — runtime-checked by sqlx (no compile-time safety);
#     only its tests exercise those queries. Touch its SQL → run its tests.
#
# Exit 0 = compiles + JS clean. Any failure prints the failing check's output
# and the script exits 1 after running the rest, so one run shows every problem.

set -uo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1

failed=()

printf '=== cargo check\n'
if ! SQLX_OFFLINE=true cargo check --workspace --all-targets; then
  failed+=("cargo check")
fi

printf '\n=== node --check (static JS)\n'
if command -v node >/dev/null 2>&1; then
  js_bad=0
  for f in static/*.js drinkinggame/assets/*.js; do
    [[ -e "$f" ]] || continue
    if node --check "$f"; then
      printf 'ok   %s\n' "$f"
    else
      printf 'FAIL %s\n' "$f"
      js_bad=1
    fi
  done
  (( js_bad == 0 )) || failed+=("node --check")
else
  printf 'SKIPPED — node not on PATH\n'
fi

printf '\n'
if (( ${#failed[@]} == 0 )); then
  printf 'CHECK OK — compiles, JS syntax clean. (Logic touched? Run targeted tests too.)\n'
  exit 0
fi

printf 'CHECK FAILED: %s\n' "${failed[*]}"
exit 1
