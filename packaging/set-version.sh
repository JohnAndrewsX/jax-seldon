#!/usr/bin/env bash
# Point packaging/PKGBUILD at a release: pkgver, pkgrel=1 and the source
# tarball's sha256. The release workflow runs it, then regenerates
# .SRCINFO with `makepkg --printsrcinfo` (packaging/README.md).
#
# Usage: packaging/set-version.sh VERSION SHA256
set -euo pipefail

version=${1:?usage: set-version.sh VERSION SHA256}
sum=${2:?usage: set-version.sh VERSION SHA256}
[[ $version =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "set-version: bad version '$version'" >&2; exit 1; }
[[ $sum =~ ^[0-9a-f]{64}$ ]] || { echo "set-version: bad sha256 '$sum'" >&2; exit 1; }

pkgbuild="$(dirname "$0")/PKGBUILD"
sed -i \
  -e "s/^pkgver=.*/pkgver=$version/" \
  -e "s/^pkgrel=.*/pkgrel=1/" \
  -e "s/^sha256sums=.*/sha256sums=('$sum')/" \
  "$pkgbuild"

# every line must have matched exactly once
[[ $(grep -c "^pkgver=$version\$" "$pkgbuild") == 1 ]]
[[ $(grep -c '^pkgrel=1$' "$pkgbuild") == 1 ]]
[[ $(grep -c "^sha256sums=('$sum')\$" "$pkgbuild") == 1 ]]
echo "set-version: PKGBUILD at $version ($sum)"
