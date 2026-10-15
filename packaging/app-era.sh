#!/usr/bin/env bash
# Exit 0 when VERSION is an app-era release (0.5.0 or newer): the release
# workflow builds the `seldon` package (packaging/seldon/) only then; before
# that the `jax-seldon` package (packaging/PKGBUILD) stays the only one
# (WP-230; packaging/README.md, "The seldon package").
#
# Usage: packaging/app-era.sh VERSION
set -euo pipefail

version=${1:?usage: app-era.sh VERSION}
[[ $version =~ ^([0-9]+)\.([0-9]+)\.[0-9]+$ ]] || { echo "app-era: bad version '$version'" >&2; exit 2; }
major=${BASH_REMATCH[1]}
minor=${BASH_REMATCH[2]}

(( major > 0 || minor >= 5 ))
