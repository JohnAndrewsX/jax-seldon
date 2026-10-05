# TESTING.md — How Seldon is checked

One command gates every work package: `just check`, run from the repository
root. It must exit 0 before a handover (AGENTS.md §5).

## `just check`

| Step | Recipe | What it runs | CI |
|---|---|---|---|
| Format | `fmt-check` | `cargo fmt --check` on `engine/` | yes |
| Lint | `clippy` | `cargo clippy --all-targets -- -D warnings` | yes |
| Tests | `test` | `cargo test` (unit + CLI tests in `engine/tests/`) | yes |
| Watch feature | `check-watch` | `cargo clippy --all-targets --features watch -- -D warnings`, `cargo test --features watch` (see "The `watch` feature") | yes |
| Packaging | `check-packaging` | `bash -n` and (when installed) `shellcheck` on `packaging/PKGBUILD` and its scripts, `packaging/check-srcinfo.sh` (`.SRCINFO` in step with the PKGBUILD), `bash tests/release/release-notes.test.sh` (the release body from `CHANGELOG.md`: the real `0.1.0` section, a middle and a last section, outer blank lines trimmed; missing, empty and prefix-only versions, malformed input exit 1) | yes |
| Install script | `check-install` | `bash tests/install/install.test.sh`: `install.sh` against a mock of the release layout served as `file://` URLs, scratch `HOME` and prefixes, no network — latest via the API with and without `jq`, a re-run changes nothing (bytes and mtimes), update and downgrade, `--unit` (the unit byte-identical for `~/.local`, `ExecStart` rewritten for other prefixes, never enabled), refusals before the first write (checksum mismatch, no `SHA256SUMS` line, wrong binary version, missing release, bad arguments, a foreign `jax-seldon`), the script piped to `bash` and truncated, `--uninstall` (only matching files; refused while the unit is enabled), the man page and the completions (WP-049: the fake binary answers `completions`/`mangen`; a scratch `/usr/share` via `SELDON_INSTALL_SHARE` has the bash-completion, fish and zsh directories, and fake `fish`/`zsh` in a PATH dir decide which shells exist (the host's zsh and fish are left off PATH): zsh's directory without `zsh` installs nothing, a fake `zsh` adds its completion and the `fpath` hint, `fish` without its directory installs nothing; a release without the commands skips them; a foreign completion is kept unless `--force`; a completion of a shell that is gone stays in the manifest and `--uninstall` removes it), the build-provenance check (WP-080: the host's `gh` is left off PATH; `gh` stubs that verify against the mock releases' attestations, fail on their own, are not logged in, too old or missing an option, or absent, each with and without `--require-verified`, plus `--skip-provenance`; a tampered release, one attested only for a branch, one from a self-hosted runner, one before attestations (v0.1.1), a `GH_HOST` of another server, the exact `gh` argv), no `sudo`/`systemctl` call, the real `~/.local/bin`, `~/.config/systemd/user`, completions and man page untouched; `shellcheck` when installed | yes (`shellcheck` in the release workflow's container) |
| Deploy script | `check-deploy` | `bash tests/deploy/deploy-test-host.test.sh` (WP-098): `scripts/deploy-test-host.sh` in a scratch git repository with a bare origin, against a fake test host — an `ssh` stub runs the remote scripts here under `env -i` with a scratch `HOME` and a `PATH` of stubs (`omarchy-shell`, `omarchy-restart-shell`, `omarchy`, `curl`, `git clone`) plus single linked tools, so the host's real `omarchy-*`, `quickshell`, `hyprctl` and `systemctl` are out of reach; a `cargo` stub builds a fake engine. Refusals before any build or change (no or unlisted host, a machine-id that does not match the pin (one ssh call, no id printed), no pin or no pin file (with the hint; a commented pin does not count), a prefix or comment word of a listed one, an ssh option as host, no host list, the host is this machine, not on `main`, a modified or untracked file, HEAD not pushed, a check log missing, not ending in `exit 0`, without a first line `head <full sha>`, with a short, unknown or other-branch sha, or with `engine/`, `plugin/`, `schema/` or the script changed since that sha — a docs-only commit passes —, Windows line endings, bad arguments, a symlinked plugin dir, a missing remote tool); dry run (no build, the host unchanged); first deploy (marked build with `--features watch` into the repo's target dir, `ssh -G` and the engine found in the dry run, a host without an engine, `ping` before the restart, `seldon.prev`, the release clone moved out of the plugins dir, HEAD's plugin files plus `.seldon-dev-build`, one restart, smoke, log line); an engine-only change (no restart, unchanged plugin files keep their mtime, an exported `CARGO_TARGET_DIR` ignored); the settle wait; removed and added plugin files; each locked state and an unreadable lock status (restart pending, caught up by the next deploy on an unlocked session); a restart notice while the restart is pending (a note); a failing restart, doctor, capture, service version, restart notice, host-side validation, build, a build without the marker, and no graphical session (named in the summary); an active `seldon-watch.service` restarted on the new binary, an inactive one left alone, a failed unit restart (exit 2); `--release` (install.sh with `--force`, the watcher restarted on the release binary, the clone at the tag, the dev copy moved aside), a clone that fails validation, a checksum mismatch and a missing release; the real `~/.local/bin/seldon`, plugin dir and `~/.local/state/seldon-dev` untouched; `shellcheck` when installed | yes |
| Contract | `schema-validate` | `bash scripts/validate-fixtures.sh` (WP-002); skipped with a notice while the script does not exist | yes |
| User guide | `docs-check` | `bash scripts/docs-check.sh` (WP-045): builds the engine (debug), then checks `docs/user/`: relative links, images (with alt text) and anchors resolve; every language folder has the same pages as `en/` with the same heading levels, code blocks, tables and images; every translated page has its `<!-- source: en/<page> @ <commit> -->` line (a source commit older than the English page's last change is a warning; a commit missing from a shallow clone is a notice); every `seldon …` in a code span or a `sh` block names commands and options that `--help` lists (`PLANNED` in the script holds commands the guide names as planned); the help blocks of `05-cli-reference.md` equal `seldon <command> --help` with the global options left out. The front pages (`FRONT_PAGES`: `README.md`, `plugin/README.md`, `plugin/SECURITY.md`, `docs/DEVELOPMENT.md`, `llms.txt`, WP-046) get the same link, anchor and `seldon …` checks; a page under `plugin/` may link or embed only files inside `plugin/` by relative path (it is published on its own by `git subtree split`); an absolute link into the public repositories (`github.com/JohnAndrewsX/jax-seldon[-plugin]` blob/tree/main, `raw.githubusercontent.com`, the repository root, a workflow badge) must name a file and heading that exist here; every image is at most 1 MB. Other URLs are not fetched. `--write` regenerates the help blocks. `SELDON_BIN` skips the build | yes |
| Plugin manifest | `plugin-validate` | `omarchy plugin validate plugin/` | **no** (dev host) |
| QML lint | `qmllint` | `qmllint` on `plugin/*.qml`, `plugin/components/*.qml` and `plugin/components/overlay/*.qml` against `$OMARCHY_PATH/shell`, then the token check `tests/plugin/check-tokens.py` | **no** (dev host) |
| Plugin logic | `plugin-test` | `node tests/plugin/model.test.js`, `node tests/plugin/model.bench.js`, `bash tests/plugin/service-states.sh`, `bash tests/plugin/panel-view.sh`, `bash tests/plugin/overlay-view.sh`, `bash tests/plugin/bar-view.sh` (see "Plugin") | **no** (dev host) |

Other recipes: `just check-rss` (the `seldon watch` memory bound on an
optimised build; not in `check`, not in CI, required before the handover
of a WP that touches `engine/src/index/` or `engine/src/commands/watch.rs`;
see "The `watch` feature"), `just check-perf` (SPEC-ENGINE §1's time
budgets at the stated scale, WP-076; opt-in, not in `check`, not in CI:
it needs an optimised build (`--profile bench`, the tests refuse a debug
build) and a quiet host, since a busy one roughly doubles a timing. It
runs `cargo bench --bench index` with `SELDON_BENCH_X150=1` (the index
build ×10 and ×150 < 100 ms; `just bench` in CI asserts ×10 only and
prints ×150), then the ignored tests of `tests/index.rs`,
`tests/hooks.rs` and `tests/redaction.rs` one at a time: `status` at
10 011 ledger lines, 304 cases and 365 journal files < 100 ms,
`hook claude-code` at 10 000 lines and at 950 lines (with the index
rebuild) < 5 ms, for a call it does not record and a recorded one, with
the temp dir on tmpfs, and the redaction of long lines (16 KB < 1 ms and
64 KB < 2 ms without a masked value; 128 KB with many masked values
< 20 ms or < 10 ms, WP-087). Every
check measures a median over budget once more before it fails. Required
before the handover of a WP that touches the index build, `status` or
the hooks), `just build-release` (static musl binary,
`x86_64-unknown-linux-musl`), `just fixtures-refresh` (stub until the engine
builds an index), `just e2e` (engine ↔ plugin end to end, host only; see
"Integration"), `just deploy-test-host` (the main build onto the test
host; see "Test host follows main").

## Engine tests

Run them all with `just test`, or directly:

```
cargo test --manifest-path engine/Cargo.toml --locked            # everything
cargo test --manifest-path engine/Cargo.toml round_trip::         # frontmatter vs fixtures/logbook
cargo test --manifest-path engine/Cargo.toml --test init          # `seldon init`
cargo test --manifest-path engine/Cargo.toml --test doctor        # `seldon doctor`
cargo test --manifest-path engine/Cargo.toml plan::               # case state machine, folder moves
cargo test --manifest-path engine/Cargo.toml log::                # notes, journal, Log section
```

| Where | What |
|---|---|
| `engine/src/**` (`#[cfg(test)]`) | unit tests: frontmatter parser and writer, models, config precedence, lock, templates, subprocess runner |
| `engine/tests/cli.rs` | `--version`, `contract-version`, parse errors (exit 1, JSON error shape, `--json` detection past free text), `--help`, `--config` > `SELDON_CONFIG` > XDG config |
| `engine/tests/manual.rs` | `seldon completions` and `seldon mangen` (WP-049): the bash script names every command and subcommand `--help` lists and passes `bash -n`; zsh and fish scripts (`zsh -n`, `fish -n` when installed, else a note); no home needed (empty environment); `tcsh` → exit 1 with the JSON error; the man page has NAME to FILES, a `seldon <command>` usage line for every command, no `seldon-<command>(1)` references, renders under `groff -man -ww` without a warning and under `man -l` (each skipped with a note when the tool is missing); `--json` shapes. Unit tests in `commands/manual.rs` check the roff escaping |
| `engine/tests/hooks.rs` | `hook claude-code`, `hook generic`, session start and stop, `hook install claude-code` (WP-009); `uninstall::` (WP-049): install then uninstall gives the user's file back as it was, a second uninstall changes no byte, a user's hook added to Seldon's group stays, a file with only Seldon's hooks is deleted (its directory stays), the logbook default commits `seldon: hook uninstall claude-code`, a broken file is refused unchanged, no logbook → exit 3 (not an agent hook's silent 0) |
| `engine/tests/own_writes.rs` | SPEC-ENGINE §5 rule 7 (WP-038): `init --theme-hook` and `hook install` under a watched path, the next capture explains the `config-add`/`config-change` and opens no drift. The removals (WP-049): `init --remove-theme-hook` deletes the hook and its script, records `op: delete`, the next capture's `config-remove` is explained `removed by seldon init --remove-theme-hook` and drift stays 0; a second removal does nothing; removed before any capture saw it → no event; edited by hand, then removed → the removal is drift; `hook uninstall` of a watched file with only Seldon's hooks → an explained `config-remove`, of a shared file → an explained `config-change` (`op: remove`); the theme hook under the lock (WP-074, WP-052): an `omarchy` stub that copies the hook and then runs a real `seldon capture` gets exit 4 for that capture, and the next capture explains the `config-add` (a lock-after-write mutant lets the capture record unexplained drift); with the lock held, `init --theme-hook` exits 4 with the temp tree byte-identical, no `owned.json` and no `omarchy` call. The unit test `commands::setup::tests::the_theme_hook_step_writes_nothing_while_the_lock_is_held` covers the step alone |
| `engine/tests/own_changes.rs` | SPEC-ENGINE §5 rule 8 (WP-086): updating, enabling and disabling `jax.seldon` and upgrading `jax-seldon` are explained by the capture that writes them (actor kept, other plugins and packages stay drift, adding, downgrading and removing Seldon stay drift, idempotent); WP-088: own changes left open by an earlier capture are explained by the next one, a dismissed row keeps its resolution, add and downgrade stay drift, no case is created; a month file that cannot be read still lets the new change be explained (with the "not checked" warning) and the next capture catches up; an own change dated after the capture clock is explained at its own time, the index folds it, and a dismissal written before it in the ledger is kept |
| `engine/tests/frontmatter.rs` | `round_trip::` every case, journal, decision, area, memory file and `PROJECT.md` of `fixtures/logbook/` parses into its typed record and re-serialises byte-identical; a lossless update changes only the edited lines; `refused_saves::` (WP-077): `log`, `event` and `drift link` on a case whose save would be refused (an `events:` flow list continued at column 0) exit 1 and leave every file of the logbook as it was (`hooks.rs::claude_code::a_case_whose_save_is_refused_records_nothing` for the hook); `case_ids::a_refused_value_is_named_escaped` (WP-077): an ESC sequence in a case, decision, journal or area value is named as `\u{1b}`, serde's `unknown variant` messages included; `hooks.rs::generic::a_refused_payload_value_is_named_escaped` does the same for the generic hook's `actor`, `case` and `startedAt` |
| `engine/tests/init.rs` | `init::` layout (SPEC-LOGBOOK §2), JSON output, git first commit, `--no-commit`, German templates, Obsidian, path precedence, refusals (existing logbook, non-empty dir, no terminal), lock held → exit 4. `setup::` (WP-024): the first capture (stubbed sources, cursors set, a second capture writes nothing, `--no-capture`), `--since` backfill → open drift, `--baseline` → zero open drift with one `dismissed` "pre-Seldon baseline" line per member and the commit `seldon: first capture and pre-Seldon baseline`, flag errors before anything is written, `--harness claude-code` (settings in the first commit, `hook install` afterwards changes nothing), `--harness omarchy-agent` with a kit (copied, modes kept, merged with Claude Code's hooks) and without one, the theme hook (a recording `omarchy` stub: exactly one `hook install theme-set <script>` on opt-in, none without, a failure with its fix, an existing hook not reinstalled), the templates (written as rendered; frontmatter keys, headings, fences and table headers identical in `en` and `de` and equal to `tests/golden/init-skeleton.txt`, `SELDON_BLESS=1` rewrites it; German prose); the agent rules (WP-047): the generated `AGENTS.md` in `en` and `de` has the ten sections in order (Session start, The engine is the only writer, Work in cases, Zones, Commands, Journal and memory, Drift, Hooks, Ending a session, Never), each with its key `seldon` commands, links `docs/AGENT-GUIDE.md`, and `init` writes no `CLAUDE.md`; the config (WP-074): a config without `language` leaves it to the locale (`LANG=de_DE.UTF-8` → `de`, written back), a `language` key wins over the locale; a read-only config folder → exit 2 with nothing in the logbook folder, the same `init` runs once it is writable; a layout that cannot start (read-only parent) → the config byte-identical, or still absent; a layout stopped half-way (a logbook path whose `areas/hyprland/README.md` exceeds PATH_MAX) → the config restored, the folder `init` created removed, an empty folder that was there emptied again, a second `init` succeeds; `layout::create` writes the marker last; `--no-git` writes `autocommit = false` and `doctor`'s git check is ok; a re-run keeps a hand-set `autocommit = false` (no repository) unless `--git`. WP-079: the read grant is the optional snapper step; a user listed in `ALLOW_USERS` gets the revert plus the read grant as a recommended step, another user no snapper step |
| `engine/tests/plan.rs` | `plan::` new (template, canonical frontmatter, area on first use, ids never reused), start/verify/done/drop (folder moves per ADR-0012 §9, `started`/`closed`/`snapshotBefore`, `.seldon/active-case`, body byte-identical outside the Log), invalid transitions → exit 1 and nothing written, list/show against `case.schema.json`, `plan list` with one invalid case file (a warning line naming it, the others listed, exit 0; WP-077), a case file name with an ESC sequence named escaped in the warnings of `plan list` and `index` (`a_warning_names_a_case_file_escaped`), the fixture logbook (a copy), git autocommit with `--no-commit` and `git.autocommit = false` |
| `engine/tests/log.rs` | `log::` notes with and without a case (`case.events`, `agents`), the Log section append-only over three steps, the journal appended not rewritten, free text as one argument (spaces, quotes, `$(…)`, `--json` after `--`), month and day by timestamp, redaction, exit 3/4 |
| `engine/tests/journal.rs` | `journal::` appends to a fixture day (only the `cases:` line changes), CRLF days, the `plan done` stub in the logbook language |
| `engine/tests/commands.rs` | `event::` (fixture line shape, typed meta, engine-only kinds refused), `decide::` (ADR numbering, the logbook's own template, the editor gets the path as one argument), `open::` (paths, `--editor` without a terminal) |
| `engine/tests/agent.rs` | `seldon agent start` (WP-022): a recording stub `omarchy` gets exactly one argv, the prompt one element (a title with quotes, `$(…)` and backticks stays text), cwd and `SELDON_LOGBOOK` the logbook, the case becomes the active case, under 1 s; `[agent] launcher` and `[agent.launchers]` from config; a shell launcher refused before anything changes; a queued case → exit 1 with the `seldon plan start` hint; verification, unknown and malformed ids; a missing launcher and one that exits 1 at once (its stderr is the message) → exit 1 with the previous active case restored; a launcher that keeps running is detached (own process group, alive); exit 3 without a logbook. Unit tests in `commands/agent.rs` check the launcher rules |
| `engine/tests/rebuild.rs` | `seldon rebuild` (WP-032) on a copy of `fixtures/logbook/`: `outputs/REBUILD.md` equals `tests/golden/REBUILD.md` (`SELDON_BLESS=1 cargo test --test rebuild` rewrites it; the golden test first runs `seldon dossier --section packages` with the query shims, so the document has the "Before the logbook" group and the test checks that it lists every `pre-logbook` package of class `user` of `packages.explicit` under the command of its origin, WP-035, and one line counting the six `omarchy-base` ones, WP-036), the seven English headings in order, German prose, `--json` `sections` counts, and every package line's last code span is the id of an explicit `install` event of that package; a second run at a later clock writes nothing (`files: []`, same bytes, no commit); text above and below the `rebuild` fence survives a change; the autocommit `seldon: rebuild` happens once per change (git repository made in the test); `drift dismiss`/`explain` move items to "Deliberately not reproduced" and out of the open questions; appended ledger lines prove `pacman -U` → `omarchy pkg aur add`, no command → "repository unknown", a later `remove` drops the package (English logbook); an empty logbook says "none"; exit 3 without a logbook. Unit tests in `rebuild/mod.rs` check the repo/AUR rule and the fence merge |
| `engine/tests/dossier.rs` | `seldon dossier` (WP-035) on a copy of `fixtures/logbook/` with every host query shimmed (`Env::query_shims`: the package manager, `systemctl`, `omarchy` print `fixtures/logs/pacman-Q*.txt`, `systemctl-*.txt`, `plugin-list-after.json` for exactly the query argument lists, exit 64 for anything else, and log each call; `SELDON_HARDWARE_ROOT=fixtures/logs/hardware`; `common::Env` sets `SELDON_OMARCHY_PACKAGES=fixtures/logs/omarchy-packages`, copies of Omarchy's two package lists, for every test, WP-036): the system files equal `tests/golden/dossier.md` (`SELDON_BLESS=1` rewrites it), all eight fences present, the text outside the fences byte-identical, only read-only queries ran (each once), `seldon: dossier` committed once; a second run a day later writes nothing ("Nothing changed"); emptied fences are all filled (the ledger's four installs marked `since`, eleven `pre-logbook`, seven of class `omarchy-base` and the rest `user`, cased config rows, hardware from files); without Omarchy's lists every package is `user`, with exactly one warning, and a second run changes nothing; a later cased `config-change` fills only the empty case cell of an existing deviations row (a row with a case keeps it), and a second run changes nothing; a missing program skips its fences with a warning and keeps them; `--section` writes only its fences (comma list and repeats, unknown value exit 1); a cased `config-change` adds a deviations row and keeps the old rows byte for byte; an agent's `systemctl --user enable` with a case fills the unit's case; `capture` and `status` never touch the dossier; exit 3 and 4. WP-075: a Latin-1 `system/notes-latin1.md` is skipped with the index's warning, kept byte for byte, and every fence is built as without it (`a_file_that_is_not_utf8_is_skipped_and_the_fences_are_built`); a Latin-1 `plugins.md` keeps its bytes and `plugins.list` is skipped with a warning, never appended elsewhere (`a_default_file_that_is_not_utf8_keeps_its_fences`); a cased config path holding `<!-- seldon:end -->` gets one neutralised row, a user row of such a path gets its case filled, and two more runs change nothing (`a_marker_in_a_cased_path_is_listed_once`). Unit tests in `dossier/` cover history rows, the explicit-line format (with and without a class), reading Omarchy's lists (comments, missing files), unit cases, the deviations rows and the case fill (reason with `|`, an editor-padded `| --- |` separator in `deviations.table` and `packages.history`, row date newer than the event, other column orders), appending a missing fence under a heading in the logbook language, `/proc` parsing, and (WP-075) unreadable files: a Latin-1 file is never written, a missing fence is not appended while its default file or a file whose bytes hold its begin marker is unread, an unreadable file blocks only appending, a non-UTF-8 file name is written back to itself, and fence bodies are neutralised |
| `engine/tests/import.rs` | `seldon import omarchy-agent` (WP-043) on the synthetic vault `fixtures/vaults/omarchy-agent/` (copied into the test home, so the report says `~/omarchy-agent-vault`), against a German logbook that already has C-2026-001 (the kit's C-2026-001 collides) and a journal entry on a kit session day: the dry run writes only `outputs/IMPORT-omarchy-agent.md`, equal to `tests/golden/IMPORT-omarchy-agent.md` (`SELDON_BLESS=1 cargo test --test import` rewrites it), commits it as `seldon: import omarchy-agent (dry run)`, and a second dry run writes nothing; `--json` counts and the collision; neither the fake token nor `/home/user` reaches the report. `--apply`: one commit `seldon: import omarchy-agent`, clean tree, the vault byte-identical; ids kept or renumbered (tag, title line), status folders (nothing active), Intent/Plan/History with demoted headings and the fenced `##` line kept, redaction and `~` in the case; `index --check` valid without warnings; seven ledger notes: the apply's own (no case, subject `omarchy-agent`, at the apply's time, WP-075) and six at local midnight of `created`, attached to their cases; journal days created or appended (`cases:` from the kit links), fenced headings stay text; memory sections (lessons appended, new topic files, no file for an empty topic); two deviation rows, the duplicate path and resolved entries left out; the marker; a second `--apply`, `--apply --json` and dry run change nothing ("Nothing changed", same bytes, no commit), also with the marker removed (the ledger's notes). The fixture names the colliding C-2026-001 as a wikilink and as bare ids: they come out as C-2026-007 in case bodies (the case's own Protokoll included), the journal (and its `cases:`), a lessons section and a deviation reason, counted per file under "Id rewrites"; `Ergebnis` is the case's `## Result`; a `done` case without `closed` gets `created` and an "Assumptions" row; an existing memory file's `updated` moves to the import day. Import notes without the marker → exit 1 with the undo hint. An Obsidian-padded `| --- |` separator in `deviations.table` keeps its user row on `--apply`. A failed apply (read-only `system/`, skipped as root) exits 2 with "nothing was committed" and an undo that names only the import's files (`git --literal-pathspecs checkout <commit> -- … && rm -f -- …`), no commit and no marker; the next run is refused with the same undo until it is run, then the import runs once (WP-061). Pending changes are committed first (`seldon: before import omarchy-agent`): a note, an inbox file and an edit of `memory/lessons.md` made before a failed apply, and a new file and an edit made after it, all survive the printed undo (run with `sh -c`); with `--no-commit` a dirty logbook is refused before anything is written. A tampered `.seldon/imports/omarchy-agent.undo.json` (no hash base, `.git/…`, a file outside the import's folders, `..`, `:/`, `*`, or a folder replaced by a symbolic link to a place outside the logbook) is never printed as a command; a successful apply removes a leftover undo file. An unknown kit status blocks `--apply` (exit 1, nothing written but the report, which lists the error). Not a vault or not a directory → exit 1, no logbook → exit 3, `--apply --dry-run` → exit 1. WP-075: a vault without cases (only `journal/` and `knowledge/`) is refused a second apply after the marker is deleted and after a failed apply (read-only `memory/`), the journal day keeping one imported block (`a_vault_without_cases_is_never_imported_twice`); a case file with a Latin-1 name is a report error ("file name is not UTF-8", the path with U+FFFD), an inbox file with one is only listed, an unreadable case is an error too (skipped as root), the dry run exits 0 and `--apply` 1, and without the bad case the rest imports (`a_file_name_that_is_not_utf8_is_a_report_error`); a vault folder with a Latin-1 name is read (`a_vault_path_that_is_not_utf8_is_read`); a kit case with a BOM and padded `---` fences imports (`a_bom_and_padded_fences_import`); `<!-- seldon:end -->` in a kit deviation heading stays inside the `deviations.table` fence, neutralised (`a_marker_in_a_kit_heading_cannot_end_the_deviations_fence`). Unit tests in `import/` cover the scrubber, the id rewriter (one pass, no partial ids), a renumbered done case without `closed`, lenient frontmatter, fence-aware sections and demotion, the case body, bad kit cases, session splitting, deviation entries and knowledge sections |
| `engine/tests/doctor.rs` | `doctor::` green after init with snapper degraded, exit 3 when not initialised, invalid frontmatter, misplaced case, the fixture logbook (and that doctor leaves it untouched); snapper's permission error in a German locale (issue #1, WP-053): with `LANG=de_DE.UTF-8` and a stub that answers `Keine Berechtigungen.` unless `LC_ALL=C`, `init --non-interactive` prints the read-grant hint (`setfacl`, ADR-0026), `cursors.json` holds `NO_PERMISSIONS` for snapper, and `doctor` prints the `fix:` line. The unit test `collectors::snapper::tests::list_command_runs_in_the_c_locale` pins the argv and `LC_ALL=C` / no `LANGUAGE`. The `git` check (WP-061) is degraded, each with its fix line, for a stale `.git/index.lock`, a read-only `.git`, a detached HEAD (ok with autocommit off), a committer git cannot resolve (empty `user.name`), and an empty `.git` inside another repository; doctor leaves `.git` byte-identical even when a tracked file's stat data is stale. WP-070: an unparsable `config.toml` → exit 1, config error first, logbook "not checked", `"logbook": null` (also with `--path`, which is still checked); an invalid `[redaction] patterns` entry, and each corrupt state file (`cursors.json`, `manifest.json`, `owned.json`) → error with its fix; bad ledger lines → `ledger` degraded with month, count and lines; a case id in two files → `cases` error; a STATUS.md or DECISIONS.md fence without its end marker, and a stray end marker after a removed one → `fences` degraded; the snapper probe runs `SELDON_SNAPPER`; with every new check failing (a duplicate case and an invalid pattern included), the home (state, config) and the logbook are byte-identical after doctor, no index and no lock appear. Review round: a collector whose last capture failed (this logbook's cursors only, disabled ones left out) → `collectors` degraded with its message and fix; an unreadable config.toml → config error with a `chmod` fix, exit 1, logbook not checked; the parse error's fix line; the `init` fix prefixed "after fixing config.toml:"; an unreadable state file's text matches its `chmod` fix; the omarchy probe runs `SELDON_OMARCHY_VERSION`. WP-079 (ADR-0026): a user still listed in `ALLOW_USERS` (the stub answers `get-config` only with `LC_ALL=C`) gets an `ok` row whose fix is the revert, then the read grant (also by `LOGNAME`, also under a German locale); a user not listed, a partial name, `USER` not listed with a listed `LOGNAME` (`USER` wins), no user, a refused `get-config` and a missing snapper get none; the stub logs that only `list` and `get-config` ran, and that a refused `list` asks no `get-config`; WP-081: a corrupt manifest's error row says the capture records a state reset, the capture after it leaves a degraded `state` row with the restore fix (exit 0), and the next capture clears it, and another logbook's later cursors with an earlier `lastRun` do not show it (`a_state_reset_is_shown_until_the_next_capture`); WP-083: before that capture a degraded `state` row predicts the reset, with the restore-now fix for missing and unreadable cursors and the nothing-to-restore fix for another logbook's; none for a fresh logbook, a collector that never ran here (no entry while bound here), a disabled collector, or restored cursors; after the capture the WP-081 row instead (`a_state_reset_is_predicted_before_the_capture`) |
| `engine/tests/idempotency.rs` | `state_reset::` (WP-081): a removed state directory after a capture with events writes one `seldon` note `state-reset` (sources, files, detail, the warning on stdout and in `--json`), the next capture writes nothing; the first capture of a logbook and collectors whose source has no event in the ledger write no note; an unreadable pacman cursor, a corrupt `manifest.json` and a corrupt `owned.json` (moved to `owned.json.bad`) each give one note; plugins and theme with unreadable cursors and events give one; review round: the first successful theme run after a degraded `init` and a hook-written `theme-set` gives none, nor does a collector disabled at the first capture and enabled later; cursors of another logbook give files `logbook`, the "nothing can be restored" warning and doctor fix; a corrupt `owned.json` waits for a run of the config collector (`--source pacman` leaves it). `collectors.rs`: the snapper info-file path flags a missing cursor. `hooks.rs`: `session_stop_prints_a_state_reset_on_stderr`. WP-083: before every capture of these scenarios, `doctor`'s "the next capture will record a state reset" row names exactly the sources of the note that capture writes, and no row before a capture that writes none (`predicted`); a state directory restored before the capture removes the row and records nothing; every collector's `cursor_reads` accepts its own saved cursor and rejects a stray value. WP-088: a collector degraded in the capture that records a reset (or the one that alone lost its state) is marked `pendingBaseline` (`cursors`), keeps it while degraded or not run, and its first successful run writes its own note and clears it; with the state bound to another logbook the mark is `logbook` and the later note says `logbook` with the "nothing can be restored" warning; a degraded collector without a cursor here or without events of its source is not marked; a `cursors.json` entry without the field reads unchanged, `cursors` and `logbook` round-trip. WP-091: a collector not run (`--source pacman`) in the capture after a removed state directory gets an entry with only the mark (no `lastRun`), its index row equals a row without an entry, it keeps the mark while not run and its first run writes its own note; a collector disabled while the state was another logbook's is marked `logbook` and its note after enabling says so; a collector not run without events of its source gets no entry, and one with an entry here (an unreadable cursor) keeps it unchanged; doctor shows a marked collector in its own row (`waiting`: the WP-091 wording for `cursors` and `logbook`, the `--source` fix), not in the "next capture" row, and both rows when a marked collector and an unreadable cursor meet (`doctor::tests` covers mixed reasons and the plural); `snapper_access::` (WP-091): a degraded first run writes no note, the read grant (info files appear) writes one `ok again` note with both messages, actor `system`, no case, no own change, no drift, its removal one `degraded` note, a repeat writes nothing; a list failure and recovery are recorded the same way, a capture without snapper compares nothing; no note after a lost state directory or for an entry with only the mark; the note's detail is redacted (`the_note_is_redacted_like_every_event`, WP-099). `crash::` (WP-099, `SELDON_TEST_CAPTURE_CRASH=before-append|after-append`, debug builds, exit 99): a crash after the append leaves the `state-reset` note (lost state directory) or the snapper note in the ledger and `cursors.json` as loaded with the note's time in `pendingNotes`; the next capture writes neither again, gives the reset warning and saves without `pendingNotes`; a later loss or the change back is recorded; a crash before the append leaves the note to the next capture; a source the crashed note did not name (`--source pacman`, then `--all`) gets a note of its own; a second crash keeps the first mark besides its own; a mark at 00:00:05 on 1 March local time (February in UTC) is found in the March file. WP-104: a source the crashed capture recorded first gets no reset note (P2), nor does a crashed first capture (P1), and a later genuine loss is recorded; the marked file holds `silentBaselines` (exact shape), also from a crash before the append and for a collector not run (a theme-hook event before the next capture is no loss); a mark under another logbook's path does not hide a loss here; a file without the field reads unchanged; doctor after a crashed reset gives the "will warn" row, not "will record", and both rows when the next capture records a source the crashed note did not name. The crash tests exist in debug test builds only (`#[cfg(debug_assertions)]`, like the crash point) |
| `engine/tests/git.rs` | the autocommit (WP-061): with `GIT_DIR`, `GIT_WORK_TREE`, `GIT_INDEX_FILE`, `GIT_OBJECT_DIRECTORY` and `GIT_COMMON_DIR` of a second scratch repository set, `seldon log` commits into the logbook and the other repository's HEAD and index stay unchanged; `index` reads the logbook's HEAD under the same variables; an empty `.git` inside a parent repository → nothing committed anywhere, one warning ("not a usable repository"), the index shows no HEAD; a `.git` file whose `gitdir:` names the parent's git directory → nothing committed, one warning, doctor degraded; a linked work tree of the logbook's own repository (`git worktree add`) is committed to; a detached HEAD → no commit, nothing staged, one stderr line and `--json` `git.error`; a stale `.git/index.lock` → exit 0, one warning line, `git.error`, no stdout duplicate, and the next write after the lock is gone commits everything. git runs in the engine's process group (a wrapper in place of the `git` link records `/proc/$$/stat` and execs the host's git; snapper, recorded the same way, leads its own group). The unit test `logbook::git::tests::the_command_drops_every_repository_variable` pins the environment |
| `engine/tests/watch.rs` | `seldon watch` (WP-034). Without the feature: exit 1, "built without the watch feature", JSON error. With `--features watch` (`just check-watch`): one rebuild at start (`trigger: "start"`; an edit made before the start is in it), then one change → exactly one rebuild after the 2 s quiet interval and nothing after it (the rebuild's own reads and its `index.json` write stay silent); a burst of 24 writes plus a new folder → one rebuild, and a later write in that folder is seen; generated `ledger/*.md`, `STATUS.md`, temp/backup files, `PROJECT.md`, reads of every watched file, and `seldon index`/`status` runs → none, while `.seldon/logbook.toml` counts; a held lock → no rebuild and still running, the rebuild within 2 s of the release; a folder renamed away and recreated → its watch moves to the new folder (a write in the old one is quiet, one in the new one counts); a new area (`areas/printer/README.md`) → one rebuild with the area in `system.areas`, and `areas/` renamed away and recreated is watched again (WP-075); SIGTERM and SIGINT → exit 0 with a final `stopped` line; not initialised → exit 3; `--interval 1` → exit 1; RSS on the ×10 fixture (below) |
| `engine/tests/index.rs` | `seldon index` (WP-007) against `fixtures/index.sample.json` and the variants, the mutation self-checks of `scripts/validate-fixtures.py` in-process, atomic writes under a concurrent reader. The text clip of ADR-0025: `clip_keeps_short_texts_and_marks_long_ones` (WP-076: a text of at most 256 JSON bytes unchanged, a longer one cut on a character boundary to at most 256 bytes with `… (N more characters in the ledger)`, N exact) and `the_reference_derive_clips_texts_as_the_engine_does` (WP-077: long `detail`, `resolutionDetail` and `meta` texts in a copy of the fixture logbook, cut on multi-byte, JSON-escaped, white-space and U+001C..U+001F characters; `index.events` and `index.drift` of `seldon index` equal those of `validate-fixtures.py --derive`; skipped with a note without `python3`); `long_texts_keep_the_index_under_its_size_budget`, `a_long_command_line_is_whole_in_the_ledger_and_clipped_in_the_index` and `an_index_over_its_budget_warns` (WP-076) |

**Isolation.** The integration tests never see the real home, config,
state or logbook (AGENTS.md §6). `engine/tests/common/mod.rs` gives each
test a temporary `HOME` (so `~/.config/seldon` and `~/.local/state/seldon`
live under it), and a `PATH` that contains only:
- stub `omarchy-version` and `snapper` scripts (the snapper stub prints
  `No permissions.`, a snapshot list, is absent, or prints the German
  `Keine Berechtigungen.` unless `LC_ALL=C`, per test);
- a link to the host's `git`.

So the results do not depend on what the host has installed or how snapper
is configured. Every run also gets `SELDON_TEST_GUARD=<temp dir>`: the
engine then refuses to start (exit 2, "refusing to run outside the test
guard") unless its home, config and state directories, resolved with
symbolic links and `..`, all lie under that directory
(`config::Dirs::from_vars`, unit test
`config::tests::the_test_guard_checks_the_resolved_dirs`, integration test
`init.rs::setup::the_test_guard_refuses_a_home_outside_it`). Two engine variables make tests deterministic:
- `SELDON_NOW` (RFC 3339 with offset) fixes the clock of one invocation:
  event `ts`, journal headings, Log lines, the case id year
  (`Env::at(now, args)`). Not for normal use.
- `SELDON_CONFIG` (or the global `--config FILE`) points the engine at
  another `config.toml`; `--config` wins over the variable, which wins over
  `$XDG_CONFIG_HOME/seldon/config.toml`.

Every line any test writes to a ledger is validated against
`schema/event.schema.json` (`common::ledger`, `jsonschema` with formats),
and case JSON against `schema/case.schema.json` (`common::assert_valid_case`).
Editors are never started: tests run without a terminal, so `--editor`
goes to `omarchy-launch-editor`, which a test stubs to record its argv.
No agent is ever started either: `agent start` tests stub `omarchy` (the
default launcher) or a configured launcher with a script that records its
argv NUL-separated. A manual `agent start` demo on the dev host follows
the same rule: its `PATH` holds only stub launchers (a temp dir, no
`/usr/bin`, no `/usr/local/bin`; reach other tools by absolute path), and
`HOME` is a temp dir. With `/usr/bin` on `PATH` a missing stub falls
through to the host's real `omarchy agent prompt`, which opens the
operator's default agent (WP-022 handover: it happened once, harmlessly,
because the dev host has no default agent). Tests that need git skip themselves when the host has none.
`fixtures/logbook/` is read-only input.

**Monorepo layout.** Some tests read files outside the crate, so they only
compile and pass in a checkout of the whole repository, not from the
`engine/` directory alone (for example a crate tarball or an AUR source
that ships only the engine):
- `cli.rs::contract_version_matches_plugin_manifest` embeds
  `plugin/manifest.json` with `include_str!`;
- `frontmatter.rs` and `doctor.rs` read `fixtures/logbook/`;
- `index.rs` reads `fixtures/` and runs `scripts/validate-fixtures.py`.

The packaging WP (WP-040) has to either ship those directories or build
with `cargo build` only (no tests).

**Manual runs: scratch dirs and the test guard.** Every manual run of the
engine on a dev or test host starts with one scratch directory that holds
home, config, state and data, and `SELDON_TEST_GUARD` set to it:

```
S=$(mktemp -d)
export SELDON_TEST_GUARD=$S HOME=$S/home \
       XDG_CONFIG_HOME=$S/config XDG_STATE_HOME=$S/state XDG_DATA_HOME=$S/data
mkdir -p $HOME
B=$PWD/engine/target/debug/seldon
$B --json doctor     # the "config" check names $S/config/seldon/config.toml
```

Set all four, not only `HOME`: a desktop session usually exports
`XDG_CONFIG_HOME`/`XDG_STATE_HOME`/`XDG_DATA_HOME` pointing into the real
home, and an absolute XDG variable wins over `HOME`. With the guard set, a
run whose directories still point outside `$S` exits 2 before it reads or
writes anything. The real package log and `snapper` are still read (they
are absolute paths); the theme file and the watched config paths are read
under `$S/home` (point `SELDON_THEME_FILE` at the real `theme.name` to read
that one).

**Manual acceptance (WP-003).** With the scratch environment above:

```
$B init --non-interactive --no-capture --path $S/logbook
$B doctor --path $S/logbook --json   # "ok": true, snapper "degraded"
```

`seldon init` refuses an existing logbook, so remove `$S/logbook`
before running it again.

**Manual run of `seldon rebuild` (WP-032).** With the scratch environment
above, on a copy of the fixture (never on `fixtures/logbook/` itself):

```
cp -r fixtures/logbook $S/lb
SELDON_NOW=2026-10-01T17:05:12+02:00 $B --logbook $S/lb --json rebuild
#   {"files":["outputs/REBUILD.md"], "sections":{"packages":4,"deviations":5,
#    "plugins":3,"units":2,"open":4}, …}
$B --logbook $S/lb --json rebuild        # "files": [] — nothing changed
diff $S/lb/outputs/REBUILD.md engine/tests/golden/REBUILD.md
```

The document has no clock in it ("as of" is the newest ledger event), so
the second run writes nothing whatever the time. A plain fixture copy has
an empty `packages.explicit` fence (WP-036), which counts as none, so this
`diff` shows exactly the golden's "Before the logbook" group (the golden
test runs the dossier first).

**Dry run of `seldon import omarchy-agent` on a real vault (WP-043).**
The vault is read only; the output goes to a scratch copy of the fixture
logbook, never to `~/Seldon` (the operator runs `--apply` there). With the
scratch environment above:

```
cp -r fixtures/logbook $S/lb
V=<the vault copy>                      # the folder with pipeline/, journal/, knowledge/
(cd "$V" && find . -type f -print0 | sort -z | xargs -0 sha256sum) > $S/before.sha
$B --logbook $S/lb --json import omarchy-agent "$V"
#   {"mode":"dry-run","changed":true,"errors":0,"counts":{…},"collisions":[…],
#    "files":["outputs/IMPORT-omarchy-agent.md"], …}
(cd "$V" && find . -type f -print0 | sort -z | xargs -0 sha256sum) | cmp - $S/before.sha
less $S/lb/outputs/IMPORT-omarchy-agent.md
```

The fixture logbook has C-2026-001 to C-2026-008, so kit cases with those
ids show up as collisions; on the real `~/Seldon` the list differs. Read
the report before any `--apply`: it names every renumbered id, every file
left out and every redacted line.

**Manual run of `seldon dossier` (WP-035).** The dossier only *reads* the
host: `pacman -Qqe`, `-Qqm`, `-Q`, `systemctl --system|--user
list-unit-files --state=enabled`, `omarchy plugin list --json`,
`omarchy-version`, the theme file and `/proc`, `/sys` files. With the
scratch environment above, on a copy of the fixture:

```
cp -r fixtures/logbook $S/lb
export SELDON_THEME_FILE=/home/<you>/.local/state/omarchy/current/theme.name  # optional, read-only
$B --logbook $S/lb --no-commit --json dossier
#   {"files":[…], "sections":{"packages.explicit":"written",…},
#    "counts":{"explicit":…,"preLogbook":…,"omarchyBase":…,"total":…,"aur":…,"units":…,"plugins":…}, "warnings":[]}
$B --logbook $S/lb dossier              # "Nothing changed (8 fence(s) checked)"
$B --logbook $S/lb --no-commit rebuild  # §2 now has "### Before the logbook"
```

Programs and files can be pointed elsewhere: `SELDON_PACMAN`,
`SELDON_SYSTEMCTL`, `SELDON_OMARCHY`, `SELDON_OMARCHY_VERSION`,
`SELDON_THEME_FILE`, `SELDON_HARDWARE_ROOT` (default `/`),
`SELDON_OMARCHY_PACKAGES` (default `$OMARCHY_PATH/install`, else
`/usr/share/omarchy/install`; WP-036). Dev host,
2026-10-01: 169 explicit packages (167 from before the logbook), 966 in
total, 0 foreign, 40 enabled units, 38 plugins, no warnings; nothing
under the real `~/.config` or `~/.local/state` was written. WP-036, same
host with the real Omarchy 4.0.4-1 lists: 157 of the 169 are
`omarchy-base` (156 of them from before the logbook), 12 `user` (11);
REBUILD.md's "Before the logbook" block shrinks to 11 packages in 3
lines (at most 71 columns), plus "156 more come with Omarchy 4.0.4-1".

**Tests that need a logbook without a capture.** Since WP-024, `init` runs
the first capture, which records the first state of every diff collector
(config, theme, plugins) and the pacman cursor. A test that builds the
machine state *after* `init` and expects its own first capture to be the
baseline passes `--no-capture`; `Env::init_logbook*` does so for every
test. Only `tests/init.rs` exercises the wizard's capture.

**Real-host run of the wizard (WP-024).** With the scratch environment
above. It reads the real package log and `snapper` (read-only) and writes
only under `$S`. Never pass `--theme-hook` on the dev host: it runs
`omarchy hook install`, a red-zone write under `~/.config/omarchy/hooks/`
(the repository's guard blocks it; the tests stub `omarchy`).

```
$B init --non-interactive --path $S/logbook --language de \
   --harness claude-code --harness omarchy-agent --since "$(date -d '-7 days' +%F)"
#   First capture: N event(s) since …; M open drift item(s), M crisis  (dev host
#   2026-10-01: 1230 events, 22 items, all crises — WP-013 FINDINGS §2.2)
#   Harness omarchy-agent: no kit at $S/data/seldon/harness/omarchy-agent; nothing copied …
rm -rf $S/logbook $S/state $S/config
$B --json init --non-interactive --path $S/logbook --since "$(date -d '-7 days' +%F)" --baseline
#   capture.baseline {"items": 22, "events": 1230, "reason": "pre-Seldon baseline"}, openDrift 0
$B --json drift          # "openDrift": 0, "crisis": 0
$B --json capture --all  # "written": 0
git -C $S/logbook log --format=%s   # first capture and pre-Seldon baseline / init logbook
jq .logbook.git $S/state/seldon/index.json   # head = git rev-parse --short HEAD, dirty false
```

The interactive wizard needs a terminal; `script` provides one. Keys:
Enter takes the default, Space toggles a multi-select item, `y`/`n` answer a
confirmation. Export the scratch environment and `SELDON_TEST_GUARD`
*before* `script` (as above; `script` passes the environment on), pass
`--path` so the path step is skipped (its default is `~/Seldon`), and stub
`omarchy` with `SELDON_OMARCHY` in case the theme hook is answered with yes:

```
export SELDON_OMARCHY=$S/omarchy-stub    # a script that only records "$*"
(sleep 1; for k in '\r' '\r' '\r' '\r' '\r' ' ' '\r' '\r' '\r'; do printf "$k"; sleep 0.4; done
 printf "$(date -d '-3 days' +%F)\r"; sleep 4; printf '\r'; sleep 3) \
  | script -qec "$B init --path $S/logbook" /dev/null
# language, Obsidian, collectors, watched paths, more paths, harnesses (Space:
# claude-code), theme hook (no), git (yes), backfill date, then after the
# capture: "The backfill opened N drift item(s) … Mark them as the pre-Seldon
# baseline?" (Enter: yes)
```

Why the guard: on 2026-10-01 a wizard run with only `HOME` overridden
(`env HOME=<scratch> script -qec "seldon init …"`) wrote the real
`~/.config/seldon/config.toml` and `~/.local/state/seldon/`. The session
exported `XDG_CONFIG_HOME` and `XDG_STATE_HOME` into the real home, and
those win over `HOME` (WP-024 handover). With `SELDON_TEST_GUARD` set, the
same command exits 2 and writes nothing.

The engine needs Rust ≥ 1.89 (`File::try_lock`, let-chains); both hosts
have 1.98.

### The `watch` feature

`seldon watch` is compiled only with `--features watch` (ADR-0005: optional,
off by default); `just build-release` leaves it out. Its unit tests
(`commands::watch::tests`: which paths and event kinds count) and
`engine/tests/watch.rs` run with the feature:

```
cargo test --manifest-path engine/Cargo.toml --features watch --test watch
cargo test --manifest-path engine/Cargo.toml --features watch watch::
```

The watcher tests wait for real inotify events and the 2 s debounce, so the
file takes ~12 s; the timing assertions allow 4 s of slack for a loaded
machine. `Watch::start` consumes the `watching` line and the rebuild at
start, so each test sees only the rebuilds its own writes cause.

**Memory bound (PLAN.md: RSS < 10 MB).** The test runs the watcher on the
×10 fixture (`tests/common/scale.rs`) with the state lock held (so the
rebuild at start waits), reads the idle size, releases the lock, lets the
rebuild at start and one change-triggered rebuild run (500 events in the
index) and reads `VmRSS` and `VmHWM` (peak) from
`/proc/<pid>/status`. The bound is about the shipped, optimised binary; a
debug binary carries ~6 MB more code, so under the test profile only the
growth of the heap (`RssAnon`) over the idle watcher is bounded (< 6 MB;
~1.8 MB on the ×10 fixture). `VmRSS` and the peak are printed there for
information only: most of them are the debug binary's file-mapped pages,
whose idle share moves by up to ~0.8 MB between builds of the same code
(WP-091 round 3). `just check-rss` runs
the test under `--profile bench`, where the peak must stay under 10 MB.
It is not part of `just check` and CI does not run it; run it before the
handover of any WP that touches `engine/src/index/` or
`engine/src/commands/watch.rs`. To measure another binary, e.g. the musl release build with the
feature:

```
cargo build --manifest-path engine/Cargo.toml --release --features watch --target x86_64-unknown-linux-musl
SELDON_WATCH_BIN=$PWD/engine/target/x86_64-unknown-linux-musl/release/seldon \
  cargo test --manifest-path engine/Cargo.toml --features watch --test watch rss -- --nocapture
```

Dev host, 2026-10-01 (idle → after the rebuild at start and one change,
peak): debug 13.0 → 17.4 MB; bench profile 6.0 → 9.1 MB; musl release
4.8 → 7.5 MB, peak 7.9 MB.

The systemd user unit (`engine/systemd/seldon-watch.service`) is never
enabled or started by a test or an agent (AGENTS.md §6); `systemd-analyze
--user verify` checks its syntax.

## What CI cannot run, and where it runs instead

CI (`.github/workflows/ci.yml`) runs `just check` in an `archlinux:base-devel`
container with `SELDON_SKIP_HOST_CHECKS=1`. That variable makes the
host-only steps print a skip notice and exit 0:

- **`omarchy plugin validate`** needs the `omarchy` CLI, which only exists on
  an Omarchy install.
- **qmllint against the shell** needs the installed shell tree
  (`$OMARCHY_PATH/shell`, default `/usr/share/omarchy/shell`) and Quickshell's
  QML modules (`/usr/lib/qt6/qml/Quickshell`).
- **`plugin-test`** needs `node`, `quickshell`, `jq` and `python3`.

All run on the **dev host**: `just check` there runs them, and a missing
tool is an error, not a skip. Never set `SELDON_SKIP_HOST_CHECKS` on the dev
host.

## qmllint details

- **Binary.** `qmllint` from `qt6-declarative`. On the dev host it is at
  `/usr/lib/qt6/bin/qmllint` (not on `PATH`); on the test host it is
  `/usr/bin/qmllint`. The recipe uses `PATH` first, then the Qt libexec dir.
- **`qs.*` imports.** Quickshell serves the shell root as the module prefix
  `qs` (`import qs.Ui`, `import qs.Commons`), but the installed tree has no
  `qs/` directory, so `-I $OMARCHY_PATH/shell` alone does not resolve them.
  The recipe builds a temporary import root with `qs/<Module>/qmldir` files
  whose entries point back at the shell's files by relative path. Nothing
  from the shell is copied or linked.
- **Zero warnings** (`--max-warnings 0`), except two categories demoted to
  info because first-party plugins hit them as well and no plugin code can
  avoid them:
  - `missing-property` — nested token objects such as `Style.font.body` or
    `Color.popups.text` are declared as `QtObject` properties, so qmllint
    sees them as plain `QObject`.
  - `uncreatable-type` — Quickshell's qmltypes register `PanelWindow` with
    `isCreatable: false`; it is created fine at runtime.
- **Token check.** Because `missing-property` is demoted, qmllint cannot see
  a typo such as `Style.font.bodySmal`. `tests/plugin/check-tokens.py` reads
  `Commons/{Style,Color,Border,Util}.qml` of the installed shell and fails on
  any member the plugin references that they do not declare (it passes the
  first-party clock, agents and media plugins without a false positive). It
  does not replace the runtime smoke test: it knows nothing about other
  types.

## Dev host vs test host

| | Dev host | Test host |
|---|---|---|
| Rust | rustup, musl target | pacman `rust`, no musl target → `build-release` fails |
| `just` | via mise | not installed |
| qmllint | `/usr/lib/qt6/bin/qmllint` | `/usr/bin/qmllint` |

The first build fetches the engine's crates from crates.io; after that the
build itself needs no network (`cargo build --offline` works).

## Plugin

Four layers, cheapest first. The first three run in `just check`
(`just plugin-test`); the fourth is the hard acceptance gate of every plugin
WP (SPEC-PLUGIN §9).

### 1. `Model.js` under node

`node tests/plugin/model.test.js` loads `plugin/Model.js` into a plain VM
context (the file has no Qt dependencies, by design) and checks pill text
and colour rules, status precedence, tooltip wording, one banner with a
constant fix per non-ok status, that `validateArgs` accepts exactly the
command forms of CONTRACT.md (free text one non-empty argument after `--`,
`drift show <id> --json`, `[--only]`), the `XDG_STATE_HOME` index path, and
the tab helpers against the fixture: 62 Changelog rows, one "+2" group (3 members), 7
folded resolution details, 6 snapshot rows, the source filter, the crisis
strip text, the snapper banner, the Today view and the System sections with
every field optional. For the panel actions (WP-012): the case picker lists
the open cases only, active first, with ids checked; `logArgs` keeps the
note one argument after `--` (`--help`, quotes, a newline, `$(…)`) and
refuses blank text and a malformed case id; `openArgs` takes journal,
ledger, status or a case id and nothing else; the result lines read the
`log`, `open` and `capture` JSON of SPEC-ENGINE §3. For the Work tab
(WP-020): `workColumns` puts the sample's cases into Queued 3, Active 3
(2 active + 1 verification, in that order) and Completed 2, with the
`proposedEvents` count on C-2026-005 only, and survives broken entries;
`wipStatus` says "2 / 3 active" (verification does not count) with its
tone at and over the limit; `caseActions` gives each status its actions;
`planArgs` builds `plan new --zone --risk [--area] [--priority] --json --
<title>` (the title one argument after `--` for `--help`, quotes,
`-rf --zone red`, `$(…)`) and `plan <step> <id> --json`, and refuses a
blank title, a bad zone, risk, priority or area slug, a malformed id and
any other step; `validateArgs` accepts `--area` and `--priority` only in
that order; `planResult` reads both `plan --json` shapes and the engine's
refusal. For the drift sheet (WP-021): `driftItemFor` finds the four
sample items (the theme item with its proposed C-2026-005, the two
crises, the firefox group with 3 members oldest first) and a group
member's item named by that member, and nothing for resolved events;
`caseOptionsFor` puts the proposed case first, else "Pick a case";
`driftArgs` builds `drift link <id> <case> [--only] --json`, `drift
explain <id> [--only] [--zone] [--risk] [--area] --json -- <text>` (zone
only when it differs from the item's, risk only when not R1) and `drift
dismiss <id> [--only] --json -- <text>`, and refuses a malformed id or
case, no case, blank or two-line text, a bad zone or slug and any other
action; `validateArgs` accepts explain's options only in that order;
`driftResult` reads the resolving shape, the no-op (`resolved: 0`,
`already` → "Already resolved: linked to C-…") and refusals;
`driftShowResult`, `memberLines` ("… and N more"), `rowStatus` with
`explained · C-…` (ADR-0021), `firstCrisis` and `moreDriftText` ("+N
more", ADR-0020). For Decisions and Memory (WP-023): `decisionRows` lists
the sample's four decisions newest first (by id; ADR-0004 *proposed* with
the accent tone), puts a malformed id last and never actionable, and
survives broken entries; `decisionSummary` ("4 decisions · 1 proposed");
`decideArgs` builds `decide --no-edit --json -- <title>` with the title
one argument after `--` (`--help`, quotes, `-rf --case …`, `$(reboot)`,
`--`) and refuses a blank, two-line or non-text title; `decideResult`
reads the `decide --json` shape and passes on only an id matching
`^ADR-[0-9]{4}$`; `openArgs` and `validateArgs` take `logbook` and a
decision id (not `ADR-4`, `adr-0004`, a padded id, a path or `memory`);
`memoryRows` gives the sample's three lessons and two topics (path and
`updated`), every part optional, all opening the fixed target `logbook`. For the Prime Radiant (WP-030): the period ids, keys `1`–`4` and
←/→ wrapping, the summon payload (`{"period":"30"}`, anything else keeps
the period); `periodWindow` (inclusive days ending on the index's today,
across a leap day, *All* unbounded); `isoWeekMonday` (week 53 only in long
years); `seriesInPeriod` (heatmap and packages by date, a drift week that
touches the window, case spans that overlap it, open cases, broken rows
left out); `periodTable` on the sample (rows per slot for 30/90/365/All:
`30,2,5,3,17`, `90,3,5,3,18`, `365,3,5,3,18`, `366,3,5,3,18`, with the
count and detail lines) and without an index; `overlayMeta`;
`overlayBanner` keeps only *Copy*; `overlayGrid` in its three modes (exact
gaps, rows by weight, minimum heights that make the grid scroll).
For the charts (WP-031): six slots (The Plan last, `…,plan=2`); the grid
gives rows at their minimum height first and shares the rest by weight,
filling the height exactly (`rowHeights`); `dayNumber`/`dateOfDay` against
`Date` for every day of 1899–2101 and impossible dates (2026-02-30)
rejected; ISO weeks across year ends; colour steps (square root of
count/max, 0 for none); `splitSeries` gives exactly `seriesInPeriod`'s rows
for every period, on the sample and on edge rows; per chart on the sample:
heatmap cells, offset, months, hover text with sources and the hit test;
series lanes, padded flat lanes and the sample that holds at a day; drift
weeks with gaps filled (also across week 53); risk shares and the part at
an angle; the drift peak is the week with the most opened (a
non-monotonic series, ties to the later week); timeline markers, spans clipped to the window, lanes, months
and the hit test; the plan's cards and columns; empty charts without an
index; `aggregationCount` counts exactly the table's passes.

`node tests/plugin/model.bench.js` times `periodTable` (what the service
does on every index write) on the sample, the sample ×10 and 7000 timeline
rows, in a plain function scope and in a vm sandbox (as the tests load
Model.js; slow global lookups, about 8× slower and load-sensitive, so only
reported), against the WP-030 cut. It fails when the fastest of 31 plain
runs on ×10 takes more than 10 ms (idle about 1.9 ms, about 4 ms with the
host fully loaded).

### 2. `Service.qml` in a private headless Quickshell

`bash tests/plugin/service-states.sh` starts `tests/plugin/harness/shell.qml`
with `quickshell -p` and `QT_QPA_PLATFORM=offscreen`, once per scenario. The
harness loads `plugin/Service.qml` the way the shell loads a third-party
service (no parent), prints a JSON snapshot (`status`, `pill`, `banner`,
`engine`, …) and quits. It is a separate Quickshell instance: it never talks
to the running omarchy-shell and writes only to a temp dir.

Scenarios: every status (fixture index, `PATH` without `seldon`,
`index-variants/not-initialised.json`, a missing path, broken JSON,
`SELDON_NOW` three hours after `generatedAt`, `invalid/index.contract-v2.json`),
a relative `SELDON_INDEX`, an index that appears after start, an atomic
replace (temp file + rename), an engine installed while running ("Check
again"), the live loop without the dev override (capture, then status
writes the index; calls never overlap), engine exit 3, the exact argv of
the banner fixes (fake `wl-copy` and terminal launcher record it), the
crisis strip text, `index-variants/snapper-degraded.json` with the argv of
its *Copy* and *Run in terminal*, its three actions and the hint after
*Run in terminal*, which a reload of the unchanged index keeps (WP-054);
live, the hint after *Run in terminal*, then *Check again* running the
same `capture` and `status` as *Capture now* (`["fix", action, banner]`
and `["snapshot"]` in `HARNESS_ACTIONS`): with snapper fixed the banner
is gone, still failing it stays with the new message and without the
hint; `XDG_STATE_HOME` (absolute and the
ignored relative form), and dev mode never running the engine.
`tests/plugin/fake-seldon` stands in for the engine.

The panel actions (WP-012) run through `HARNESS_ACTIONS`, a JSON array of
`["log", text, caseId]`, `["open", what]` and `["capture"]`, started once
the start-up capture is done. The fake engine appends each call's exact
argv to `$HOME/argv.log` (`printf %q`, one line per call), and its `open`
hands the path to a recorded `omarchy-launch-editor`, as the real engine
does without a terminal. Checked: the note is one argument after `--` for
`--help`, `a "b" c` and a two-line text, with and without `--case`; capture
runs before status and a second *Capture now* while one is queued is
dropped; the four open variants reach the launcher with the right path and
the result line reads `open --json`; calls never overlap; blank notes, a
malformed case id and an unknown open target never reach the engine; the
engine's errors reach the result lines; dev mode and a missing engine
refuse with a reason.

The Work tab's calls (WP-020) use `["plan", action, input]` (input: a case
id, or the new case's form) and `["wait"]`, which holds the remaining
actions until no engine call is queued or running, so plan calls run one
after another as the panel sends them. The fake engine's `plan` treats the
index it last wrote as its logbook (a line `<id> <status>` in
`$HOME/cases` overrides it, for a logbook the index has not caught up
with), follows the engine's transition rules and refusal message, and
rewrites the index with the case in its new column. Checked: the exact argv
of two `plan new` calls (an option-like title, one with quotes, zone, risk,
area and priority from the form) and of start, verify, done, drop; the
cases in the index the fake wrote afterwards; `done` on an active case
refused with the engine's message as `planResult`, `lastError` empty; five
refusals in the plugin that never reach the engine; a held lock (exit 4);
dev mode.

The drift sheet's calls (WP-021) use `["drift", action, form]` and
`["driftShow", eventId]`. The fake engine's `drift link|explain|dismiss`
works on the same state index with the engine's checks and messages (a
link's case first, then the event), selects a group's open members or,
with `--only`, the named event, folds the resolution (`resolution`,
`resolutionDetail`, `case`, also for explain per ADR-0021), drops the item
or re-keys the rest of a group to a new leader, recounts the summary,
creates explain's completed case, and answers a re-run with the engine's
no-op (`resolved: 0`, `already`); `$HOME/resolved` stands for a logbook the
index has not caught up with. `drift show` lists the open members plus
those in `$HOME/extra-events.json`. Checked: the exact argv of `drift
show`, a link with the proposed case, an explain with the text `--help`
(zone as the item's, so no `--zone`), a dismiss of a group member with
`--only` and the text `say "hi"; $(reboot)`, an explain of the rest of
that group with zone, risk and area, the re-run, and a link to an unknown
case (the engine's refusal is `driftResult`, `lastError` stays empty); the
fake engine's index afterwards (five folded events, one item left,
summary 1/1, the two new cases); the no-op alone ("Already resolved:
linked to C-2026-005"); eight refusals that never reach the engine; a held
lock; dev mode.

Decisions (WP-023) use `["decide", title]`; the fake engine's `decide`
adds the next ADR (status proposed, newest first) to the state index and
answers in the SPEC-ENGINE §3 shape, its `open ADR-NNNN` opens the path
the index lists for it and `open logbook` the logbook folder;
`FAKE_SELDON_LOCKED` covers `decide` too. Checked: the exact argv of two
`decide --no-edit --json -- <title>` calls (`--help`, a title with
quotes), each followed by `open ADR-0005|ADR-0006 --editor --json` with
the id from the answer, then `open ADR-0004` and `open logbook`; the new
decisions in the fake's index; the editor launcher's paths; eight
refusals that never reach the engine (blank, two-line and non-text
titles, `ADR-4`, `ADR-0004; reboot`, a path, `memory`); a held lock (the
decide result, nothing opened) and an unknown decision (the open result
and the panel's error line); dev mode.

*Start agent* (WP-022) uses `["agent", caseId]` (`Service.startAgent`).
The fake engine's `agent start` checks the case like the engine (active
only; queued gets the `seldon plan start` hint), launches nothing, writes
the id to `$HOME/active-case` and answers with the engine's JSON for the
default launcher; `FAKE_SELDON_NO_LAUNCHER` makes it answer as the engine
does without `omarchy`. Checked: a malformed id never reaches the engine;
the exact argv `agent start <id> --json`; a second call while one runs is
refused (one at a time, shared with `plan`); the answer is `planResult`
with `action: "agent"` and "Agent started on C-2026-003 · launcher default
(omarchy)"; the queued refusal and the missing launcher are `planResult`,
`lastError` stays empty; dev mode refuses.

Isolation: the scenarios run with a `PATH` made of symlinks to the few
tools the fakes need, so a `seldon` installed system-wide never leaks in.
Every run, here and in layer 3, gets its own `HOME`, `XDG_STATE_HOME` and
`XDG_CONFIG_HOME` inside the temp dir (a case may name its own `HOME`; the
XDG dirs follow it). Both scripts end with a check
(`tests/plugin/real-home-guard.sh`) that the real `~/.local/state/seldon`
and `~/.config/seldon` neither appeared nor changed during the run; it
compares existence, size, mtime and ctime of every entry, so an engine run
by hand at the same time also fails it. One exception (WP-039): on a host
where the operator's real Seldon is live (the installed plugin runs
`seldon capture` every 15 minutes), a capture during the run rewrites the
state dir. The guard passes that as "changed by the operator's live engine
(not a leak)" only if nothing under `~/.config/seldon` changed
(`config.toml` byte-identical), no path appeared or disappeared, every
changed entry is the state dir or its `index.json`, `lock`,
`cursors.json` or `manifest.json`, and the post-run `index.json` names the
logbook `config.toml` names (`logbook.path`) and the machine it named
before the run (`logbook.machine`). It reads only `config.toml` and those
two index fields, never the logbook. Anything else still fails, with the
reason. `tests/plugin/real-home-guard.test.sh` (part of `just plugin-test`)
proves both sides in scratch HOMEs.

To watch one case by hand:

```bash
QT_QPA_PLATFORM=offscreen HARNESS_PLUGIN_DIR=$PWD/plugin \
  SELDON_INDEX=$PWD/fixtures/index.sample.json \
  quickshell -p tests/plugin/harness/shell.qml
```

A missing engine makes Quickshell log `WARN: Process failed to start,
likely because the binary could not be found` — expected, and the only log
line the plugin may cause. The service probes for the engine once at start
and again only on *Check again*, so it appears once per shell start.

### 3. `Panel.qml` in a private headless Quickshell

`bash tests/plugin/panel-view.sh` runs the real panel against the real shell
components. Quickshell serves `qs.*` from the config root of the instance,
so the script builds a temp root with copies of `$OMARCHY_PATH/shell/Commons`
and `shell/Ui`, replaces only `Ui/KeyboardPanel.qml` (a layer-shell window,
which an offscreen instance cannot create) with
`tests/plugin/harness/KeyboardPanel.qml`, and starts
`tests/plugin/harness/panel.qml` as its `shell.qml`. That harness loads
Service.qml (dev mode, a fixture), puts Panel.qml in an offscreen window,
opens it and runs `HARNESS_STEPS`: real key presses through the shell's own
`PanelKeyCatcher` (QtTest `keyClick`), tab and filter selection. After each
step it prints `Panel.view()` and every visible text.

Checks: with the sample every tab renders (Today: 4 entries, yesterday
collapsed and opened with Enter; Changelog: 62 rows, Enter on the "+2"
group opens its drift sheet with the three members, 7 folded resolution details, 6 highlighted snapshot rows,
the pacman filter narrows to 12, `f` cycles; System: seven sections), the
strip "2 changes in the red zone need a reason" on every tab, the keys
(Tab/Shift-Tab hand over to the bar, ←/→ and h/l switch tabs and wrap over all six, digits fixed per tab id 1–6, ↑/↓, Enter, Esc), the snapper banner on every
tab, the not-initialised variant, an empty and a sparse `system`, and a log
free of warnings, `TypeError`s and binding loops. The shell's `Style.qml`
asks `hyprctl` and `fc-match` for gaps and the font; the script gives it
stubs that fail, and Style keeps its defaults.

Two live scenarios (WP-012) run without `SELDON_INDEX`: the service reads
the state index the fake engine writes. They type into the QuickEntry with
real keys (`--help` would switch tabs if a key leaked to the panel), refuse
a blank note, pick a case with Tab, ↓ and Enter, open the journal, ledger
and STATUS.md with `e`, and press `c`: the fake engine's next `status`
writes an index with one more event, and the Changelog shows 59 rows
through the FileView, without a restart. The exact argv and the editor
paths are compared, and a note the engine refuses keeps its text.

The Work tab (WP-020) has three scenarios. On the sample (dev mode): the
columns Queued 3 · Active 3 · Completed 2 in cursor order, "2 / 3 active",
the "1 proposed" badge on exactly one tile, the card of the case under the
cursor with its actions by status, and nothing armed or run without an
engine. Live: `3`, `+`, the title `--help` typed into the sheet (no tab
switch), zone, risk and priority picked with Tab, ←/→ and Enter, the area
`Dev` refused in the plugin, `dev-env` accepted, Enter: the new case
appears in Queued through the FileView with the cursor on it; then start →
verify → done on C-2026-005 with Enter twice each, the case moving columns
(after start "3 / 3 active · at the limit") and the cursor following it;
Enter on the completed case opens it; Done on C-2026-008, which the fake
engine's logbook has active (`$HOME/cases`), shows the engine's refusal
and changes nothing; a cursor move disarms; x twice drops C-2026-004; `e`
opens it; the exact argv of all of it. Locked: the engine refuses the new
case (exit 4), the sheet shows the message and keeps the title, Esc and
`+` bring it back intact.

*Start agent* (WP-022): on the sample (dev mode) the active case's card
lists *Verify, Start agent, Drop, Open* and "agent: claude-code", and
neither `a` nor a click arms it ("Dev mode is read-only"). Live: `a` on a
queued case does nothing; on C-2026-003 the first `a` arms (the hint
"Start agent on C-2026-003? Press a again or click Confirm start agent.",
the button "Confirm start agent"), Enter re-arms Verify instead, `a` twice
runs and the result line names the launcher; with the mouse on C-2026-004
a click arms and the second click runs; the fake engine's logbook has
C-2026-004 queued (`$HOME/cases`), so its refusal with the `seldon plan
start` hint is the result line and the banner stays empty; the exact argv
of both calls.

The drift sheet (WP-021) has eight scenarios. On the sample (dev mode):
Enter on the theme row and `resolve:<id>` for the other three items open
the sheet with Link and C-2026-005 preselected for the theme item, Explain
with the item's zone for the two crises and the group, the group's three
members and "All 3 / Only firefox", a member row naming its own package,
and a click on the red strip opening the first crisis with the cursor on
its row; nothing can be sent. Capped: `summary.openDrift` 250 shows "+246
more open drift items not listed here". Live, with real keys: Enter,
Enter, Enter links the theme item to C-2026-005 (hint "Press Enter again:
Link tokyo-night to C-2026-005", then `linked to C-2026-005` folded, pill
`2 · 3`); a click on the strip opens the first crisis, which is
explained with the text `--help`, risk R2 (the change disarms) and area
`dev-env`; the strip drops to "1 change …", *Open C-2026-009* opens the
new case and Work lists it as completed; the firefox group is dismissed
as one (three rows `dismissed: routine update`, no badge, pill `2 ·
1`); the exact argv. `--only`: Link without a case is refused in the
plugin, C-2026-004 is picked in the case picker by keys, *Only firefox*
links the leader alone and the rest returns as "noto-fonts +1" with two
members. Already: an item `$HOME/resolved` lists shows "Already resolved:
linked to C-2026-005" and nothing changes. Locked: the refusal keeps the
text, Esc and reopening bring the draft back, another item gets its own
defaults. Members: with one member missing from `index.events`, the sheet
shows "… and 1 more", asks `seldon drift show` (always for the group's leader) and
lists all three, also when opened from a member row.

Decisions and Memory (WP-023) have three scenarios. On the sample (dev
mode): `4` shows ADR-0004 (proposed) to ADR-0001 with id, status, title,
date and file, ↑/↓ and a click on a title move the cursor, Enter and `e`
are refused with dev mode's reason, `d` opens no sheet; `6` shows LESSONS
(3) and TOPICS (2, with path and `updated`). Live, with real keys: `d`,
the title `--help "q"` (no tab switch), Enter arms ("Press Enter again:
create the decision “…”"), Backspace disarms, Enter twice sends `decide
--no-edit --json -- '--help "q"'` and then `open ADR-0005`; the sheet
closes, the keys come back and the cursor sits on ADR-0005 once the index
lists it; Enter, *Open* and `e` open ADR-0003/ADR-0004; on Memory Enter
and *Open* open the logbook folder; the exact argv and editor paths.
Refused: Enter on a blank title is refused in the plugin; the engine's
refusal (lock held) shows in the sheet and keeps the title; Esc gives the
keys back, the tab shows the refusal, and `d` brings the title back.

The step format is documented in the header of
`tests/plugin/harness/panel.qml`, e.g.
`HARNESS_STEPS="view;tab:changelog;key:Down*5;key:Return"`, plus
`type:<text>`, `settle` (no engine call queued or running),
`wait:<view path>=<value>`, `resolve:<event id|crisis>`, `click:<text>`
(the centre of the first visible item with that text) and `shot:<name>`
(saves the window to `$HARNESS_SHOTS/<name>.png`); a new scenario is one
`run` line plus its `expect`/`shows` checks in `panel-view.sh`.

Offscreen theme renders: copy a theme's `colors.toml` from
`$OMARCHY_PATH/themes/<theme>/` to
`<harness HOME>/.local/state/omarchy/current/theme/colors.toml` and add
`shot:` steps; the harness paints the theme's background under the panel.
They show the real components in the theme's colours, not the live
layer-shell window; the live sweep below stays the acceptance check.
`PANEL_SHOTS=<dir> bash tests/plugin/panel-view.sh` does this for the
Today tab in Osaka Jade, Tokyo Night and Catppuccin Latte
(`<dir>/panel-<theme>-today.png`): a live run against the fake engine, so
the render has no dev-mode note (which would print the index path) and the
QuickEntry looks as a user sees it. Since WP-051 it also renders the
not-initialised banner with its pictogram (`<dir>/panel-<theme>-uninit.png`,
fake engine in mode `uninit`) and checks the header mark, the day's state
and the banner pictogram in each.

Label fit (WP-039): every report also carries `overflow`, the visible
texts that do not fit (`elided:` a Text elided or cut at its line limit,
`wide:` content wider than its box, `button:` a qs.Ui Button narrower than
its label and padding, `outside:` text past the panel's right edge), and
`contentWidth`, the panel's width. The `fit-*` cases put a
`~/.config/omarchy/shell.toml` with `[font] base-size` 12 and 15 (font
scale 1.0 and 1.25; `Style.space` follows the font) into the harness HOME,
walk every tab and require: width 460 and 575, the tab strip on one line
with the same cell widths whatever tab is selected, no `button:`, `wide:`
or `outside:` entry anywhere, the Changelog header never elided, and no
elision at all on Today, Decisions, System and Memory (only Changelog row
text and Work mini-card titles, user content, may elide). `fit-narrow`
sets `HARNESS_CARD_WIDTH=300` (the stand-in KeyboardPanel's
`availableCardWidth`, a screen narrower than the panel): the strip wraps
and still nothing that is a label is cut. `PANEL_FIT_SHOTS=<dir>` runs
the two scales in Tokyo Night, Osaka Jade and Catppuccin Latte and saves
every tab (`<dir>/fit-100-<theme>-<tab>.png`, `fit-125-…`).

### 3b. `Overlay.qml` in a private headless Quickshell

`bash tests/plugin/overlay-view.sh` runs the Prime Radiant the same way:
copies of the shell's `Commons/` and `Ui/`, `tests/plugin/harness/overlay.qml`
as `shell.qml`, and a copy of `plugin/` whose layer-shell window
(`components/overlay/OverlayWindow.qml`) is replaced by
`tests/plugin/harness/OverlayWindow.qml`, an Item that fills the harness
window. The harness creates Overlay.qml the way the shell's overlay
Loader does: without properties, then it assigns `shell`, `manifest` and
`service`, so the overlay's bindings first run with `service === null`
and must do no work then (SPEC-PLUGIN §6; with `service` as a creation
property the harness missed the 23 aggregation passes the live shell
showed, WP-013 FINDINGS §5.1). `fresh` reports that as `firstFrame.bare`,
and the script asserts it next to `firstFrame.overlay == 0`. The harness
hands the overlay a stand-in shell facade whose
`hide()` records the id and calls `close()`, as the shell's does, and
after each step prints `Overlay.view()`, the hidden ids, every visible
text, and every text that leaves its slot or the window.

Steps (header of `harness/overlay.qml`): `toggle[:<json>]` (what `shell
toggle` does: hide when open, else `open(json)`), `fresh[:<json>]` (what
the shell's Loader does on summon: a new Overlay.qml, then `open(json)`;
the report's `firstFrame` holds the aggregation counts and paints sampled
on its first swapped frame and the frame by which every chart painted),
`summon[:<json>]`, `hide`, `key:<Left|Right|Escape|…>`, `text:<c>`,
`click:<text>`, `clickAt:<x>,<y>`, `hover:<slot>:<fx>,<fy>` and
`hoverItem:<slot>:<i>` (a real mouse move onto a point of a chart, or onto
its item i as `chart.locate(i)` places it), `leave`, `resize:<W>x<H>`,
`call:<method>:<arg>` (what `shell call jax.seldon` does), `shot:<name>`,
`view`.

Checks: closed until toggled; toggle opens on 90 d with the header (title,
machine, Omarchy version, index time, the period's dates) and the six
slots with the sample's counts and each chart's summary (its caption);
`1`–`4`, ←/→ and `h`/`l` pick periods (wrapping) and the counts and
summaries follow (`30,2,5,3,17,2` for 30 d, `366,…` for All); Esc closes through `shell.hide("jax.seldon")`; toggle closes; a click
on the scrim closes; `summon` with `{"period":"365"}` opens on 365 d; a
click on *30 d*, `call setPeriod all` (an unknown id changes nothing) and
`call view` work; *Close* closes. Layout at 1920×1080, 2560×1440 and, for
a 1.25 output scale, 1536×864 and 2048×1152 (each with every period) and
once with `QT_SCALE_FACTOR=1.25`: six slots with a size, all inside the
window, every chart with a plot of its own, no text outside its slot or
the window, no scrolling, three columns. 760×1000 reflows to two columns,
560×700 to one and scrolls. Charts (WP-031): after a `fresh` open the
first frame has run no aggregation (the service's count is the one from
before, the overlay's own 0) and painted nothing (Canvas gets its context
then); by frame 2 every chart has painted exactly once. A period switch
aggregates nothing and repaints only the charts whose data changed
(RiskDonut and The Plan have no period); hovering repaints nothing; a
resize (one dimension, the harness sets width and height separately)
repaints each chart once, also through the medium and narrow modes. Hover
read-outs from real mouse moves onto items of every chart and from `call
hover` (exact texts, e.g. the heatmap's 2026-10-01 with its counts by
source); a malformed `call hover` (no such slot, `.`, `1.2.3`, a point
outside [0, 1], one number) returns `{ error }` and leaves the hover as it
was. The aggregation count covers every chart file's own Model.js
instance: a `Model.heatmapChart(…)` call slipped into
`Heatmap.onPaintRequested` fails `fresh #2 .view.aggregations.overlay`. The not-initialised variant shows the banner with *Copy* only
and the hint, every chart in its empty state ("no data in this period",
"no cases yet · all time", "no active cases") and nothing painted; every
other index variant renders every chart. Every run's log is free of
warnings and errors.

`OVERLAY_SHOTS=<dir> bash tests/plugin/overlay-view.sh` also renders the
overlay at 1920×1080 and 2560×1440 in Osaka Jade, Tokyo Night and
Catppuccin Latte into `<dir>`, each once on 90 d and once on 365 d with
the pointer on the heatmap's last day (offscreen renders with each
theme's `colors.toml`, not live screenshots).

### 3c. The pill (`BarWidget.qml`) in a private headless Quickshell

`bash tests/plugin/bar-view.sh` (WP-051) renders the real pill the way the
bar hosts it: `tests/plugin/harness/bar.qml` gives BarWidget.qml the
shell's own `PluginBarApi` facade, bound to the theme's bar colours and
font, in a strip one bar tall on the bar background, with Service.qml in
dev mode on the sample (2 active, 4 open drift, crises: urgent tone) and
the fake engine on PATH. It reports the pill's IPC read-out, the glyph
file and box (and, for the record only, the centres the widget computes;
the glyph is placed by that formula, so they cannot disagree), and saves
the window as a PNG; `tests/plugin/png-ink.py` (standard library only)
then measures the ink of the glyph box and of the counts in that PNG.

Cases: Tokyo Night, Catppuccin Latte and Osaka Jade, each at `100` (font
base size 12: bar 26, box 16, `a4-bar-glyph-16.svg`), `125` (base size
15: bar 33, box 20, `a4-bar-glyph-20.svg`) and `out125` (base size 12 on
a 1.25 output, `QT_SCALE_FACTOR=1.25`: box 16 logical = 20 device px, the
20 px file); per theme the accent and the default tone at `100` (the
sample with no crisis, and with no crisis and no active case, derived in
the scratch dir; the three tones must give three colours); plus the
not-initialised variant (the glyph alone, dimmed). Checks: file, box,
image loaded, text, tone; brief check 4 —
`|glyph centre − digit centre| ≤ 1` measured in the pixels (device px);
the tint — the hinted glyph's pixels are exactly the pill's ink colour; a
clean log; the real-home guard. Each case prints a `measure` line (ink rows and centres).
`BAR_SHOTS=<dir>` keeps the renders (`bar-<theme>-<scale>.png`);
`BAR_WORK=<dir>` keeps the scratch dir (logs, reports).

The harness window is a whole number of device pixels tall (a multiple of
4 logical px): at 1.25, a 26 px window would be 32.5 device px, rounded to
33, and the grab stretched by 33 / 32.5, which doubles one row of the
hinted glyph.

The harness renders with Qt Quick's software renderer (the offscreen
platform), which does not paint shader effects such as `MultiEffect`;
that is one reason the plugin tints its masks through the SVG root colour
(SPEC-PLUGIN §4).

**`plugin/preview.png`** (the marketplace image, WP-041) is composed from
two of these renders, kept in `docs/images/` (monorepo only, so the plugin
repository stays small): `overlay-tokyo-night-1920x1080.png` (the
`OVERLAY_SHOTS` render on 90 d) and `panel-tokyo-night-today.png` (the
`PANEL_SHOTS` render). To refresh it after a visible change, render both,
copy them over the files in `docs/images/`, and run (ImageMagick 7; the
colours are Tokyo Night's `background` and `accent` from its
`colors.toml`):

```sh
magick docs/images/panel-tokyo-night-today.png -crop 460x538+0+0 +repage \
  -bordercolor '#1a1b26' -border 18 -bordercolor '#7aa2f7' -border 2 /tmp/panel-framed.png
magick -size 2480x1080 xc:'#1a1b26' \
  docs/images/overlay-tokyo-night-1920x1080.png -geometry +0+0 -composite \
  /tmp/panel-framed.png -geometry +1942+24 -composite \
  -strip -define png:compression-level=9 plugin/preview.png
```

The crop is the panel's 460-unit width (the tab strip ends flush with it
since WP-039) and the bottom of the journal's last entry at the default
font. The framed panel is 500×578, placed 22 px right of the overlay with
36 px to spare, so the canvas is 2480×1080. Re-check both if the panel's
layout changes. The image must stay under 1 MB (it is about 150 KB).

### 4. Runtime smoke test in the shell

The bar widget, panel and banner import `qs.Ui`/`qs.Commons`, which only the
running shell provides; layer 3 covers them against copies, the live shell
is the final check (layer-shell window, bar anchoring, focus, theme).

**Dev host.** You may copy the plugin to its dev install and validate it
there, nothing more: enabling it writes `~/.config/omarchy/shell.json` (red
zone, AGENTS.md §6).

```bash
rsync -a --delete plugin/ ~/.config/omarchy/plugins/jax.seldon/
omarchy plugin validate ~/.config/omarchy/plugins/jax.seldon
```

**Test host** (the runtime target; ssh alias in `memory/local.md`). Over
ssh, export `OMARCHY_PATH=/usr/share/omarchy` and put `$OMARCHY_PATH/bin` on
`PATH` first; non-interactive shells have neither.

1. Install and enable (docs/HERDR-SETUP.md §5):
   ```bash
   rsync -a --delete plugin/ test:.config/omarchy/plugins/jax.seldon/
   ssh test 'omarchy plugin validate ~/.config/omarchy/plugins/jax.seldon \
     && omarchy-shell shell rescanPlugins; omarchy plugin enable jax.seldon'
   ```
   `rescanPlugins` and `enable` may print "omarchy-shell is not responding"
   while the shell rescans many plugins; check with `omarchy-shell shell
   listPlugins`.
2. **After every code change, restart the shell** (`omarchy-restart-shell`,
   works over ssh). Saving files triggers the shell's hot reload, but it
   re-creates the plugin from cached components (`Qt.clearComponentCache`
   does not exist in Quickshell 0.3.1), so new code does not load.
3. Force the states. The running shell gets Hyprland's environment, not
   yours, so `SELDON_INDEX` cannot be set for it; use the real paths instead
   (on the test host only):
   - engine: copy a `seldon` binary (`just build-release`) to `~/.local/bin`
     (on the shell's `PATH`) or remove it, then
     `omarchy-shell jax.seldon.service refresh`. For screenshots in status
     `ok` before the engine writes indexes, a stand-in script that answers
     `--version` with `{"name":"seldon","version":"…"}` and exits 0 on
     `capture`/`status` without writing anything is enough; remove it after;
   - index: `cp` a fixture to `~/.local/state/seldon/index.json.tmp`, then
     `mv` it over `index.json` (the watch picks it up; a file that did not
     exist at start is found by the 5 s poll). For `ok`, rewrite
     `generatedAt` (and `state.lastCapture`) to now first; for `indexStale`,
     to three hours ago; use `index-variants/not-initialised.json`,
     `index-variants/snapper-degraded.json` and
     `invalid/index.contract-v2.json` as they are, and delete the file for
     `indexMissing`.
4. Read the result:
   ```bash
   omarchy-shell jax.seldon.service status   # status, pill, banner, crisis, snapper, engine, lastError
   omarchy-shell jax.seldon.panel pill       # what the WidgetButton shows
   omarchy-shell jax.seldon.panel open
   omarchy-shell jax.seldon.panel tab changelog       # today | changelog | work | decisions | system | memory
   omarchy-shell jax.seldon.panel filter all          # or a source
   omarchy-shell jax.seldon.panel resolve crisis      # or an event id: the drift sheet
   omarchy-shell jax.seldon.panel view       # tab, cursor, rows, badges, banners, strip
   omarchy-shell shell toggle jax.seldon     # Prime Radiant
   omarchy-shell shell call jax.seldon view ""   # while it is open: period, slots, geometry, charts
   omarchy-shell shell call jax.seldon setPeriod 30
   omarchy-shell shell call jax.seldon hover "series 0.5,0.5"    # a chart's read-out at a point
   omarchy-shell shell call jax.seldon hover "heatmap 0.15,0.5"  # a heatmap cell: see below
   omarchy-shell shell hide jax.seldon
   ```
   Keys: `wtype -k Tab`, `wtype -M shift -k Tab -m shift`, `wtype -k Down`,
   `wtype -k Return`, `wtype f`, `wtype -k Escape`, each followed by
   `jax.seldon.panel view`. Tab opens the bar's next panel
   (`Bar.switchPanelFrom`); if that neighbour opens a window instead of a
   popup panel (OmaSettings on the test host), the Seldon panel stays open.
   Heatmap probe: the grid is square and bound by the slot's height, left
   aligned with the legend beside it, so a fixed fraction such as `0.9`
   lands on empty space except at 365 d/All on wide screens. Take the point
   from `Model.heatmapLayout(w, h, weeks, labelW, labelH)`: `w`, `h` are the
   heatmap slot's `chart.w`/`chart.h` in `view`, `weeks` the number of week
   columns (5–6 at 30 d, 13–14 at 90 d, 53–54 at 365 d), `labelW` =
   3 × `Style.font.caption`, `labelH` = `Style.font.caption` +
   `Style.spacing.sm` (30 and 14 at the default tokens); then `pitch` =
   ⌊min((w − labelW)/weeks, (h − labelH)/7)⌋ and the cell in column `c`
   (oldest week 0), row `r` (Monday 0) is at
   `fx = (labelW + (c + ½)·pitch)/w`, `fy = (labelH + (r + ½)·pitch)/h`.
   `fy = 0.5` is a middle row while the grid is height-bound. Example (test
   host, 1536×864 logical, 90 d, WP-037): `w` 1410, `h` 108 → `pitch` 13,
   14 columns; today (column 13) is at `0.15,0.5`. In the headless harness
   use `hoverItem:heatmap:<i>` instead (`chart.locate(i)`, `-1` = today).
5. Panel actions (WP-012), with the real engine (`just build-release`, copy
   to `~/.local/bin/seldon`, `chmod 755`; `seldon init --non-interactive
   --path ~/Seldon-smoke`; restart the shell): `jax.seldon.panel open`,
   `wtype n`, `wtype -- "<note>"`, `wtype -k Return`, then `view` shows
   `today.quickEntry.result` with the event id; check the line in
   `~/Seldon-smoke/ledger/*.jsonl` and the journal. `wtype c` on the
   Changelog shows `capturing: true`, then the capture result. `wtype e`
   opens the tab's file through `omarchy-launch-editor` (see
   `hyprctl clients`); with a terminal editor (nvim, the Omarchy default)
   the engine on main waits 10 s for the launcher, then kills it and
   reports "did not return" (WP-012 handover). To see the case picker, put a fixture index in place
   (step 3): its cases are unknown to the smoke logbook, so a case note
   shows the engine's "unknown case" and keeps its text.
   Work tab (WP-020), with the engine's own index: `wtype 3`, `wtype +`,
   type a title, `wtype -k Tab` / `-k Right` / `-k Return` through the
   pickers, `wtype -k Return` in a text field; `view` shows `work.result`
   "Created C-…" and the case under `work.ids`; `find ~/Seldon-smoke/work`
   shows the file in `work/queued/`. Then `wtype -k Return` twice for each
   of start, verify and done: the file moves to `work/active/`, stays there
   for verification, and moves to `work/completed/`; `work.columns` follows
   each step without a restart.
   Drift sheet (WP-021) needs open drift, which a fresh logbook does not
   have: after `seldon init`, copy the fixture logbook over it (`rsync -a
   fixtures/logbook/ test:Seldon-smoke/`, no `--delete`, so init's
   `.seldon/templates` stay), set `created` in its `.seldon/logbook.toml`
   to now (so the start-up capture baselines instead of importing the
   host's history) and run `seldon status --json`; the panel then shows the
   sample's four items. `jax.seldon.panel resolve <event id>` (or
   `crisis`) opens the sheet without keys; `view` shows `drift` (action,
   case, members, hint, result). With keys: Enter twice on the action
   button, `wtype -- "<text>"` in a text field. Check the ledger lines
   (`resolution`, `refersTo`, `meta.txId` on a group), the explain case in
   `work/completed/`, and that `changelog.resolved`, `pill` and `crisis`
   follow. While the screen is locked the running shell keeps its old
   plugin code (`omarchy-restart-shell` refuses), so run the plugin's argv
   with the real engine over ssh and read the folded rows through `view`;
   the new sheet itself can then only be driven in a private offscreen
   instance (layer 3's harness with the real engine on `PATH`).
   Decisions and Memory (WP-023): `jax.seldon.panel tab decisions` and
   `view` show `decisions.rows` (the logbook's ADRs, newest first);
   `wtype 4`, `wtype d`, `wtype -- "<title>"`, `wtype -k Return` twice
   creates a decision: `view` shows `decisions.result` "Created ADR-…",
   `~/Seldon-smoke/decisions/` has the file with `status: proposed`, and
   the editor opens it (`hyprctl clients`). Enter on a row opens that
   decision; on `tab memory`, Enter opens the logbook folder. Locked
   session: run the plugin's argv (`decide --no-edit --json -- <title>`,
   then `open <id> --editor --json`, `open logbook --editor --json`) over
   ssh and read the rows through `view`; the sheet itself only in the
   private offscreen instance.
   Start agent (WP-022) launches a real agent window, so only on an
   **unlocked** session (`omarchy-shell lock status`, ORCHESTRATION §11)
   and only with a default agent set (`omarchy default agent`): on an
   active case of the smoke logbook, `wtype 3`, move to the case, `wtype
   a` twice. Pass: `work.result` reads "Agent started on C-… · launcher
   default (omarchy)", a terminal window with app-id `org.omarchy.agent`
   appears (`hyprctl clients`), its agent starts in `~/Seldon-smoke` with
   a prompt that names the case and the logbook, and
   `~/Seldon-smoke/.seldon/active-case` names the case. Restore: close the
   agent window (end the agent session first, so its hooks finish) and
   remove `~/.local/state/seldon/agent-launch.log` with the state dir in
   step 8. Without a default agent the result line shows the launcher's
   own "Choose default agent with: omarchy default agent <name>".
6. Screenshots: `grim -g "<x>,<y> <w>x<h>"` takes **logical** coordinates;
   the test host's output is scaled 1.25, so a region read off a full
   screenshot (physical pixels) must be divided by the scale. Over ssh also
   export `XDG_RUNTIME_DIR=/run/user/$(id -u)` and `WAYLAND_DISPLAY=wayland-1`.
   Shrink before committing: `magick in.png -strip -resize 80% -colors 64 out.png`.
7. Check the log of the running shell:
   ```bash
   quickshell log --pid "$(pgrep -x quickshell)" | grep -E "WARN|ERROR"
   ```
   `omarchy theme set` and `omarchy-restart-shell` start a new shell
   process, so check every instance of the run: the logs are under
   `/run/user/$(id -u)/quickshell/by-pid/<pid>/`, and
   `quickshell log <path>/log.qslog` reads one.
   Pass: nothing naming `jax.seldon` or a plugin file except the expected
   "Process failed to start" while the engine is missing (once per shell
   start). Not ours: on every bar rebuild the shell logs two
   `QObject::connect(QJSEngine, QtObject): invalid nullptr parameter` lines
   and "Handler was registered but will not be used" for other plugins' IPC
   targets; both appear with jax.seldon disabled too. Other third-party
   plugins on the test host log their own warnings (superproductivity,
   omalauncher, finder); filter by path.
8. Clean up: remove `~/.local/bin/seldon` and `~/.local/state/seldon/` unless
   the next WP needs them; after step 5 also `~/Seldon-smoke` and the
   `~/.config/seldon/` that `seldon init` wrote, then restart the shell (the
   plugin shows engineMissing again). The guard hook allows writes to
   `~/.config/seldon` only in a plain `ssh <test host> …` command (no `;`,
   `&` or `|` before the path).

**Three themes (SPEC-PLUGIN §7)**, on the test host only — switching the
theme is a system change, so never on the dev host. Every plugin WP that
changes what the panel draws repeats it:

1. Note the current theme: `omarchy theme current`.
2. With the sample index in status `ok` (step 3 above), for each of a dark
   theme, a second dark theme and a light theme — e.g. *Osaka Jade*, *Tokyo
   Night*, *Catppuccin Latte* (`omarchy theme list`):
   `omarchy theme set "<theme>"`, wait about 6 s (the shell restarts), open
   the panel and capture Today, Changelog, System and the Changelog filtered
   to pacman with the cursor on the "+2" row and Enter pressed (since
   WP-021 that opens the drift sheet); since WP-020 also Work (the columns
   and a card) and its new-case sheet; since WP-021 the drift sheet in
   Link (theme item), Explain (a crisis, from the red strip) and Dismiss
   (the group, with "Press Enter again: …" armed); since WP-023 Decisions
   (the list, and the new-decision sheet armed) and Memory.
3. Look for: every colour follows the theme — panel border, tab and chip
   fills, the zone stripes (red = the theme's urgent colour, yellow = its
   accent), the red strip, banners, the snapshot row highlight; dim text
   stays legible on the light theme; nothing keeps a colour of the previous
   theme.
4. Restore the noted theme with `omarchy theme set "<noted>"` and check it
   with `omarchy theme current`.
5. Save the shrunk PNGs under `work/active/WP-NNN/screenshots/`, named
   `<theme>-<view>.png`.

## Integration

`tests/integration/e2e.sh` checks the whole chain on a real machine: the
engine writes a logbook and the index, and the plugin reads that index and
shows it (WP-013). It is not part of `just check`. Run it before every
phase exit.

```bash
just e2e --engine-only                    # dev host: engine steps in scratch dirs (about 50 s; the musl build dominates)
SELDON_TEST_HOST=<alias> just e2e         # test host over ssh, full chain (about 30 s after the build)
```

`SELDON_TEST_HOST` is the ssh alias of the test host (default `test`). The
real name is in the git-ignored `memory/local.md`; never commit it. Under
`SELDON_SKIP_HOST_CHECKS` the recipe prints a skip notice and exits 0. The
script exits 0 when every check passes and 1 otherwise. Either way it prints
a summary with one line per failed check.

The full run refuses to start when `SELDON_TEST_HOST` leads back to this
machine (`localhost`, or an alias for the dev host). It compares
`/etc/machine-id` on both sides (falling back to `hostname`) right after the
first ssh call. That way the steps that write `~/.local/bin`,
`~/.config/seldon` and restart the shell never run on the dev host.

**Steps.** Both variants first build the static engine
(`cargo build --release --target x86_64-unknown-linux-musl`) and run the
same engine steps:
1. `--version`, and `contract-version` = `plugin/manifest.json`
   `seldon.contractVersion`;
2. `init --non-interactive --path ~/Seldon-e2e --since <now − 7 days>`.
   `init` runs the first capture itself (WP-024), and `--since` backfills
   that capture. A fresh logbook records nothing older than its creation
   otherwise, and a later `capture --since` is ignored by collectors that
   already have a cursor. The first capture must record at least one
   package event. `SELDON_E2E_SINCE_DAYS` changes the window; it must be a
   whole number of days, and anything else stops the run before the build;
3. `capture --all` after `init` must write 0 events (idempotency);
4. `plan new`, then `plan start`;
5. `log --case <id> -- <note>`. The note contains quotes and `$(…)` and must
   arrive verbatim;
6. `status`, which must give `state.status` `ok`;
7. `index --check`, which must give `valid: true`;
8. `doctor`, which must give `ok: true`.

Then index.json must have the contract version, one active case, the note
in `today.entries`, and at least one package event.

- **`--engine-only`** (dev host):
  - It sets `HOME`, `XDG_CONFIG_HOME` and `XDG_STATE_HOME` to a temp dir.
    The real `~/.config` and `~/.local/state` are never touched; the
    script ends with `real-home-guard.sh` to prove it.
  - It loads `plugin/Service.qml` into a private headless Quickshell
    (`tests/plugin/harness/shell.qml`) in dev mode against the index the
    engine just wrote. Status must be `ok` and the pill must equal the
    index counts.
  - It never touches the running shell, and it never enables the plugin.
  - The theme collector reports degraded here, because it reads the
    theme from the scratch `HOME`.
- **Full run** (test host): after the engine steps on the test host, the
  script:
  1. rsyncs `plugin/` to the dev install (`--checksum`, so unchanged
     files are not rewritten);
  2. runs `omarchy plugin validate`, and enables the plugin if it was
     disabled;
  3. waits for the shell's hot reloads, then runs `omarchy-restart-shell`;
  4. waits until `jax.seldon.service status` has settled in `ok`, after
     the plugin's own start-up capture;
  5. compares:
     - `jax.seldon.panel pill` and the service pill with the pill text
       computed from `summary` (SPEC-PLUGIN §4: the counts `A · D` after
       the glyph, zero parts hidden), and the tone;
     - Today's entry count in `jax.seldon.panel view` with
       `index.today.entries`;
     - the Changelog row count with the pacman filter and without it;
  6. opens the QuickEntry with `wtype n`, types a note that starts like
     an option, and presses Enter. The result line must say "Saved", and
     after the FileView refresh Today must show one more entry. The note
     must be in the index and in the ledger;
  7. reads the log of the shell instance (`quickshell list -a -j`, not
     `pgrep`, which also finds crash-report windows). The log must contain
     "Configuration Loaded" and no WARN/ERROR line that names `jax.seldon`;
  8. checks that no new crash report appeared under
     `~/.cache/quickshell/crashes`.

**Restore (test host).** Before it changes anything, the run:
- writes a fingerprint of the test host:
  - existence and content of `~/Seldon-e2e`, `~/.local/bin/seldon`,
    `~/.local/state/seldon` and `~/.config/seldon`;
  - the hash of the plugin dir and its enabled flag;
  - the theme;
  - the number of `quickshell` processes and of crash reports;
- copies the plugin dir to `~/.cache/seldon-e2e/`;
- writes `~/.cache/seldon-e2e/found.env`:
  - whether the plugin dir existed;
  - the enabled flag;
  - which of the four Seldon paths were absent;
- only then moves the Seldon paths that exist aside into
  `~/.cache/seldon-e2e/saved/`.

The enabled flag comes from `omarchy-shell shell listPlugins`, which
answers empty while the shell rescans. The script asks up to five times.
If it still gets no answer, the flag is recorded as unknown, and the
restore never changes it.

At the end, or on any failure through an EXIT trap, it restores the test
host in this order:
1. it removes the engine binary;
2. it restores the plugin dir;
3. it disables the plugin, but only when the flag was found `false` for
   certain and is `true` now;
4. it restarts the shell if the plugin code differs from what the shell
   runs, and otherwise sends `jax.seldon.service refresh`;
5. it waits until the service has no engine call in flight;
6. only then it removes the state, config and logbook dirs and moves the
   saved paths back.

A path is removed only when it was absent at the backup or was moved
aside. A path that the backup found but had not moved yet (a run killed
inside the loop) is the original, so the restore keeps it and says so.

It then compares the fingerprint, and checks that the service status is
the one it found (`engineMissing` on a host without an engine).

If a run was killed before its restore, `~/.cache/seldon-e2e/found.env` is
still there. The next run then restores that state first. After you kill a
run, wait about 10 s: its last ssh command may still be running on the test
host.

The lock check (below) comes before that recovery. So a run killed on a
host that is locked, or that locks later, leaves its leftovers on the test
host until the operator unlocks the session. These are `~/Seldon-e2e`,
`~/.local/bin/seldon`, the Seldon state and config dirs, the run's plugin
code and `~/.cache/seldon-e2e/`. The plugin may meanwhile keep capturing
into `~/Seldon-e2e`. The next run after the unlock restores them.

**Locked session.** The full run refuses to start while the test host's
session is locked (`omarchy-shell lock status`), because:
- `omarchy-restart-shell` refuses to restart a locked session (and
  re-locks a session that is locked);
- `wtype` would type into the lock screen.

The script checks the lock again before the restart and before every
keystroke. Unlock the test host, or keep it awake, before an unattended
run.

**Artifacts.** With `SELDON_E2E_OUT=<dir>`, the run saves these files into
`<dir>`:
- the index after the engine steps and after the plugin's capture;
- the service status and the pill;
- the panel views (Today, Changelog, Changelog with the pacman filter, and
  after the QuickEntry);
- on the dev host, the harness log.

They contain the machine id, which names the host: keep them out of the
repository. Mismatches go to the WP's `FINDINGS.md`, with the command and
an index excerpt.

## Test host follows main

The test host runs the current main build, so the operator can follow
development live (operator decision 2026-10-05, WP-098). Productive
machines run releases only (`install.sh`, `omarchy plugin update
jax.seldon`), unchanged. After every green main check and the push, the
orchestrator runs:

```bash
SELDON_TEST_HOST=<alias> just deploy-test-host <main check log>
SELDON_TEST_HOST=<alias> just deploy-test-host --dry-run <main check log>   # the plan, nothing changes
```

`--dry-run` also prints the hostname `ssh -G` resolves the alias to (no
connection), so a reader sees where the alias points. Pass the check log
as an absolute path: `just` runs the recipe in the repository root.

**Refusals** (exit 1, before anything is built or changed):
- `SELDON_TEST_HOST` unset, not a plain ssh alias, or not listed in the
  git-ignored `scripts/guard-hosts.local` (the list the guard hook
  reads; real names stay out of the repository). A productive machine is
  never listed, so it can never be a target. A host whose
  `/etc/machine-id` is this machine's is refused too, and so is one
  whose machine-id does not match its pin in the git-ignored
  `scripts/deploy-hosts.local` (one line per host, `<alias>
  <machine-id>`; the orchestrator writes it once after checking the
  alias: `ssh -- <alias> cat /etc/machine-id`), or that has no pin
  there. A dry run without a pin says "machine-id not pinned" and goes
  on; neither id is ever printed. The pin is not a second column in
  `guard-hosts.local`: guard.sh strips whitespace from its lines.
- This checkout is not on `main`, not clean, or HEAD is not
  `origin/main` (push first).
- The check log does not start with `head <full sha>` (the orchestrator's
  main check logs do, from main-check-118 on), its last line is not
  `exit 0`, the sha is neither HEAD nor an ancestor of it (a log of
  another tree), or `engine/`, `plugin/`, `schema/` (the engine compiles
  the schemas in, `engine/src/index/check.rs`) or the deploy script
  changed between that sha and HEAD. Bookkeeping commits after the check
  pass. A log with Windows line endings is refused as such.
- The host's plugin dir is a symlink, or the host lacks a tool the
  install step uses (`rsync`, `jq`, `tar`, `omarchy`, `omarchy-shell`,
  `omarchy-restart-shell`, …; for `--release`: `curl`, `git`, …). The
  refusal names what is missing.

**What it does** (exit 2 on a failure, after a log line on the host):
1. Builds the static engine as a release does (`--release --features
   watch`, release.yml and the PKGBUILD) with `SELDON_BUILD=main.<short
   sha>`, into `engine/target` whatever `CARGO_TARGET_DIR` says; the
   binary must report `<version>+main.<short sha>`.
2. Copies it to the host's `~/.local/bin/seldon`; the previous one stays
   as `seldon.prev`. When the user unit `seldon-watch.service` is
   active (`install.sh --unit`), it is restarted right after the swap so
   the watcher runs the new binary; a failed restart fails the deploy
   (exit 2). An inactive or absent unit is left alone. A later `install.sh` needs `--force` to replace it
   (it did not install it); `--release` passes it.
3. Syncs HEAD's `plugin/` (`git archive`, not the working tree) into
   `~/.config/omarchy/plugins/jax.seldon` with `rsync -rlp --checksum
   --delete`: no times, because `git archive` stamps every file with the
   commit time, and an unchanged file must keep its mtime (no hot
   reload on an engine-only deploy). A plugin dir that is not a dev copy yet — the release git
   clone, or a plain copy without the marker — is moved once to
   `~/.local/state/seldon-dev/plugin-<git|copy>-<UTC stamp>`, outside
   the plugins dir (the shell loads every dir there, and a second
   `jax.seldon` would clash). Afterwards the dir is a plain copy whose
   `.seldon-dev-build` names the build and the commit. `omarchy plugin
   validate` runs on the host copy.
4. Restarts the shell (`omarchy-restart-shell`) when the plugin files
   changed — Quickshell keeps the code it compiled until a restart
   (WP-090) — or a restart is still pending from an earlier deploy. It
   waits until the shell answers `ping` plus 5 s (the hot-reload storm,
   WP-013), and restarts only while `omarchy-shell lock status` reports
   neither `locked`, `sessionLocked` nor `secure`; an unreadable status
   counts as locked (ORCHESTRATION.md §11). Otherwise it prints "restart
   pending" and keeps `~/.local/state/seldon-dev/restart-pending`; the
   next deploy on an unlocked session restarts.
5. Smoke: `seldon --version --json` is the new version; `seldon doctor
   --json` exits 0 (degraded rows are listed, not failed); `seldon
   capture --json` on the host's configured test logbook exits 0;
   `omarchy-shell jax.seldon.service refresh`, then `jax.seldon.service
   status` settles within 60 s on `status` `ok` and `engineVersion` the
   new version, with no `restartNotice` (WP-090). While a restart is
   deliberately pending, a restart notice is reported as a note: the old
   code is still loaded. Without a graphical session (the shell does not
   answer `ping`) the smoke fails and the summary says so; engine and
   plugin are installed then.
6. Appends one JSON line to `~/.local/state/seldon-dev/deploy.jsonl`
   (`ts`, `mode`, `version`, `commit`, `pluginChanged`, `movedAside`,
   `restart`, `smoke`, `failures`) and prints a summary.

**Back to a release:** `just deploy-test-host --release vX.Y.Z` is the
only way back: `seldon.prev` holds the previous *main* build from the
second deploy on, not the release. It takes no check log and makes no `main` checks; it
downloads that release's `install.sh` and `SHA256SUMS` on the host,
refuses a mismatch, runs `install.sh --version vX.Y.Z --force`, clones
`jax-seldon-plugin` at the tag (validated, then put in place of the dev
copy, which is moved aside like above), then the same watcher restart,
shell restart, smoke and log. State written by a newer build may not
load in an older release: move `~/.local/state/seldon` aside on the host
first; the script warns but does not move it.

`just e2e` on the test host afterwards restores what it found, so the
deployed build is back after it.

## Fresh machine smoke list

A manual check for a machine that has never run Seldon, taken from the
G3 run (`work/completed/PHASE-0-EXIT-transcript.md`). The operator runs
it. It writes the real logbook, `~/.config/seldon/`,
`~/.local/state/seldon/` and, with `--theme-hook`,
`~/.config/omarchy/hooks/` (AGENTS.md §6). Agents never run it, and
never on the dev host. The commands are in `work/PHASE-0-EXIT.md`;
here is what must be true at each step. Run it before a release that
changes `init`, the collectors or the hooks.

1. **Install.** `seldon --version` prints the release version. `seldon
   doctor` without a logbook says "not initialised" and exits 3.
2. **Wizard.** `seldon init --path ~/Seldon --obsidian --harness
   claude-code --theme-hook` with a backfill date and the baseline answered
   yes:
   - the output names the logbook, `config.toml`, the Claude Code hooks
     (`3 hook(s) added`), the first git commit, the first capture with
     the backfill, `0 open drift item(s), 0 crisis` after the baseline,
     the dossier files and the installed theme hook;
   - `git -C ~/Seldon log --format=%s` is `seldon: dossier`, `seldon:
     first capture and pre-Seldon baseline`, `seldon: init logbook`;
   - no "next step" asks for `seldon hook install`, `seldon capture` or
     `seldon dossier`.
3. **Doctor.** Every line is `ok` except snapper, which is `degraded`
   with the `setfacl` read grant (ADR-0026). After the operator's read
   grant, snapper is `ok` with the count of snapshots read from the info
   files.
4. **Own install (WP-038).** The first `seldon capture --all` after
   `init` writes exactly one event: the `config-add` of
   `~/.config/omarchy/hooks/theme-set.d/seldon-theme-set.sh`. It prints
   `note: 1 config event(s) explained as written by seldon itself`.
   `seldon drift` lists nothing. The ledger holds the event and a
   `resolution` line (`source: seldon`, `explained`, `installed by seldon
   init --theme-hook`). A second capture writes 0 events.
5. **Case with Claude Code.** After `plan new` and `plan start`, `claude`
   in `~/Seldon` records every theme switch at once as `theme-set` with
   the case and `agent:claude-code` (theme hook). An agent edit of a
   `~/.config/hypr/*.lua` file becomes a `config-change` with the case at
   the next capture. Claude Code can move the case to verification from
   the logbook's AGENTS.md alone (finding F3). `/exit` adds a journal
   line and a commit `seldon: session ended (agent:claude-code)`.
6. **Close.** `seldon status`: 1 in verification, 0 open drift. After
   `seldon plan done`: 0 active, 0 in verification. STATUS.md says the
   same, with headings in English and prose in the logbook language
   (ADR-0007).
7. **Look.** Obsidian opens `~/Seldon` as a vault (`.obsidian/` present).
   With the plugin enabled, the bar pill matches `index.json` `summary`.

A failure goes to the WP that owns the step, or to a new WP, with the
command and its output (no private paths or host names).
