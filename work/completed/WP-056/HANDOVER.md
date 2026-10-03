WP-056 HANDOVER

Done:
- F-454: `docs/user/{en,de}/11-update-and-uninstall.md` step 4 now says that
  with a kept logbook `seldon hook uninstall claude-code` (no `--settings`)
  removes the logbook's own hooks, that a global install needs the
  `--settings ~/.claude/settings.json` form as well, and that without it
  Claude Code keeps calling a missing `seldon`.
- F-458: `engine/systemd/README.md` step 1: the release binary / installer
  and the AUR package (the PKGBUILD builds with `--features watch`) already
  include `watch`; the `--features watch` build is for source builds only
  and must not be copied over an installer-managed `seldon` (the installer
  refuses a foreign `seldon` without `--force`, `--uninstall` keeps it).
  Step 2 is unchanged. The source-build block is labelled "Source builds only".
- F-450 (docs): `docs/user/{en,de}/07-the-logbook.md` new section "Back up
  and restore the state directory" (de: "Den Zustandsordner sichern und
  wiederherstellen"): what lives in `~/.local/state/seldon/`, what a lost
  folder does, backup and restore with `tar`.
- de pages carry the new source stamp (11: `@ eaea010`; 07: `@ 7d77836`, the commits that changed
  the en pages). CHANGELOG `[Unreleased]` entry added.
- memory/pitfalls.md: two entries appended.
- Review corrections (4 items): README step 1 split into short sentences with
  the source-build block marked "Source builds only"; the state-directory
  table (en, de) lists `hooks/` and `agent-launch.log` (header now "Path");
  snapper deletions made while the state was gone are said not to be
  recorded; "`seldon doctor` then shows no new problem". Done in two commits
  (the de stamp needs the en commit hash first). `docs-check`: ok, no warning.

Not done:
- The engine part of F-450 (operator decision pending, per the WP).
- No engine test: docs only. The check that fails without the fix is
  `docs-check` (it warned "the English page has changed since" for both de
  pages until the stamps were set) plus the scratch runs below. Prose claims
  have no mutant.
- The finding files under `$SELDON_REVIEW` were not read: the variable was
  empty in my session and the brief gave no path. I worked from the WP text
  and verified every sentence against the code (hook.rs `uninstall` /
  `settings_file`, release.yml and PKGBUILD `--features watch`,
  `Cursors::bind`, the pacman/snapper/config collectors, install.sh
  `ours_to_replace`). Please check that the findings hold no further point.
- Scratch dir `/tmp/tmp.y5rxRLQyPK` (plus pointer file
  `/tmp/claude-1000/s056`) is NOT deleted: the path is not under
  `/tmp/claude-1000/`, so the cleanup condition did not hold. Small
  (state files, a tiny logbook); delete by hand.

Verified by: `just check` exit 0 ("check: ok"); `bash scripts/docs-check.sh`:
"docs-check: ok (382 links, 14 translated pages, 40 commands, 435 command
lines)", no warnings. Scratch runs (debug build, `SELDON_TEST_GUARD=$S`,
HOME / XDG_CONFIG_HOME / XDG_DATA_HOME under `$S`,
XDG_STATE_HOME=$S/home/.local/state so the documented `~/.local/state` is
literal; made-up logbook `$S/logbook`):

| Command in the new text | Scratch run |
|---|---|
| `seldon hook uninstall claude-code` | `…/logbook/.claude/settings.json: removed the Seldon hooks and the file, which held nothing else.` + 3 `removed` lines (PreToolUse, SessionStart, SessionEnd); a second run: `…: the Seldon hooks are not installed; nothing changed.` |
| `seldon hook uninstall claude-code --settings ~/.claude/settings.json` | `~/.claude/settings.json: removed the Seldon hooks and the file, which held nothing else.` + 3 `removed` lines (after `hook install … --settings` into the same scratch file) |
| `tar -C ~/.local/state -czf ~/seldon-state-2026-10-03.tar.gz seldon` | rc 0; archive lists `seldon/{index.json,cursors.json,manifest.json,lock}` |
| `rm -r ~/.local/state/seldon` + `tar -C ~/.local/state -xzf …` | rc 0; the four files back, same sizes |
| `seldon capture` after restoring an older backup, with a watched file (`~/.bashrc`) added meanwhile | `Captured 1 new event(s)`, `config 1` (claim: an older backup, the next capture records what changed since) |
| `seldon capture` with the state folder lost and `~/.bashrc` changed | `Captured 0 new event(s)` (claim: a change made while the state was gone is not recorded) |
| `seldon doctor` after restore | all `ok`, `doctor: ok` |
| `seldon watch` (README) | not run (debug build has no `watch` feature; text unchanged apart from the path); `engine/tests/watch.rs` covers it |

Learned: in memory/pitfalls.md (appended): scratch `XDG_STATE_HOME` vs. the
documented `~/.local/state`; stamp the de pages after the en commit.

Decisions needed: none. Question: should the "Start over" section of page 11
also mention a global `~/.claude/settings.json` hook install? Left alone
(not in the WP).

Touched outside WP scope: CHANGELOG.md `[Unreleased]` (allowed),
memory/pitfalls.md (append), work/active/WP-056/HANDOVER.md. Nothing else.
