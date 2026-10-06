#!/usr/bin/env bash
# install.sh against a local mock of the GitHub release layout (WP-044).
#
# Everything runs in a temp dir: the releases are served as file:// URLs
# (SELDON_INSTALL_DOWNLOAD_URL, SELDON_INSTALL_API_URL), HOME and
# XDG_CONFIG_HOME are scratch dirs, every install goes to a scratch prefix.
# No network. A recording `sudo` and `systemctl` first on PATH prove the
# script never calls either. The real ~/.local/bin/seldon, jax-seldon,
# ~/.config/systemd/user, the completions and the man page under
# ~/.local/share are fingerprinted before and compared after. The shells
# install.sh looks for live in a scratch /usr/share (SELDON_INSTALL_SHARE).
# The host's gh is never on PATH: a stub stands in for it (WP-080) and
# checks a download against the attestations the mock releases carry.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
script="$root/install.sh"
target=x86_64-unknown-linux-musl
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

pass=0
fail=0
ok() { pass=$((pass + 1)); echo "ok   $*"; }
bad() { fail=$((fail + 1)); echo "FAIL $*"; }
check() { # name command...
  local name=$1
  shift
  if "$@"; then ok "$name"; else bad "$name"; fi
}

if [[ $(uname -m) != x86_64 ]]; then
  echo "install.test: skipped (release binaries are x86_64 only; this is $(uname -m))"
  exit 0
fi

# ---- the real home, before ---------------------------------------------------
real_home=$HOME
real_fingerprint() {
  local p
  for p in "$real_home/.local/bin/seldon" "$real_home/.local/bin/jax-seldon" \
    "$real_home/.config/systemd/user" \
    "$real_home/.local/share/bash-completion/completions/seldon" \
    "$real_home/.local/share/zsh/site-functions/_seldon" \
    "$real_home/.local/share/fish/vendor_completions.d/seldon.fish" \
    "$real_home/.local/share/man/man1/seldon.1"; do
    if [[ -e $p || -L $p ]]; then
      find "$p" -printf '%p %y %s %T@ %C@ %l\n' 2>/dev/null | LC_ALL=C sort
    else
      echo "$p absent"
    fi
  done
}
real_before=$(real_fingerprint)

# ---- the mock releases -------------------------------------------------------
# $work/releases/download/vX.Y.Z/{seldon-X.Y.Z-<target>.tar.gz,SHA256SUMS,install.sh}
# and $work/releases/latest.json, as the GitHub API answers.
# The fake binary answers --version, and `completions <shell>` and
# `mangen` unless the release is "old" (before WP-049). Each tarball is
# attested for its tag: "<sha256> refs/tags/vX.Y.Z" in $work/attested, what
# the gh stub below looks up.
make_release() { # version [old]
  local v=$1 stage="seldon-$1-$target" dir="$work/releases/download/v$1"
  mkdir -p "$dir" "$work/build/$stage"
  {
    printf '#!/bin/sh\n'
    # shellcheck disable=SC2016 # $1 belongs to the fake binary
    printf '[ "$1" = --version ] && echo "seldon %s" && exit 0\n' "$v"
    if [[ ${2:-} != old ]]; then
      # shellcheck disable=SC2016 # $1 and $2 belong to the fake binary
      printf '[ "$1" = completions ] && echo "# seldon %s completions for $2" && exit 0\n' "$v"
      # shellcheck disable=SC2016 # $1 belongs to the fake binary
      printf '[ "$1" = mangen ] && echo ".TH SELDON 1 seldon-%s" && exit 0\n' "$v"
    fi
    printf 'echo "fake seldon %s" >&2\nexit 1\n' "$v"
  } >"$work/build/$stage/seldon"
  chmod 755 "$work/build/$stage/seldon"
  cp "$root/LICENSE" "$root/README.md" "$root/engine/systemd/seldon-watch.service" "$work/build/$stage/"
  cp "$root/engine/systemd/README.md" "$work/build/$stage/seldon-watch.md"
  tar czf "$dir/$stage.tar.gz" -C "$work/build" "$stage"
  printf 'source\n' | gzip >"$dir/jax-seldon-$v.tar.gz"
  cp "$script" "$dir/install.sh"
  (cd "$dir" && sha256sum -- * >SHA256SUMS)
  attest "$dir/$stage.tar.gz" "refs/tags/v$1"
}
attest() { # file ref
  printf '%s %s\n' "$(sha256sum <"$1" | cut -d' ' -f1)" "$2" >>"$work/attested"
}
make_release 9.9.8
make_release 9.9.9
make_release 9.9.4 old
printf '{\n  "url": "x",\n  "tag_name": "v9.9.9",\n  "name": "Seldon 9.9.9"\n}\n' >"$work/releases/latest.json"
printf '{"name":"Seldon 9.9.9","tag_name":"v9.9.9","draft":false}' >"$work/releases/latest-compact.json"

# v9.9.7: SHA256SUMS lies about the tarball (one hex digit changed).
make_release 9.9.7
sums="$work/releases/download/v9.9.7/SHA256SUMS"
awk '$2 ~ /^seldon-/ { c = substr($1, 1, 1); $1 = (c == "0" ? "1" : "0") substr($1, 2) } { print $1 "  " $2 }' \
  "$sums" >"$sums.new"
