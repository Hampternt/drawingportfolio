#!/usr/bin/env bash
# The verification gate for this workspace.
#
# Every task's acceptance line is `./scripts/verify.sh` — run it, quote its
# output. It is the deterministic half of the review policy in
# .claude/skills/plan-economics: anything this script can decide is never
# worth a model's attention.
#
# Checks:
#   1. cargo fmt --check          — formatting
#   2. cargo clippy               — lints (NOT -D warnings; see below)
#   3. cargo test (SQLX_OFFLINE)  — the workspace suite, including the
#                                   static-asset guards in tests/static_assets.rs
#   4. node --check static/*.js drinkinggame/assets/*.js — JS syntax (a nested
#                                   palette entry broke palette.js once,
#                                   c72d614; nothing else catches it)
#   5. the board suites                — 432 checks over the loading rules and
#                                   the picture they draw, in plain node. They
#                                   live beside the demo they were written for
#                                   but the code they run is the served
#                                   static/sorting-*.js, so they gate the route.
#
# clippy runs without `-D warnings` on purpose: the tree carries pre-existing
# warnings (21 distinct as of 2026-08-12 — CLAUDE.md keeps the measured count).
# Promote it to `-D warnings` once that reaches zero — a gate that is red on
# arrival teaches everyone to ignore it.
#
# Exit 0 = every check passed. Any failure prints the failing check's output and
# the script exits 1 after running the rest, so one run shows every problem.

set -uo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1

failed=()

run() {
  local name="$1"
  shift
  printf '\n=== %s\n' "$name"
  if "$@"; then
    return 0
  fi
  failed+=("$name")
  return 1
}

run "cargo fmt --check" cargo fmt --check
run "cargo clippy" cargo clippy --workspace --all-targets
SQLX_OFFLINE=true run "cargo test" cargo test --workspace

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

printf '\n=== node (the board suites)\n'
if command -v node >/dev/null 2>&1; then
  board_bad=0
  for t in docs/design/sorting-live/src/model.test.js docs/design/sorting-live/src/board.test.js; do
    [[ -e "$t" ]] || continue
    if out=$(node "$t" 2>&1) && [[ "$out" != *FAIL* ]]; then
      printf 'ok   %s — %s\n' "$t" "${out##*$'\n'}"
    else
      printf 'FAIL %s\n%s\n' "$t" "$out"
      board_bad=1
    fi
  done
  (( board_bad == 0 )) || failed+=("board suites")
else
  printf 'SKIPPED — node not on PATH\n'
fi

printf '\n'
if (( ${#failed[@]} == 0 )); then
  printf 'VERIFY OK — fmt, clippy, tests, JS syntax, board suites all clean.\n'
  exit 0
fi

printf 'VERIFY FAILED: %s\n' "${failed[*]}"
exit 1
