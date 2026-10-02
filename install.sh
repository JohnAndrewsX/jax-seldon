#!/usr/bin/env bash
# install.sh — install, update or remove the Seldon engine from a GitHub release.
#
#   install.sh [--version vX.Y.Z] [--prefix DIR] [--unit]
#   install.sh --uninstall [--prefix DIR]
#
# Downloads seldon-X.Y.Z-x86_64-unknown-linux-musl.tar.gz and SHA256SUMS
# from the release (the latest one unless --version is given), checks the
# tarball with `sha256sum -c` and refuses on a mismatch before anything is
# written. Then installs <prefix>/bin/seldon and the symlink
# <prefix>/bin/jax-seldon -> seldon (prefix default ~/.local), with --unit
# also the optional watcher unit seldon-watch.service into
# ~/.config/systemd/user/ (installed, never enabled). What it wrote is
# listed in <prefix>/share/jax-seldon/install-manifest; --uninstall removes
# exactly those files and keeps your logbook and config.
#
# Runs as your user: no root, no package manager, no system files. A re-run
# with the same version changes nothing; a newer version updates in place.
#
# Exit codes: 0 ok; 1 usage error, or a refusal you can fix (a foreign
# file in the way, the unit still enabled); 2 download, verification or
# any other failure. Every refusal and failure comes before the first write.
#
# Test hooks (tests/install/install.test.sh): SELDON_INSTALL_API_URL and
# SELDON_INSTALL_DOWNLOAD_URL replace the GitHub URLs below.

set -euo pipefail

REPO="JohnAndrewsX/jax-seldon"
TARGET="x86_64-unknown-linux-musl"
API_URL="${SELDON_INSTALL_API_URL:-https://api.github.com/repos/$REPO/releases/latest}"
DOWNLOAD_URL="${SELDON_INSTALL_DOWNLOAD_URL:-https://github.com/$REPO/releases/download}"
UNIT_NAME="seldon-watch.service"
UNIT_EXEC_DEFAULT="ExecStart=%h/.local/bin/seldon watch"
PLUGIN_URL="https://github.com/JohnAndrewsX/jax-seldon-plugin.git"

say() { printf '%s\n' "$*"; }
warn() { printf 'install.sh: %s\n' "$*" >&2; }
usage_error() {
  warn "$*"
  warn "usage: install.sh [--version vX.Y.Z] [--prefix DIR] [--unit] | --uninstall [--prefix DIR]"
  exit 1
}
refuse() {
  warn "$*"
  exit 1
}
fail() {
  warn "$*"
  exit 2
}

usage() {
  cat <<'EOF'
install.sh — install, update or remove the Seldon engine from a GitHub release

  install.sh [--version vX.Y.Z] [--prefix DIR] [--unit]
  install.sh --uninstall [--prefix DIR]

  --version vX.Y.Z  install this release (default: the latest)
  --prefix DIR      install under DIR/bin (default: ~/.local)
  --unit            also install the optional watcher unit into
                    ~/.config/systemd/user/ (installed, not enabled)
  --uninstall       remove what this script installed under DIR; your
                    logbook, config and index stay
  -h, --help        this text

The download is checked against the release's SHA256SUMS; a mismatch
installs nothing. No root needed, never uses it.
EOF
}

# sha256 of a file, the hash only.
sha_of() {
  local sum
  sum=$(sha256sum -- "$1")
  printf '%s\n' "${sum%% *}"
}

need() {
  local tool
  for tool in "$@"; do
    command -v "$tool" >/dev/null 2>&1 || fail "needs '$tool', which is not installed"
  done
}

# https only (file:// for the tests), also after GitHub's redirect to its CDN.
CURL=(curl -fsSL --proto '=https,file' --proto-redir '=https' --retry 2)

fetch() { # url dest
  "${CURL[@]}" -o "$2" -- "$1"
}

