# ADR-0024 — While the AUR package does not exist, the plugin's one-click engine install runs the verified GitHub installer

**Status:** accepted
**Date:** 2026-10-02

## Context
AGENTS.md §7 requires every degraded plugin state to have a one-click
fix. The "engine missing" fix was `omarchy pkg aur add jax-seldon`
(ADR-0016). Registration on aur.archlinux.org is paused (2026-10-02), so
the package cannot be published yet and the fix fails for every user.
WP-044 adds `install.sh`, published as a release asset: it downloads the
release tarball and `SHA256SUMS` over TLS from this project's GitHub
release, verifies the checksum, refuses on mismatch and installs under
`~/.local/bin` without sudo.

## Decision
- Until the AUR package is live, the plugin's "engine missing" fix is the
  constant command
  `curl -fsSL https://github.com/JohnAndrewsX/jax-seldon/releases/latest/download/install.sh | bash`,
  run only on the user's click in Omarchy's floating terminal, where the
  command is visible. It stays one of the plugin's fixed constants
  (SPEC-PLUGIN §5; CONTRACT.md's list of `seldon` forms is unchanged).
- The README install sections and the banner text say "AUR package:
  coming soon" and list the GitHub path first.
- **Flip-back condition:** when `jax-seldon` is on the AUR, the constant
  returns to `omarchy pkg aur add jax-seldon` (ADR-0016), the "coming
  soon" sentences go, and the AUR path moves to the front. The flip
  list lives in work/completed/WP-044/HANDOVER.md.

## Consequences
Remote code runs on one click, but only from this project's release,
over TLS, checksum-verified for the binary, and visible to the user.
The script itself is listed in `SHA256SUMS` from the release that ships
it; a user who wants the stricter path follows the README's four-step
form (download, read, verify, run).
