#!/usr/bin/env bash
# Omarchy's shell tree at the commit packaging/omarchy-pin names, for CI's
# QML checks (WP-191, ADR-0055): the `plugin-qml` job of ci.yml runs
# `just qmllint`, `just plugin-test` and Omarchy's text-format scan with
# OMARCHY_PATH=DEST. A sparse, blob-filtered fetch of that one commit over
# HTTPS, with only what those checks read:
#   shell/                                   the harnesses copy Commons/ and Ui/
#   themes/*/colors.toml                     the harnesses' themes
#   default/pacman/mirrorlist-*              the channel mirror CI installs from
#   test/shell.d/qml-text-format-scan.py     Omarchy's QML text-format scan
# Then it proves that DEST is the pinned commit and that the tree is
# self-contained for those checks, and prints its digest. The pin mirrors
# the test host's installed tree; it never replaces it as the reference
# (AGENTS.md §1).
#
# Usage: packaging/omarchy-tree.sh DEST           (DEST must not exist)
#        packaging/omarchy-tree.sh --digest DIR   (the digest of an
#          installed tree, e.g. /usr/share/omarchy on the test host)
#
# The digest: sha256 over `sha256sum` of every file under shell/ and of
# themes/*/colors.toml, paths relative to the tree, sorted bytewise.
#
# Tests only (tests/release/omarchy-tree.test.sh): SELDON_OMARCHY_PIN names
# another pin file (as for omarchy-validate.sh); SELDON_OMARCHY_GIT
# replaces https://github.com and may then be a file:// URL.
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
git_base=${SELDON_OMARCHY_GIT:-https://github.com}

# The themes the harnesses render (bar-view.sh, desk-view.sh, ipc-restart.sh).
themes=(tokyo-night catppuccin-latte osaka-jade)
scan=test/shell.d/qml-text-format-scan.py

fail() { echo "omarchy-tree: $*" >&2; exit 1; }

# digest DIR: the tree digest of DIR (see the top)
digest() {
  (
    cd "$1"
    shopt -s nullglob
    files=(themes/*/colors.toml)
    { find shell -type f; printf '%s\n' "${files[@]}"; } | LC_ALL=C sort | while IFS= read -r f; do
      sha256sum -- "$f"
    done | sha256sum | cut -d' ' -f1
  )
}

arg=${1:?usage: omarchy-tree.sh DEST | --digest DIR}
if [[ $arg == --digest ]]; then
  dir=${2:?usage: omarchy-tree.sh --digest DIR}
  [[ -d $dir/shell ]] || fail "no shell/ under $dir"
  echo "$(digest "$dir")  $dir"
  exit 0
fi

dest=$arg
[[ ! -e $dest ]] || fail "$dest exists; name a new directory"
pin=$(bash "$root/packaging/omarchy-validate.sh" --print-pin) || fail "cannot read the pin"
read -r repo commit _ <<< "$pin"

# HTTPS only; a file:// base only when a test set one
allow=(-c protocol.allow=never -c protocol.https.allow=always)
[[ $git_base == file://* && -n ${SELDON_OMARCHY_GIT:-} ]] \
  && allow=(-c protocol.allow=never -c protocol.file.allow=always)
url="$git_base/$repo.git"

git init -q "$dest"
git -C "$dest" sparse-checkout set --no-cone \
  '/shell/' '/themes/*/colors.toml' '/default/pacman/mirrorlist-*' "/$scan"
git "${allow[@]}" -C "$dest" fetch -q --depth 1 --filter=blob:none "$url" "$commit" \
  || fail "cannot fetch $commit from $url"
git "${allow[@]}" -C "$dest" checkout -q --detach FETCH_HEAD \
  || fail "cannot check out $commit"
head=$(git -C "$dest" rev-parse HEAD)
[[ $head == "$commit" ]] || fail "checked out $head, the pin says $commit"

# Self-contained for the checks: every file the Commons and Ui qmldir files
# name exists, and their QML and JS import only Qt, Quickshell, qs.Commons,
# qs.Ui and files inside shell/Commons or shell/Ui.
problems=()
for module in Commons Ui; do
  qmldir=$dest/shell/$module/qmldir
  if [[ ! -f $qmldir ]]; then
    problems+=("shell/$module/qmldir is missing")
    continue
  fi
  while read -r f; do
    [[ -f $dest/shell/$module/$f ]] || problems+=("shell/$module/qmldir names $f, which is missing")
  done < <(awk '$NF ~ /\.(qml|js)$/ { print $NF }' "$qmldir")
done
inside=$(cd "$dest/shell" && pwd -P)
shopt -s nullglob
for file in "$dest"/shell/Commons/*.qml "$dest"/shell/Commons/*.js "$dest"/shell/Ui/*.qml "$dest"/shell/Ui/*.js; do
  rel=${file#"$dest"/}
  while IFS=: read -r n line; do
    if [[ $line =~ ^[[:space:]]*\.?import[[:space:]]+\"([^\"]+)\" ]]; then
      target=$(cd "$(dirname "$file")" && realpath -m -- "${BASH_REMATCH[1]}")
      if [[ ! -e $target ]]; then
        problems+=("$rel:$n imports ${BASH_REMATCH[1]}, which is missing")
      elif [[ $target != "$inside/Commons/"* && $target != "$inside/Ui/"* ]]; then
        problems+=("$rel:$n imports ${BASH_REMATCH[1]}, outside shell/Commons and shell/Ui")
      fi
    elif [[ ! $line =~ ^[[:space:]]*\.?import[[:space:]]+(Qt[A-Za-z0-9.]*|Quickshell[A-Za-z0-9.]*|qs\.Commons|qs\.Ui)([[:space:]]|$) ]]; then
      problems+=("$rel:$n imports outside the tree: ${line#"${line%%[![:space:]]*}"}")
    fi
  done < <(grep -n -E '^[[:space:]]*\.?import[[:space:]]' "$file" || true)
done
shopt -u nullglob
for theme in "${themes[@]}"; do
  [[ -f $dest/themes/$theme/colors.toml ]] || problems+=("themes/$theme/colors.toml is missing")
done
[[ -f $dest/$scan ]] || problems+=("$scan is missing (ADR-0055: a pin without it needs a decision)")
if ((${#problems[@]} > 0)); then
  printf 'omarchy-tree: %s\n' "${problems[@]}" >&2
  fail "the tree at ${commit:0:12} is not what the checks need"
fi

files=$(find "$dest/shell" -type f | wc -l)
echo "omarchy-tree: $repo@$commit at $dest"
echo "omarchy-tree: $files files under shell/, Commons and Ui self-contained, themes ${themes[*]}, $scan"
echo "omarchy-tree: digest $(digest "$dest")"
