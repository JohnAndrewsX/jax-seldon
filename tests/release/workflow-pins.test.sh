#!/usr/bin/env bash
# The GitHub workflows run pinned code, and cargo audit gates the release
# (packaging/README.md, "Pinned actions and image"). Runs in
# `just check-packaging`; reads the YAML as text, no network.
#
#   - every `uses:` is `owner/repo@<40-hex commit> # vX.Y.Z`;
#   - every `container:` and `image:` is `name@sha256:<64 hex> # <tag>`;
#   - one action or image has one pin across all workflows;
#   - release.yml's build job installs cargo-audit and runs
#     `cargo audit` with the reviewed ignore list before `just check`;
#     the step has no `if:` or `shell:` of its own, nothing in its run
#     block ignores a failure, and `cargo audit` is its last line;
#     release.yml has no `continue-on-error`; release needs build, and
#     bump, aur and plugin need [build, release].
#
# Limit: a SHA is not checked against its version comment (that needs
# the network); the refresh steps in packaging/README.md resolve both.
#
# Usage: bash tests/release/workflow-pins.test.sh
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
fails=0

pass() { echo "ok   $1"; }
fail() { echo "FAIL $1" >&2; fails=$((fails + 1)); }

uses_re='^[[:space:]]*(-[[:space:]]+)?uses:[[:space:]]+[A-Za-z0-9_.-]+/[A-Za-z0-9_./-]+@[0-9a-f]{40}[[:space:]]+#[[:space:]]+v[0-9]+(\.[0-9]+){0,2}[[:space:]]*$'
image_re='^[[:space:]]*(-[[:space:]]+)?(container|image):[[:space:]]+[a-z0-9._/-]+(:[A-Za-z0-9._-]+)?@sha256:[0-9a-f]{64}[[:space:]]+#[[:space:]]*[^[:space:]]'

