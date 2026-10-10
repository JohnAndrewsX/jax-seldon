#!/usr/bin/env bash
# packaging/mirror-image.sh (WP-195) against a fake skopeo: a GHCR that
# has the digest or not, a copy that fails or serves other bytes, and
# workflows that pin no or two mirror digests. Runs in
# `just check-packaging`; no network, no registry.
#
# Usage: bash tests/release/mirror-image.test.sh
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
fails=0

pass() { echo "ok   $1"; }
fail() { echo "FAIL $1" >&2; fails=$((fails + 1)); }

# the image's manifest; its sha256 is the digest the workflows pin
printf '{"schemaVersion":2,"mediaType":"application/vnd.oci.image.index.v1+json"}\n' > "$tmp/manifest"
digest=$(sha256sum < "$tmp/manifest" | cut -d' ' -f1)
mirror=ghcr.io/johnandrewsx/jax-seldon/archlinux

# the fake skopeo: $FAKE/ghcr/<digest> is what GHCR serves; every call is
# logged to $FAKE/calls; FAKE_COPY=ok|fail|wrong decides a copy
cat > "$tmp/skopeo" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
echo "$*" >> "$FAKE/calls"
case $1 in
  login)
    [[ $* == *"--authfile "* && $* == *"--password-stdin ghcr.io" ]] || exit 9
    [[ $(cat) == "$GITHUB_TOKEN" ]] || exit 9
    ;;
  inspect)
    ref=${*: -1}
    d=${ref##*@sha256:}
    [[ -f $FAKE/ghcr/$d ]] || { echo "manifest unknown" >&2; exit 1; }
    cat "$FAKE/ghcr/$d"
    ;;
  copy)
    case ${FAKE_COPY:-ok} in
      fail) echo "toomanyrequests" >&2; exit 1 ;;
      wrong) echo other > "$FAKE/ghcr/$DIGEST" ;;
      ok) cp "$FAKE/manifest" "$FAKE/ghcr/$DIGEST" ;;
    esac
    ;;
  *) exit 9 ;;
esac
EOF
chmod +x "$tmp/skopeo"

# workflows NAME LINE...: a scratch workflow dir with these image lines
workflows() {
  local dir=$tmp/$1 line
  shift
  mkdir -p "$dir"
  for line in "$@"; do
    printf 'jobs:\n  check:\n    container:\n      image: %s\n' "$line"
  done > "$dir/ci.yml"
  echo "$dir"
}

# run NAME DIR [ENV...]: the script on DIR with a fresh fake GHCR; sets
# $code and $out, the calls in $tmp/NAME/calls
run() {
  local name=$1 dir=$2
  shift 2
  local fake=$tmp/$name.fake
  rm -rf "$fake"
  mkdir -p "$fake/ghcr" "$tmp/$name.runner"
  cp "$tmp/manifest" "$fake/manifest"
  : > "$fake/calls"
  [[ -n ${PRESENT:-} ]] && cp "${PRESENT}" "$fake/ghcr/$digest"
  code=0
  out=$(env FAKE="$fake" DIGEST="$digest" SKOPEO="$tmp/skopeo" RUNNER_TEMP="$tmp/$name.runner" \
    GITHUB_TOKEN=token-for-test GITHUB_ACTOR=someone "$@" \
    bash "$root/packaging/mirror-image.sh" "$dir" 2>&1) || code=$?
  calls=$(cat "$fake/calls")
}

pinned=$(workflows pinned "$mirror@sha256:$digest # base-devel-20260927.0.600689")

PRESENT=$tmp/manifest run present "$pinned"
if [[ $code == 0 && $out == *"already on GHCR"* && $calls != *copy* ]]; then
  pass "on GHCR: no copy, Docker Hub not asked"
else
  fail "on GHCR (exit $code): $out / $calls"
fi

PRESENT= run absent "$pinned"
want="copy --all --preserve-digests --retry-times 3 --dest-authfile "
if [[ $code == 0 && $out == *"copied; GHCR serves the digest"* \
  && $calls == *"$want"*" docker://docker.io/library/archlinux@sha256:$digest docker://$mirror:base-devel-20260927.0.600689"* ]]; then
  pass "not on GHCR: copied by digest from Docker Hub to the tag, digests preserved"
else
  fail "not on GHCR (exit $code): $out / $calls"
fi
if [[ $(grep -c '^inspect' <<< "$calls") == 2 ]]; then
  pass "not on GHCR: checked again after the copy"
else
  fail "not on GHCR: inspect calls: $calls"
fi
if [[ -z $(ls -A "$tmp/absent.runner") ]]; then
  pass "the auth file is removed"
else
  fail "left in RUNNER_TEMP: $(ls -A "$tmp/absent.runner")"
fi

echo other > "$tmp/other"
PRESENT=$tmp/other run corrupt "$pinned"
if [[ $code == 0 && $calls == *copy* ]]; then
  pass "GHCR serves other bytes under the digest: copied again"
else
  fail "other bytes on GHCR (exit $code): $out"
fi

PRESENT= run refused "$pinned" FAKE_COPY=fail
if [[ $code == 2 && $out == *"the copy failed"*"Docker Hub"*"GHCR may be unavailable"*"packages: write"* ]]; then
  pass "a failed copy: exit 2, the hint names Docker Hub, GHCR and the token"
else
  fail "a failed copy (exit $code): $out"
fi

PRESENT= run wrong "$pinned" FAKE_COPY=wrong
if [[ $code == 2 && $out == *"does not serve"* ]]; then
  pass "a copy GHCR does not serve: exit 2"
else
  fail "a copy GHCR does not serve (exit $code): $out"
fi

two=$(workflows two "$mirror@sha256:$digest # base-devel-1" "$mirror@sha256:$(printf '%064d' 0) # base-devel-2")
[[ $two == "$pinned" ]] && fail "the second digest is the first"
PRESENT= run two "$two"
if [[ $code == 1 && $out == *"more than one digest"* && -z $calls ]]; then
  pass "two mirror digests: exit 1 before any registry call"
else
  fail "two mirror digests (exit $code): $out"
fi

hub=$(workflows hub "archlinux:base-devel@sha256:$digest # base-devel-1")
PRESENT= run hub "$hub"
if [[ $code == 1 && $out == *"no workflow"* && -z $calls ]]; then
  pass "no mirror pin: exit 1"
else
  fail "no mirror pin (exit $code): $out"
fi

PRESENT= run notoken "$pinned" GITHUB_TOKEN=
if [[ $code != 0 && $out == *"GITHUB_TOKEN is not set"* && -z $calls ]]; then
  pass "no token: refused before any registry call"
else
  fail "no token (exit $code): $out"
fi

# the real workflows pin exactly one mirror digest, the one ci.yml names,
# and the copy asks for it (the fake GHCR cannot serve it: exit 2)
real=$(sed -n -E 's|^ *image: ghcr\.io/johnandrewsx/jax-seldon/archlinux@sha256:([0-9a-f]{64}) # (.*)|\1 \2|p' "$root/.github/workflows/ci.yml")
PRESENT= run real "$root/.github/workflows"
if [[ -n $real && $out == *"mirror-image: $mirror@sha256:${real% *} (${real#* })"* \
  && $calls == *" docker://docker.io/library/archlinux@sha256:${real% *} docker://$mirror:${real#* }"* ]]; then
  pass "the real workflows: one mirror digest (${real% *}, ${real#* })"
else
  fail "the real workflows (exit $code): $out"
fi

if ((fails > 0)); then
  echo "mirror-image.test: $fails failure(s)" >&2
  exit 1
fi
echo "mirror-image.test: ok"
