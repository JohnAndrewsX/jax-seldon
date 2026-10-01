#!/usr/bin/env bash
# Validate every JSON fixture against its schema and check that
# fixtures/index.sample.json derives from fixtures/logbook/ (WP-002).
# Usage: bash scripts/validate-fixtures.sh [--validator auto|jsonschema|check-jsonschema|builtin] [--write-index] [-q]
# Exit: 0 ok, 1 any problem (including a missing python3).
set -u
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
if ! command -v python3 >/dev/null 2>&1; then
  echo "validate-fixtures: python3 is required (the schema validators are python or python-based)" >&2
  exit 1
fi
python3 "$here/validate-fixtures.py" "$@" || exit 1
