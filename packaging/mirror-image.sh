#!/usr/bin/env bash
# The build image from GHCR, not from Docker Hub (WP-195). Docker Hub's
# unauthenticated pull limit stopped CI on GitHub's shared runners
# ("toomanyrequests") before any step ran. The workflows' job containers
# therefore pull `ghcr.io/johnandrewsx/jax-seldon/archlinux@sha256:<digest>`,
# a copy of `docker.io/library/archlinux@sha256:<digest>` with the same
# digest: the pinned content is the same, only the registry differs.
#
# The `mirror` job runs this script on a push to main or next only (ci.yml
# and audit.yml; never on a pull request, so the token that can write
# packages never meets a pull request's code), before the jobs that use
# the image. It reads the one mirror digest the workflows pin (and its tag
# comment), and copies that image from Docker Hub to GHCR only when GHCR
# does not have it yet, so Docker Hub is asked once per digest, not once
# per run. After a copy it checks that GHCR serves the digest. Pull
# requests and the release workflow only pull by digest.
#
# Needs skopeo (GitHub's ubuntu-24.04 runner image has it), GITHUB_TOKEN
# (packages: write for a copy) and GITHUB_ACTOR. No fallback to Docker
# Hub: when GHCR is down, the copy fails here and the pulls fail in the
# jobs that use the image; run them again later.
#
# Usage: bash packaging/mirror-image.sh [WORKFLOW_DIR]
#   (default .github/workflows; SKOPEO=<program> replaces skopeo, tests)
# Exit: 0 mirrored or already there, 1 the workflows do not pin exactly
# one mirror digest, 2 the copy or its check failed.
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
dir=${1:-$root/.github/workflows}
skopeo=${SKOPEO:-skopeo}
mirror=ghcr.io/johnandrewsx/jax-seldon/archlinux
upstream=docker.io/library/archlinux

shopt -s nullglob
files=("$dir"/*.yml)
shopt -u nullglob
if ((${#files[@]} == 0)); then
  echo "mirror-image: no workflow in $dir" >&2
  exit 1
fi
# `image: <mirror>@sha256:<64 hex> # <tag>` lines; one digest, one tag
pins=$(sed -n -E "s|^[[:space:]]*image:[[:space:]]+${mirror//./\\.}@sha256:([0-9a-f]{64})[[:space:]]+#[[:space:]]*([A-Za-z0-9_.-]+)[[:space:]]*$|\1 \2|p" "${files[@]}" | sort -u)
if [[ -z $pins ]]; then
  echo "mirror-image: no workflow in $dir pins $mirror@sha256:<digest> # <tag>" >&2
  exit 1
fi
if (($(wc -l <<< "$pins") > 1)); then
  echo "mirror-image: the workflows pin $mirror to more than one digest or tag:" >&2
  echo "$pins" >&2
  exit 1
fi
read -r digest tag <<< "$pins"
echo "mirror-image: $mirror@sha256:$digest ($tag)"

: "${GITHUB_TOKEN:?mirror-image: GITHUB_TOKEN is not set}"
: "${GITHUB_ACTOR:?mirror-image: GITHUB_ACTOR is not set}"
work=$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/mirror-image.XXXXXX")
trap 'rm -rf "$work"' EXIT
auth=$work/auth.json
"$skopeo" login --authfile "$auth" --username "$GITHUB_ACTOR" --password-stdin ghcr.io <<< "$GITHUB_TOKEN" >/dev/null

# served: GHCR returns the manifest, and the sha256 of its exact bytes
# (a file: command substitution would drop a trailing newline) is the digest
served() {
  "$skopeo" inspect --authfile "$auth" --raw "docker://$mirror@sha256:$digest" > "$work/manifest" 2>/dev/null || return 1
  [[ $(sha256sum < "$work/manifest" | cut -d' ' -f1) == "$digest" ]]
}

if served; then
  echo "mirror-image: already on GHCR; Docker Hub not asked"
  exit 0
fi

echo "mirror-image: not on GHCR yet; copying $upstream@sha256:$digest to $mirror:$tag"
if ! "$skopeo" copy --all --preserve-digests --retry-times 3 --dest-authfile "$auth" \
  "docker://$upstream@sha256:$digest" "docker://$mirror:$tag"; then
  echo "mirror-image: the copy failed. Docker Hub may have refused the pull (its limit), or GHCR may be unavailable: run the job again later. Or the token cannot write packages (the job needs packages: write; packaging/README.md, \"Pinned actions and image\")" >&2
  exit 2
fi
if ! served; then
  echo "mirror-image: GHCR does not serve $mirror@sha256:$digest after the copy" >&2
  exit 2
fi
echo "mirror-image: copied; GHCR serves the digest"