mv "$sums.new" "$sums"
# v9.9.6: SHA256SUMS has no line for the tarball.
make_release 9.9.6
sed -i "/seldon-9\.9\.6-/d" "$work/releases/download/v9.9.6/SHA256SUMS"
# v9.9.5: the tarball's binary prints another version.
make_release 9.9.5
stage="seldon-9.9.5-$target"
sed -i 's/seldon 9\.9\.5/seldon 1.0.0/' "$work/build/$stage/seldon"
tar czf "$work/releases/download/v9.9.5/$stage.tar.gz" -C "$work/build" "$stage"
(cd "$work/releases/download/v9.9.5" && rm SHA256SUMS && sha256sum -- * >SHA256SUMS)
attest "$work/releases/download/v9.9.5/$stage.tar.gz" refs/tags/v9.9.5
# v9.9.3: tarball and SHA256SUMS both replaced after the release (a binary
# that still says 9.9.3); its attestation is the original tarball's.
make_release 9.9.3
stage="seldon-9.9.3-$target"
echo "# tampered" >>"$work/build/$stage/seldon"
tar czf "$work/releases/download/v9.9.3/$stage.tar.gz" -C "$work/build" "$stage"
(cd "$work/releases/download/v9.9.3" && rm SHA256SUMS && sha256sum -- * >SHA256SUMS)
# v9.9.2: attested by a dry run of a branch, not for its tag.
make_release 9.9.2
sed -i 's| refs/tags/v9\.9\.2$| refs/heads/wp/080-review|' "$work/attested"
# v0.1.1: released before attestations; none exists for it.
make_release 0.1.1
sed -i '/ refs\/tags\/v0\.1\.1$/d' "$work/attested"
# v9.9.1: attested for its tag, but built on a self-hosted runner.
make_release 9.9.1
sed -i 's| refs/tags/v9\.9\.1$| refs/tags/v9.9.1 self-hosted|' "$work/attested"

# ---- gh stubs (WP-080) -------------------------------------------------------
# gh-ok verifies like `gh attestation verify FILE [--hostname H] --repo R
# [--signer-workflow W] [--source-ref REF] [--deny-self-hosted-runners]`:
# the file's sha256 must be in $work/attested (with REF, when given; not
# self-hosted, when denied); the host is --hostname, else $GH_HOST, else
# github.com, and only github.com has the attestations. gh-noauth is a gh
# that is not logged in (exit 4, as gh does); gh-fail exits 1 for a reason
# of its own (no Sigstore verifier behind a dead proxy); gh-partial lacks
# --deny-self-hosted-runners; gh-old has no `attestation` command; gh-none
# is no gh at all. Every call is logged to $work/gh.log.
mkdir -p "$work/gh-ok" "$work/gh-noauth" "$work/gh-fail" "$work/gh-partial" "$work/gh-old" "$work/gh-none"
cat >"$work/gh-ok/gh" <<'STUB'
#!/bin/sh
echo "gh $*" >>@WORK@/gh.log
[ "$1 $2" = "attestation verify" ] || { echo "unknown command \"$1\" for \"gh\"" >&2; exit 1; }
if [ "$3" = --help ]; then
  printf '      --deny-self-hosted-runners\n      --hostname string\n  -R, --repo string\n      --signer-workflow string\n      --source-ref string\n'
  exit 0
fi
file=$3 host=${GH_HOST:-github.com} repo="" workflow="" ref="" deny=""
shift 3
while [ $# -gt 0 ]; do
  case $1 in
    --deny-self-hosted-runners) deny=1; shift; continue ;;
    --hostname) host=$2 ;;
    --repo) repo=$2 ;;
    --signer-workflow) workflow=$2 ;;
    --source-ref) ref=$2 ;;
    *) echo "unknown flag: $1" >&2; exit 1 ;;
  esac
  shift 2
done
[ "$host" = github.com ] || { echo "Error: HTTP 404: Not Found (https://$host/api/v3/...)" >&2; exit 1; }
[ "$repo" = JohnAndrewsX/jax-seldon ] || { echo "Error: no attestations in $repo" >&2; exit 1; }
[ -z "$workflow" ] || [ "$workflow" = JohnAndrewsX/jax-seldon/.github/workflows/release.yml ] \
  || { echo "Error: signer workflow $workflow does not match" >&2; exit 1; }
sum=$(sha256sum <"$file" | cut -d' ' -f1)
while read -r s r h; do
  if [ "$s" = "$sum" ] && { [ -z "$ref" ] || [ "$r" = "$ref" ]; } && { [ -z "$deny" ] || [ "$h" != self-hosted ]; }; then
    echo "Verification succeeded!"
    exit 0
  fi
done <@WORK@/attested
echo "Error: no attestation for sha256:$sum matches" >&2
exit 1
STUB
cat >"$work/gh-noauth/gh" <<'STUB'
#!/bin/sh
echo "gh $*" >>@WORK@/gh.log
if [ "$3" = --help ]; then
  printf '      --deny-self-hosted-runners\n      --hostname string\n      --signer-workflow string\n      --source-ref string\n'
  exit 0
fi
echo "To get started with GitHub CLI, please run:  gh auth login" >&2
exit 4
STUB
cat >"$work/gh-fail/gh" <<'STUB'
#!/bin/sh
echo "gh $*" >>@WORK@/gh.log
if [ "$3" = --help ]; then
  printf '      --deny-self-hosted-runners\n      --hostname string\n      --signer-workflow string\n      --source-ref string\n'
  exit 0
fi
echo "Error: failed to create verifier: no valid Sigstore verifiers could be initialized" >&2
exit 1
STUB
cat >"$work/gh-partial/gh" <<'STUB'
#!/bin/sh
echo "gh $*" >>@WORK@/gh.log
if [ "$3" = --help ]; then
  printf '      --hostname string\n      --signer-workflow string\n      --source-ref string\n'
  exit 0
fi
echo "unknown flag: --deny-self-hosted-runners" >&2
exit 1
STUB
cat >"$work/gh-old/gh" <<'STUB'
#!/bin/sh
echo "gh $*" >>@WORK@/gh.log
echo "unknown command \"$1\" for \"gh\"" >&2
exit 1
STUB
sed -i "s|@WORK@|$work|g" "$work"/gh-*/gh
chmod 755 "$work"/gh-*/gh

