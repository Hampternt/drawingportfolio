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
#   4. node --check static/*.js   — JS syntax (a nested palette entry broke
#                                   palette.js once, c72d614; nothing else
#                                   catches it)
#
# clippy runs without `-D warnings` on purpose: the tree carries 19 pre-existing
# warnings as of 2026-08-02. Promote it to `-D warnings` once that reaches zero —
# a gate that is red on arrival teaches everyone to ignore it.
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
  for f in static/*.js; do
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
  printf 'VERIFY OK — fmt, clippy, tests, JS syntax all clean.\n'
  exit 0
fi

printf 'VERIFY FAILED: %s\n' "${failed[*]}"
exit 1
