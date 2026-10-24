#!/usr/bin/env bash
# Visual studio: renders ONE thing of the game into a contact sheet PNG.
#
#   bash scripts/studio.sh list                    # every target + the code that draws it
#   bash scripts/studio.sh mob:skitter             # -> studio_out/mob_skitter.png (+ .txt labels)
#   bash scripts/studio.sh model:chest:large --out studio_out/chest_v2.png
#   bash scripts/studio.sh --map 1 mob:lavaboar    # options before the target: --map 0-2
#   bash scripts/studio.sh --check                 # cargo clippy on the current code
#
# Made for helper agents working in parallel, each in its own git worktree:
# - every worktree builds into the SAME target folder (the main checkout's),
#   so the dependencies (Bevy...) are never rebuilt, only the game itself;
# - builds take turns (a lock folder), and the built game is copied to a
#   name of its own before running, so agents never run each other's code.
set -euo pipefail

here="$(cd "$(dirname "$0")/.." && pwd)"
cd "$here"
# Main checkout = parent of the shared .git folder (works from a worktree too).
common="$(git rev-parse --path-format=absolute --git-common-dir)"
main="$(dirname "$common")"
export CARGO_TARGET_DIR="$main/target"
name="$(basename "$here")"
bin_dir="$CARGO_TARGET_DIR/studio-bin"
mkdir -p "$bin_dir"
exe="$bin_dir/prism_rush_$name.exe"

lock="$CARGO_TARGET_DIR/studio.lock"
take_lock() {
    local waited=0
    until mkdir "$lock" 2>/dev/null; do
        # A lock older than 15 minutes was left by a killed build.
        if [ -n "$(find "$lock" -maxdepth 0 -mmin +15 2>/dev/null)" ]; then
            rmdir "$lock" 2>/dev/null || true
            continue
        fi
        if [ $waited -eq 0 ]; then echo "studio: another build is running, waiting for it..."; fi
        waited=1
        sleep 3
    done
    trap 'rmdir "$lock" 2>/dev/null || true' EXIT
}
drop_lock() {
    rmdir "$lock" 2>/dev/null || true
    trap - EXIT
}

if [ "${1:-}" = "--check" ]; then
    take_lock
    cargo clippy --all-targets 2>&1 | grep -E "^(warning|error)" -A8 || echo "clippy: aucun avertissement"
    drop_lock
    exit 0
fi

take_lock
if ! cargo build 2>&1 | grep -E "^(error|warning)" -A12; then :; fi
if ! cargo build -q 2>/dev/null; then
    drop_lock
    echo "STUDIO ERROR: the game does not compile (see the errors above)"
    exit 1
fi
cp -f "$CARGO_TARGET_DIR/debug/prism_rush.exe" "$exe"
drop_lock

# Options (--map N) go before the subcommand; the rest goes to `studio`.
global=()
while [ "${1:-}" = "--map" ] || [ "${1:-}" = "--difficulty" ]; do
    global+=("$1" "$2")
    shift 2
done
exec "$exe" "${global[@]}" studio "$@"