# ---- recording sudo / systemctl, and a PATH without jq -----------------------
mkdir -p "$work/trap"
for tool in sudo systemctl; do
  # shellcheck disable=SC2016 # $* belongs to the recorder
  printf '#!/bin/sh\necho "%s $*" >>"%s/trap.log"\nexit 1\n' "$tool" "$work" >"$work/trap/$tool"
  chmod 755 "$work/trap/$tool"
done
# The host's programs without zsh and fish (install.sh looks for them with
# `command -v`; the fakes in $work/shells decide), and without jq too.
mkdir -p "$work/host" "$work/nojq" "$work/shells"
IFS=: read -ra path_dirs <<<"$PATH"
for dir in "${path_dirs[@]}"; do
  [[ -d $dir ]] || continue
  for f in "$dir"/*; do
    name=${f##*/}
    [[ $name == zsh || $name == fish || $name == gh || -e $work/host/$name ]] && continue
    [[ -x $f ]] || continue
    ln -s "$f" "$work/host/$name"
    [[ $name == jq ]] || ln -s "$f" "$work/nojq/$name"
  done
done
fake_shell() { # name
  printf '#!/bin/sh\nexit 0\n' >"$work/shells/$1"
  chmod 755 "$work/shells/$1"
}
has_jq=0
command -v jq >/dev/null && has_jq=1

# ---- the shells of this scratch system: bash-completion and fish; zsh has
# its directory but is not on PATH (both are needed) -------------------------
share="$work/usrshare"
mkdir -p "$share/bash-completion/completions" "$share/fish/vendor_completions.d" \
  "$share/zsh/site-functions"
fake_shell fish

# ---- runner ------------------------------------------------------------------
home="$work/home"
mkdir -p "$home"
# run <path-mode: jq|nojq> <api json> args... → $out, $rc; $GH picks the
# gh stub (ok, noauth, fail, partial, old, none); a non-empty $GH_HOST_ENV
# is passed on as GH_HOST; a non-empty $RUN_HOME replaces $home
GH=ok
GH_HOST_ENV=""
run_with() {
  local mode=$1 api=$2 path
  shift 2
  if [[ $mode == nojq ]]; then path="$work/nojq"; else path="$work/host"; fi
  path="$work/trap:$work/gh-$GH:$work/shells:$path"
  rc=0
  out=$(env -i HOME="${RUN_HOME:-$home}" PATH="$path" LANG=C.UTF-8 \
    SELDON_INSTALL_DOWNLOAD_URL="file://$work/releases/download" \
    SELDON_INSTALL_API_URL="file://$work/releases/$api" \
    SELDON_INSTALL_SHARE="$share" ${GH_HOST_ENV:+"GH_HOST=$GH_HOST_ENV"} \
    bash "$script" "$@" 2>&1) || rc=$?
}
run() { run_with jq latest.json "$@"; }

snap() { # dir → listing with types, sizes, mtimes, link targets and hashes
  [[ -e $1 ]] || { echo "absent"; return; }
  (cd "$1" && find . -printf '%p %y %s %T@ %l\n' | LC_ALL=C sort &&
    find . -type f -exec sha256sum {} + | LC_ALL=C sort)
}
has() { [[ $out == *"$1"* ]]; }

# ---- 1. fresh install of the latest release ----------------------------------
p="$work/p1"
run --prefix "$p"
check "latest: exit 0" test "$rc" -eq 0
check "latest: binary installed, executable" test -x "$p/bin/seldon"
check "latest: the 9.9.9 binary" cmp -s "$p/bin/seldon" "$work/build/seldon-9.9.9-$target/seldon"
check "latest: jax-seldon -> seldon" test "$(readlink "$p/bin/jax-seldon")" = seldon
check "latest: prints seldon --version" has "seldon 9.9.9 is installed in $p/bin."
check "latest: says it verified" has "verified   seldon-9.9.9-$target.tar.gz"
check "latest: says it checked the attestation" \
  has "attested   seldon-9.9.9-$target.tar.gz (built by release.yml for v9.9.9"
check "latest: gh got the repo, the release workflow and the tag's ref" grep -qE \
  "^gh attestation verify /.*/seldon-9\.9\.9-$target\.tar\.gz --hostname github\.com --repo JohnAndrewsX/jax-seldon --signer-workflow JohnAndrewsX/jax-seldon/\.github/workflows/release\.yml --source-ref refs/tags/v9\.9\.9 --deny-self-hosted-runners$" \
  "$work/gh.log"
# CI runs as root: then main()'s root warning (stderr) comes first, and
# the announce follows it; for a normal user the announce is line one
announce=$(grep -v '^install.sh: running as root: ' <<<"$out" | head -n 2)
check "latest: says first what it installs, where and how it checks" test "$announce" = \
  "Installing the Seldon engine into $p/bin as your user, no password;
the download is checked against the release checksums before anything is written."
if [[ $(id -u) -ne 0 ]]; then
  check "latest: as a normal user the announce is the first line" \
    test "$(head -n 1 <<<"$out")" = "Installing the Seldon engine into $p/bin as your user, no password;"
fi
check "latest: next steps on a fresh home: init and the plugin" has "
Next steps:
  seldon init        create your logbook
  omarchy plugin add https://github.com/JohnAndrewsX/jax-seldon-plugin.git --enable
                     the bar pill, panel and Prime Radiant
"
check "latest: update and remove after a blank line, last" test "$(tail -n 2 <<<"$out")" = "
Update: run install.sh again. Remove: install.sh --uninstall --prefix $p"
check "latest: PATH note" has "is not on your PATH"
check "latest: no unit without --unit" test ! -e "$home/.config/systemd/user/seldon-watch.service"
check "latest: manifest" test -f "$p/share/jax-seldon/install-manifest"
check "latest: bin/, the man page, bash and fish completions, the manifest" \
  test "$(cd "$p" && find . -type f -o -type l | LC_ALL=C sort | tr '\n' ' ')" = \
  "./bin/jax-seldon ./bin/seldon ./share/bash-completion/completions/seldon ./share/fish/vendor_completions.d/seldon.fish ./share/jax-seldon/install-manifest ./share/man/man1/seldon.1 "
