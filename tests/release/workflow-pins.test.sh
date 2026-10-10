#!/usr/bin/env bash
# The sed mutants below are literal workflow text with `$` and `${{ }}` on
# purpose: they must not expand (SC2016 is the point, not a bug).
# shellcheck disable=SC2016
# The GitHub workflows run pinned code, and cargo audit gates the release
# (packaging/README.md, "Pinned actions and image"). Runs in
# `just check-packaging`; reads the YAML as text, no network.
#
#   - every `uses:` is `owner/repo@<40-hex commit> # vX.Y.Z`;
#   - every `container:` and `image:` is `name@sha256:<64 hex> # <tag>`;
#   - one action or image has one pin across all workflows;
#   - every image comes from the GHCR mirror, never from Docker Hub
#     (WP-195): a job with a container needs the workflow's `image` job,
#     reads packages and logs in with the job's token; the `image` job
#     runs packaging/mirror-image.sh with exactly contents: read and
#     packages: write, and no other job may write packages;
#   - release.yml's build job installs cargo-audit and runs
#     `cargo audit` with the reviewed ignore list before `just check`;
#     the step has no `if:` or `shell:` of its own, nothing in its run
#     block ignores a failure, and `cargo audit` is its last line;
#     release.yml has no `continue-on-error`; release needs build, and
#     bump, aur and plugin need [build, release];
#   - release.yml's workflow permissions are `contents: read` alone; its
#     build job has exactly contents: read, id-token: write and
#     attestations: write (and packages: read for the image), and an unconditional `Attest the release
#     assets` step (actions/attest-build-provenance) that names the binary
#     tarball, the source tarball, SHA256SUMS and install.sh (WP-080);
#     that step is the last one before `Summary`, after every check, and
#     no other job has an `id-token:` or `attestations:` permission;
#   - release.yml's build job validates the plugin split (WP-190): an
#     unconditional `Validate the plugin split` step after `Plugin split`
#     (which outputs the split) and before `Summary` extracts the split
#     with `git archive` and ends with packaging/omarchy-validate.sh on
#     it, ignoring no failure; the build job outputs the split; a `split`
#     job (needs build, no `if:`, the plugin job's runner image, both a
#     fixed `ubuntu-NN.NN` label, never `-latest`, and container)
#     recomputes it and refuses (`exit 1` ending the `if` block) any
#     other, and release needs [build, split], so a mismatch stops the
#     release before anything is published; the plugin job refuses the
#     same way before its push, and the push is `--atomic`.
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

# refuses < RUN-BLOCK: "<fi> <push>": the line of the `fi` that ends
# `if [[ $split != "$VALIDATED" ]]; then` with `exit 1` as the block's
# last line (0 when there is none) and of the first `push "$PLUGIN_REPO"`
# (0 when there is none)
refuses() {
  awk '
    $0 == "if [[ $split != \"$VALIDATED\" ]]; then" && !open && !fi { open = 1; next }
    open && $0 == "fi" { if (prev == "  exit 1") fi = NR; open = 0 }
    open { prev = $0 }
    /push (--atomic )?"\$PLUGIN_REPO"/ && !at { at = NR }
    END { print fi + 0, at + 0 }'
}

# the job names of a workflow, in order
jobs_of() {
  awk '/^jobs:/ { on = 1; next } on && /^  [A-Za-z0-9_-]+:[[:space:]]*$/ { sub(/:.*/, ""); print $1 } on && /^[^ #]/ { on = 0 }' "$1"
}

# the lines of a job's `permissions:` block, sorted
job_perms() {
  awk '/^    permissions:/ { on = 1; next } on && /^      / { print; next } on { exit }' <<< "$1" | LC_ALL=C sort
}

