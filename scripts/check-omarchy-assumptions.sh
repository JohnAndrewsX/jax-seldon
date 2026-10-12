#!/usr/bin/env bash
# What Seldon assumes about Omarchy, checked offline (WP-194). Hashes the
# installed files that tests/omarchy/assumptions.json lists, read-only, and
# compares them with the ledger. On a mismatch it prints each affected
# assumption and what to re-measure; it also says when the installed
# validator is not the one packaging/omarchy-pin fetches for CI. No
# network, nothing written.
#
# Usage: scripts/check-omarchy-assumptions.sh [--gate]
#   default  a notice, exit 0 (`just check`)
#   --gate   exit 1 on any mismatch, a pin that differs or cannot be
#            read, and a missing tree (the release gates of WP-126 and
#            WP-188, on the test host, from the deployed clone)
# Exit 2: usage, a malformed ledger, jq missing.
#
# OMARCHY_PATH names the tree (default /usr/share/omarchy); without its
# shell/ the check is "skipped" (CI), never "passed".
# Tests only (tests/omarchy/assumptions.test.sh): SELDON_OMARCHY_LEDGER
# names another ledger; SELDON_OMARCHY_PIN another pin (as for
# packaging/omarchy-validate.sh).
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
ledger=${SELDON_OMARCHY_LEDGER:-$root/tests/omarchy/assumptions.json}
tree=${OMARCHY_PATH:-/usr/share/omarchy}
tree=${tree%/}
me=check-upstream

gate=0
case ${1:-} in
  '') ;;
  --gate) gate=1 ;;
  *) echo "usage: check-omarchy-assumptions.sh [--gate]" >&2; exit 2 ;;
esac
(($# <= 1)) || { echo "usage: check-omarchy-assumptions.sh [--gate]" >&2; exit 2; }

if [[ ! -d $tree/shell ]]; then
  if ((gate)); then
    echo "$me: skipped ($tree/shell not found); --gate refuses an unchecked tree" >&2
    exit 1
  fi
  echo "$me: skipped ($tree/shell not found; the assumptions are checked on hosts with Omarchy)"
  exit 0
fi

command -v jq >/dev/null || { echo "$me: jq not found" >&2; exit 2; }
[[ -f $ledger ]] || { echo "$me: ledger not found: $ledger" >&2; exit 2; }
# One line per assumption: path, sha256, id, version, symbol, relies, remeasure
rows=$(jq -er '
  .assumptions | if type == "array" and length > 0 then . else error("no assumptions") end
  | .[] | [.path, .sha256, .id, .omarchyVersion, .symbol, .relies, .remeasure]
  | if all(type == "string" and length > 0) then @tsv else error("an entry lacks a field") end
' "$ledger" 2>/dev/null) || { echo "$me: malformed ledger: $ledger" >&2; exit 2; }

declare -A seen=() actual=()
paths=()
while IFS=$'\t' read -r path want _; do
  [[ $path != /* && $path != *..* && $want =~ ^[0-9a-f]{64}$ ]] \
    || { echo "$me: malformed ledger entry for '$path': $ledger" >&2; exit 2; }
  [[ -n ${seen[$path]:-} ]] && continue
  seen[$path]=1
  paths+=("$path")
  if [[ -f $tree/$path ]]; then
    actual[$path]=$(sha256sum < "$tree/$path" | cut -d' ' -f1)
  else
    actual[$path]=missing
  fi
done <<< "$rows"

echo "$me: ${#paths[@]} files under $tree against ${ledger#"$root"/}"
changed=0
for path in "${paths[@]}"; do
  got=${actual[$path]}
  first=1
  while IFS=$'\t' read -r p want id version symbol relies remeasure; do
    [[ $p == "$path" && $got != "$want" ]] || continue
    if ((first)); then
      first=0
      changed=$((changed + 1))
      if [[ $got == missing ]]; then
        echo "$me: NOTICE: $path is missing (measured on Omarchy $version)"
      else
        echo "$me: NOTICE: $path changed (sha256 ${got:0:12}, ledger ${want:0:12}, measured on Omarchy $version)"
      fi
    fi
    if [[ $got == missing ]]; then
      names='gone with the file'
    elif grep -qF -- "$symbol" "$tree/$path"; then
      names="still names '$symbol'"
    else
      names="no longer names '$symbol'"
    fi
    echo "  - $id ($names): $relies"
    echo "    re-measure: $remeasure"
  done <<< "$rows"
done

# CI runs the validator of packaging/omarchy-pin, not the installed one.
# A pin that cannot be read means the comparison did not happen: a notice,
# and --gate refuses (a gate that did not look is not a pass).
pin_note=''
validator=$tree/bin/omarchy-plugin-validate
if [[ -f $validator ]]; then
  pin_sha=''
  if pin=$(bash "$root/packaging/omarchy-validate.sh" --print-pin); then
    read -r _ _ pin_sha <<< "$pin"
  fi
  if [[ ! $pin_sha =~ ^[0-9a-f]{64}$ ]]; then
    pin_note=unread
    echo "$me: NOTICE: cannot read packaging/omarchy-pin, so the installed validator was not compared with the one CI fetches"
  else
    installed=$(sha256sum < "$validator" | cut -d' ' -f1)
    if [[ $installed != "$pin_sha" ]]; then
      pin_note=differs
      echo "$me: NOTICE: CI validates with another validator: bin/omarchy-plugin-validate is ${installed:0:12} here, packaging/omarchy-pin fetches ${pin_sha:0:12} (packaging/README.md, \"The Omarchy pin\")"
    fi
  fi
fi

if ((changed == 0)) && [[ -z $pin_note ]]; then
  echo "$me: ok (every assumption's file matches the ledger)"
  exit 0
fi
case $pin_note in
  differs) pin_text='CI validates with another validator: refresh packaging/omarchy-pin (packaging/README.md, "The Omarchy pin")' ;;
  unread) pin_text='packaging/omarchy-pin could not be read, so the validator was not compared' ;;
  *) pin_text='' ;;
esac
if ((changed)); then
  summary="$changed of ${#paths[@]} files differ from the ledger${pin_text:+; $pin_text}: re-measure the assumptions above and update the ledger's sha256, omarchyVersion, measured and sentence in the same commit, never silence it (docs/TESTING.md, \"Omarchy assumptions\")"
else
  summary="every file matches the ledger, but $pin_text"
fi
if ((gate)); then
  echo "$me: refused (--gate): $summary" >&2
  exit 1
fi
echo "$me: NOTICE: $summary"