check "latest: bash completions from the new binary" \
  grep -qx '# seldon 9.9.9 completions for bash' "$p/share/bash-completion/completions/seldon"
check "latest: fish completions" \
  grep -qx '# seldon 9.9.9 completions for fish' "$p/share/fish/vendor_completions.d/seldon.fish"
check "latest: no zsh completions without zsh on PATH" test ! -e "$p/share/zsh"
# shellcheck disable=SC2016 # $1 is bash -c's argument
check "latest: no zsh hint without zsh" bash -c '[[ $1 != *fpath=* ]]' _ "$out"
check "latest: the man page" grep -qx '.TH SELDON 1 seldon-9.9.9' "$p/share/man/man1/seldon.1"
check "latest: completions and man page in the manifest" \
  test "$(grep -cE '  .*/(seldon|seldon\.fish|seldon\.1)$' "$p/share/jax-seldon/install-manifest")" -eq 4
check "latest: nothing in HOME" test -z "$(find "$home" -mindepth 1)"
[[ $rc -eq 0 ]] || printf '     %s\n' "${out//$'\n'/$'\n'     }"

# ---- 2. idempotent re-run ----------------------------------------------------
before=$(snap "$p")
sleep 0.05
run --prefix "$p" --version v9.9.9
check "re-run: exit 0" test "$rc" -eq 0
check "re-run: prefix byte- and mtime-identical" test "$(snap "$p")" = "$before"
check "re-run: says unchanged" has "unchanged  $p/bin/seldon"

# ---- 3. update and downgrade (version without the v) -------------------------
run --prefix "$p" --version 9.9.8
check "downgrade: exit 0" test "$rc" -eq 0
check "downgrade: the 9.9.8 binary" cmp -s "$p/bin/seldon" "$work/build/seldon-9.9.8-$target/seldon"
check "downgrade: its completions" \
  grep -qx '# seldon 9.9.8 completions for bash' "$p/share/bash-completion/completions/seldon"
run --prefix "$p"
check "update: exit 0, back to 9.9.9" cmp -s "$p/bin/seldon" "$work/build/seldon-9.9.9-$target/seldon"
check "update: manifest hash follows" grep -qF "$(sha256sum "$p/bin/seldon" | cut -d' ' -f1)  $p/bin/seldon" \
  "$p/share/jax-seldon/install-manifest"

# ---- 4. latest without jq (sed fallback), pretty and compact JSON ------------
run_with nojq latest.json --prefix "$work/p4"
check "no jq: exit 0" test "$rc" -eq 0
check "no jq: 9.9.9 installed" cmp -s "$work/p4/bin/seldon" "$work/build/seldon-9.9.9-$target/seldon"
run_with nojq latest-compact.json --prefix "$work/p4b"
check "no jq, compact JSON: 9.9.9 installed" cmp -s "$work/p4b/bin/seldon" "$work/build/seldon-9.9.9-$target/seldon"
[[ $has_jq == 1 ]] || echo "note: jq is not installed here; the jq path ran as the fallback too"

# ---- 4b. completions: zsh, a release without them, a foreign file -----------
fake_shell zsh
run --prefix "$work/p4z"
check "zsh: exit 0" test "$rc" -eq 0
check "zsh: completions installed" \
  grep -qx '# seldon 9.9.9 completions for zsh' "$work/p4z/share/zsh/site-functions/_seldon"
check "zsh: the fpath hint" has "fpath=($work/p4z/share/zsh/site-functions \$fpath)"
rm "$work/shells/zsh"
# zsh is gone: a re-run keeps its completion in the manifest for --uninstall
run --prefix "$work/p4z"
check "zsh gone: its completion stays listed" \
  grep -qF "  $work/p4z/share/zsh/site-functions/_seldon" "$work/p4z/share/jax-seldon/install-manifest"
run --uninstall --prefix "$work/p4z"
check "zsh gone: --uninstall removes its completion" test ! -e "$work/p4z/share/zsh/site-functions/_seldon"
# fish on PATH without its completion directory: no fish completion
rmdir "$share/fish/vendor_completions.d"
run --prefix "$work/p4n"
check "fish without its directory: not installed" test ! -e "$work/p4n/share/fish"
check "fish without its directory: bash still installed" test -f "$work/p4n/share/bash-completion/completions/seldon"
mkdir -p "$share/fish/vendor_completions.d"

run --prefix "$work/p4o" --version v9.9.4
check "release without completions: exit 0" test "$rc" -eq 0
check "release without completions: says so" has "has no man page or shell completions; skipped"
check "release without completions: none installed" test ! -e "$work/p4o/share/man"
check "release without completions: none listed" \
  test "$(grep -c '/share/' "$work/p4o/share/jax-seldon/install-manifest")" -eq 0

mkdir -p "$work/p4f/share/bash-completion/completions"
echo "# my own" >"$work/p4f/share/bash-completion/completions/seldon"
run --prefix "$work/p4f"
check "foreign completion: exit 0" test "$rc" -eq 0
check "foreign completion: kept" grep -qx '# my own' "$work/p4f/share/bash-completion/completions/seldon"
check "foreign completion: says kept" has "kept       $work/p4f/share/bash-completion/completions/seldon"
# shellcheck disable=SC2016 # $1 and $2 are bash -c's arguments
check "foreign completion: not in the manifest" \
  bash -c '! grep -qF "  $1" "$2"' _ "$work/p4f/share/bash-completion/completions/seldon" \
  "$work/p4f/share/jax-seldon/install-manifest"
