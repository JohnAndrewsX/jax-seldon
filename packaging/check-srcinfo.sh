#!/usr/bin/env bash
# Check that packaging/.SRCINFO matches packaging/PKGBUILD without makepkg
# (it is not available, or not allowed, on every host that runs `just
# check`). Renders the fields this PKGBUILD can carry in the order of
# makepkg's srcinfo.sh and diffs. The authoritative check is
# `makepkg --printsrcinfo`, which the release workflow runs.
# The PKGBUILD's variables come from `source`, which shellcheck cannot
# follow: SC1090 (non-constant source), SC2154 (pkgname etc. unassigned).
# shellcheck disable=SC1090,SC2154
set -euo pipefail

dir=$(dirname "$0")

render() (
  # a PKGBUILD only assigns variables at the top level; the functions are
  # defined, not run
  # shellcheck disable=SC1091  # the PKGBUILD is data, not a script to follow
  source "$dir/PKGBUILD"
  printf 'pkgbase = %s\n' "$pkgname"
  for f in pkgdesc pkgver pkgrel epoch url install changelog; do
    [[ -n ${!f:-} ]] && printf '\t%s = %s\n' "$f" "${!f}"
  done
  for f in arch groups license checkdepends makedepends depends optdepends \
           provides conflicts replaces noextract options backup source \
           validpgpkeys cksums md5sums sha1sums sha224sums sha256sums \
           sha384sums sha512sums b2sums; do
    declare -n ref=$f 2>/dev/null || continue
    [[ -v ref[@] ]] || { unset -n ref; continue; }
    for v in "${ref[@]}"; do printf '\t%s = %s\n' "$f" "$v"; done
    unset -n ref
  done
  printf '\npkgname = %s\n' "$pkgname"
)

if ! diff -u "$dir/.SRCINFO" <(render); then
  echo "check-srcinfo: packaging/.SRCINFO is out of date (regenerate with makepkg --printsrcinfo)" >&2
  exit 1
fi
echo "check-srcinfo: ok"
