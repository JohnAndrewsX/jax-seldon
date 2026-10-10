# WP-136 — Plan

Branch `wp/136-plugin-commits` from `next` (e9851606); merges into `next`.

## What changes

Engine (`engine/src/collectors/plugins.rs`):

1. **The plugin clone's HEAD in the cursor.** `PluginState` gains `head`
   (full hash), for third-party plugins whose directory is its own git
   clone (`<dir>/.git` exists; the same rule as the version fallback).
   One `git rev-parse HEAD --short HEAD` per such plugin and capture
   gives the full hash and the short version fallback in one process.
   A HEAD that cannot be read this time keeps the last one.
2. **One hardened git call shape** for every git call of the collector
   (the existing version fallback included): fixed argv, `-C <dir>`,
   `--no-pager`, `--no-replace-objects`, `-c core.hooksPath=/dev/null`,
   `-c core.fsmonitor=false`, `-c protocol.allow=never`,
   `-c log.showSignature=false`, `-c color.ui=false`; environment:
   the repository variables removed (`logbook::git::REPOSITORY_VARS`),
   `GIT_CEILING_DIRECTORIES` = the clone's parent, `GIT_CONFIG_NOSYSTEM=1`,
   `GIT_TERMINAL_PROMPT=0`, `GIT_OPTIONAL_LOCKS=0`, `GIT_NO_LAZY_FETCH=1`;
   own process group, 2 s timeout.
3. **On `plugin-update`** (third-party, version changed — unchanged
   trigger) with an old and a new HEAD that differ:
   `git rev-list --left-right --count OLD...NEW` (exact counts), then
   `git log -z --no-show-signature --no-notes --max-count=20
   --format=%<(200,trunc)%s RANGE` for the side that moved.
   - old HEAD an ancestor of new: `meta.git = "pull"`, `meta.commits` the
     incoming subjects; detail `1.2.0 → 1.3.0, pulled 3 commits: <newest> …`
   - new HEAD an ancestor of old: `meta.git = "rollback"`, `meta.commits`
     the subjects that left; detail `…, rolled back 2 commits: <newest> …`
   - neither (history replaced): `meta.git = "reset"`, incoming subjects;
     detail `…, reset: 3 commits in, 1 out: <newest> …`
4. **On `plugin-add`** of a git clone: `meta.git = "clone"`, detail
   `<version>, installed by git clone`.
5. Subjects: control characters → space, direction/format characters
   dropped (ADR-0038's set), trimmed, redacted by the logbook's redactor,
   then clipped to 100 characters (`…`); empty → `(no subject)`.
   `meta.commits` = the subjects, newest first, one per line.

Plugin (`plugin/Model.js` `eventDetail`): a key/value row after *What* —
`Commits in` (pull, reset) or `Rolled back` (rollback) — with
`meta.commits` as plain text; `Git` named in the row label only.

Fixtures: the 09-24 `plugin-update` of weather-plus gains `meta.git` /
`meta.commits` and the new detail; index fixtures regenerated.

Docs: SPEC-ENGINE §4 plugins bullet; SPEC-PLUGIN event detail; the
user guide line if it names plugin-update details.

## Tests (temp HOME, fixture repos built with host git)

update by pull · rollback by reset · reset to another history · clone on
add · non-git plugin (no list) · hostile subject (ESC, CR, bidi, a
`token=…` secret, a 300-character subject) · more than 20 commits ·
timeout (a `git` on PATH that sleeps: the event still fires, no list) ·
a cursor from before WP-136 (no `head`: no event, no list) · a `.git`
that is broken inside an outer repository (no walk up).

Capture cost: an ignored timing test, 8 third-party plugins with a
manifest version, with and without their own `.git` (the delta is one
rev-parse each), plus one update's two queries.