check "foreign completion: the others installed" test -f "$work/p4f/share/man/man1/seldon.1"
run --prefix "$work/p4f" --force
check "foreign completion, --force: replaced" \
  grep -qx '# seldon 9.9.9 completions for bash' "$work/p4f/share/bash-completion/completions/seldon"

# ---- 5. default prefix (~/.local) and --unit ---------------------------------
run --unit
unit="$home/.config/systemd/user/seldon-watch.service"
check "default prefix: exit 0" test "$rc" -eq 0
check "default prefix: ~/.local/bin/seldon" test -x "$home/.local/bin/seldon"
check "unit: installed" test -f "$unit"
check "unit: identical to engine/systemd" cmp -s "$unit" "$root/engine/systemd/seldon-watch.service"
check "unit: not enabled" test ! -e "$home/.config/systemd/user/default.target.wants"
check "unit: prints the enable hint" has "systemctl --user enable --now seldon-watch"
check "unit: in the manifest" grep -qF "  $unit" "$home/.local/share/jax-seldon/install-manifest"
before=$(snap "$home")
run --unit
check "unit: re-run changes nothing in HOME" test "$(snap "$home")" = "$before"
run
check "unit: a re-run without --unit keeps it in the manifest" \
  grep -qF "  $unit" "$home/.local/share/jax-seldon/install-manifest"

before=$(snap "$home")
run --unit --prefix "$home/opt/seldon"
check "unit of another prefix: refused, exit 1" test "$rc" -eq 1
check "unit of another prefix: names --force" has "Re-run with --force"
check "unit of another prefix: nothing changed" test "$(snap "$home")" = "$before"
run --unit --force --prefix "$home/opt/seldon"
check "unit, prefix under HOME: ExecStart with %h" grep -qx 'ExecStart=%h/opt/seldon/bin/seldon watch' "$unit"
run --unit --force --prefix "$work/p5"
check "unit, prefix outside HOME: absolute ExecStart" grep -qx "ExecStart=$work/p5/bin/seldon watch" "$unit"
run --unit --prefix "$work/with space"
check "unit, prefix with a space: refused, exit 1" test "$rc" -eq 1
check "unit, prefix with a space: nothing installed" test ! -e "$work/with space"
rc=0
out=$(env -i HOME="$home" PATH="$work/trap:$work/gh-ok:$PATH" XDG_CONFIG_HOME="$work/xdg" \
  SELDON_INSTALL_DOWNLOAD_URL="file://$work/releases/download" \
  SELDON_INSTALL_API_URL="file://$work/releases/latest.json" \
  bash "$script" --unit --prefix "$work/p5x" 2>&1) || rc=$?
check "unit: follows XDG_CONFIG_HOME" test -f "$work/xdg/systemd/user/seldon-watch.service"
# put the default-prefix unit back for the uninstall tests below
run --unit --force

# ---- 6. refusals: nothing installed ------------------------------------------
refused() { # name prefix message
  check "$1: non-zero exit" test "$rc" -ne 0
  check "$1: nothing installed" test ! -e "$2"
  check "$1: says why" has "$3"
}
run --prefix "$work/p6" --version v9.9.7
refused "checksum mismatch" "$work/p6" "checksum mismatch"
check "checksum mismatch: exit 2" test "$rc" -eq 2
run --prefix "$work/p6" --version v9.9.6
refused "no SHA256SUMS line" "$work/p6" "no single line"
run --prefix "$work/p6" --version v9.9.5
refused "wrong version inside" "$work/p6" "expected 'seldon 9.9.5'"
run --prefix "$work/p6" --version v1.2.3
refused "missing release" "$work/p6" "could not download"
run_with jq missing.json --prefix "$work/p6"
refused "API unreachable" "$work/p6" "could not ask GitHub"
printf '{"message":"Not Found"}' >"$work/releases/notag.json"
run_with nojq notag.json --prefix "$work/p6"
refused "no tag_name" "$work/p6" "no tag_name"
run --prefix "$work/p6" --version latest
refused "bad --version" "$work/p6" "wants vX.Y.Z"
check "bad --version: exit 1" test "$rc" -eq 1
run --prefix relative/dir
check "relative prefix: exit 1" test "$rc" -eq 1
check "relative prefix: nothing in cwd" test ! -e "relative"
run --bogus
check "unknown flag: exit 1" test "$rc" -eq 1
run --uninstall --version v9.9.9
check "--uninstall with --version: exit 1" test "$rc" -eq 1
run --help
check "--help: exit 0 and usage" test "$rc" -eq 0
check "--help: mentions --uninstall" has "--uninstall"

mkdir -p "$work/p7/bin"
echo "someone else's" >"$work/p7/bin/jax-seldon"
run --prefix "$work/p7"
check "foreign jax-seldon: refused, exit 1" test "$rc" -eq 1
check "foreign jax-seldon: seldon not written" test ! -e "$work/p7/bin/seldon"
check "foreign jax-seldon: kept" grep -q "someone else" "$work/p7/bin/jax-seldon"