# mirror FILE...: the build image comes from GHCR, never Docker Hub
# (WP-195; packaging/mirror-image.sh)
mirror() {
  local f j text
  grep -n -E '^[[:space:]]*(-[[:space:]]+)?(container|image):[[:space:]]*[^[:space:]#]' "$@" \
    | grep -v -E ':[[:space:]]*(-[[:space:]]+)?(container|image):[[:space:]]+ghcr\.io/johnandrewsx/jax-seldon/' \
    | sed 's/$/ (pulls from Docker Hub: use the GHCR mirror, packaging\/mirror-image.sh)/' || true
  for f in "$@"; do
    while read -r j; do
      [[ -n $j ]] || continue
      text=$(job "$f" "$j")
      if [[ $j == image ]]; then
        [[ $(job_perms "$text") == "$(printf '      %s\n' 'contents: read' 'packages: write')" ]] \
          || echo "$f: the image job's permissions are not exactly contents: read, packages: write"
        grep -x -q -F '        run: bash packaging/mirror-image.sh' <<< "$text" \
          || echo "$f: the image job does not run packaging/mirror-image.sh"
        ! grep -E -q '^    (if|container):' <<< "$text" \
          || echo "$f: the image job has an if: or a container"
        continue
      fi
      ! grep -E -q '^[[:space:]]+packages:[[:space:]]*write' <<< "$text" \
        || echo "$f: the $j job writes packages (only the image job may)"
      grep -E -q '^    container:' <<< "$text" || continue
      grep -E -q '^    needs:[[:space:]]*(image|\[([^]]*[[:space:],])?image([[:space:],][^]]*)?\])[[:space:]]*$' <<< "$text" \
        || echo "$f: the $j job uses the image but does not need the image job"
      grep -E -q '^      packages: read$' <<< "$text" \
        || echo "$f: the $j job uses the image but has no packages: read"
      # shellcheck disable=SC2016  # the workflow's expression, literally
      grep -x -q -F '        password: ${{ secrets.GITHUB_TOKEN }}' <<< "$text" \
        || echo "$f: the $j job's container does not log in with the job's token"
    done <<< "$(jobs_of "$f")"
    if grep -E -q '^    container:' "$f" && [[ -z $(job "$f" image) ]]; then
      echo "$f: a job uses the image but the workflow has no image job"
    fi
  done
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
  mirror "${files[@]}"
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
  grep -E -q '^    needs:[[:space:]]*\[[[:space:]]*build[[:space:]]*,[[:space:]]*split[[:space:]]*\][[:space:]]*$' <<< "$(job "$release" release)" \
    || echo "$release: the release job does not need [build, split]"
  # build provenance (WP-080): only the build job may sign, and it reads
  # the repository only
  local top perms want attest subject
  top=$(awk '/^permissions:/ { on = 1; next } on && /^  / { print; next } on { exit }' "$release")
  [[ $top == "  contents: read" ]] \
    || echo "$release: the workflow permissions are not contents: read alone"
  perms=$(job_perms "$build")
  want=$(printf '      %s\n' 'attestations: write' 'contents: read' 'id-token: write' 'packages: read')
  [[ $perms == "$want" ]] \
    || echo "$release: the build job's permissions are not exactly contents: read, packages: read, id-token: write, attestations: write"
  attest=$(step "$build" "Attest the release assets")
  if [[ -z $attest ]]; then
    echo "$release: the build job has no step named Attest the release assets"
  else
    grep -E -q '^        uses: actions/attest-build-provenance@' <<< "$attest" \
      || echo "$release: the attest step does not use actions/attest-build-provenance"
    ! grep -E -q '^        if:' <<< "$attest" \
      || echo "$release: the attest step has an if: condition"
    # shellcheck disable=SC2016  # the workflow's expressions, literally
    for subject in 'dist/seldon-${{ steps.version.outputs.version }}-${{ env.MUSL_TARGET }}.tar.gz' \
      'dist/jax-seldon-${{ steps.version.outputs.version }}.tar.gz' dist/SHA256SUMS dist/install.sh; do
      grep -x -q -F "            $subject" <<< "$attest" \
        || echo "$release: the attest step does not name $subject"
    done
    # the step before Summary (a step is `- name:` or a bare `- uses:`)
    local before
    before=$(awk '/^      - / { if ($0 == "      - name: Summary") { print prev; exit } prev = $0 }' <<< "$build")
    [[ $before == "      - name: Attest the release assets" ]] \
      || echo "$release: the attest step is not the last step before Summary"
  fi
  # the plugin split (WP-190): the store installs exactly what the pinned
  # validator checked
  local split validate vblock order push
  split=$(step "$build" "Plugin split")
  validate=$(step "$build" "Validate the plugin split")
  vblock=$(run_block "$validate")
  # shellcheck disable=SC2016  # the workflow's lines, literally
  {
    grep -x -q -F '        id: split' <<< "$split" \
      && grep -x -q -F 'echo "split=$split" >> "$GITHUB_OUTPUT"' <<< "$(run_block "$split")" \
      || echo "$release: the Plugin split step does not output the split (id: split)"
    grep -x -q -F '      plugin_split: ${{ steps.split.outputs.split }}' <<< "$build" \
      || echo "$release: the build job does not output plugin_split"
    if [[ -z $validate ]]; then
      echo "$release: the build job has no step named Validate the plugin split"
    else
      ! grep -E -q '^        (if|shell):' <<< "$validate" \
        || echo "$release: the Validate the plugin split step has an if: or a shell: of its own"
      grep -x -q -F '          SPLIT: ${{ steps.split.outputs.split }}' <<< "$validate" \
        || echo "$release: the Validate the plugin split step does not take the split step's output"
      grep -x -q -F 'git archive "$SPLIT" | tar -x -C "$tree"' <<< "$vblock" \
        || echo "$release: the Validate the plugin split step does not extract the split with git archive"
      ! grep -E -q '\|\||set \+[eo]' <<< "$vblock" \
        || echo "$release: the Validate the plugin split step ignores a failure (|| or set +e)"
      [[ $(tail -n 1 <<< "$vblock") == 'bash packaging/omarchy-validate.sh "$tree"' ]] \
        || echo "$release: the pinned validator on the extracted split is not the last line of its step"
    fi
    # step order in the build job: Plugin split, Validate the plugin split, Summary
    order=$(grep -E '^      - name: (Plugin split|Validate the plugin split|Summary)$' <<< "$build" | sed 's/^      - name: //' | paste -sd '|')
    [[ $order == 'Plugin split|Validate the plugin split|Summary' ]] \
      || echo "$release: the build job's order is not Plugin split, Validate the plugin split, Summary ($order)"
    # the split job: before release, on the plugin job's runner, a dry run too
    local sj cmp
    sj=$(job "$release" split)
    if [[ -z $sj ]]; then
      echo "$release: no split job"
    else
      grep -x -q '    needs: build' <<< "$sj" || echo "$release: the split job does not need build"
      ! grep -E -q '^    if:' <<< "$sj" || echo "$release: the split job has an if: condition"
      [[ $(grep -E '^    (runs-on|container):' <<< "$sj") == "$(grep -E '^    (runs-on|container):' <<< "$(job "$release" plugin)" | sed 's/[[:space:]]*#.*//')" ]] \
        || echo "$release: the split job does not run on the plugin job's runner and container"
      local image
      for image in "$(grep -E '^    runs-on:' <<< "$sj")" "$(grep -E '^    runs-on:' <<< "$(job "$release" plugin)")"; do
        [[ $image =~ ^\ {4}runs-on:\ ubuntu-[0-9]{2}\.[0-9]{2}([[:space:]]+#.*)?$ ]] \
          || echo "$release: the split and plugin jobs must name a fixed runner image (ubuntu-NN.NN, not -latest): ${image:-none}"
      done
      cmp=$(step "$sj" "Compare the plugin split")
      ! grep -E -q '^        (if|shell):' <<< "$cmp" \
        || echo "$release: the split job's comparison has an if: or a shell: of its own"
      grep -x -q -F '          VALIDATED: ${{ needs.build.outputs.plugin_split }}' <<< "$cmp" \
        && grep -x -q -F 'split=$(git subtree split --prefix=plugin)' <<< "$(run_block "$cmp")" \
        && [[ $(refuses <<< "$(run_block "$cmp")") =~ ^[1-9] ]] \
        || echo "$release: the split job does not refuse a split other than the validated one"
      ! grep -E -q '\|\||set \+[eo]' <<< "$(run_block "$cmp")" \
        || echo "$release: the split job's comparison ignores a failure (|| or set +e)"
    fi
    # the plugin job: the same refusal before the push, which is atomic
    push=$(run_block "$(step "$(job "$release" plugin)" "Push the plugin split")")
    grep -x -q -F '          VALIDATED: ${{ needs.build.outputs.plugin_split }}' <<< "$(job "$release" plugin)" \
      && [[ $(refuses <<< "$push") =~ ^([1-9][0-9]*)\ ([1-9][0-9]*)$ ]] && ((BASH_REMATCH[1] < BASH_REMATCH[2])) \
      || echo "$release: the plugin job does not refuse a split other than the validated one before the push"
    grep -E -q '^  push --atomic "\$PLUGIN_REPO" ' <<< "$push" \
      || echo "$release: the plugin push is not --atomic"
  }
  # only build signs: no other job may ask for an OIDC token or attestations
  local j
  while read -r j; do
    [[ $j == build ]] && continue
    ! grep -E -q '^[[:space:]]+(id-token|attestations):' <<< "$(job "$release" "$j")" \
      || echo "$release: the $j job has id-token or attestations permissions (only build signs)"
  done <<< "$(jobs_of "$release")"
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
  's/image: (ghcr[^ ]+)@sha256:[0-9a-f]{64}/image: \1:base-devel/' audit.yml
expect_problem "image without the tag comment" "image not pinned" \
  's/(image: [^ ]+@sha256:[0-9a-f]{64}).*/\1/' ci.yml
expect_problem "image from Docker Hub" "pulls from Docker Hub" \
  's/image: ghcr.io\/johnandrewsx\/jax-seldon\/archlinux@/image: archlinux:base-devel@/' ci.yml
expect_problem "image from Docker Hub, explicit" "pulls from Docker Hub" \
  's/image: ghcr.io\/johnandrewsx\/jax-seldon\/archlinux@/image: docker.io\/library\/archlinux@/' audit.yml
expect_problem "check job writes packages" "check job writes packages" \
  's/^      packages: read$/      packages: write/' ci.yml
expect_problem "build job writes packages" "build job writes packages" \
  's/^      packages: read$/      packages: write/'
expect_problem "check job without needs: image" "does not need the image job" \
  '/^    needs: image$/d' ci.yml
expect_problem "build job needs another job" "does not need the image job" \
  's/^    needs: image$/    needs: [imagery]/'
expect_problem "audit job without packages: read" "has no packages: read" \
  '/^      packages: read$/d' audit.yml
expect_problem "container without the token" "does not log in" \
  '/^        password: /d' ci.yml
expect_problem "no image job" "has no image job" \
  's/^  image:$/  mirror:/' audit.yml
expect_problem "image job without the script" "does not run packaging/mirror-image.sh" \
  's/^        run: bash packaging\/mirror-image.sh$/        run: true/' ci.yml
expect_problem "image job with id-token" "image job's permissions are not exactly" \
  's/^(      packages: write)$/\1\n      id-token: write/' ci.yml
expect_problem "image job only on push" "image job has an if:" \
  's/^(  image:)$/\1\n    if: github.event_name == '"'push'"'/' release.yml
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
  's/ cargo-audit( |$)/\1/'
expect_problem "cargo audit after just check" "before just check" \
  's/^(      - name: cargo audit)$/      - name: early\n        run: just check\n\n\1/'
expect_problem "release job without build" "does not need \[build, split\]" \
  's/^    needs: \[build, split\]$/    needs: split/'
expect_problem "release job without split" "does not need \[build, split\]" \
  's/^    needs: \[build, split\]$/    needs: build/'
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
expect_problem "workflow may write contents" "not contents: read alone" \
  '0,/^  contents: read$/s//  contents: write/'
expect_problem "build job without id-token" "permissions are not exactly" \
  '/^      id-token: write$/d'
expect_problem "build job with packages: write" "permissions are not exactly" \
  's/^(      attestations: write)$/\1\n      packages: write/'
expect_problem "build job without packages: read" "permissions are not exactly" \
  '/^  build:$/,/^    steps:$/{/^      packages: read$/d}'
expect_problem "no attest step" "no step named Attest" \
  's/^      - name: Attest the release assets$/      - name: Attest/'
expect_problem "attest step with if: false" "attest step has an if:" \
  's/^(      - name: Attest the release assets)$/\1\n        if: false/'
expect_problem "install.sh not attested" "does not name dist/install.sh" \
  '/^            dist\/install\.sh$/d'
expect_problem "source tarball not attested" "does not name dist/jax-seldon-" \
  '/^            dist\/jax-seldon-/d'
expect_problem "id-token on the release job" "release job has id-token" \
  '/^  release:$/,/^    permissions:$/s/^(    permissions:)$/\1\n      id-token: write/'
expect_problem "attest step before Build the package" "not the last step before Summary" \
  '/^      - name: Attest the release assets$/,/^      - name: Summary$/{/^      - name: Summary$/!d}; s/^      - name: Build the package$/      - name: Attest the release assets\n        uses: actions\/attest-build-provenance@4d101475d8b20a2381f78447822ac1eab6504dd8 # v4.2.2\n\n&/'
expect_problem "no split validation" "no step named Validate the plugin split" \
  's/^      - name: Validate the plugin split$/      - name: Validate/'
expect_problem "split validation with if: false" "has an if: or a shell:" \
  's/^(      - name: Validate the plugin split)$/\1\n        if: false/'
expect_problem "split validation || true" "ignores a failure" \
  's/^(          bash packaging\/omarchy-validate.sh "\$tree")$/\1 || true/'
expect_problem "split validation of the work tree" "not the last line" \
  's/^(          bash packaging\/omarchy-validate.sh )"\$tree"$/\1plugin\//'
expect_problem "split validation without the extraction" "does not extract the split" \
  's/^          git archive "\$SPLIT" \| tar -x -C "\$tree"$/          cp -r plugin\/. "$tree"/'
expect_problem "split validation of another commit" "does not take the split" \
  's/^(          SPLIT: )\$\{\{ steps.split.outputs.split \}\}$/\1HEAD/'
expect_problem "split without its output" "does not output the split" \
  '/^          echo "split=\$split" >> "\$GITHUB_OUTPUT"$/d'
expect_problem "build job without plugin_split" "does not output plugin_split" \
  '/^      plugin_split: /d'
expect_problem "split validation before the split" "order is not" \
  '/^      - name: Validate the plugin split$/,/^      - name: Attest the release assets$/{/^      - name: Attest the release assets$/!d}; s/^      - name: Plugin split$/      - name: Validate the plugin split\n        env:\n          SPLIT: $\{\{ steps.split.outputs.split \}\}\n        run: |\n          tree=$(mktemp -d)\n          git archive "$SPLIT" | tar -x -C "$tree"\n          bash packaging\/omarchy-validate.sh "$tree"\n\n&/'
expect_problem "plugin push without the comparison" "does not refuse a split" \
  '/^          if \[\[ \$split != "\$VALIDATED" \]\]; then$/,/^          fi$/d'
expect_problem "plugin push compares after the push" "does not refuse a split" \
  '/^          if \[\[ \$split != "\$VALIDATED" \]\]; then$/,/^          fi$/d; s/^(            push "\$PLUGIN_REPO" .*)$/\1\n          if [[ $split != "$VALIDATED" ]]; then exit 1; fi/'
expect_problem "no split job" "no split job" \
  's/^  split:$/  splitcheck:/'
expect_problem "split job without needs" "split job does not need build" \
  '/^  split:$/,/^    needs:/{/^    needs:/d}'
expect_problem "split job on tags only" "split job has an if:" \
  's/^(  split:)$/\1\n    if: github.event_name == '"'push'"'/'
expect_problem "split job in the Arch container" "plugin job's runner and container" \
  '/^  split:$/,/^    steps:$/s/^(    runs-on: ubuntu-24.04)$/\1\n    container: archlinux:base-devel@sha256:51dd3d24f7fba779e7c471caeee7804c50e8c134ad948e19685a1c83a42facc3 # base-devel-20260927.0.600689/'
expect_problem "split job on ubuntu-latest" "fixed runner image" \
  '/^  split:$/,/^    steps:$/s/^    runs-on: ubuntu-24.04$/    runs-on: ubuntu-latest/; /^  plugin:$/,/^    steps:$/s/^    runs-on: ubuntu-24.04 .*/    runs-on: ubuntu-latest/'
expect_problem "plugin job on another image" "plugin job's runner and container" \
  '/^  plugin:$/,/^    steps:$/s/^    runs-on: ubuntu-24.04 /    runs-on: ubuntu-22.04 /'
expect_problem "split job without exit 1" "split job does not refuse" \
  '/^  split:$/,/^  release:$/{/^            exit 1$/d}'
expect_problem "split job compares || true" "split job's comparison ignores a failure" \
  '/^  split:$/,/^  release:$/s/^(          split=\$\(git subtree split --prefix=plugin\))$/\1 || true/'
expect_problem "split job compares another value" "split job does not refuse" \
  '/^  split:$/,/^  release:$/s/^(          VALIDATED: ).*/\1HEAD/'
expect_problem "plugin push without exit 1" "plugin job does not refuse" \
  '/^  plugin:$/,${/^            exit 1$/d}'
expect_problem "plugin push exits 0 on a mismatch" "plugin job does not refuse" \
  '/^  plugin:$/,${s/^            exit 1$/            exit 0/}'
expect_problem "plugin push not atomic" "not --atomic" \
  's/push --atomic "\$PLUGIN_REPO"/push "$PLUGIN_REPO"/'

if ((fails > 0)); then
  echo "workflow-pins.test: $fails failure(s)" >&2
  exit 1
fi
echo "workflow-pins.test: ok"
