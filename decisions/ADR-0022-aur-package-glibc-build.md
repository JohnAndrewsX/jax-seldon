# ADR-0022 — The AUR package builds against glibc; the static musl binary is a release asset

**Status:** accepted
**Date:** 2026-10-02

## Context
AGENTS.md §7 asks for a static musl release build. The AUR package
`jax-seldon` (ADR-0001, ADR-0016) is built on the user's machine by
`makepkg`, with whatever `cargo` satisfies `makedepends`. Arch's musl
target for the packaged toolchain is `rust-musl`, which depends on `rust`
and therefore conflicts with `rustup` — the toolchain Omarchy's
`omarchy install dev-env rust` sets up. A rustup user has the target only
after `rustup target add`, which a PKGBUILD cannot require. The Arch Rust
package guidelines build for the host target (glibc).

## Decision
- The AUR package builds `seldon` for the host target (glibc, dynamically
  linked against `glibc` and `gcc-libs`) with `makedepends=(cargo)` and
  `--features watch`.
- The static musl binary (`x86_64-unknown-linux-musl`, `--features
  watch`) is built by the release workflow and published as a GitHub
  release asset with checksums. This ADR reads AGENTS.md §7's static
  musl release build as the GitHub release asset; AGENTS.md is
  unchanged. Supersedes nothing.

## Consequences
`omarchy pkg aur add jax-seldon` works with either `rust` or `rustup`
installed. The packaged binary depends on the system glibc, as every
Arch package does. Users who want a single static file take the release
asset and put it in `~/.local/bin` (the route engine/systemd/README.md
describes for the watcher).