# a self-built seldon (no manifest), and one rebuilt over an installed one
mkdir -p "$work/p11/bin"
printf '#!/bin/sh\necho "seldon 0.0.0-dev"\n' >"$work/p11/bin/seldon"
chmod 755 "$work/p11/bin/seldon"
before=$(snap "$work/p11")
run --prefix "$work/p11"
check "self-built seldon: refused, exit 1" test "$rc" -eq 1
check "self-built seldon: names --force" has "not installed by install.sh (a self-built seldon?); nothing changed. Re-run with --force"
check "self-built seldon: nothing changed" test "$(snap "$work/p11")" = "$before"
run --prefix "$work/p11" --force
check "self-built seldon, --force: exit 0" test "$rc" -eq 0
check "self-built seldon, --force: replaced" cmp -s "$work/p11/bin/seldon" "$work/build/seldon-9.9.9-$target/seldon"
check "self-built seldon, --force: in the manifest" test -f "$work/p11/share/jax-seldon/install-manifest"
printf '#!/bin/sh\necho "seldon 0.0.0-dev"\n' >"$work/p11/bin/seldon"
run --prefix "$work/p11" --version v9.9.8
check "rebuilt over an installed seldon: refused" test "$rc" -eq 1
check "rebuilt over an installed seldon: kept" grep -q 0.0.0-dev "$work/p11/bin/seldon"
mkdir -p "$work/p12/bin"
ln -s /bin/true "$work/p12/bin/seldon"
run --prefix "$work/p12"
check "seldon as a symlink: refused" test "$rc" -eq 1
check "seldon as a symlink: kept" test -L "$work/p12/bin/seldon"
run --uninstall --force
check "--uninstall with --force: exit 1" test "$rc" -eq 1

# ---- 6b. build provenance (WP-080): gh ok, failing, absent, not logged in,
# too old; with and without --require-verified, --skip-provenance -----------
provenance() { # name gh-mode args... → runs into a fresh prefix $pp
  local name=$1
  GH=$2
  shift 2
  pp="$work/pv-$name"
  run --prefix "$pp" "$@"
  GH=ok
}
installed() { cmp -s "$pp/bin/seldon" "$work/build/seldon-$1-$target/seldon"; }
count() { grep -cF -- "$1" <<<"$out" || true; }

provenance ok-req ok --require-verified
check "gh ok, --require-verified: exit 0" test "$rc" -eq 0
check "gh ok, --require-verified: installed" installed 9.9.9
check "gh ok, --require-verified: attested" has "attested   seldon-9.9.9-$target.tar.gz"

provenance tampered ok --version v9.9.3
refused "tampered tarball, gh ok" "$pp" "gh attestation verify did not confirm seldon-9.9.3-$target.tar.gz as built by JohnAndrewsX/jax-seldon's release workflow for v9.9.3 (gh's reason above); nothing installed"
check "tampered tarball, gh ok: names --skip-provenance" has "--skip-provenance installs on the checksum alone"
check "tampered tarball, gh ok: exit 2" test "$rc" -eq 2
check "tampered tarball, gh ok: checksum passed first" has "verified   seldon-9.9.3-$target.tar.gz"
check "tampered tarball, gh ok: gh's own words shown" has "gh: Error: no attestation for sha256:"
provenance tampered-req ok --version v9.9.3 --require-verified
refused "tampered tarball, gh ok, --require-verified" "$pp" "gh attestation verify did not confirm"
check "tampered tarball, gh ok, --require-verified: exit 2" test "$rc" -eq 2

provenance branch ok --version v9.9.2
refused "attested by a branch dry run, not the tag" "$pp" "gh attestation verify did not confirm"
provenance selfhosted ok --version v9.9.1
refused "attested from a self-hosted runner" "$pp" "gh attestation verify did not confirm"
GH_HOST_ENV=ghe.example.invalid
provenance ghhost ok
GH_HOST_ENV=""
check "GH_HOST of another server: still checked on github.com, exit 0" test "$rc" -eq 0
check "GH_HOST of another server: attested" has "attested   seldon-9.9.9-$target.tar.gz"

provenance fail fail
refused "gh failing on its own (no Sigstore verifier)" "$pp" "gh attestation verify did not confirm seldon-9.9.9-$target.tar.gz"
check "gh failing on its own: exit 2" test "$rc" -eq 2
check "gh failing on its own: gh's reason shown" has "gh: Error: failed to create verifier"
: >"$work/gh.log"
provenance fail-skip fail --skip-provenance
check "gh failing, --skip-provenance: exit 0" test "$rc" -eq 0
check "gh failing, --skip-provenance: installed" installed 9.9.9
check "gh failing, --skip-provenance: the note" \
  has "note       build provenance not checked at your request; only the SHA256SUMS checksum was checked"
check "gh failing, --skip-provenance: gh not asked" test ! -s "$work/gh.log"
provenance tampered-skip ok --version v9.9.3 --skip-provenance
check "tampered tarball, --skip-provenance: the checksum alone decides (installed)" test "$rc" -eq 0
provenance both ok --require-verified --skip-provenance
check "--require-verified with --skip-provenance: exit 1" test "$rc" -eq 1
check "--require-verified with --skip-provenance: says why" has "exclude each other"
check "--require-verified with --skip-provenance: nothing installed" test ! -e "$pp"

provenance none none
check "no gh: exit 0" test "$rc" -eq 0
check "no gh: installed" installed 9.9.9
check "no gh: one note line, checksum only" test "$(count "only the SHA256SUMS checksum was checked")" -eq 1
check "no gh: the note names gh" has "note       the GitHub CLI (gh) is not installed: only the SHA256SUMS checksum was checked"
# shellcheck disable=SC2016 # $1 is bash -c's argument
check "no gh: not attested" bash -c '[[ $1 != *"attested   "* ]]' _ "$out"
provenance none-tampered none --version v9.9.3
check "no gh, tampered tarball: the checksum alone decides (installed)" test "$rc" -eq 0
provenance none-req none --require-verified
refused "no gh, --require-verified" "$pp" "--require-verified: the GitHub CLI (gh) is not installed"
check "no gh, --require-verified: exit 1" test "$rc" -eq 1

provenance noauth noauth
check "gh not logged in: exit 0" test "$rc" -eq 0
check "gh not logged in: installed" installed 9.9.9
check "gh not logged in: says so once" test "$(count "gh is not logged in (gh auth login): only the SHA256SUMS checksum")" -eq 1
provenance noauth-req noauth --require-verified
refused "gh not logged in, --require-verified" "$pp" "--require-verified: gh is not logged in"
check "gh not logged in, --require-verified: exit 1" test "$rc" -eq 1

