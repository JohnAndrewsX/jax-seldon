#!/usr/bin/env bash
# AGENTS.md §7: "No async runtime. No network." as a check (WP-195). Runs
# in `just check-packaging`; reads the crate graph offline, no network.
#
#   - the shipped crate graph (`cargo tree -e normal,build --all-features`:
#     every feature, no dev-dependencies, so `jsonschema` is out of it) has
#     no crate of the deny list below;
#   - engine/src (and engine/build.rs, if there is one) names no
#     `std::net`, `TcpStream`, `UdpSocket` or `TcpListener`.
#
# Usage: bash scripts/check-no-network.sh [--tree FILE] [--src DIR]
#   --tree FILE  read the crate graph from FILE (`cargo tree --prefix none
#                --format '{p}'` lines) instead of running cargo (tests)
#   --src DIR    scan DIR instead of engine/src and engine/build.rs (tests)
# Exit: 0 ok, 1 a denied crate or a network name in the source, 2 the
# crate graph could not be read.
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
tree_file=
src=()
while (($# > 0)); do
  case $1 in
    --tree) tree_file=${2:?--tree needs a file}; shift 2 ;;
    --src) src=("${2:?--src needs a directory}"); shift 2 ;;
    *) echo "check-no-network: unknown argument: $1" >&2; exit 2 ;;
  esac
done
if ((${#src[@]} == 0)); then
  src=("$root/engine/src")
  [[ -f $root/engine/build.rs ]] && src+=("$root/engine/build.rs")
fi

# Network clients and servers, TLS stacks, async runtimes and DNS
# resolvers; `mio` is allowed: it is notify's poll loop (feature `watch`),
# no socket is opened.
deny=(reqwest hyper h2 tokio async-std ureq curl isahc rustls native-tls openssl openssl-sys 'hickory-*' 'trust-dns-*')

if [[ -n $tree_file ]]; then
  tree=$(cat "$tree_file")
else
  if ! tree=$(cargo tree --manifest-path "$root/engine/Cargo.toml" --locked --offline \
    -e normal,build --all-features --prefix none --format '{p}'); then
    echo "check-no-network: cargo tree failed (an offline registry is needed: run cargo fetch or a build first)" >&2
    exit 2
  fi
fi
crates=$(awk 'NF { print $1 }' <<< "$tree" | LC_ALL=C sort -u)
if [[ -z $crates ]]; then
  echo "check-no-network: the crate graph is empty" >&2
  exit 2
fi

fails=0
while read -r crate; do
  for pattern in "${deny[@]}"; do
    # shellcheck disable=SC2053  # a glob on the right on purpose (hickory-*)
    if [[ $crate == $pattern ]]; then
      echo "check-no-network: denied crate in the shipped graph: $crate (AGENTS.md §7: no network, no async runtime; cargo tree -i $crate shows who pulls it in)" >&2
      fails=$((fails + 1))
    fi
  done
done <<< "$crates"

# `std::net` also as `use std::{io, net}`; the type names whatever the path
names='std::net\b|std::\{[^}]*\bnet\b|\b(TcpStream|UdpSocket|TcpListener)\b'
if hits=$(grep -r -n -E --include='*.rs' "$names" "${src[@]}"); then
  while IFS= read -r hit; do
    echo "check-no-network: network name in the engine source: ${hit#"$root"/}" >&2
    fails=$((fails + 1))
  done <<< "$hits"
fi

if ((fails > 0)); then
  exit 1
fi
echo "check-no-network: ok ($(wc -l <<< "$crates") crates, none denied; no network name in ${src[*]#"$root"/})"
