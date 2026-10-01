# ADR-0016 — Engine install and update commands go through the AUR helper

**Status:** accepted (supersedes ADR-0004 on the install command only)
**Date:** 2026-10-01

## Context
ADR-0004 and SPEC-PLUGIN §5 told the engine-missing banner to run
`omarchy pkg add jax-seldon`. WP-010 and its reviewer verified read-only on
Omarchy 4.0.4 that `omarchy-pkg-add` is `sudo pacman -S --noconfirm
--needed` against the official repositories only; it cannot install an AUR
package. `omarchy-pkg-aur-add` is `yay -S --noconfirm --needed`, which
installs but never upgrades. `omarchy-plugin-update` is a fast-forward
pull plus validate and rescan.

## Decision
- The banner's install command is `omarchy pkg aur add jax-seldon`.
- Contract mismatch, engine older than the plugin: `yay -S jax-seldon`
  (an explicit upgrade; `pkg aur add` would be a no-op).
- Contract mismatch, plugin older than the index: `omarchy plugin update
  jax.seldon`.
- All three are constant strings in `plugin/Model.js`; the plugin offers
  them as "Copy" and "Run in terminal" (Omarchy's floating terminal with a
  constant command) and never runs them itself.
- Everything else in ADR-0004 stands: engine via AUR, plugin thin, the
  plugin holds no binary.

## Consequences
SPEC-PLUGIN §5 and the marketplace README name these commands. WP-040
publishes `jax-seldon` on the AUR under exactly that name.
