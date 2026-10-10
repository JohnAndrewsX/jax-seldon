#!/usr/bin/env bash
# Self-test of the house rules in tests/plugin/check-tokens.py (WP-177): each
# rule catches its violation in a scratch QML file, and what the rules allow
# passes; the alpha allow-list holds per colour property. Needs python3; no shell tree (the rules run with --rules).
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
command -v python3 >/dev/null || { echo "check-tokens.test: python3 not found" >&2; exit 1; }
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

pass=0
fail=0

# rule_case <name> <want: ok|FAIL> <qml body>
rule_case() {
  local file="$work/$1.qml" got=ok
  printf 'import QtQuick\n%s\n' "$3" >"$file"
  python3 "$root/tests/plugin/check-tokens.py" --rules "$file" >"$work/$1.out" 2>&1 || got=FAIL
  if [[ $got == "$2" ]]; then
    pass=$((pass + 1))
    echo "ok   $1: $got"
  else
    fail=$((fail + 1))
    echo "FAIL $1: $got (want $2)"
    cat "$work/$1.out"
  fi
}

rule_case text-muted FAIL 'Text { color: Color.muted }'
rule_case text-muted-ternary FAIL 'Item {
  Text {
    color: root.bad
      ? Color.urgent
      : Color.muted
  }
}'
rule_case inline-text-muted FAIL 'Item {
  component Line: Text {}
  Line { color: Color.muted }
}'
rule_case muted-property FAIL 'Item { readonly property color dim: Color.muted }'
rule_case literal-alpha FAIL 'Rectangle { color: Util.alpha(Color.accent, 0.3) }'
rule_case literal-alpha-nested FAIL 'Rectangle { color: Util.alpha(Style.hoverFillFor(Color.accent, Color.accent), .5) }'
rule_case rectangle-muted ok 'Rectangle { color: Color.muted }'
rule_case text-after-rectangle ok 'Item {
  Rectangle { color: Color.muted }
  Text { color: root.tone.dim }
}'
rule_case alpha-token ok 'Rectangle { color: Util.alpha(Color.accent, Style.selectedFillAlpha) }'
rule_case commented ok 'Text { color: root.tone.dim } // not Color.muted, not Util.alpha(x, 0.5)'

# The allow-list is per colour property: in a copy of the plugin tree, the
# graph's listed data colour passes; the same alpha on another property, a
# new literal on a listed one, or a literal outside a property fails.
graph_case() {
  local name=$1 want=$2 line=$3 got=ok
  mkdir -p "$work/plugin/components/graph"
  printf 'import QtQuick\nItem {\n%s\n}\n' "$line" >"$work/plugin/components/graph/GraphCanvas.qml"
  CHECK_TOKENS_PLUGIN_ROOT="$work/plugin" python3 "$root/tests/plugin/check-tokens.py" --rules \
    "$work/plugin/components/graph/GraphCanvas.qml" >"$work/$name.out" 2>&1 || got=FAIL
  if [[ $got == "$want" ]]; then
    pass=$((pass + 1))
    echo "ok   $name: $got"
  else
    fail=$((fail + 1))
    echo "FAIL $name: $got (want $want)"
    cat "$work/$name.out"
  fi
}

graph_case graph-listed ok '  readonly property color edgeColor: Util.alpha(root.foreground, 0.22)'
graph_case graph-other-property FAIL '  readonly property color labelDim: Util.alpha(root.foreground, 0.22)'
graph_case graph-other-alpha FAIL '  readonly property color edgeColor: Util.alpha(root.foreground, 0.65)'
graph_case graph-no-property FAIL '  function f(ctx) { ctx.fillStyle = Util.alpha(root.foreground, 0.22) }'

echo "check-tokens.test: $pass passed, $fail failed"
((fail == 0))
