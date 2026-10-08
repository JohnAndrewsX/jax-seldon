#!/usr/bin/env bash
# make-trim.sh <repo root> <out>: desk-view.sh with only 10a and 10c' (WP-135 mutants)
set -euo pipefail
r=$1 f="$1/tests/plugin/desk-view.sh"
s10a=$(grep -n "^# 10a\. " "$f" | cut -d: -f1); s10b=$(grep -n "^# 10b\. " "$f" | cut -d: -f1)
s10cp=$(grep -n "^# 10c'\. " "$f" | cut -d: -f1); s10d=$(grep -n "^# 10d\. " "$f" | cut -d: -f1)
s8=$(grep -n "^# 8\. Sections 1" "$f" | cut -d: -f1); s8a=$(grep -n "^# 8a\. " "$f" | cut -d: -f1)
s1=$(grep -n "^# 1\. Width" "$f" | cut -d: -f1)
{
  sed -n "1,$((s1 - 2))p" "$f" | sed "s|^root=.*|root=$r|"
  sed -n "${s8},$((s8a - 1))p" "$f"
  sed -n "${s10a},$((s10b - 1))p" "$f"
  sed -n "${s10cp},$((s10d - 1))p" "$f"
  echo 'echo "desk-view-trim: $pass passed, $fail failed"'
  echo '((fail == 0))'
} >"$2"
