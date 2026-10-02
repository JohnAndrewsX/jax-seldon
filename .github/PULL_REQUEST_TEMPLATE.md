## What and why

<!-- What this pull request changes and why. Link the issue or the work
package (WP-NNN) it implements. -->

## How it was verified

<!-- Commands you ran and their key output. Name every step that did not
run, e.g. the host-only steps without an Omarchy install. -->

## Checklist

- [ ] `just check` exits 0 (or `SELDON_SKIP_HOST_CHECKS=1 just check`, and the skipped host steps are named above)
- [ ] Tests cover the change; a collector change proves idempotency (a second run writes no new events)
- [ ] Specs (`docs/SPEC-*.md`, `docs/CONTRACT.md`) and `docs/TESTING.md` match the code
- [ ] A contract change has an ADR, a `contractVersion` bump and updated fixtures, with engine and plugin changed together
- [ ] A line under `## [Unreleased]` in `CHANGELOG.md`, or no user-visible change
- [ ] English everywhere; no secrets, tokens, real host names or private paths