# The tag of the latest release, from the GitHub API; jq when present,
# else the first "tag_name" in the response.
latest_tag() {
  local json tag
  json=$("${CURL[@]}" \
    -H 'Accept: application/vnd.github+json' -- "$API_URL") \
    || fail "could not ask GitHub for the latest release ($API_URL); try --version vX.Y.Z"
  if command -v jq >/dev/null 2>&1; then
    tag=$(jq -r '.tag_name // empty' <<<"$json") || tag=""
  else
    tag=$(sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' <<<"$json" | head -n 1)
  fi
  [[ -n $tag ]] || fail "no tag_name in the answer from $API_URL"
  printf '%s\n' "$tag"
}

# Paths and the hash of each file this script installed, one per line:
# "<sha256>  <path>" (a symlink records "symlink" and its target instead).
manifest_path() { printf '%s/share/jax-seldon/install-manifest\n' "$PREFIX"; }

# Writes $2 to $1 only when the content differs; never touches an equal file.
write_if_changed() { # dest content-file mode
  local dest=$1 src=$2 mode=$3 tmp
  if [[ -f $dest && ! -L $dest ]] && cmp -s -- "$src" "$dest"; then
    say "  unchanged  $dest"
    return 0
  fi
  mkdir -p -- "$(dirname -- "$dest")"
  tmp="$(dirname -- "$dest")/.$(basename -- "$dest").install.$$"
  install -m "$mode" -- "$src" "$tmp"
  mv -f -- "$tmp" "$dest"
  say "  installed  $dest"
}

do_install() {
  need curl sha256sum tar gzip install cmp
  [[ $(uname -m) == x86_64 ]] || fail "release binaries exist for x86_64 only (this is $(uname -m)); build from source instead"

  local tag
  if [[ -n $VERSION ]]; then
    tag=$VERSION
  else
    tag=$(latest_tag)
  fi
  [[ $tag =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]] || fail "unexpected release tag '$tag'"
  local version=${tag#v}
  local asset="seldon-$version-$TARGET.tar.gz"
  local stage="seldon-$version-$TARGET"

  WORK=$(mktemp -d)
  trap 'rm -rf -- "$WORK"' EXIT

  say "Seldon $version from $DOWNLOAD_URL/$tag"
  fetch "$DOWNLOAD_URL/$tag/$asset" "$WORK/$asset" \
    || fail "could not download $asset (does release $tag exist?)"
  fetch "$DOWNLOAD_URL/$tag/SHA256SUMS" "$WORK/SHA256SUMS" \
    || fail "could not download SHA256SUMS of $tag"

  # Only the tarball's own line; the file lists the other assets too.
  awk -v a="$asset" '$2 == a || $2 == "*" a' "$WORK/SHA256SUMS" > "$WORK/asset.sha256"
  [[ $(wc -l < "$WORK/asset.sha256") -eq 1 ]] \
    || fail "SHA256SUMS of $tag has no single line for $asset; nothing installed"
  (cd "$WORK" && sha256sum -c --strict --quiet asset.sha256) \
    || fail "checksum mismatch for $asset: the download does not match SHA256SUMS; nothing installed"
  say "  verified   $asset (sha256 $(sha_of "$WORK/$asset"))"

  tar -xzf "$WORK/$asset" -C "$WORK" "$stage/seldon" "$stage/$UNIT_NAME" \
    || fail "$asset does not have the expected layout ($stage/seldon, $stage/$UNIT_NAME)"
  local new_bin="$WORK/$stage/seldon"
  local got
  got=$("$new_bin" --version 2>/dev/null) || fail "the downloaded seldon does not run on this machine"
  [[ $got == "seldon $version" ]] || fail "the downloaded seldon says '$got', expected 'seldon $version'"

  local bin_dir="$PREFIX/bin"
  local link="$bin_dir/jax-seldon"
  local unit_dir="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
  local unit_path="$unit_dir/$UNIT_NAME"
  local manifest
  manifest=$(manifest_path)

  # Refusals come before the first write.
  if [[ -e $link || -L $link ]] && [[ ! -L $link || $(readlink -- "$link") != seldon ]]; then
    refuse "$link exists and is not the symlink to seldon this script makes; move it away first"
  fi
  local unit_src=""
  if [[ $UNIT == 1 ]]; then
    local exec_line
    case "$PREFIX" in
      *[[:space:]]* | *[\\\"\'%\$]*) refuse "--unit needs a prefix without spaces, quotes, % or \$ (systemd ExecStart)" ;;
      "$HOME/.local") exec_line=$UNIT_EXEC_DEFAULT ;;
      "$HOME"/*) exec_line="ExecStart=%h/${PREFIX#"$HOME"/}/bin/seldon watch" ;;
      *) exec_line="ExecStart=$PREFIX/bin/seldon watch" ;;
    esac
    grep -qxF "$UNIT_EXEC_DEFAULT" "$WORK/$stage/$UNIT_NAME" \
      || fail "the release's $UNIT_NAME has no '$UNIT_EXEC_DEFAULT' line; not installing it"
    unit_src="$WORK/unit"
    local line
    while IFS= read -r line; do
      if [[ $line == "$UNIT_EXEC_DEFAULT" ]]; then
        printf '%s\n' "$exec_line"
      else
        printf '%s\n' "$line"
      fi
    done < "$WORK/$stage/$UNIT_NAME" > "$unit_src"
  fi

  write_if_changed "$bin_dir/seldon" "$new_bin" 755
  if [[ -L $link ]]; then
    say "  unchanged  $link -> seldon"
  else
    ln -s seldon "$link"
    say "  installed  $link -> seldon"
  fi
  if [[ -n $unit_src ]]; then
    write_if_changed "$unit_path" "$unit_src" 644
  fi

  # The manifest lists everything installed so far under this prefix,
  # including a unit from an earlier --unit run.
  local entries="$WORK/manifest"
  {
    printf '# generated by install.sh (jax-seldon); --uninstall removes these\n'
    printf '%s  %s\n' "$(sha_of "$bin_dir/seldon")" "$bin_dir/seldon"
    printf 'symlink:seldon  %s\n' "$link"
    if [[ -n $unit_src ]]; then
      printf '%s  %s\n' "$(sha_of "$unit_path")" "$unit_path"
    elif [[ -f $manifest ]]; then
      grep -F "  $unit_path" "$manifest" || true
    fi
  } > "$entries"
  write_if_changed "$manifest" "$entries" 644

  say ""
  say "$("$bin_dir/seldon" --version) is installed in $bin_dir."
  case ":$PATH:" in
    *":$bin_dir:"*) ;;
    *) say "Note: $bin_dir is not on your PATH; add it, or the plugin cannot find seldon." ;;
  esac
  say ""
  say "Next steps:"
  say "  seldon init        create your logbook (once; skip it when you update)"
  say "  omarchy plugin add $PLUGIN_URL --enable"
  say "                     the bar pill, panel and Prime Radiant"
  if [[ -n $unit_src ]]; then
    say "  systemctl --user daemon-reload && systemctl --user enable --now seldon-watch"
    say "                     only if you want the optional watcher (it is not enabled)"
  fi
  say "Update: run install.sh again. Remove: install.sh --uninstall${PREFIX_ARG}"
}

do_uninstall() {
  need sha256sum
  local manifest
  manifest=$(manifest_path)
  if [[ ! -f $manifest ]]; then
    say "Nothing to remove: no $manifest (install.sh did not install Seldon under $PREFIX)."
    return 0
  fi

  local unit_dir="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
  local wants="$unit_dir/default.target.wants/$UNIT_NAME"
  if grep -qF "  $unit_dir/$UNIT_NAME" "$manifest" && [[ -L $wants ]]; then
    refuse "$UNIT_NAME is enabled; run 'systemctl --user disable --now seldon-watch' first. Nothing removed."
  fi

  local line sum path kept=0
  while IFS= read -r line; do
    [[ -z $line || $line == \#* ]] && continue
    sum=${line%%  *}
    path=${line#*  }
    if [[ $sum == symlink:* ]]; then
      if [[ -L $path && $(readlink -- "$path") == "${sum#symlink:}" ]]; then
        rm -f -- "$path"
        say "  removed    $path"
      elif [[ -e $path || -L $path ]]; then
        say "  kept       $path (changed since install)"
        kept=1
      fi
    elif [[ -f $path && ! -L $path ]]; then
      if [[ $(sha_of "$path") == "$sum" ]]; then
        rm -f -- "$path"
        say "  removed    $path"
      else
        say "  kept       $path (changed since install)"
        kept=1
      fi
    fi
  done < "$manifest"
  rm -f -- "$manifest"
  rmdir -- "$(dirname -- "$manifest")" 2>/dev/null || true
  say "  removed    $manifest"

  say ""
  say "Seldon's engine is removed from $PREFIX."
  [[ $kept == 0 ]] || say "Files changed since the install were kept (above); remove them yourself if you want."
  say "These stay: your logbook (~/Seldon unless you chose another place),"
  say "its config in ~/.config/seldon/ and the index in ~/.local/state/seldon/."
  say "The plugin, if installed, stays too: omarchy plugin remove jax.seldon"
}

main() {
  VERSION=""
  PREFIX="$HOME/.local"
  PREFIX_ARG=""
  UNIT=0
  local uninstall=0
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --version)
        [[ $# -ge 2 ]] || usage_error "--version needs a value"
        VERSION=$2
        shift 2
        ;;
      --version=*) VERSION=${1#*=}; shift ;;
      --prefix)
        [[ $# -ge 2 ]] || usage_error "--prefix needs a value"
        PREFIX=$2
        PREFIX_ARG=" --prefix $(printf '%q' "$2")"
        shift 2
        ;;
      --prefix=*) PREFIX=${1#*=}; PREFIX_ARG=" --prefix $(printf '%q' "${1#*=}")"; shift ;;
      --unit) UNIT=1; shift ;;
      --uninstall) uninstall=1; shift ;;
      -h | --help) usage; exit 0 ;;
      *) usage_error "unknown argument '$1'" ;;
    esac
  done

  [[ -n ${HOME:-} ]] || usage_error "HOME is not set"
  if [[ -n $VERSION ]]; then
    [[ $VERSION == v* ]] || VERSION="v$VERSION"
    [[ $VERSION =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]] || usage_error "--version wants vX.Y.Z, not '$VERSION'"
  fi
  [[ $PREFIX == /* ]] || usage_error "--prefix must be an absolute path, not '$PREFIX'"
  PREFIX=${PREFIX%/}
  [[ -n $PREFIX ]] || usage_error "--prefix / is not allowed"
  if [[ $(id -u) -eq 0 ]]; then
    warn "running as root: this installs for root only; run it as your user instead"
  fi

  if [[ $uninstall == 1 ]]; then
    [[ -z $VERSION && $UNIT == 0 ]] || usage_error "--uninstall takes only --prefix"
    do_uninstall
  else
    do_install
  fi
}

# Everything runs from here, so a truncated `curl … | bash` download does
# nothing at all.
main "$@"
