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
# `mangen` unless the release is "old" (before WP-049).
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

# ---- recording sudo / systemctl, and a PATH without jq -----------------------
mkdir -p "$work/trap"
for tool in sudo systemctl; do
  # shellcheck disable=SC2016 # $* belongs to the recorder
  printf '#!/bin/sh\necho "%s $*" >>"%s/trap.log"\nexit 1\n' "$tool" "$work" >"$work/trap/$tool"
  chmod 755 "$work/trap/$tool"
done
mkdir -p "$work/nojq"
IFS=: read -ra path_dirs <<<"$PATH"
for dir in "${path_dirs[@]}"; do
  [[ -d $dir ]] || continue
  for f in "$dir"/*; do
    name=${f##*/}
    [[ $name == jq || -e $work/nojq/$name ]] && continue
    [[ -x $f ]] && ln -s "$f" "$work/nojq/$name"
  done
done
has_jq=0
command -v jq >/dev/null && has_jq=1

# ---- the shells of this scratch system: bash-completion and fish, no zsh -----
share="$work/usrshare"
mkdir -p "$share/bash-completion/completions" "$share/fish/vendor_completions.d"

# ---- runner ------------------------------------------------------------------
home="$work/home"
mkdir -p "$home"
# run <path-mode: jq|nojq> <api json> args... → $out, $rc
run_with() {
  local mode=$1 api=$2 path
  shift 2
  if [[ $mode == nojq ]]; then path="$work/trap:$work/nojq"; else path="$work/trap:$PATH"; fi
  rc=0
  out=$(env -i HOME="$home" PATH="$path" LANG=C.UTF-8 \
    SELDON_INSTALL_DOWNLOAD_URL="file://$work/releases/download" \
    SELDON_INSTALL_API_URL="file://$work/releases/$api" \
    SELDON_INSTALL_SHARE="$share" \
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
check "latest: next step seldon init" has "seldon init"
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
check "latest: no zsh completions without zsh" test ! -e "$p/share/zsh"
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
mkdir -p "$share/zsh/site-functions"
run --prefix "$work/p4z"
check "zsh: exit 0" test "$rc" -eq 0
check "zsh: completions installed" \
  grep -qx '# seldon 9.9.9 completions for zsh' "$work/p4z/share/zsh/site-functions/_seldon"
check "zsh: the fpath hint" has "fpath=($work/p4z/share/zsh/site-functions \$fpath)"
rmdir "$share/zsh/site-functions" "$share/zsh"
# zsh is gone: a re-run keeps its completion in the manifest for --uninstall
run --prefix "$work/p4z"
check "zsh gone: its completion stays listed" \
  grep -qF "  $work/p4z/share/zsh/site-functions/_seldon" "$work/p4z/share/jax-seldon/install-manifest"
run --uninstall --prefix "$work/p4z"
check "zsh gone: --uninstall removes its completion" test ! -e "$work/p4z/share/zsh/site-functions/_seldon"

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
out=$(env -i HOME="$home" PATH="$work/trap:$PATH" XDG_CONFIG_HOME="$work/xdg" \
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

# ---- 7. the one-liner form: script on stdin ----------------------------------
rc=0
out=$(env -i HOME="$home" PATH="$work/trap:$PATH" \
  SELDON_INSTALL_DOWNLOAD_URL="file://$work/releases/download" \
  SELDON_INSTALL_API_URL="file://$work/releases/latest.json" \
  bash -s -- --prefix "$work/p8" <"$script" 2>&1) || rc=$?
check "piped to bash: exit 0" test "$rc" -eq 0
check "piped to bash: installed" cmp -s "$work/p8/bin/seldon" "$work/build/seldon-9.9.9-$target/seldon"
# a truncated download defines main but never calls it
head -n -3 "$script" >"$work/truncated.sh"
rc=0
out=$(env -i HOME="$home" PATH="$work/trap:$PATH" \
  SELDON_INSTALL_DOWNLOAD_URL="file://$work/releases/download" \
  SELDON_INSTALL_API_URL="file://$work/releases/latest.json" \
  bash -s -- --prefix "$work/p9" <"$work/truncated.sh" 2>&1) || rc=$?
check "truncated script: does nothing" test ! -e "$work/p9"

# ---- 8. the release's own three-step check -----------------------------------
# shellcheck disable=SC2016 # $1 is bash -c's argument
check "three-step: install.sh verifies against SHA256SUMS" \
  bash -c 'cd "$1" && sha256sum -c --ignore-missing --quiet SHA256SUMS' _ "$work/releases/download/v9.9.9"

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
