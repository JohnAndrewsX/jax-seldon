#!/usr/bin/env bash
# scripts/check-no-network.sh (WP-195) on fake crate graphs and scratch
# sources: a network crate or a network name in the source turns it red,
# an allowed crate does not. Runs in `just check-packaging`; reads no
# registry, no network. The real graph and engine/src are checked by the
# script itself in the same recipe.
#
# Usage: bash tests/release/no-network.test.sh
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
fails=0

pass() { echo "ok   $1"; }
fail() { echo "FAIL $1" >&2; fails=$((fails + 1)); }

# a shipped graph as `cargo tree --prefix none --format '{p}'` prints it
cat > "$tmp/clean.tree" <<'EOF'
seldon v0.1.4 (/build/engine)
anyhow v1.0.104
clap v4.6.7
clap_derive v4.6.7 (proc-macro)
serde v1.0.229
serde v1.0.229 (*)
notify v8.2.0
mio v1.2.3
inotify v0.11.5
libc v0.2.189
curly v0.1.0
tokio-free-parser v0.1.0
EOF
mkdir -p "$tmp/src/commands"
cat > "$tmp/src/main.rs" <<'EOF'
use std::{fs, io};
// a network word in prose: networked, netrc, std::network_name
fn main() -> io::Result<()> { fs::read("x").map(|_| ()) }
EOF
cp "$tmp/src/main.rs" "$tmp/src/commands/watch.rs"

# check NAME TREE SRC WANT-CODE [PATTERN]: the script on TREE and SRC
check() {
  local name=$1 tree=$2 src=$3 want=$4 pattern=${5:-} code=0 out
  out=$(bash "$root/scripts/check-no-network.sh" --tree "$tree" --src "$src" 2>&1) || code=$?
  if [[ $code == "$want" && ( -z $pattern || $out == *"$pattern"* ) ]]; then
    pass "$name"
  else
    fail "$name: exit $code, want $want${pattern:+ and \"$pattern\"}: $out"
  fi
}

# mutant MUTANT TEXT...: a copy of the clean tree with these crate lines
mutant() {
  local name=$1
  shift
  cp "$tmp/clean.tree" "$tmp/$name.tree"
  printf '%s\n' "$@" >> "$tmp/$name.tree"
  echo "$tmp/$name.tree"
}

check "clean graph and source: green (mio, curly and tokio-free-parser allowed)" "$tmp/clean.tree" "$tmp/src" 0 "none denied"

for crate in "reqwest v0.12.9" "hyper v1.5.0" "h2 v0.4.6" "tokio v1.41.0" "async-std v1.13.0" \
  "ureq v2.10.1" "curl v0.4.47" "isahc v1.7.2" "rustls v0.23.16" "native-tls v0.2.12" \
  "openssl v0.10.68" "openssl-sys v0.9.104" "hickory-resolver v0.24.1" "trust-dns-proto v0.23.2"; do
  name=${crate%% *}
  check "$name in the graph: red" "$(mutant "$name" "$crate")" "$tmp/src" 1 "denied crate in the shipped graph: $name"
done
check "reqwest deep in the graph, marked (*): red" "$(mutant deep "reqwest v0.12.9 (*)")" "$tmp/src" 1 "reqwest"
check "two denied crates: both named" "$(mutant two "tokio v1.41.0" "rustls v0.23.16")" "$tmp/src" 1 "rustls"

# the source mutants: one scratch file each
source_mutant() {
  local name=$1 line=$2
  rm -rf "$tmp/m"
  cp -r "$tmp/src" "$tmp/m"
  printf '%s\n' "$line" > "$tmp/m/commands/$name.rs"
  check "$name in the source: red" "$tmp/clean.tree" "$tmp/m" 1 "network name in the engine source"
}
source_mutant "std::net" 'use std::net::TcpStream;'
source_mutant "std::net path" 'fn f() { let _ = std::net::Ipv4Addr::LOCALHOST; }'
source_mutant "std::{net}" 'use std::{io, net};'
source_mutant "TcpListener" 'fn f(_: n::TcpListener) {}'
source_mutant "UdpSocket" 'type S = UdpSocket;'
source_mutant "TcpStream" 'type S = TcpStream;'

echo 'use std::net::TcpStream;' > "$tmp/notes.txt"
cp -r "$tmp/src" "$tmp/txt"
cp "$tmp/notes.txt" "$tmp/txt/"
check "a network name outside a .rs file: green" "$tmp/clean.tree" "$tmp/txt" 0

: > "$tmp/empty.tree"
check "an empty graph: exit 2" "$tmp/empty.tree" "$tmp/src" 2 "empty"

if ((fails > 0)); then
  echo "no-network.test: $fails failure(s)" >&2
  exit 1
fi
echo "no-network.test: ok"