# the lines of a job (2-space key under `jobs:`) up to the next job
job() {
  awk -v want="  $2:" '
    /^jobs:/ { in_jobs = 1; next }
    in_jobs && /^  [A-Za-z0-9_-]+:[[:space:]]*$/ { on = ($0 == want) }
    in_jobs && /^[^ #]/ { in_jobs = 0; on = 0 }
    on { print }
  ' "$1"
}

# the lines of one step (`      - name: NAME`) of a job's text, up to the next step
step() {
  awk -v want="      - name: $2" '
    /^      - / { on = ($0 == want) }
    on { print }
  ' <<< "$1"
}

# the lines of a step's `run: |` block, without their indentation
run_block() {
  awk '
    /^        run: \|/ { on = 1; next }
    on && /^          / { print substr($0, 11); next }
    on && /^[[:space:]]*$/ { next }
    on { on = 0 }
  ' <<< "$1"
}

# problems DIR: one line per problem in the workflows of DIR; none = ok
problems() {
  local dir=$1 f
  shopt -s nullglob
  local files=("$dir"/*.yml "$dir"/*.yaml)
  shopt -u nullglob
  ((${#files[@]} > 0)) || {
    echo "$dir: no workflows"
    return
  }
  for f in "${files[@]}"; do
    # comment lines may name a tag; a `uses:` or image key must not
    grep -n -E '^[[:space:]]*(-[[:space:]]+)?uses:' "$f" | while IFS=: read -r n line; do
      [[ $line =~ $uses_re ]] || echo "$f:$n: action not pinned to a commit with a version comment: $line"
    done || true
    grep -n -E '^[[:space:]]*(-[[:space:]]+)?(container|image):[[:space:]]*[^[:space:]#]' "$f" | while IFS=: read -r n line; do
      [[ $line =~ $image_re ]] || echo "$f:$n: image not pinned to a digest with a tag comment: $line"
    done || true
  done
  # the same action (or image) pinned twice to different commits (digests)
  sed -n -E 's/^[[:space:]]*(-[[:space:]]+)?uses:[[:space:]]+([^@[:space:]]+)@([^[:space:]]+).*/\2 \3/p;
             s/^[[:space:]]*(-[[:space:]]+)?(container|image):[[:space:]]+([^@[:space:]]+)@([^[:space:]]+).*/\3 \4/p' \
    "${files[@]}" | sort -u | awk '{ n[$1]++ } END { for (a in n) if (n[a] > 1) print "pinned to more than one version: " a }'

  local release=$dir/release.yml build
  [[ -f $release ]] || {
    echo "$release: missing"
    return
  }
  grep -n 'continue-on-error' "$release" | sed "s|^|$release:|; s|\$| (release.yml must not soften a step)|" || true
  build=$(job "$release" build)
  [[ -n $build ]] || echo "$release: no build job"
  grep -E -q '^[[:space:]]*run:.*[[:space:]]cargo-audit([[:space:]]|$)' <<< "$build" \
    || echo "$release: the build job does not install cargo-audit"
  local audit_at check_at
  audit_at=$(grep -n -E '^[[:space:]]*cargo audit --file engine/Cargo\.lock --deny warnings "\$\{args\[@\]\}"' <<< "$build" | head -n 1 | cut -d: -f1 || true)
  check_at=$(grep -n -E '^[[:space:]]*run: just check$' <<< "$build" | head -n 1 | cut -d: -f1 || true)
  if [[ -z $audit_at ]]; then
    echo "$release: the build job does not run cargo audit with the ignore list's arguments"
  elif [[ -z $check_at || $audit_at -gt $check_at ]]; then
    echo "$release: cargo audit does not run before just check in the build job"
  fi
  # the step itself: unconditional, the workflow's `bash -eo pipefail`,
  # every command's failure fails it, and cargo audit decides last.
  # Here-strings, not `| grep -q`: an early exit of grep would SIGPIPE
  # the writer and fail the pipeline under pipefail.
  local audit block
  audit=$(step "$build" "cargo audit")
  block=$(run_block "$audit")
  if [[ -z $audit ]]; then
    echo "$release: the build job has no step named cargo audit"
  else
    ! grep -E -q '^        if:' <<< "$audit" \
      || echo "$release: the cargo audit step has an if: condition"
    ! grep -E -q '^        shell:' <<< "$audit" \
      || echo "$release: the cargo audit step sets its own shell"
    # shellcheck disable=SC2016  # the workflow's line, literally
    grep -x -q -F 'ids=$(bash packaging/audit-ignore.sh)' <<< "$block" \
      || echo "$release: the cargo audit step does not read packaging/audit-ignore.sh"
    ! grep -E -q '\|\||set \+[eo]' <<< "$block" \
      || echo "$release: the cargo audit step ignores a failure (|| or set +e)"
    grep -E -q '^cargo audit --file engine/Cargo\.lock --deny warnings "\$\{args\[@\]\}"$' <<< "$(tail -n 1 <<< "$block")" \
      || echo "$release: cargo audit is not the last line of its step"
  fi
  grep -E -q '^    needs:[[:space:]]*(build|\[[[:space:]]*build[[:space:]]*\])[[:space:]]*$' <<< "$(job "$release" release)" \
    || echo "$release: the release job does not need build"
  local j
  for j in bump aur plugin; do
    grep -E -q '^    needs:[[:space:]]*\[[[:space:]]*build[[:space:]]*,[[:space:]]*release[[:space:]]*\][[:space:]]*$' <<< "$(job "$release" "$j")" \
      || echo "$release: the $j job does not need [build, release]"
  done
}

# --- the real workflows ---
problems "$root/.github/workflows" > "$tmp/real"
if [[ ! -s $tmp/real ]]; then
  pass "workflows: actions and images pinned, cargo audit gates the release"
else
  fail "workflows"
  cat "$tmp/real" >&2
fi
uses=$(cat "$root"/.github/workflows/*.yml | grep -c -E '^[[:space:]]*(-[[:space:]]+)?uses:' || true)
if ((uses >= 10)); then
  pass "workflows: $uses uses: lines checked"
else
  fail "workflows: only $uses uses: lines found; is the pattern still right?"
fi

# --- the check itself: each change below must be found ---
# expect_problem NAME PATTERN SED-EXPR [FILE]: apply SED-EXPR to a copy of
# FILE (default release.yml); the problems must match PATTERN
expect_problem() {
  local name=$1 pattern=$2 expr=$3 file=${4:-release.yml}
  rm -rf "$tmp/m"
  cp -r "$root/.github/workflows" "$tmp/m"
  sed -i -E "$expr" "$tmp/m/$file"
  if cmp -s "$tmp/m/$file" "$root/.github/workflows/$file"; then
    fail "$name: the change did not apply"
  elif problems "$tmp/m" > "$tmp/m.out" && grep -q -- "$pattern" "$tmp/m.out"; then
    pass "$name"
  else
    fail "$name: not found"
    problems "$tmp/m" >&2
  fi
}

expect_problem "action by tag" "not pinned to a commit" \
  '0,/uses: actions\/checkout@[0-9a-f]{40}.*/s//uses: actions\/checkout@v4/' ci.yml
expect_problem "action by short SHA" "not pinned to a commit" \
  '0,/(uses: actions\/upload-artifact@[0-9a-f]{7})[0-9a-f]{33}/s//\1/'
expect_problem "action without the version comment" "not pinned to a commit" \
  '0,/(uses: actions\/download-artifact@[0-9a-f]{40}).*/s//\1/'
expect_problem "image by tag" "image not pinned" \
  's/container: archlinux:base-devel@sha256:[0-9a-f]{64}/container: archlinux:base-devel/' audit.yml
expect_problem "image without the tag comment" "image not pinned" \
  's/(container: [^ ]+@sha256:[0-9a-f]{64}).*/\1/' ci.yml
expect_problem "service image by tag" "image not pinned" \
  's/^(    runs-on: ubuntu-latest)$/\1\n    services:\n      db:\n        image: postgres:16/' ci.yml
expect_problem "one action, two commits" "more than one version: actions/checkout" \
  '0,/uses: actions\/checkout@[0-9a-f]{40}/s//uses: actions\/checkout@0000000000000000000000000000000000000000/' ci.yml
expect_problem "continue-on-error in release.yml" "must not soften" \
  's/^(      - name: cargo audit.*)$/\1\n        continue-on-error: true/'
expect_problem "no cargo audit step" "does not run cargo audit" \
  's/^([[:space:]]*)cargo audit --file/\1echo cargo audit --file/'
expect_problem "cargo audit without the ignore list" "does not run cargo audit" \
  's/ "\$\{args\[@\]\}"$//'
expect_problem "cargo-audit not installed" "does not install cargo-audit" \
  's/ cargo-audit$//'
expect_problem "cargo audit after just check" "before just check" \
  's/^(      - name: cargo audit)$/      - name: early\n        run: just check\n\n\1/'
expect_problem "release job without build" "does not need build" \
  '0,/^    needs: build$/{/^    needs: build$/d}'
expect_problem "cargo audit step with if: false" "has an if: condition" \
  's/^(      - name: cargo audit)$/\1\n        if: false/'
expect_problem "cargo audit step with its own shell" "sets its own shell" \
  's/^(      - name: cargo audit)$/\1\n        shell: bash {0}/'
expect_problem "set +e and a command after cargo audit" "not the last line" \
  's/^(          )(cargo audit --file .*)$/\1set +e\n\1\2\n\1echo done/'
expect_problem "set +e alone" "ignores a failure" \
  's/^(          )(ids=\$\(bash packaging\/audit-ignore.sh\))$/\1set +e\n\1\2/'
expect_problem "|| true on the ids line" "ignores a failure" \
  's/^(          ids=\$\(bash packaging\/audit-ignore.sh\))$/\1 || true/'
expect_problem "cargo audit || true" "ignores a failure" \
  's/^(          cargo audit --file .*)$/\1 || true/'
expect_problem "aur without release" "aur job does not need" \
  '/^  aur:$/,/^    needs:/s/^    needs: \[build, release\]$/    needs: build/'
expect_problem "plugin without build" "plugin job does not need" \
  '/^  plugin:$/,/^    needs:/s/^    needs: \[build, release\]$/    needs: release/'
expect_problem "bump without needs" "bump job does not need" \
  '/^  bump:$/,/^    needs:/{/^    needs:/d}'

if ((fails > 0)); then
  echo "workflow-pins.test: $fails failure(s)" >&2
  exit 1
fi
echo "workflow-pins.test: ok"