provenance old old
check "gh without attestation: exit 0" test "$rc" -eq 0
check "gh without attestation: installed" installed 9.9.9
check "gh without attestation: says update gh" has "this gh's 'gh attestation verify' has no --hostname (update gh): only the SHA256SUMS"
provenance old-req old --require-verified
refused "gh without attestation, --require-verified" "$pp" "--require-verified: this gh's 'gh attestation verify' has no"
check "gh without attestation, --require-verified: exit 1" test "$rc" -eq 1
provenance partial partial
check "gh without --deny-self-hosted-runners: exit 0" test "$rc" -eq 0
check "gh without --deny-self-hosted-runners: says update gh" \
  has "has no --deny-self-hosted-runners (update gh): only the SHA256SUMS"

: >"$work/gh.log"
provenance pre ok --version v0.1.1
check "release before attestations: exit 0" test "$rc" -eq 0
check "release before attestations: installed" installed 0.1.1
check "release before attestations: says so" has "v0.1.1 was released before attestations (they start after v0.1.1): only the SHA256SUMS"
check "release before attestations: gh not asked" test ! -s "$work/gh.log"
provenance pre-req ok --version v0.1.1 --require-verified
refused "release before attestations, --require-verified" "$pp" "--require-verified: v0.1.1 was released before attestations"
check "release before attestations, --require-verified: exit 1" test "$rc" -eq 1

run --uninstall --require-verified
check "--uninstall with --require-verified: exit 1" test "$rc" -eq 1
run --uninstall --skip-provenance
check "--uninstall with --skip-provenance: exit 1" test "$rc" -eq 1
check "--uninstall with --skip-provenance: usage error" has "--uninstall takes only --prefix"
run --help
check "--help: describes --require-verified" has "--require-verified"
check "--help: describes --skip-provenance" has "--skip-provenance do not ask gh"
check "--help: names gh attestation verify" has "gh attestation verify"

# ---- 7. the one-liner form: script on stdin ----------------------------------
rc=0
out=$(env -i HOME="$home" PATH="$work/trap:$work/gh-ok:$PATH" \
  SELDON_INSTALL_DOWNLOAD_URL="file://$work/releases/download" \
  SELDON_INSTALL_API_URL="file://$work/releases/latest.json" \
  bash -s -- --prefix "$work/p8" <"$script" 2>&1) || rc=$?
check "piped to bash: exit 0" test "$rc" -eq 0
check "piped to bash: installed" cmp -s "$work/p8/bin/seldon" "$work/build/seldon-9.9.9-$target/seldon"
# a truncated download defines main but never calls it
head -n -3 "$script" >"$work/truncated.sh"
rc=0
out=$(env -i HOME="$home" PATH="$work/trap:$work/gh-ok:$PATH" \
  SELDON_INSTALL_DOWNLOAD_URL="file://$work/releases/download" \
  SELDON_INSTALL_API_URL="file://$work/releases/latest.json" \
  bash -s -- --prefix "$work/p9" <"$work/truncated.sh" 2>&1) || rc=$?
check "truncated script: does nothing" test ! -e "$work/p9"

# ---- 8. the release's own three-step check -----------------------------------
# shellcheck disable=SC2016 # $1 is bash -c's argument
check "three-step: install.sh verifies against SHA256SUMS" \
  bash -c 'cd "$1" && sha256sum -c --ignore-missing --quiet SHA256SUMS' _ "$work/releases/download/v9.9.9"

# ---- 8b. next steps from what is there (WP-118) -------------------------------
# Each case in a home of its own; the default prefix shows as ~/.local/bin.
plugin_line="omarchy plugin add https://github.com/JohnAndrewsX/jax-seldon-plugin.git --enable"
next_home() { # name [config] [plugin] → $RUN_HOME prepared
  RUN_HOME="$work/homes/$1"
  mkdir -p "$RUN_HOME"
  if one_of config "${@:2}"; then
    mkdir -p "$RUN_HOME/.config/seldon"
    echo 'logbook = "~/Seldon"' >"$RUN_HOME/.config/seldon/config.toml"
  fi
  if one_of plugin "${@:2}"; then
    mkdir -p "$RUN_HOME/.config/omarchy/plugins/jax.seldon"
  fi
}
# one_of <needle> items... → true when the needle is one of the items
one_of() { local n=$1 i; shift; for i in "$@"; do [[ $i == "$n" ]] && return 0; done; return 1; }

next_home fresh
run
check "fresh home: exit 0" test "$rc" -eq 0
check "fresh home: announce names ~/.local/bin" \
  has "Installing the Seldon engine into ~/.local/bin as your user, no password;"
check "fresh home: seldon init" has "  seldon init        create your logbook"$'\n'
check "fresh home: the plugin line" has "  $plugin_line"

next_home update config
run
check "with a config: exit 0" test "$rc" -eq 0
# shellcheck disable=SC2016 # $1 is bash -c's argument
check "with a config: no seldon init" bash -c '[[ $1 != *"seldon init"* ]]' _ "$out"
check "with a config: says the logbook is set up" has "Your logbook is already set up.
Next steps:
  $plugin_line
"

next_home panel plugin
run
check "with the plugin: exit 0" test "$rc" -eq 0
# shellcheck disable=SC2016 # $1 is bash -c's argument
check "with the plugin: no plugin line" bash -c '[[ $1 != *"plugin add"* ]]' _ "$out"
check "with the plugin: init, or Create in the panel" has "
Next steps:
  seldon init        create your logbook (or press Create in the Seldon panel)

Update: run install.sh again."

next_home both config plugin
run
check "with both: exit 0" test "$rc" -eq 0
check "with both: nothing else to do" has "
Your logbook is already set up; nothing else to do.

