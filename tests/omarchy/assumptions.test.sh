#!/usr/bin/env bash
# tests/omarchy/assumptions.json and scripts/check-omarchy-assumptions.sh
# (WP-194). Everywhere (fixture): the ledger is well-formed, each cited
# plugin line is code (not a comment) and still holds its pluginSymbol, its
# validator hash is the pin's; on a synthetic tree the check says ok, names
# every assumption of a changed or missing file, says whether the file
# still names the symbol, notes a validator other than the pin and a pin
# it cannot read, refuses both with --gate, prints "skipped" (never "ok")
# without a tree and writes nothing. Where Omarchy is installed: every
# symbol is in its file where the hash matches; a scratch copy of the
# ledger's files checks as the tree does; a change to shell.qml alone
# names the shell's settings write and service lookup, one to
# PluginShellApi.qml the facade's. Each check runs with its own HOME and
# XDG_RUNTIME_DIR. Runs in `just check-assumptions`.
#
# Usage: bash tests/omarchy/assumptions.test.sh
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
script=$root/scripts/check-omarchy-assumptions.sh
ledger=$root/tests/omarchy/assumptions.json
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
fails=0

pass() { echo "ok   $1"; }
fail() { echo "FAIL $1" >&2; fails=$((fails + 1)); }

# --- the ledger ---

