#!/usr/bin/env bash
# Check the user guide in docs/user/ (WP-045): links, the en/de page set and
# structure, the German source lines, and the CLI reference against the
# engine's --help. Builds the engine (debug, default features) first unless
# SELDON_BIN names a binary.
# Usage: bash scripts/docs-check.sh [--write] [-q]
#   --write  regenerate the help blocks of docs/user/*/05-cli-reference.md
# Exit: 0 ok, 1 any problem (including a missing python3 or a failed build).
set -euo pipefail
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
root=$(dirname "$here")
command -v python3 >/dev/null 2>&1 || { echo "docs-check: python3 is required" >&2; exit 1; }

bin=${SELDON_BIN:-}
if [[ -z $bin ]]; then
  manifest="$root/engine/Cargo.toml"
  cargo build --manifest-path "$manifest" --locked --quiet \
    || { echo "docs-check: cannot build the engine" >&2; exit 1; }
  target=$(cargo metadata --manifest-path "$manifest" --format-version 1 --no-deps \
    | python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])')
  bin="$target/debug/seldon"
fi
[[ -x $bin ]] || { echo "docs-check: no engine binary at $bin" >&2; exit 1; }

python3 "$here/docs-check.py" --seldon "$bin" "$@"