Update: run install.sh again."
# shellcheck disable=SC2016 # $1 is bash -c's argument
check "with both: no next steps" bash -c '[[ $1 != *"Next steps"* ]]' _ "$out"
check "with both: config and plugin folder untouched" test \
  "$(cat "$RUN_HOME/.config/seldon/config.toml")" = 'logbook = "~/Seldon"'

# XDG_CONFIG_HOME, as the engine reads it: the config there counts
RUN_HOME="$work/homes/xdg"
mkdir -p "$RUN_HOME/xdg/seldon" "$RUN_HOME/.config/omarchy/plugins/jax.seldon"
touch "$RUN_HOME/xdg/seldon/config.toml"
rc=0
out=$(env -i HOME="$RUN_HOME" XDG_CONFIG_HOME="$RUN_HOME/xdg" PATH="$work/trap:$work/gh-ok:$work/shells:$work/host" \
  SELDON_INSTALL_DOWNLOAD_URL="file://$work/releases/download" \
  SELDON_INSTALL_API_URL="file://$work/releases/latest.json" \
  SELDON_INSTALL_SHARE="$share" bash "$script" 2>&1) || rc=$?
check "XDG_CONFIG_HOME config: nothing else to do" has "Your logbook is already set up; nothing else to do."
# a relative XDG_CONFIG_HOME is ignored, as by the engine: ~/.config counts,
# and a config under the relative path in the working directory does not
next_home relxdg config plugin
mkdir -p "$RUN_HOME/cwd/rel" "$RUN_HOME/cwd2/rel/seldon"
touch "$RUN_HOME/cwd2/rel/seldon/config.toml"
rc=0
out=$(cd "$RUN_HOME/cwd" && env -i HOME="$RUN_HOME" XDG_CONFIG_HOME=rel PATH="$work/trap:$work/gh-ok:$work/shells:$work/host" \
  SELDON_INSTALL_DOWNLOAD_URL="file://$work/releases/download" \
  SELDON_INSTALL_API_URL="file://$work/releases/latest.json" \
  SELDON_INSTALL_SHARE="$share" bash "$script" 2>&1) || rc=$?
check "relative XDG_CONFIG_HOME: ~/.config counts" has "Your logbook is already set up; nothing else to do."
rm "$RUN_HOME/.config/seldon/config.toml"
rc=0
out=$(cd "$RUN_HOME/cwd2" && env -i HOME="$RUN_HOME" XDG_CONFIG_HOME=rel PATH="$work/trap:$work/gh-ok:$work/shells:$work/host" \
  SELDON_INSTALL_DOWNLOAD_URL="file://$work/releases/download" \
  SELDON_INSTALL_API_URL="file://$work/releases/latest.json" \
  SELDON_INSTALL_SHARE="$share" bash "$script" 2>&1) || rc=$?
check "relative XDG_CONFIG_HOME: a config under it does not count" has "  seldon init        create your logbook"
RUN_HOME=""

# ---- 9. uninstall ------------------------------------------------------------
mkdir -p "$home/.config/systemd/user/default.target.wants"
ln -s ../seldon-watch.service "$home/.config/systemd/user/default.target.wants/seldon-watch.service"
before=$(snap "$home")
run --uninstall
check "uninstall, unit enabled: refused" test "$rc" -eq 1
check "uninstall, unit enabled: says disable first" has "systemctl --user disable --now seldon-watch"
check "uninstall, unit enabled: nothing removed" test "$(snap "$home")" = "$before"
rm -r "$home/.config/systemd/user/default.target.wants"

run --uninstall
check "uninstall: exit 0" test "$rc" -eq 0
check "uninstall: seldon removed" test ! -e "$home/.local/bin/seldon"
check "uninstall: jax-seldon removed" test ! -L "$home/.local/bin/jax-seldon"
check "uninstall: unit removed" test ! -e "$unit"
check "uninstall: completions removed" test ! -e "$home/.local/share/bash-completion/completions/seldon"
check "uninstall: man page removed" test ! -e "$home/.local/share/man/man1/seldon.1"
check "uninstall: manifest removed" test ! -e "$home/.local/share/jax-seldon"
check "uninstall: says what stays" has "These stay: your logbook"
check "uninstall: the opt prefix untouched" test -x "$home/opt/seldon/bin/seldon"
run --uninstall
check "uninstall twice: exit 0" test "$rc" -eq 0
check "uninstall twice: nothing to remove" has "Nothing to remove"

run --prefix "$work/p10"
echo "# my edit" >>"$work/p10/bin/seldon"
run --uninstall --prefix "$work/p10"
check "uninstall, edited binary: kept" test -f "$work/p10/bin/seldon"
check "uninstall, edited binary: says kept" has "kept       $work/p10/bin/seldon"
check "uninstall, edited binary: symlink removed" test ! -L "$work/p10/bin/jax-seldon"

run --uninstall --prefix "$work/never"
check "uninstall, never installed: exit 0" test "$rc" -eq 0
check "uninstall, never installed: nothing created" test ! -e "$work/never"

# ---- 10. never sudo, never systemctl; the real home untouched ----------------
check "no sudo or systemctl call" test ! -e "$work/trap.log"
[[ ! -e $work/trap.log ]] || sed 's/^/     /' "$work/trap.log"
check "real ~/.local/bin and ~/.config/systemd/user untouched" test "$(real_fingerprint)" = "$real_before"

# ---- 11. shellcheck ----------------------------------------------------------
if command -v shellcheck >/dev/null; then
  check "shellcheck install.sh" shellcheck "$script"
  check "shellcheck install.test.sh" shellcheck "$0"
else
  echo "note: shellcheck not installed; bash -n only (the release workflow runs shellcheck)"
  check "bash -n install.sh" bash -n "$script"
fi

echo "install.test: $pass passed, $fail failed"
[[ $fail -eq 0 ]]