# lint LEDGER: prints one line per problem
lint() {
  local file=$1 pin_sha
  jq -e . "$file" >/dev/null 2>&1 || { echo "not JSON"; return; }
  jq -r '
    .assumptions as $a
    | ($a | map(.id) | group_by(.) | map(select(length > 1) | "duplicate id \(.[0])")[]),
      ($a | group_by(.path) | map(select((map(.sha256) | unique | length) > 1) | "two sha256 for \(.[0].path)")[]),
      ($a[] | . as $e
        | (["id", "path", "sha256", "omarchyVersion", "measured", "symbol", "pluginSymbol", "plugin", "relies", "remeasure"][]
            | select(($e[.] | type) != "string" or $e[.] == "") | "\($e.id // "?"): no \(.)"),
          (select((.path | type) == "string" and (.path | test("^/|\\.\\.")) ) | "\(.id): path not relative"),
          (select((.sha256 | type) == "string" and (.sha256 | test("^[0-9a-f]{64}$") | not)) | "\(.id): sha256 not 64 hex"),
          (select((.measured | type) == "string" and (.measured | test("^[0-9]{4}-[0-9]{2}-[0-9]{2}$") | not)) | "\(.id): measured not a date"),
          (select((.plugin | type) == "string" and (.plugin | test("^plugin/[^:]+:[1-9][0-9]*$") | not)) | "\(.id): plugin not plugin/<file>:<line>"),
          (select((.relies | type) == "string" and (.plugin | type) == "string")
            | .plugin as $p | select((.relies | startswith("Seldon relies on ") and endswith(" at " + $p + ".")) | not)
            | "\(.id): relies is not \"Seldon relies on … at \(.plugin).\""))
  ' "$file" || echo "jq could not read the assumptions"
  # each cited line is code, not a comment, and still holds its pluginSymbol
  local f n line
  while IFS=$'\t' read -r id ref symbol; do
    f=${ref%:*} n=${ref##*:}
    line=$( [[ -f $root/$f ]] && sed -n "${n}p" "$root/$f")
    if [[ ! -f $root/$f ]]; then
      echo "$id: $f not found"
    elif [[ $line =~ ^[[:space:]]*(//|/\*|\*|#) ]]; then
      echo "$id: $ref is a comment, not code"
    elif ! grep -qF -- "$symbol" <<< "$line"; then
      echo "$id: $ref does not hold '$symbol'"
    fi
  done < <(jq -r '.assumptions[] | select(.plugin and .pluginSymbol) | [.id, .plugin, .pluginSymbol] | @tsv' "$file")
  # the validator CI fetches is the one the ledger measured
  read -r _ _ pin_sha < <(bash "$root/packaging/omarchy-validate.sh" --print-pin)
  jq -r --arg pin "$pin_sha" '.assumptions[] | select(.path == "bin/omarchy-plugin-validate" and .sha256 != $pin)
    | "\(.id): validator sha256 is not packaging/omarchy-pin'"'"'s"' "$file"
}

problems=$(lint "$ledger")
if [[ -z $problems ]]; then
  pass "tests/omarchy/assumptions.json: well-formed, every cited plugin line is code holding its pluginSymbol, validator = pin"
else
  fail "tests/omarchy/assumptions.json: $problems"
fi
count=$(jq '.assumptions | length' "$ledger")
files=$(jq '.assumptions | map(.path) | unique | length' "$ledger")
if ((files >= 8 && count >= files)); then
  pass "the ledger: $count assumptions over $files files"
else
  fail "the ledger: $count assumptions over $files files"
fi

# lint mutants: each must be caught
# mutate NAME JQ-FILTER PATTERN [JQ-ARGS...]
mutate() {
  local name=$1 filter=$2 pattern=$3 out
  shift 3
  jq "$@" "$filter" "$ledger" > "$tmp/mutant.json"
  out=$(lint "$tmp/mutant.json")
  if grep -q -- "$pattern" <<< "$out"; then pass "lint catches: $name"; else fail "lint misses: $name ($out)"; fi
}
mutate "a cited line that moved" '.assumptions[0].plugin |= sub(":[0-9]+$"; ":1")' "does not hold"
# a comment line of Model.js that names RESTART_SHELL_ARGV, the code line's symbol
comment=$(grep -n '^//.*RESTART_SHELL_ARGV\|^// .*restart' "$root/plugin/Model.js" | head -n 1 | cut -d: -f1)
# shellcheck disable=SC2016 # $ref is jq's, not the shell's
mutate "a cited comment line" '(.assumptions[] | select(.id == "restart-shell")) |= (.plugin = $ref | .pluginSymbol = "/" | .relies |= sub(" at plugin/[^ ]+$"; " at " + $ref + "."))' "is a comment" \
  --arg ref "plugin/Model.js:${comment:-1}"
mutate "a missing pluginSymbol" 'del(.assumptions[2].pluginSymbol)' "no pluginSymbol"
mutate "a sentence without its line" '.assumptions[0].relies = "Seldon relies on something."' "relies is not"
mutate "a missing field" 'del(.assumptions[1].remeasure)' "no remeasure"
# shellcheck disable=SC2016 # $i is jq's, not the shell's
mutate "two hashes for one file" '(.assumptions | map(.path) | index("shell/shell.qml")) as $i | .assumptions[$i].sha256 = ("0" * 64)' "two sha256"
mutate "a validator hash other than the pin" '(.assumptions[] | select(.path == "bin/omarchy-plugin-validate") | .sha256) = ("1" * 64)' "packaging/omarchy-pin"
mutate "an absolute path" '.assumptions[0].path = "/etc/passwd"' "path not relative"
mutate "a duplicate id" '.assumptions[1].id = .assumptions[0].id' "duplicate id"

# --- the check on a synthetic tree ---

sha() { sha256sum < "$1" | cut -d' ' -f1; }
tree=$tmp/omarchy
mkdir -p "$tree/shell/Ui" "$tree/bin"
printf 'function alpha() {}\nfunction beta() {}\n' > "$tree/shell/Api.qml"
printf 'Item { property int gamma: 1 }\n' > "$tree/shell/Ui/Widget.qml"
printf '#!/bin/bash\necho validate\n' > "$tree/bin/omarchy-plugin-validate"
synth_ledger() {
  jq -n --arg api "$(sha "$tree/shell/Api.qml")" --arg widget "$(sha "$tree/shell/Ui/Widget.qml")" \
    --arg val "$(sha "$tree/bin/omarchy-plugin-validate")" '
    def e($id; $path; $sha; $symbol): {id: $id, path: $path, sha256: $sha, omarchyVersion: "9.9.9-1",
      measured: "2026-10-12", symbol: $symbol, plugin: "plugin/X.qml:1",
      relies: "Seldon relies on \($symbol) at plugin/X.qml:1.", remeasure: "re-measure \($id)"};
    {assumptions: [e("api-alpha"; "shell/Api.qml"; $api; "alpha"), e("api-beta"; "shell/Api.qml"; $api; "beta"),
      e("widget-gamma"; "shell/Ui/Widget.qml"; $widget; "gamma"), e("validator"; "bin/omarchy-plugin-validate"; $val; "validate")]}'
}
synth_ledger > "$tmp/ledger.json"
pin_file() {
  printf 'repo=omacom/omarchy\ncommit=0123456789abcdef0123456789abcdef01234567\nvalidator_sha256=%s\n' "$1" > "$tmp/pin"
}
pin_file "$(sha "$tree/bin/omarchy-plugin-validate")"

# run TREE LEDGER [ARGS...]: the check with its own HOME and runtime dir;
# sets rc, leaves stdout and stderr in $tmp/out and $tmp/err
run() {
  local t=$1 l=$2
  shift 2
  rm -rf "${tmp:?}/home" "${tmp:?}/run"
  mkdir -p "$tmp/home"
  mkdir -m 0700 "$tmp/run"
  rc=0
  env HOME="$tmp/home" XDG_RUNTIME_DIR="$tmp/run" OMARCHY_PATH="$t" SELDON_OMARCHY_LEDGER="$l" \
    SELDON_OMARCHY_PIN="$tmp/pin" bash "$script" "$@" > "$tmp/out" 2> "$tmp/err" || rc=$?
  rm -rf "${tmp:?}/home" "${tmp:?}/run"
}
# expect NAME RC [ARGS...] -- PATTERNS... (each on stdout+stderr); NOT:PATTERN must be absent
expect() {
  local name=$1 want=$2 t=$3 l=$4 p bad=''
  shift 4
  local args=()
  while (($#)) && [[ $1 != -- ]]; do args+=("$1"); shift; done
  shift
  run "$t" "$l" "${args[@]}"
  cat "$tmp/out" "$tmp/err" > "$tmp/all"
  [[ $rc == "$want" ]] || bad="exit $rc, not $want"
  for p in "$@"; do
    if [[ $p == NOT:* ]]; then
      grep -qF -- "${p#NOT:}" "$tmp/all" && bad+="; has '${p#NOT:}'"
    else
      grep -qF -- "$p" "$tmp/all" || bad+="; lacks '$p'"
    fi
  done
  if [[ -z $bad ]]; then pass "$name"; else fail "$name: $bad: $(cat "$tmp/all")"; fi
}

before=$(cd "$tree" && find . -type f -exec sha256sum {} + | sort)
expect "a matching tree: ok, exit 0" 0 "$tree" "$tmp/ledger.json" -- "check-upstream: 3 files" "ok (every assumption" NOT:NOTICE
expect "a matching tree with --gate: ok, exit 0" 0 "$tree" "$tmp/ledger.json" --gate -- "ok (every assumption"

printf '// changed\n' >> "$tree/shell/Api.qml"
expect "a changed file: a notice naming each of its assumptions, exit 0" 0 "$tree" "$tmp/ledger.json" -- \
  "NOTICE: shell/Api.qml changed" "measured on Omarchy 9.9.9-1" "api-alpha (still names 'alpha')" \
  "api-beta (still names 'beta')" "re-measure: re-measure api-alpha" "1 of 3 files differ" NOT:widget-gamma NOT:"ok (every"
expect "a changed file with --gate: refused, exit 1" 1 "$tree" "$tmp/ledger.json" --gate -- \
  "api-alpha" "refused (--gate): 1 of 3 files differ"
printf 'function alpha() {}\n' > "$tree/shell/Api.qml"
expect "a symbol gone: 'no longer names'" 0 "$tree" "$tmp/ledger.json" -- \
  "api-beta (no longer names 'beta')" "api-alpha (still names 'alpha')"
rm "$tree/shell/Ui/Widget.qml"
expect "a missing file: named as missing" 0 "$tree" "$tmp/ledger.json" -- \
  "NOTICE: shell/Ui/Widget.qml is missing" "widget-gamma (gone with the file)" "2 of 3 files differ"
printf 'function alpha() {}\nfunction beta() {}\n' > "$tree/shell/Api.qml"
printf 'Item { property int gamma: 1 }\n' > "$tree/shell/Ui/Widget.qml"
expect "the tree restored: ok again" 0 "$tree" "$tmp/ledger.json" -- "ok (every assumption" NOT:NOTICE

printf '#!/bin/bash\necho validate v2\n' > "$tree/bin/omarchy-plugin-validate"
expect "a new validator: its assumption and the pin notice" 0 "$tree" "$tmp/ledger.json" -- \
  "NOTICE: bin/omarchy-plugin-validate changed" "validator (still names 'validate')" \
  "NOTICE: CI validates with another validator" "1 of 3 files differ from the ledger; CI validates with another validator"
synth_ledger > "$tmp/ledger2.json"
expect "a ledger updated, the pin not: the pin notice alone" 0 "$tree" "$tmp/ledger2.json" -- \
  "NOTICE: CI validates with another validator" "refresh packaging/omarchy-pin" NOT:"changed (" NOT:"files differ"
expect "a ledger updated, the pin not, --gate: refused" 1 "$tree" "$tmp/ledger2.json" --gate -- "refused (--gate)"
printf '#!/bin/bash\necho validate\n' > "$tree/bin/omarchy-plugin-validate"
# a pin that cannot be read: the validator was not compared, never "ok"
mv "$tmp/pin" "$tmp/pin.kept"
expect "no pin: a notice, exit 0, never ok" 0 "$tree" "$tmp/ledger.json" -- \
  "NOTICE: cannot read packaging/omarchy-pin" "could not be read, so the validator was not compared" NOT:"ok ("
expect "no pin with --gate: refused" 1 "$tree" "$tmp/ledger.json" --gate -- \
  "refused (--gate): every file matches the ledger, but packaging/omarchy-pin could not be read"
printf 'repo=omacom/omarchy\ncommit=short\n' > "$tmp/pin"
expect "a malformed pin with --gate: refused" 1 "$tree" "$tmp/ledger.json" --gate -- \
  "cannot read packaging/omarchy-pin" "refused (--gate)" NOT:"ok ("
printf '// changed\n' >> "$tree/shell/Api.qml"
expect "no pin and a changed file with --gate: both named" 1 "$tree" "$tmp/ledger.json" --gate -- \
  "api-alpha" "1 of 3 files differ from the ledger; packaging/omarchy-pin could not be read"
printf 'function alpha() {}\nfunction beta() {}\n' > "$tree/shell/Api.qml"
mv "$tmp/pin.kept" "$tmp/pin"
expect "the pin back: ok again" 0 "$tree" "$tmp/ledger.json" --gate -- "ok (every assumption"
rm "$tree/bin/omarchy-plugin-validate"
mv "$tmp/pin" "$tmp/pin.kept"
jq '.assumptions |= map(select(.id != "validator"))' "$tmp/ledger.json" > "$tmp/noval.json"
expect "no validator installed: the pin is not needed" 0 "$tree" "$tmp/noval.json" --gate -- "ok (every assumption" NOT:"cannot read"
mv "$tmp/pin.kept" "$tmp/pin"
printf '#!/bin/bash\necho validate\n' > "$tree/bin/omarchy-plugin-validate"
after=$(cd "$tree" && find . -type f -exec sha256sum {} + | sort)
if [[ $before == "$after" ]]; then pass "the check wrote nothing into the tree"; else fail "the tree changed under the check"; fi

expect "no tree: skipped, exit 0, never ok" 0 "$tmp/none" "$tmp/ledger.json" -- "check-upstream: skipped" NOT:"ok (" NOT:passed
expect "no tree with --gate: skipped and refused" 1 "$tmp/none" "$tmp/ledger.json" --gate -- "skipped" "refuses an unchecked tree"
mkdir -p "$tmp/noshell"
expect "a tree without shell/: skipped" 0 "$tmp/noshell" "$tmp/ledger.json" -- "check-upstream: skipped"
expect "an unknown argument: exit 2" 2 "$tree" "$tmp/ledger.json" --gat -- "usage:"
expect "two arguments: exit 2" 2 "$tree" "$tmp/ledger.json" --gate --gate -- "usage:"
echo '{"assumptions": [{"id": "x"}]}' > "$tmp/bad.json"
expect "an entry without fields: exit 2" 2 "$tree" "$tmp/bad.json" -- "malformed ledger"
echo '{"assumptions": []}' > "$tmp/empty.json"
expect "an empty ledger: exit 2" 2 "$tree" "$tmp/empty.json" -- "malformed ledger"
jq '.assumptions[0].path = "../etc/x"' "$tmp/ledger.json" > "$tmp/escape.json"
expect "a path out of the tree: exit 2" 2 "$tree" "$tmp/escape.json" -- "malformed ledger entry"
expect "no ledger: exit 2" 2 "$tree" "$tmp/missing.json" -- "ledger not found"

# --- a scratch copy of the installed tree (hosts with Omarchy) ---

installed=${OMARCHY_PATH:-/usr/share/omarchy}
installed=${installed%/}
if [[ ! -d $installed/shell ]]; then
  echo "skip installed tree: $installed/shell not found (CI); the synthetic tree above covers the check"
else
  copy=$tmp/copy
  mkdir -p "$copy/shell"
  while IFS= read -r p; do
    [[ -f $installed/$p ]] || continue
    install -D -m 0644 "$installed/$p" "$copy/$p"
  done < <(jq -r '.assumptions[].path' "$ledger" | sort -u)
  cp "$root/packaging/omarchy-pin" "$tmp/pin"
  run "$installed" "$ledger"
  sed "s#$installed#TREE#" "$tmp/out" > "$tmp/want"
  want_rc=$rc
  run "$copy" "$ledger"
  sed "s#$copy#TREE#" "$tmp/out" > "$tmp/got"
  if [[ $rc == "$want_rc" ]] && diff -q "$tmp/want" "$tmp/got" >/dev/null; then
    pass "a scratch copy checks as $installed does (exit $rc)"
  else
    fail "the scratch copy checks differently: $(diff "$tmp/want" "$tmp/got")"
  fi
  # each symbol is found in its file wherever the file is the one measured
  sym_problems=''
  while IFS=$'\t' read -r id path want symbol; do
    [[ -f $installed/$path && $(sha "$installed/$path") == "$want" ]] || continue
    grep -qF -- "$symbol" "$installed/$path" || sym_problems+=" $id ('$symbol' not in $path)"
  done < <(jq -r '.assumptions[] | [.id, .path, .sha256, .symbol] | @tsv' "$ledger")
  if [[ -z $sym_problems ]]; then
    pass "every symbol is in its Omarchy file where the hash matches"
  else
    fail "symbols not in their Omarchy file:$sym_problems"
  fi
  measured=$(jq -r --arg p shell/Ui/Button.qml '[.assumptions[] | select(.path == $p)][0].sha256' "$ledger")
  if [[ -f $installed/shell/Ui/Button.qml && $(sha "$installed/shell/Ui/Button.qml") == "$measured" ]]; then
    jq '(.assumptions[] | select(.path == "shell/Ui/Button.qml") | .symbol) = "noSuchSymbolInButton"' "$ledger" > "$tmp/badsym.json"
    caught=0
    while IFS=$'\t' read -r id path want symbol; do
      [[ -f $installed/$path && $(sha "$installed/$path") == "$want" ]] || continue
      grep -qF -- "$symbol" "$installed/$path" || caught=1
    done < <(jq -r '.assumptions[] | [.id, .path, .sha256, .symbol] | @tsv' "$tmp/badsym.json")
    if ((caught)); then pass "lint catches: a symbol not in its Omarchy file"; else fail "lint misses: a symbol not in its Omarchy file"; fi
  fi

  # a change to shell.qml alone names the shell's own settings write and service lookup
  target=shell/shell.qml
  want_sha=$(jq -r --arg p "$target" '[.assumptions[] | select(.path == $p)][0].sha256' "$ledger")
  if [[ -f $copy/$target && $(sha "$copy/$target") == "$want_sha" ]]; then
    cp "$copy/$target" "$tmp/shell.qml.kept"
    printf '\n// scratch change\n' >> "$copy/$target"
    expect "a changed shell.qml: the notice names shell-settings-write and shell-service-lookup" 0 "$copy" "$ledger" -- \
      "NOTICE: $target changed" "shell-settings-write (still names 'updateEntryInline')" \
      "shell-service-lookup (still names 'serviceFor')" "shell-overlay-routing" "1 of 17 files differ" \
      NOT:"facade-settings-write" NOT:"no longer names"
    cp "$tmp/shell.qml.kept" "$copy/$target"
  else
    echo "skip changed shell.qml: $target already differs from the ledger here (see \`just check-upstream\`)"
  fi

  target=shell/services/PluginShellApi.qml
  want_sha=$(jq -r --arg p "$target" '[.assumptions[] | select(.path == $p)][0].sha256' "$ledger")
  if [[ -f $copy/$target && $(sha "$copy/$target") == "$want_sha" ]]; then
    printf '\n// scratch change\n' >> "$copy/$target"
    expect "a changed installed file: the notice names facade-settings-write" 0 "$copy" "$ledger" -- \
      "NOTICE: $target changed" "facade-settings-write (still names 'updateEntryInline')" \
      "at plugin/Desk.qml:527" "re-measure: the desk's settings writes" NOT:"shell-settings-write" NOT:"no longer names"
    expect "a changed installed file with --gate: refused" 1 "$copy" "$ledger" --gate -- "refused (--gate)"
  else
    echo "skip changed installed file: $target already differs from the ledger here (see \`just check-upstream\`)"
  fi
fi

if ((fails)); then
  echo "assumptions.test: $fails failure(s)" >&2
  exit 1
fi
echo "assumptions.test: ok"
