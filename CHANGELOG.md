# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Engine crate `seldon` with `--version` and `contract-version`; every
  command accepts `--json`.
- Plugin `jax.seldon`: manifest and stub `Service.qml`, `BarWidget.qml`,
  `Overlay.qml`.
- `justfile` with `check`, `build-release`, `fixtures-refresh`.
- CI running `just check` in an Arch Linux container.
- `docs/TESTING.md`, MIT `LICENSE`.
