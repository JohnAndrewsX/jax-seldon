# TESTING.md — How Seldon is checked

One command gates every work package: `just check`, run from the repository
root. It must exit 0 before a handover (AGENTS.md §5).

## `just check`

| Step | Recipe | What it runs | CI |
|---|---|---|---|
| Runtime space | `check-runtime-space` | `df -h /run/user/<uid>` and the number of entries in its `quickshell/by-id`; a warning above 50 % full, a refusal above 80 % (see "The session's runtime dir"); `just` runs a recipe once per invocation, so in `check` it runs once, at the start, and `just plugin-test` on its own runs it first | skipped (no `/run/user/<uid>`) |
| Format | `fmt-check` | `cargo fmt --check` on `engine/` | yes |
| Lint | `clippy` | `cargo clippy --all-targets -- -D warnings` | yes |
| Tests | `test` | `cargo test` (unit + CLI tests in `engine/tests/`) | yes |
| Watch feature | `check-watch` | `cargo clippy --all-targets --features watch -- -D warnings`, `cargo test --features watch` (see "The `watch` feature") | yes |
| Packaging | `check-packaging` | `bash -n` and (when installed) `shellcheck` on `packaging/PKGBUILD` and its scripts, `packaging/check-srcinfo.sh` (`.SRCINFO` in step with the PKGBUILD), `bash tests/release/release-notes.test.sh` (the release body from `CHANGELOG.md`: the real `0.1.0` and `0.1.4` sections whole, `[Unreleased]` opening with `### Highlights`; before 0.2.0 a middle and a last section, outer blank lines trimmed; from 0.2.0 the standing paragraph, the Highlights and the link to the section, with and without a date; missing, empty and prefix-only versions, malformed input exit 1, and from 0.2.0 each Highlights rule against a passing ten-bullet base: missing or not first, no bullet, eleven, wrapped or nested, a WP number, prose or a `*` bullet, an empty bullet, a missing, `http` or wrong-tag link reference), `bash tests/release/omarchy-pin.test.sh` (WP-190, offline: `packaging/omarchy-pin` names `omacom/omarchy`, a 40-hex commit and a 64-hex sha256; against a scratch mirror served as `file://` URLs, `packaging/omarchy-validate.sh` runs a validator with the pinned sha256 on the folder and fails with it, refuses and never runs one with another sha256 (also through the real pin), refuses a malformed pin (short or upper-case commit, a key missing, twice or unknown, a trailing space, a repo with a path), a missing file, a plain `http://` URL and a missing folder; where the installed validator equals the pin, it is served from the mirror and passes `plugin/` and fails a missing entry point file, a kind without its entry point and an `omarchy.*` id), `bash tests/release/workflow-pins.test.sh` (the pinned actions and image, the image pulled from its GHCR mirror, only the `mirror` job writing packages and only on a push to `main` or `next`, the jobs after it running when it was skipped and not when it failed (WP-195), the release gate, and the plugin split's validation in `release.yml`, each with mutants), `bash scripts/check-no-network.sh` (AGENTS.md §7, WP-195: the shipped crate graph, `cargo tree --locked --offline -e normal,build --all-features`, has none of the denied network, TLS, async-runtime and DNS crates listed in the script; `mio` is allowed, notify's poll loop; and `engine/src` names no `std::net`, `TcpStream`, `UdpSocket` or `TcpListener`), `bash tests/release/no-network.test.sh` (that check on fake crate graphs and scratch sources: each denied crate, one marked `(*)`, and each network name in a scratch `.rs` file turn it red; `mio`, `curly` and `tokio-free-parser` and a name in prose or outside a `.rs` file do not; an empty graph is exit 2), `bash tests/release/mirror-image.test.sh` (`packaging/mirror-image.sh` against a fake `skopeo`: no copy when GHCR serves the digest; a copy by digest with digests preserved when it does not, checked again after it; a GHCR that serves other bytes copied again; a failed copy and a copy GHCR does not serve exit 2; no or two mirror digests and no token refused before any registry call; the real workflows pin one digest), `bash tests/release/store-readme.test.sh` (the plugin split as the Omarchy plugin store scans it, WP-042: no command in `plugin/README.md` or `plugin/SECURITY.md` pipes a download into a shell, no agent files under `plugin/`; one mutant per case, and controls that a code span in prose may name the panel's one-liner) | yes |
| Install script | `check-install` | `bash tests/install/install.test.sh`: `install.sh` against a mock of the release layout served as `file://` URLs, scratch `HOME` and prefixes, no network — latest via the API with and without `jq`, a re-run changes nothing (bytes and mtimes), update and downgrade, `--unit` (the unit byte-identical for `~/.local`, `ExecStart` rewritten for other prefixes, never enabled), refusals before the first write (checksum mismatch, no `SHA256SUMS` line, wrong binary version, missing release, bad arguments, a foreign `jax-seldon`), the script piped to `bash` and truncated, `--uninstall` (only matching files; refused while the unit is enabled), the man page and the completions (WP-049: the fake binary answers `completions`/`mangen`; a scratch `/usr/share` via `SELDON_INSTALL_SHARE` has the bash-completion, fish and zsh directories, and fake `fish`/`zsh` in a PATH dir decide which shells exist (the host's zsh and fish are left off PATH): zsh's directory without `zsh` installs nothing, a fake `zsh` adds its completion and the `fpath` hint, `fish` without its directory installs nothing; a release without the commands skips them; a foreign completion is kept unless `--force`; a completion of a shell that is gone stays in the manifest and `--uninstall` removes it), the build-provenance check (WP-080: the host's `gh` is left off PATH; `gh` stubs that verify against the mock releases' attestations, fail on their own, are not logged in, too old or missing an option, or absent, each with and without `--require-verified`, plus `--skip-provenance`; a tampered release, one attested only for a branch, one from a self-hosted runner, one before attestations (v0.1.1), a `GH_HOST` of another server, the exact `gh` argv), no `sudo`/`systemctl` call, the real `~/.local/bin`, `~/.config/systemd/user`, completions and man page untouched; `shellcheck` when installed | yes (`shellcheck` in the release workflow's container) |
| Deploy script | `check-deploy` | `bash tests/deploy/deploy-test-host.test.sh` (WP-098): `scripts/deploy-test-host.sh` in a scratch git repository with a bare origin, against a fake test host — an `ssh` stub runs the remote scripts here under `env -i` with a scratch `HOME` and a `PATH` of stubs (`omarchy-shell`, `omarchy-restart-shell`, `omarchy`, `curl`, `git clone`) plus single linked tools, so the host's real `omarchy-*`, `quickshell`, `hyprctl` and `systemctl` are out of reach; a `cargo` stub builds a fake engine. Refusals before any build or change (no or unlisted host, a machine-id that does not match the pin (one ssh call, no id printed), no pin or no pin file (with the hint; a commented pin does not count), a prefix or comment word of a listed one, an ssh option as host, no host list, the host is this machine, not on `main`, a modified or untracked file, HEAD not pushed, a check log missing, not ending in `exit 0`, saying the Quickshell harnesses were skipped (WP-161), without a first line `head <full sha>`, with a short, unknown or other-branch sha, or with `engine/`, `plugin/`, `schema/` or the script changed since that sha — a docs-only commit passes —, Windows line endings, bad arguments, a symlinked plugin dir, a missing remote tool); `--branch next` (WP-155): the same refusals for `next` (not on next, `--branch main` or no `--branch` on next, no `origin/next`, a modified file, HEAD not pushed, a plugin change since the checked commit, main's check log, a log that skipped the harnesses), a bad or missing `--branch` value, `--branch` with `--release`; a dry run (the `next.<sha>` build, the backup planned, nothing changed), next over a main build (`+next.<sha>` engine and marker, `seldon.prev` the main build, the backup `backup-before-next-<UTC stamp>` with `shell.json`, `~/.config/seldon`, the state dir, the logbook, the main engine and the main plugin copy, a `RESTORE.txt` with a line per entry, in the order watcher stop, engine, logbook, config, state, `shell.json`, then the main deploy, then the watcher start, named and printed in the summary and the log line), next onto next (no second backup), next onto a release (the backup and `RESTORE.txt` hold what exists), an active watcher (stopped before the copy, started again on the old engine before the swap, restarted on the new one), a watcher that does not stop or start again (exit 2, engine and plugin unchanged), no engine and an engine that names no logbook (refused, with the fix), an engine that is already next under a main marker (no backup; a main deploy warns), the engine and `shell.json` as links to files outside the fake home (backed up as links), a newline in the logbook path (refused, nothing measured), a logbook the engine names that does not exist (the dry run says "absent"), a host `du` that prints nothing (refused), a logbook at `/`, a parent of the home, the home, above `seldon-dev`, outside the home or linked out of it (refused before any build, never measured or copied), the size cap (refused, its message; a cap that is not a number; under the default cap), a logbook that turns into a link out of the home after the probe (the install step refuses it before the copy), a failed backup (the fake `cp` fails on the state dir, as root too: exit 2, engine, plugin and state unchanged, logged with the partial backup, only the state dir missing, the watcher started again; the retry backs up again), main and `--release` onto next (a warning pointing to `RESTORE.txt`); the test dir under `target/`, and `cp` and `du` on the fake host refuse every path outside it; dry run (no build, the host unchanged); first deploy (marked build with `--features watch` into the repo's target dir, `ssh -G` and the engine found in the dry run, a host without an engine, `ping` before the restart, `seldon.prev`, the release clone moved out of the plugins dir, HEAD's plugin files plus `.seldon-dev-build`, one restart, smoke, log line); an engine-only change (no restart, unchanged plugin files keep their mtime, an exported `CARGO_TARGET_DIR` ignored); the settle wait; removed and added plugin files; each locked state and an unreadable lock status (restart pending, caught up by the next deploy on an unlocked session); a restart notice while the restart is pending (a note); a failing restart, doctor, capture, service version, restart notice, host-side validation, build, a build without the marker, and no graphical session (named in the summary); an active `seldon-watch.service` restarted on the new binary, an inactive one left alone, a failed unit restart (exit 2); `--release` (install.sh with `--force`, the watcher restarted on the release binary, the clone at the tag, the dev copy moved aside), a clone that fails validation, a checksum mismatch and a missing release; the real `~/.local/bin/seldon`, plugin dir and `~/.local/state/seldon-dev` untouched; `shellcheck` when installed | yes |
| Dev-host guard | `check-guard` | The PreToolUse guard hook `scripts/guard.sh` (WP-130; it runs `scripts/guard.py`, a bash parser that decides on the command position): `bash scripts/guard-test.sh`, the expectation table (one row per allowed or blocked case, a fixed fake `HOME` and working directory, nothing is executed), then `python3 scripts/guard-mutants.py`: each mutant drops one rule of `guard.py` and the table must fail for every one; `shellcheck` when installed | yes |
| Runtime dir | `check-runtime-dir` | `bash tests/plugin/runtime-dir.test.sh` (WP-161): under `tests/` and `scripts/`, outside comments, no mention of the name `XDG_RUNTIME_DIR` other than an assignment (no expansion with or without a default, no `printenv XDG_RUNTIME_DIR`, no `v=XDG_RUNTIME_DIR` for a `${!v}`) and no `/run/user` path unless the line carries `# live runtime dir: <reason>`; every Quickshell (`"$qs_bin"`, `${qs_bin}`, `$qs_bin`, `quickshell`, `qs`) started with `-p`, `--path` or `--path=` sets `XDG_RUNTIME_DIR` in the same command, itself or through an array it expands (`"${envs[@]}"`) whose definition sets it; every file that makes `rt=$(mktemp …)` removes `"$rt"` in an EXIT trap, itself or in the function the trap calls (`trap cleanup EXIT`); no `*.rs` under `engine/` names `XDG_RUNTIME_DIR`. Then mutants of the harnesses, `e2e.sh`, `deploy-test-host.sh` and the leak guard (the old `${XDG_RUNTIME_DIR:-…}` back, `:=`, the session's dir passed on directly, through `printenv` or `${!v}`, the setting dropped under each start spelling, the trap without `"$rt"`, a marker dropped) must each be caught for the right reason; `shellcheck` when installed | yes |
| Contract | `schema-validate` | `bash scripts/validate-fixtures.sh` (WP-002); skipped with a notice while the script does not exist | yes |
| User guide | `docs-check` | `bash scripts/docs-check.sh` (WP-045): builds the engine (debug), then checks `docs/user/`: relative links, images (with alt text) and anchors resolve; every language folder has the same pages as `en/` with the same heading levels, code blocks, tables and images; every translated page has its `<!-- source: en/<page> @ <commit> -->` line (a source commit older than the English page's last change is a warning; a commit missing from a shallow clone is a notice); every `seldon …` in a code span or a `sh` block names commands and options that `--help` lists (`PLANNED` in the script holds commands the guide names as planned); the help blocks of `05-cli-reference.md` equal `seldon <command> --help` with the global options left out. The front pages (`FRONT_PAGES`: `README.md`, `plugin/README.md`, `plugin/SECURITY.md`, `docs/DEVELOPMENT.md`, `llms.txt`, WP-046) get the same link, anchor and `seldon …` checks; a page under `plugin/` may link or embed only files inside `plugin/` by relative path (it is published on its own by `git subtree split`); an absolute link into the public repositories (`github.com/JohnAndrewsX/jax-seldon[-plugin]` blob/tree/main, `raw.githubusercontent.com`, the repository root, a workflow badge) must name a file and heading that exist here; every image is at most 1 MB. Other URLs are not fetched. `--write` regenerates the help blocks. `SELDON_BIN` skips the build | yes |
| Plugin manifest | `plugin-validate` | Omarchy's plugin validator on `plugin/`: `omarchy plugin validate plugin/` where the omarchy CLI is installed (a notice when `$OMARCHY_PATH/bin/omarchy-plugin-validate` is not the pinned one); without it `packaging/omarchy-validate.sh plugin/`, the validator at the commit of `packaging/omarchy-pin`, fetched over HTTPS and refused unless its sha256 matches (WP-190; it is bash and jq, no Omarchy install needed) | yes (the pinned validator) |
| QML lint | `qmllint` | `qmllint` on `plugin/*.qml`, `plugin/components/*.qml` and `plugin/components/overlay/*.qml` against `$OMARCHY_PATH/shell`, then the token check `tests/plugin/check-tokens.py` | **no** (dev host) |
| Plugin logic | `plugin-test` | `node tests/plugin/model.test.js`, `node tests/plugin/model.bench.js`, `bash tests/plugin/terminal-scripts.sh`, `bash tests/plugin/real-home-guard.test.sh`, `bash tests/plugin/check-tokens.test.sh` and `python3 tests/plugin/check-tokens.py --rules` on every plugin QML file (SPEC-PLUGIN §7's house rules, WP-177), then the Quickshell harnesses `bash tests/plugin/service-states.sh`, `bash tests/plugin/desk-view.sh`, `bash tests/plugin/bar-view.sh`, `bash tests/plugin/ipc-restart.sh` — only when something under `plugin/`, `tests/plugin/`, `schema/`, `fixtures/` or the `justfile` changed against the merge base with `main` (committed, staged, unstaged or untracked), always when `HEAD` is the merge base (on `main`, a detached `main`, a branch without its own commit), and with `SELDON_FULL_CHECK=1`; otherwise a notice says they were skipped, and `deploy-test-host` refuses such a log (see "Plugin") | the node and bash parts yes (WP-190; the bench with `SELDON_BENCH_BUDGET_SCALE=3`); the Quickshell harnesses **no** (dev host; WP-191) |

Other recipes: `just check-rss` (the `seldon watch` memory bound on an
optimised build; not in `check`; CI measures it after `just bench` without
failing on it (WP-195); required before the handover
of a WP that touches `engine/src/index/` or `engine/src/commands/watch.rs`;
see "The `watch` feature"), `just check-perf` (SPEC-ENGINE §1's time
budgets at the stated scale, WP-076; opt-in, not in `check`, not in CI:
it needs an optimised build (`--profile bench`, the tests refuse a debug
build) and a quiet host, since a busy one roughly doubles a timing. It
runs `cargo bench --bench index` with `SELDON_BENCH_X150=1` (the index
build ×10 and ×150 < 100 ms; `just bench` in CI asserts ×10 only and
prints ×150), then the ignored tests of `tests/index.rs`,
`tests/hooks.rs` and `tests/redaction.rs` one at a time: `status` at
11 656 ledger lines, 304 cases and 365 journal files < 100 ms,
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
"Integration"), `just deploy-test-host` (the main or next build onto the
test host; see "Test host follows main").

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
| `engine/tests/hooks.rs` | `hook claude-code`, `hook generic`, session start and stop, `hook install claude-code` (WP-009); `uninstall::` (WP-049): install then uninstall gives the user's file back as it was, a second uninstall changes no byte, a user's hook added to Seldon's group stays, a file with only Seldon's hooks is deleted (its directory stays), the logbook default commits `seldon: hook uninstall claude-code`, a broken file is refused unchanged, no logbook → exit 3 (not an agent hook's silent 0). `privileged::a_secret_given_as_an_argument_is_never_recorded` (WP-140): a line with `chpasswd`, `htpasswd -b`, `passwd --stdin`, `usermod -p`, `smbpasswd -s`, a key piped into `cryptsetup` or `sh -c` around them is recorded as `<program> ‹redacted›`; an nmcli secret is masked by its §7 rule and the line stays; `usermod -aG` and a bare `cryptsetup open` stay whole; no secret in the ledger, `detail`, `meta.command` or the index. Round 2: a key file written on the same line before `cryptsetup` (and `2>/dev/null`, which feeds nothing), `passwd` fed by a pipe or a here-string (`passwd -S` stays), `usermod --passw=…`, `useradd --password $(openssl passwd …)`, `openssl passwd`, `wpa_passphrase`. Round 3: `privileged::format_characters_hide_no_secret_on_a_command_line` (U+200B inside `token=` and `Authorization:`, U+2060 and U+00AD dropped from the recorded line and masked; a note keeps its U+200D). Unit tests in `commands/hook/secret_args.rs` cover each form, clusters, value options and getopt prefixes |
| `engine/tests/own_writes.rs` | SPEC-ENGINE §5 rule 7 (WP-038): `init --theme-hook` and `hook install` under a watched path, the next capture explains the `config-add`/`config-change` and opens no drift. The removals (WP-049): `init --remove-theme-hook` deletes the hook and its script, records `op: delete`, the next capture's `config-remove` is explained `removed by seldon init --remove-theme-hook` and drift stays 0; a second removal does nothing; removed before any capture saw it → no event; edited by hand, then removed → the removal is drift; `hook uninstall` of a watched file with only Seldon's hooks → an explained `config-remove`, of a shared file → an explained `config-change` (`op: remove`); the theme hook under the lock (WP-074, WP-052): an `omarchy` stub that copies the hook and then runs a real `seldon capture` gets exit 4 for that capture, and the next capture explains the `config-add` (a lock-after-write mutant lets the capture record unexplained drift); with the lock held, `init --theme-hook` exits 4 with the temp tree byte-identical, no `owned.json` and no `omarchy` call. The unit test `commands::setup::tests::the_theme_hook_step_writes_nothing_while_the_lock_is_held` covers the step alone |
| `engine/tests/own_changes.rs` | SPEC-ENGINE §5 rule 8 (WP-086): updating, enabling and disabling `jax.seldon` and upgrading `jax-seldon` are explained by the capture that writes them (actor kept, other plugins and packages stay drift, adding, downgrading and removing Seldon stay drift, idempotent); WP-088: own changes left open by an earlier capture are explained by the next one, a dismissed row keeps its resolution, add and downgrade stay drift, no case is created; a month file that cannot be read still lets the new change be explained (with the "not checked" warning) and the next capture catches up; an own change dated after the capture clock is explained at its own time, the index folds it, and a dismissal written before it in the ledger is kept |
| `engine/tests/frontmatter.rs` | `round_trip::` every case, journal, decision, area, memory file and `PROJECT.md` of `fixtures/logbook/` parses into its typed record and re-serialises byte-identical; a lossless update changes only the edited lines; `refused_saves::` (WP-077): `log`, `event` and `drift link` on a case whose save would be refused (an `events:` flow list continued at column 0) exit 1 and leave every file of the logbook as it was (`hooks.rs::claude_code::a_case_whose_save_is_refused_records_nothing` for the hook); `case_ids::a_refused_value_is_named_escaped` (WP-077): an ESC sequence in a case, decision, journal or area value is named as `\u{1b}`, serde's `unknown variant` messages included; `hooks.rs::generic::a_refused_payload_value_is_named_escaped` does the same for the generic hook's `actor`, `case` and `startedAt` |
| `engine/tests/init.rs` | `init::` layout (SPEC-LOGBOOK §2), JSON output, git first commit, `--no-commit`, German templates, Obsidian, path precedence, refusals (existing logbook, non-empty dir, plain `init` and `--ask` without a terminal naming `--defaults`), `--defaults`/`--ask`/`--non-interactive` exclusive, lock held → exit 4. `defaults::` (WP-119, ADR-0033): `--defaults` with a stdin pipe held open and never written (a prompt that read it would hang: the run must end within its deadline) looks back from local midnight 90 days before `SELDON_NOW` (a transaction 100 days back stays out, the first day's is in), dismisses every opened item "before Seldon", prints the one History line, and a second capture writes nothing; Obsidian's settings only with `obsidian.desktop` in `$XDG_DATA_DIRS` or the Flatpak's in `$XDG_DATA_HOME`; `--since` and `--no-capture` still decide; `--non-interactive` looks back too, without detection or a harness, and the Agents row says how to add one. `not_initialised::` (CONTRACT.md rule 10): exit 3's `--json` error equals `fixtures/errors/not-initialised.json` for no folder and an empty one, `not-initialised-not-empty.json` for a folder with a file (and `init --defaults` refuses it), `reason: logbook-folder-not-a-folder` for a file in its place; the text form is unchanged. `setup::` (WP-024): the first capture (stubbed sources, cursors set, a second capture writes nothing, `--no-capture`), `--since` backfill → open drift, `--baseline` → zero open drift with one `dismissed` "before Seldon" line per member and the commit `seldon: first capture and pre-Seldon baseline`, flag errors before anything is written, `--harness claude-code` (settings in the first commit, `hook install` afterwards changes nothing), `--harness omarchy-agent` with a kit (copied, modes kept, merged with Claude Code's hooks) and without one, the theme hook (a recording `omarchy` stub: exactly one `hook install theme-set <script>` on opt-in, none without, a failure with its fix, an existing hook not reinstalled), the templates (written as rendered; frontmatter keys, headings, fences and table headers identical in `en` and `de` and equal to `tests/golden/init-skeleton.txt`, `SELDON_BLESS=1` rewrites it; German prose); the agent rules (WP-047): the generated `AGENTS.md` in `en` and `de` has the ten sections in order (Session start, The engine is the only writer, Work in cases, Zones, Commands, Journal and memory, Drift, Hooks, Ending a session, Never), each with its key `seldon` commands, links `docs/AGENT-GUIDE.md`, and `init` writes no `CLAUDE.md`; the config (WP-074): a config without `language` leaves it to the locale (`LANG=de_DE.UTF-8` → `de`, written back), a `language` key wins over the locale; a read-only config folder → exit 2 with nothing in the logbook folder, the same `init` runs once it is writable; a layout that cannot start (read-only parent) → the config byte-identical, or still absent; a layout stopped half-way (a logbook path whose `areas/hyprland/README.md` exceeds PATH_MAX) → the config restored, the folder `init` created removed, an empty folder that was there emptied again, a second `init` succeeds; `layout::create` writes the marker last; `--no-git` writes `autocommit = false` and `doctor`'s git check is ok; a re-run keeps a hand-set `autocommit = false` (no repository) unless `--git`. WP-079: the read grant is the optional snapper step; a user listed in `ALLOW_USERS` gets the revert plus the read grant as a recommended step, another user no snapper step |
| `engine/tests/plan.rs` | `plan::` new (template, canonical frontmatter, area on first use, ids never reused), start/verify/done/drop (folder moves per ADR-0012 §9, `started`/`closed`/`snapshotBefore`, `.seldon/active-case`, body byte-identical outside the Log), invalid transitions → exit 1 and nothing written, list/show against `case.schema.json`, `plan list` with one invalid case file (a warning line naming it, the others listed, exit 0; WP-077), a case file name with an ESC sequence named escaped in the warnings of `plan list` and `index` (`a_warning_names_a_case_file_escaped`), the fixture logbook (a copy), git autocommit with `--no-commit` and `git.autocommit = false` |
| `engine/tests/log.rs` | `log::` notes with and without a case (`case.events`, `agents`), the Log section append-only over three steps, the journal appended not rewritten, free text as one argument (spaces, quotes, `$(…)`, `--json` after `--`), month and day by timestamp, redaction, exit 3/4 |
| `engine/tests/journal.rs` | `journal::` appends to a fixture day (only the `cases:` line changes), CRLF days, the `plan done` stub in the logbook language |
| `engine/tests/commands.rs` | `event::` (fixture line shape, typed meta, engine-only kinds refused), `decide::` (ADR numbering, the logbook's own template, the editor gets the path as one argument), `open::` (paths, `--editor` without a terminal) |
| `engine/tests/agent.rs` | `seldon agent start` (WP-022): a recording stub `omarchy` gets exactly one argv, the prompt one element (a title with quotes, `$(…)` and backticks stays text), cwd and `SELDON_LOGBOOK` the logbook, the case becomes the active case, under 1 s; `[agent] launcher` and `[agent.launchers]` from config; a shell launcher refused before anything changes; a queued case → exit 1 with the `seldon plan start` hint; verification, unknown and malformed ids; a missing launcher and one that exits 1 at once (its stderr is the message) → exit 1 with the previous active case restored; a launcher that keeps running is detached (own process group, alive); exit 3 without a logbook. Unit tests in `commands/agent.rs` check the launcher rules |
| `engine/tests/rebuild.rs` | `seldon rebuild` (WP-032) on a copy of `fixtures/logbook/`: `outputs/REBUILD.md` equals `tests/golden/REBUILD.md` (`SELDON_BLESS=1 cargo test --test rebuild` rewrites it; the golden test first runs `seldon dossier --section packages` with the query shims, so the document has the "Before the logbook" group and the test checks that it lists every `pre-logbook` package of class `user` of `packages.explicit` under the command of its origin, WP-035, and one line counting the six `omarchy-base` ones, WP-036), the seven English headings in order, German prose, `--json` `sections` counts, and every package line's last code span is the id of an explicit `install` event of that package; a second run at a later clock writes nothing (`files: []`, same bytes, no commit); text above and below the `rebuild` fence survives a change; the autocommit `seldon: rebuild` happens once per change (git repository made in the test); `drift dismiss`/`explain` move items to "Deliberately not reproduced" and out of the open questions; appended ledger lines prove `pacman -U` → `omarchy pkg aur add`, no command → "repository unknown", a later `remove` drops the package (English logbook); an empty logbook says "none"; exit 3 without a logbook. Unit tests in `rebuild/mod.rs` check the repo/AUR rule and the fence merge |
| `engine/tests/dossier.rs` | `seldon dossier` (WP-035) on a copy of `fixtures/logbook/` with every host query shimmed (`Env::query_shims`: the package manager, `systemctl`, `omarchy` print `fixtures/logs/pacman-Q*.txt`, `systemctl-*.txt`, `plugin-list-after.json` for exactly the query argument lists, exit 64 for anything else, and log each call; `SELDON_HARDWARE_ROOT=fixtures/logs/hardware`; `common::Env` sets `SELDON_OMARCHY_PACKAGES=fixtures/logs/omarchy-packages`, copies of Omarchy's two package lists, for every test, WP-036): the system files equal `tests/golden/dossier.md` (`SELDON_BLESS=1` rewrites it), all eight fences present, the text outside the fences byte-identical, only read-only queries ran (each once), `seldon: dossier` committed once; a second run a day later writes nothing ("Nothing changed"); emptied fences are all filled (the ledger's four installs marked `since`, eleven `pre-logbook`, seven of class `omarchy-base` and the rest `user`, cased config rows, hardware from files); without Omarchy's lists every package is `user`, with exactly one warning, and a second run changes nothing; a later cased `config-change` fills only the empty case cell of an existing deviations row (a row with a case keeps it), and a second run changes nothing; a missing program skips its fences with a warning and keeps them; `--section` writes only its fences (comma list and repeats, unknown value exit 1); a cased `config-change` adds a deviations row and keeps the old rows byte for byte; an agent's `systemctl --user enable` with a case fills the unit's case; `capture` and `status` never touch the dossier; exit 3 and 4. WP-075: a Latin-1 `system/notes-latin1.md` is skipped with the index's warning, kept byte for byte, and every fence is built as without it (`a_file_that_is_not_utf8_is_skipped_and_the_fences_are_built`); a Latin-1 `plugins.md` keeps its bytes and `plugins.list` is skipped with a warning, never appended elsewhere (`a_default_file_that_is_not_utf8_keeps_its_fences`); a cased config path holding `<!-- seldon:end -->` gets one neutralised row, a user row of such a path gets its case filled, and two more runs change nothing (`a_marker_in_a_cased_path_is_listed_once`). Unit tests in `dossier/` cover history rows, the explicit-line format (with and without a class), reading Omarchy's lists (comments, missing files), unit cases, the deviations rows and the case fill (reason with `|`, an editor-padded `| --- |` separator in `deviations.table` and `packages.history`, row date newer than the event, other column orders), appending a missing fence under a heading in the logbook language, `/proc` parsing, and (WP-075) unreadable files: a Latin-1 file is never written, a missing fence is not appended while its default file or a file whose bytes hold its begin marker is unread, an unreadable file blocks only appending, a non-UTF-8 file name is written back to itself, and fence bodies are neutralised |
| `engine/tests/import.rs` | `seldon import omarchy-agent` (WP-043) on the synthetic vault `fixtures/vaults/omarchy-agent/` (copied into the test home, so the report says `~/omarchy-agent-vault`), against a German logbook that already has C-2026-001 (the kit's C-2026-001 collides) and a journal entry on a kit session day: the dry run writes only `outputs/IMPORT-omarchy-agent.md`, equal to `tests/golden/IMPORT-omarchy-agent.md` (`SELDON_BLESS=1 cargo test --test import` rewrites it), commits it as `seldon: import omarchy-agent (dry run)`, and a second dry run writes nothing; `--json` counts and the collision; neither the fake token nor `/home/user` reaches the report. `--apply`: one commit `seldon: import omarchy-agent`, clean tree, the vault byte-identical; ids kept or renumbered (tag, title line), status folders (nothing active), Intent/Plan/History with demoted headings and the fenced `##` line kept, redaction and `~` in the case; `index --check` valid without warnings; seven ledger notes: the apply's own (no case, subject `omarchy-agent`, at the apply's time, WP-075) and six at local midnight of `created`, attached to their cases; journal days created or appended (`cases:` from the kit links), fenced headings stay text; memory sections (lessons appended, new topic files, no file for an empty topic); two deviation rows, the duplicate path and resolved entries left out; the marker; a second `--apply`, `--apply --json` and dry run change nothing ("Nothing changed", same bytes, no commit), also with the marker removed (the ledger's notes). The fixture names the colliding C-2026-001 as a wikilink and as bare ids: they come out as C-2026-007 in case bodies (the case's own Protokoll included), the journal (and its `cases:`), a lessons section and a deviation reason, counted per file under "Id rewrites"; `Ergebnis` is the case's `## Result`; a `done` case without `closed` gets `created` and an "Assumptions" row; an existing memory file's `updated` moves to the import day. Import notes without the marker → exit 1 with the undo hint. An Obsidian-padded `| --- |` separator in `deviations.table` keeps its user row on `--apply`. A failed apply (read-only `system/`, skipped as root) exits 2 with "nothing was committed" and an undo that names only the import's files (`git --literal-pathspecs checkout <commit> -- … && rm -f -- …`), no commit and no marker; the next run is refused with the same undo until it is run, then the import runs once (WP-061). Pending changes are committed first (`seldon: before import omarchy-agent`): a note, an inbox file and an edit of `memory/lessons.md` made before a failed apply, and a new file and an edit made after it, all survive the printed undo (run with `sh -c`); with `--no-commit` a dirty logbook is refused before anything is written. A tampered `.seldon/imports/omarchy-agent.undo.json` (no hash base, `.git/…`, a file outside the import's folders, `..`, `:/`, `*`, or a folder replaced by a symbolic link to a place outside the logbook) is never printed as a command; a successful apply removes a leftover undo file. An unknown kit status blocks `--apply` (exit 1, nothing written but the report, which lists the error). Not a vault or not a directory → exit 1, no logbook → exit 3, `--apply --dry-run` → exit 1. WP-075: a vault without cases (only `journal/` and `knowledge/`) is refused a second apply after the marker is deleted and after a failed apply (read-only `memory/`), the journal day keeping one imported block (`a_vault_without_cases_is_never_imported_twice`); a case file with a Latin-1 name is a report error ("file name is not UTF-8", the path with U+FFFD), an inbox file with one is only listed, an unreadable case is an error too (skipped as root), the dry run exits 0 and `--apply` 1, and without the bad case the rest imports (`a_file_name_that_is_not_utf8_is_a_report_error`); a vault folder with a Latin-1 name is read (`a_vault_path_that_is_not_utf8_is_read`); a kit case with a BOM and padded `---` fences imports (`a_bom_and_padded_fences_import`); `<!-- seldon:end -->` in a kit deviation heading stays inside the `deviations.table` fence, neutralised (`a_marker_in_a_kit_heading_cannot_end_the_deviations_fence`). Unit tests in `import/` cover the scrubber, the id rewriter (one pass, no partial ids), a renumbered done case without `closed`, lenient frontmatter, fence-aware sections and demotion, the case body, bad kit cases, session splitting, deviation entries and knowledge sections; `scrubber_masks_secrets_over_lines` (WP-140): a PEM private key and a continued `mysql … -p` over CRLF and LF lines are masked whole, line ends kept, each changed line counted under its rule |
| `engine/tests/redaction.rs` | SPEC-ENGINE §7: one `TABLE` row (input, secret gone, text kept, rule matched) per built-in rule and its forms, `CLEAR` texts that stay unchanged, `CONTINUED` rows for every place a rule reads a line end (LF and CRLF, WP-128), triggers, the order of rules, disjointness, a second pass that changes nothing, user patterns, the ledger end to end and every command that writes free text. WP-140: `private-key` (OpenSSH, RSA with headers, EC inside a shell string with `\n`, PKCS#8 with CRLF, encrypted, PGP, cut before its END and before its BEGIN), quoted header values (`"…"`, `\"…\"`, `'…'`, a quoted name, not `curl -H 'Authorization:'`), `"x-api-key"`, `nmcli-secret`; `a_match_of_markers_only_is_left_as_it_is` (a user pattern for `\r`, `;`, `&`, white space or the END line next to a built-in marker: the second pass is the same; glued markers count no rule; every `TABLE` and `CONTINUED` row, redacted, matches no rule); `an_empty_header_value_is_no_value`; `the_index_drops_every_format_character_before_the_redaction` (a `token=` split by each code point of the widened set, over a case's Intent and a decision's lead, a PEM key in a Result). Round 2: a public block in a text that says "private key" stays; a quoted header value with text glued after its quote, Python string prefixes (`f'…'`, `rb'…'`); the second pass that masks text glued after a user pattern's gap (SPEC §7). `long_lines_with_a_marker_stay_fast` (ignored; `just check-perf`) times 16/64/128 KB lines, with WP-140 rows for key mentions, nmcli properties and quoted headers. Unit tests `import::tests::the_invisible_set_holds_each_code_point` (WP-159: the fillers and variation selectors too) and `collectors::plugins::tests::a_subject_drops_every_format_character_before_the_redaction`; `index.rs` `the_reference_drops_the_engines_format_characters` compares the set of `scripts/validate-fixtures.py` with the engine's over every code point (skipped without `python3`). `work/active/WP-140/mutants.py` (own target dir `engine/target/mutants-wp140`) runs 68 mutants, one per rule of the WP. Round 3: HTTPie's and xh's `Authorization:'Bearer …'`, `Authorization:"…"`, `X-Api-Key:'…'` masked, `curl -H 'Authorization:' -H 'X: y'` and `grep -i 'authorization:' f 'x'` unchanged, a bench row of glued header values. WP-159: `redact::tests::a_secret_split_by_an_invisible_character_is_masked` (`to<X>ken=`, `Authorization: Bearer<X>`, `ghp_0123<X>4567…` for 24 code points, in `redact`, `redact_keeping_lines`, `matching_rules`, `matching_rules_by_line`; a user pattern), `invisible_characters_away_from_a_secret_stay` (a text without a secret byte for byte; runs away from a match, at the start and the end, kept; at a match's edges dropped; twice is once), `keeping_lines_over_an_invisible_character`, `without_invisible_drops_the_set`; end to end `log.rs` `a_secret_split_by_an_invisible_character_is_masked` (ledger, journal), `import_task.rs` `a_secret_split_by_an_invisible_character_never_reaches_an_imported_case` (the case file, the report, `plan show`), `plan.rs` `a_secret_split_by_an_invisible_character_is_masked_in_the_closing_commit`, and the plugin commit subject unit test with the three examples; bench rows `invisible characters` (no secret, 16/64 KB) and `split secrets` (128 KB). Round 2: `an_invisible_character_before_a_secret_is_a_boundary` (one `x<X>…` row per rule anchored at a word boundary: `sk-key`, `db-client-password` ×3, `curl-user`, `proxy-option`, `cookie-option`, `cert-password`, `sshpass-password`, `cookie-header`, `secret-header`, `registry-login-password` ×2, `nmcli-secret`, `httpie-auth`, plus `url-userinfo` and `email`; four code points; every entry point; the copy alone leaks, the result does not; twice is once), `a_run_at_the_start_before_a_match_goes_with_it`, `a_control_character_splits_no_secret` (BS, NUL, BEL, CSI, ESC, DEL, US; tab, line ends, VT, FF, NEL read as white space; CRLF); `index::build::tests::shown_texts_are_redacted_before_invisible_characters_go`; `import_task.rs` `plan_show_redacts_before_it_marks`; the boundary and control rows in `log.rs`, the hook test, the closing summary and the plugin subject. `work/active/WP-159/mutants.py` runs the hand mutants of the helper |
| `engine/tests/inbox.rs` | `seldon inbox add` (WP-166): a report on stdin by `$SELDON_ACTOR` becomes `inbox/<date>-<slug>.md` (frontmatter `type`, `created`, `actor`, `tags`, then `# title` and the text without its outer blank lines), mode 0600, in one commit `seldon: inbox add` of that file alone while the user's pending edit stays uncommitted, and the index rebuilt after it (`logbook.git.head`, `autocommit`); a file with `--actor` and two tags (the source unchanged), CRLF as LF; a GitHub token, a bearer header, a `token=` split by U+200B, a PEM key over lines, `password=` in the title, a token as a tag and two `/home/alice` paths never reach the file, `--json` or `git log -p` (`redactedLines` 8 — the title's line 1 apart from the text's —, `privatePaths` 2, `droppedCharacters` 1); the same title and text again — same day or not, another actor, a CRLF file — writes nothing (same tree and HEAD, `filed: false`, the path named), another text gets `-2` and `-3`, a numbered text and the first one behind the numbered names are found again, the same text under another title is filed, a user's file and a dangling link at a name are skipped and never written through; a title without letters is `note`, a missing `inbox/` is made (0700); refused with the tree unchanged: no logbook (3), empty, blank or only-U+200B text, a multi-line, U+2028, blank or only-U+200B title, a title over 120 characters, a symbolic link, Latin-1, over 1 MiB (a file or stdin), missing, a directory, a bad tag, `--actor system` (1), the lock held (4); a 120-character title and 1 MiB on stdin are filed, Latin-1 on stdin is refused, with a name and 97 numbered ones taken the text gets `-99` and the next one exit 1; the skill's recipe from `SKILL.md`, run through bash, files the report verbatim and runs none of its `$(…)` or backticks. Round 2: an `inbox` that is a link to a folder outside (nothing lands there) or a regular file is refused (1); a link at an inbox name to a file with the same filing is not "already filed"; a title's U+200B, ESC, BEL and U+009B are dropped and counted alone (`droppedCharacters` 4), no control character reaches the human line, a title of controls only is empty (1); `/proc/self/status` and `/proc/self/environ` are refused as size-0 views, an empty file as empty text; a pseudo-terminal on stdin (util-linux `script`; skipped without it) is refused with "pipe the text on stdin". Round 3: the text's backspace (inside `to\x08ken=`, masked then), ESC, a lone CR and BEL are dropped and counted (`droppedCharacters` 5), tab and newline kept. Round 3b: `x<U+200B>sk-…` in title and text and `/ho<U+200B>me/alice` are masked and rewritten (redaction before the invisible characters go, WP-159); the linked or odd `inbox` gets `Logbook::checked_dir`'s message (WP-168). Round 3c: every control code point (C0, DEL, C1, NEL included; tab and newline aside in the text, the line ends refused in the title, NUL not in an argv) splitting `to<c>ken=`, a `ghp_` token and `/ho<c>me/alice`, and gluing `done<c>sk-…` (text) or `x<c>sk-…` (title), never reaches the file, the file name, `--json` or `git log -p`, and the filed text redacts to itself. |
| `engine/tests/import_task.rs` | `seldon import task` (WP-102) with task files in the test home: open items become queued cases (title rule, Intent with the indented block and `Section:`, tag `imported`, Log `imported from ~/…#line`, `meta.risk` on `case-created`, one commit `seldon: import task`, a valid index) and the source keeps its bytes and mtime; a file without items is one case titled by its first `# ` heading or its name, its headings and fences escaped so the case's own `## Log` stays one (a later `plan start` writes there); two re-runs create nothing (same ledger, same HEAD, `already-imported` with the case), a ticked item is not imported again, a reworded item makes a new case `changed since` the old one, the marker holds no text; the same item twice → `duplicate`; empty file and empty items → `empty`; `--include-done` makes completed cases (`case-created` + `case-completed`, zone/risk/area from the flags) and is refused for an agent actor, which may import open items; `--dry-run` leaves the whole temp tree and HEAD as they were (JSON and human form); refusals exit 1 with the tree unchanged even after a good file: outside the home (absolute, `..`, a symlink out), inside the logbook (a logbook under the home), not `.md`, a directory, not UTF-8, over 1 MiB, missing, a control character in the name; more than 200 new cases → exit 1; a token, a bearer header and `/home/alice` never reach the logbook or the index (`redactedLines` 2); an invalid marker → exit 1, no case. WP-127 (ADR-0038 §3): each case's frontmatter has `source: "~/…#line"` (`~/…` for a whole file) and the index carries it with the intent after the provenance line; an edited or removed `source` imports nothing again (`the_marker_not_the_source_keeps_an_import_idempotent`); a path of more than 512 characters keeps `~/…` and its end (`a_long_source_keeps_its_end`). Round 2: a `mysql … \` continued `-p` and a JSON `"password":` with its value on the next line never reach the case, ledger, index, marker or `git log -p`, the line numbers stay those of the file and the same item with another secret is the same task; an agent session (`SELDON_ACTOR=agent:x`) with `--include-done --actor human` is refused with the ledger unchanged; a linked folder with a newline or U+202E in its name is refused; `SELDON_TEST_IMPORT_CRASH=after-create:2` (exit 99) leaves a pending entry that the next run settles (no case made twice), a pending entry without its case or without this import's Log line is dropped and its task imported again; a new item inserted at an occupied line replaces nothing; a secret in the path is redacted in `source` and the logbook; every Intent opens with the `Imported from …` line. Round 3: both multi-line forms with CRLF line ends are redacted with the same line numbers; folders with U+200B, U+2060 or U+FEFF in the name are refused; `plan start` of an imported case by an agent (`--actor`, `$SELDON_ACTOR`, or `--actor human` in an agent's session) and `agent start` on it are refused with the ledger unchanged and no launch, the user's start and an agent's start of a case that is not imported go through (`an_agent_cannot_start_an_imported_case`). Unit tests in `import/task.rs` cover items, blocks, fences, CRLF, tabs and the whole-file form; `redact::tests::redact_keeping_lines_keeps_the_line_count`. `work/active/WP-102/mutants.py` (own target dir `engine/target/mutants`) runs 27 mutants (CRLF normalisation, format characters, the imported-start refusal in `plan start`, its session part and in `agent start`, redaction, whole-text redaction, the session check, the resolved-path character check, the pending entry, settle, same-path dedupe, `replaces` with the old text still there, path redaction, the provenance line, idempotency, both path refusals, escaping, done skip, dry run, limit, changed-item link, duplicate, extension, size): each is killed WP-102b round 2: the reviewer's 2400-line file is skipped `too-long`; 21 tag characters, U+202E and U+200B never reach the case file, its title or `plan show` (`droppedCharacters` 23); `plan show` of a hand-made case marks 21 tag characters `‹U+E0072›`… (`hidden` 21), redacts a hand-written token, cuts before a two-byte character at byte 65 536, and gives `intent: null` while the patterns do not compile. |
| `engine/tests/collectors_user.rs` (`plugin_commits::`) | plugin updates name their commits (WP-136), on clones built with the host's git in the test home (no system or global config; skipped without git): a pull of three commits (`meta.git` `pull`, `meta.commits` newest first, detail `<from> → <to>, pulled 3 commits: … …`), a capture with nothing moved writes nothing, a rollback (`rolled back 2 commits`, the subjects that left), a reset to another history (`reset: 1 commit in, 1 out`), one commit without `…`, 25 commits (20 subjects, the count 25), the HEAD read from a packed ref and from a detached HEAD; a hostile subject (ESC, CR, BEL, tab, U+202E, U+200B inside `token=`, a 300-character subject, a token at the cut, a bare GitHub token across the cut) cleaned, redacted before the clip, at most 100 characters; no clone (the version step only, no `head`), a cursor from before WP-136 (no list for that step, then the list), a HEAD unreadable for one capture (the last one kept), a cursor head git does not have or shaped like an option (no list, git never takes it as an option), an empty `.git` inside another repository (no head, no version); `plugins::enable_disable_remove_and_update` reproduces the fixture's 09-24 line from a real clone. `capture_cost_of_clone_heads` (ignored; `--profile bench -- --ignored --nocapture`) prints the capture cost: 8 plugins without and with clones and one moved update. Unit tests in `collectors/plugins.rs`: the git argv and environment literally, a slow git cut off at the timeout, heads that are not object names never reach git, the HEAD from the files (detached, loose, packed, sha256; traversal, dot, `.lock`, tag and reftable names, a `.git` file and a linked HEAD are git's), summaries, subject cleaning. Round 2: a clone's `info/grafts` (a graft that makes the new HEAD a root, then 200 000 bad lines) is not read and the pull is named (`a_grafts_file_is_not_read`); a partial clone whose promisor remote is an `ext::` script, allowed by the clone's own `protocol.ext.allow=always`, with the old HEAD's object removed, never runs the script (`a_partial_clone_never_fetches`); a linked `.git`, a `gitdir:` file, `objects/info/alternates`, `commondir`, `include.path` and `includeIf` each give the version step with `commit history not read (the repository points outside the plugin folder)`, no `meta.git`, no `head` (`a_repository_pointing_outside_names_no_commits`); a clone's `i18n.logOutputEncoding=UTF-16` changes nothing and a Latin-1 subject comes out as git converts it (`the_log_is_utf8_whatever_the_clone_says`). Unit tests: `sys::tests::a_capped_run_keeps_the_head_of_a_flood_and_reads_the_rest` (16 MiB stderr, 1 MiB stdout, 64 KiB kept each), a flooding fake git is no answer, a fake git's child is killed with it at the deadline (the process group), the 2 s timeout, bytes that are not UTF-8 read lossily, `GitDir` per case (alternates, commondir, `[include]`, `[includeIf …]`, `[Include]`, `config.worktree`, a non-UTF-8 or linked config, a linked `.git`, a `gitdir:` file; git never asked), U+2066–U+2069 dropped and U+2028/U+2029 as spaces. Round 3: a config whose `[include]` follows a BOM, a lone CR, another header on the same line, CRLF lines or a backslash-continued value, and a link at `.git/objects`, `.git/refs`, `.git/packed-refs` or `.git/HEAD`, are each `Outside` (`a_git_dir_that_points_outside_is_not_read`); the BOM and a linked `.git/objects` are two more cases of `a_repository_pointing_outside_names_no_commits`. `work/active/WP-136/mutants.py` (own target dir `engine/target/mutants`, `--no-fail-fast`; names as arguments run a subset) runs the round 1–3 mutants; see the handover for the result. WP-154 `head_files_are_read_byte_for_byte_as_git_writes_them`: HEAD, a loose ref and `packed-refs` byte for byte (CRLF, a CR, no final LF, two LFs, a BOM, a leading space, upper case, 39 and 64 digits, a `#` line further down, unsorted or repeated refs, a `^` line first or twice, two spaces), a linked or non-UTF-8 loose ref (never the packed one behind it), a config or `config.worktree` naming an object format; each noted with git 2.55's answer; the include scan's 1 MiB limit to the byte |
| `engine/tests/doctor.rs` | `doctor::` green after init with snapper degraded, exit 3 when not initialised, invalid frontmatter, misplaced case, the fixture logbook (and that doctor leaves it untouched); snapper's permission error in a German locale (issue #1, WP-053): with `LANG=de_DE.UTF-8` and a stub that answers `Keine Berechtigungen.` unless `LC_ALL=C`, `init --non-interactive` prints the read-grant hint (`setfacl`, ADR-0026), `cursors.json` holds `NO_PERMISSIONS` for snapper, and `doctor` prints the `fix:` line. The unit test `collectors::snapper::tests::list_command_runs_in_the_c_locale` pins the argv and `LC_ALL=C` / no `LANGUAGE`. The `git` check (WP-061) is degraded, each with its fix line, for a stale `.git/index.lock`, a read-only `.git`, a detached HEAD (ok with autocommit off), a committer git cannot resolve (empty `user.name`), and an empty `.git` inside another repository; doctor leaves `.git` byte-identical even when a tracked file's stat data is stale. WP-070: an unparsable `config.toml` → exit 1, config error first, logbook "not checked", `"logbook": null` (also with `--path`, which is still checked); an invalid `[redaction] patterns` entry, and each corrupt state file (`cursors.json`, `manifest.json`, `owned.json`) → error with its fix; bad ledger lines → `ledger` degraded with month, count and lines; a case id in two files → `cases` error; a STATUS.md or DECISIONS.md fence without its end marker, and a stray end marker after a removed one → `fences` degraded; the snapper probe runs `SELDON_SNAPPER`; with every new check failing (a duplicate case and an invalid pattern included), the home (state, config) and the logbook are byte-identical after doctor, no index and no lock appear. Review round: a collector whose last capture failed (this logbook's cursors only, disabled ones left out) → `collectors` degraded with its message and fix; an unreadable config.toml → config error with a `chmod` fix, exit 1, logbook not checked; the parse error's fix line; the `init` fix prefixed "after fixing config.toml:"; an unreadable state file's text matches its `chmod` fix; the omarchy probe runs `SELDON_OMARCHY_VERSION`. WP-079 (ADR-0026): a user still listed in `ALLOW_USERS` (the stub answers `get-config` only with `LC_ALL=C`) gets an `ok` row whose fix is the revert, then the read grant (also by `LOGNAME`, also under a German locale); a user not listed, a partial name, `USER` not listed with a listed `LOGNAME` (`USER` wins), no user, a refused `get-config` and a missing snapper get none; the stub logs that only `list` and `get-config` ran, and that a refused `list` asks no `get-config`; WP-081: a corrupt manifest's error row says the capture records a state reset, the capture after it leaves a degraded `state` row with the restore fix (exit 0), and the next capture clears it, and another logbook's later cursors with an earlier `lastRun` do not show it (`a_state_reset_is_shown_until_the_next_capture`); WP-083: before that capture a degraded `state` row predicts the reset, with the restore-now fix for missing and unreadable cursors and the nothing-to-restore fix for another logbook's; none for a fresh logbook, a collector that never ran here (no entry while bound here), a disabled collector, or restored cursors; after the capture the WP-081 row instead (`a_state_reset_is_predicted_before_the_capture`) |
| `engine/tests/idempotency.rs` | `state_reset::` (WP-081): a removed state directory after a capture with events writes one `seldon` note `state-reset` (sources, files, detail, the warning on stdout and in `--json`), the next capture writes nothing; the first capture of a logbook and collectors whose source has no event in the ledger write no note; an unreadable pacman cursor, a corrupt `manifest.json` and a corrupt `owned.json` (moved to `owned.json.bad`) each give one note; plugins and theme with unreadable cursors and events give one; review round: the first successful theme run after a degraded `init` and a hook-written `theme-set` gives none, nor does a collector disabled at the first capture and enabled later; cursors of another logbook give files `logbook`, the "nothing can be restored" warning and doctor fix; a corrupt `owned.json` waits for a run of the config collector (`--source pacman` leaves it). `collectors.rs`: the snapper info-file path flags a missing cursor. `hooks.rs`: `session_stop_prints_a_state_reset_on_stderr`. WP-083: before every capture of these scenarios, `doctor`'s "the next capture will record a state reset" row names exactly the sources of the note that capture writes, and no row before a capture that writes none (`predicted`); a state directory restored before the capture removes the row and records nothing; every collector's `cursor_reads` accepts its own saved cursor and rejects a stray value. WP-088: a collector degraded in the capture that records a reset (or the one that alone lost its state) is marked `pendingBaseline` (`cursors`), keeps it while degraded or not run, and its first successful run writes its own note and clears it; with the state bound to another logbook the mark is `logbook` and the later note says `logbook` with the "nothing can be restored" warning; a degraded collector without a cursor here or without events of its source is not marked; a `cursors.json` entry without the field reads unchanged, `cursors` and `logbook` round-trip. WP-091: a collector not run (`--source pacman`) in the capture after a removed state directory gets an entry with only the mark (no `lastRun`), its index row equals a row without an entry, it keeps the mark while not run and its first run writes its own note; a collector disabled while the state was another logbook's is marked `logbook` and its note after enabling says so; a collector not run without events of its source gets no entry, and one with an entry here (an unreadable cursor) keeps it unchanged; doctor shows a marked collector in its own row (`waiting`: the WP-091 wording for `cursors` and `logbook`, the `--source` fix), not in the "next capture" row, and both rows when a marked collector and an unreadable cursor meet (`doctor::tests` covers mixed reasons and the plural); `snapper_access::` (WP-091): a degraded first run writes no note, the read grant (info files appear) writes one `ok again` note with both messages, actor `system`, no case, no own change, no drift, its removal one `degraded` note, a repeat writes nothing; a list failure and recovery are recorded the same way, a capture without snapper compares nothing; no note after a lost state directory or for an entry with only the mark; the note's detail is redacted (`the_note_is_redacted_like_every_event`, WP-099). `messages::` (WP-105): snapper's stderr with a made-up token and a host the user's `[redaction] patterns` entry names is masked in `capture --json`, the `capture` text, `cursors.json`, `index.json` (`state.collectors`) and `STATUS.md`, the ledger's note is unchanged, and a second capture keeps all of them; a raw message as an older engine saved it in `cursors.json` is masked by `seldon index` before any capture and in the file by a capture that does not run snapper; the file a crash after the append leaves (WP-099 mark) holds the masked message (debug builds). Round 2: doctor's `collectors` row (a message an older engine saved), its `snapper` probe (also in `init`'s text and `--json`) and its `omarchy` probe show the masked text, as text and in `--json`; with a user pattern that matches across `›` the saved message is byte-identical over three captures and the index row stays the same; with an invalid pattern `seldon index` exits 0 and the index row, doctor's `collectors` row and its `snapper` probe show `MESSAGE_WITHHELD`, and so does the probe while `config.toml` does not parse (doctor exit 1). `crash::` (WP-099, `SELDON_TEST_CAPTURE_CRASH=before-append|after-append`, debug builds, exit 99): a crash after the append leaves the `state-reset` note (lost state directory) or the snapper note in the ledger and `cursors.json` as loaded with the note's time in `pendingNotes`; the next capture writes neither again, gives the reset warning and saves without `pendingNotes`; a later loss or the change back is recorded; a crash before the append leaves the note to the next capture; a source the crashed note did not name (`--source pacman`, then `--all`) gets a note of its own; a second crash keeps the first mark besides its own; a mark at 00:00:05 on 1 March local time (February in UTC) is found in the March file. WP-104: a source the crashed capture recorded first gets no reset note (P2), nor does a crashed first capture (P1), and a later genuine loss is recorded; the marked file holds `silentBaselines` (exact shape), also from a crash before the append and for a collector not run (a theme-hook event before the next capture is no loss); a mark under another logbook's path does not hide a loss here; a file without the field reads unchanged; doctor after a crashed reset gives the "will warn" row, not "will record", and both rows when the next capture records a source the crashed note did not name. Round 2: theme not run twice after a silent crash (a theme-hook event in between) gets no `pendingBaseline` and no waiting row; a completed save here drops another logbook's marks; after a crashed reset, a `--source` capture that does not run a collector the note names neither marks it waiting nor keeps a mark it had before the crash (entry dropped), so the next `--all` records no second reset; nor does a collector that degrades in the capture after a crashed reset (not marked waiting). The crash tests exist in debug test builds only (`#[cfg(debug_assertions)]`, like the crash point) |
| `engine/tests/pacman_stream.rs` | the first capture of a large package log (WP-198): `init --defaults` on a synthetic 96 MiB `pacman.log` generated in the run's temp dir (`TMPDIR`; removed with it, never committed), nearly all of it older than the 90-day look-back, ends within 30 s and a peak resident size of 64 MiB (debug build; `getrusage(RUSAGE_CHILDREN)`, so the binary holds this one test; killed and failed at 180 s), records exactly the look-back's events (a transaction open across the look-back's start keeps its `txId` and `meta.command`), and two captures after it write nothing. `SELDON_PACMAN_STREAM_MIB=<n>` sets another size and only reports the numbers. Unit tests in `collectors/pacman.rs`: `lines_through_the_buffer` (lines across the read buffer's edges, an over-long line passed empty, the unterminated last line not passed), `lines_open_regular_files_only`, `the_look_back_start_keeps_what_the_whole_log_keeps` (every baseline around every line of logs of every shape, under every lock state: the events and the resume offset of a parse of the whole log), `the_look_back_start_rewinds_to_the_open_block` |
| `engine/tests/git.rs` | the autocommit (WP-061): with `GIT_DIR`, `GIT_WORK_TREE`, `GIT_INDEX_FILE`, `GIT_OBJECT_DIRECTORY` and `GIT_COMMON_DIR` of a second scratch repository set, `seldon log` commits into the logbook and the other repository's HEAD and index stay unchanged; `index` reads the logbook's HEAD under the same variables; an empty `.git` inside a parent repository → nothing committed anywhere, one warning ("not a usable repository"), the index shows no HEAD; a `.git` file whose `gitdir:` names the parent's git directory → nothing committed, one warning, doctor degraded; a linked work tree of the logbook's own repository (`git worktree add`) is committed to; a detached HEAD → no commit, nothing staged, one stderr line and `--json` `git.error`; a stale `.git/index.lock` → exit 0, one warning line, `git.error`, no stdout duplicate, and the next write after the lock is gone commits everything. git runs in the engine's process group (a wrapper in place of the `git` link records `/proc/$$/stat` and execs the host's git; snapper, recorded the same way, leads its own group). The unit test `logbook::git::tests::the_command_drops_every_repository_variable` pins the environment. WP-154: a wrapper `git` (`Env::wrap_git`, which removes the link first so nothing is written through it into the host's git) that writes 100 MB of `x` lines to stderr for `rev-parse --show-toplevel` leaves 1 MiB in doctor's message (`a_hundred_megabytes_from_git_are_capped`); one that refuses `--no-lazy-fetch` like git before 2.44 still gives `status` its HEAD and doctor `ok`, the option tried once per process (`a_git_without_no_lazy_fetch_still_answers`); a logbook made a partial clone with an `ext::` promisor that would leave a marker, allowed by its own `protocol.ext.allow=always`, and HEAD's tree removed: `status`, `doctor` and `import --apply` (its `is_dirty`) never run it (`a_partial_clone_logbook_never_fetches_during_status`); a commit signs with the user's `gpg.program` (a stub that speaks git's status-fd contract; `gpgsig` in the commit) and runs the user's `core.hooksPath` hooks, which see neither `GIT_ALLOW_PROTOCOL` nor `GIT_NO_LAZY_FETCH` (`commits_still_sign_and_run_hooks_as_configured`). Unit tests in `logbook/git.rs`: the query and the commit argv and environment literally, the old-git refusal (exit 129 and the option named, also translated), a status over the cap has changes, the `worktrees/<name>/gitdir` back link byte for byte (CRLF and no final LF read; a BOM or a space not). `tests/status.rs` `the_fast_rebuild_reads_git_files_as_git_does`: a `.git` file and `commondir` byte for byte as git 2.55 reads them (`gitdir: ` exactly; CRLF read; no space, two spaces, a trailing space or tab, a BOM, upper case, an unreadable `commondir`: no HEAD). `sys::tests`: a stdout over the cap is `Run::Cut` whatever the exit code, exactly the cap is whole, 100 MB of stderr in the engine's group keep `OUTPUT_MAX`. Round 2: a wrapper `git` logs every call of `log`, `status`, a `capture` that upgrades the v3 agent rules (`is_clean_path`, then the rules commit) and `doctor` (also on a detached HEAD, for `branches`); every call but `add` and `commit` starts with `--no-lazy-fetch` and sees `GIT_ALLOW_PROTOCOL=none` and `GIT_NO_LAZY_FETCH=1`, `add` and `commit` see neither (`every_git_call_but_add_and_commit_is_a_query_without_network`); `sys::run` with `OUTPUT_MAX` (a stdout flood is `Run::Cut`, a stderr flood keeps `OUTPUT_MAX`); the old-git refusal against git 2.55's real usage text; an empty or newline-only `gitdir: ` or `commondir` has no HEAD. `work/active/WP-154/mutants.py` (target `engine/target/mutants`) runs the WP-154 mutants; see the handover. WP-199: `init_leaves_no_git_running_in_the_logbook` runs `init` three times under `GIT_TRACE2` (every git `seldon` starts and their children) with `core.fsmonitor = true` in the test home's `.gitconfig`: no `maintenance`, `gc` or `fsmonitor` child is started (git 2.55 detached a maintenance after each commit, and the fsmonitor daemon outlives the command; the test fails without either option), the trace saw the commits, and no process has its working directory in the logbook when `init` returns (`/proc/<pid>/cwd`); `git fsmonitor--daemon stop` runs in each logbook before the asserts, so a regression leaves no daemon. The unit test `a_query_never_reaches_the_network_and_a_commit_is_the_users` pins `-c gc.auto=0 -c maintenance.auto=false -c core.fsmonitor=false` first in a commit's argv and `-c core.fsmonitor=false` after `--no-lazy-fetch` in a query's; `every_git_call_but_add_and_commit_is_a_query_without_network` checks the same prefixes on every call the engine makes |
| `engine/tests/watch.rs` | `seldon watch` (WP-034). Without the feature: exit 1, "built without the watch feature", JSON error. With `--features watch` (`just check-watch`): one rebuild at start (`trigger: "start"`; an edit made before the start is in it), then one change → exactly one rebuild after the 2 s quiet interval and nothing after it (the rebuild's own reads and its `index.json` write stay silent); a burst of 24 writes plus a new folder → one rebuild, and a later write in that folder is seen; generated `ledger/*.md`, `STATUS.md`, temp/backup files, `PROJECT.md`, reads of every watched file, and `seldon index`/`status` runs → none, while `.seldon/logbook.toml` counts; a held lock → no rebuild and still running, the rebuild within 2 s of the release; a folder renamed away and recreated → its watch moves to the new folder (a write in the old one is quiet, one in the new one counts); a new area (`areas/printer/README.md`) → one rebuild with the area in `system.areas`, and `areas/` renamed away and recreated is watched again (WP-075); SIGTERM and SIGINT → exit 0 with a final `stopped` line; not initialised → exit 3; `--interval 1` → exit 1; RSS on the ×10 fixture (below) |
| `engine/tests/index.rs` | `seldon index` (WP-007) against `fixtures/index.sample.json` and the variants, the mutation self-checks of `scripts/validate-fixtures.py` in-process, atomic writes under a concurrent reader. The text clip of ADR-0025: `clip_keeps_short_texts_and_marks_long_ones` (WP-076: a text of at most 256 JSON bytes unchanged, a longer one cut on a character boundary to at most 256 bytes with `… (N more characters in the ledger)`, N exact) and `the_reference_derive_clips_texts_as_the_engine_does` (WP-077: long `detail`, `resolutionDetail` and `meta` texts in a copy of the fixture logbook, cut on multi-byte, JSON-escaped, white-space and U+001C..U+001F characters; `index.events` and `index.drift` of `seldon index` equal those of `validate-fixtures.py --derive`; skipped with a note without `python3`); `long_texts_keep_the_index_under_its_size_budget`, `a_long_command_line_is_whole_in_the_ledger_and_clipped_in_the_index` and `an_index_over_its_budget_warns` (WP-076); contract 2 (WP-120): the golden run puts `fixtures/proposals/` into its state directory (`triage`), `meta.truncated`/drift `truncated` mark exactly the clipped texts (in the reference test), `a_ledger_truncated_mark_is_dropped`. ADR-0038 (WP-127): the reference test also puts long texts into Intent, Result and Decision sections and compares `cases` and `decisions` with the script's; clipping with `in the file` and control characters as spaces, the imported case's intent after its provenance line (and the line itself without the tag), withheld texts without a redactor, a `source` out of shape dropped with a warning (512 characters still pass), `rule` per item and `attention-all`, the four fields optional and closed in the schema; `redaction.rs` `the_index_masks_intent_result_lead_and_source` (built-in rules and a `[redaction] patterns` entry, hand-edited files); `drift.rs` holds `drift list`'s `rule` to the index's; a `[redaction] patterns` entry that does not compile withholds the four texts. Round 2: `a_secret_at_the_cut_or_over_lines_is_masked_before_the_clip` (a `ghp_` token across the 256-byte cut of intent, result and lead masked whole; a `mysql … \` continued `-p`, a JSON `"password":` with its value on the next line and a token split by U+200B masked); direction and format characters dropped and a text of control characters only absent (`case_and_decision_texts_are_clipped_with_the_file_marker`); `source` capped at 512 bytes (a 302-character, 602-byte path dropped; `case_source` keeps whole characters); `plan_show_carries_only_a_source_in_shape`; `paragraphs_leave_comments_out_as_strip_comments_does` (the streaming read equals comments-stripped-then-lines, for every `max`); `the_first_paragraphs_of_a_large_intent_cost_what_they_hold` (ignored, `just check-perf`: two paragraphs of a 1 MiB Intent < 50 µs). `work/active/WP-127/mutants.py` (own target dir `engine/target/mutants-wp127`) runs 26 engine and 6 plugin mutants (round 2 adds clip before redaction, `plan show`'s source filter, a blank text, direction characters kept, the cap in characters, a split character, reading past the paragraphs needed — the last one against the timing test; rule, redaction of texts and source, the source's shape, clipping and its marker, control characters, withholding, the provenance skip with and without the tag, the sections, headings, comments, the import's `source`, its shortening and characters, the frontmatter key only when present and read leniently; the plugin's index rule and its shape, intent, the source row, the lead, a non-string text): each is killed |
| `engine/tests/contract_v2.rs` | contract 2 (ADR-0035, WP-120): `meta.risk` on every new `case-created`/`case-started` (plan new and start, `drift explain`'s completed case), `case-updated` from `plan set` (also for zone only), none on drop; `seldon event` refuses `case-updated`, `state-loss`, `--meta risk`, `--meta truncated`; `logbook.git.autocommit` after a commit, after a failed one (stale lock), unchanged after `--no-commit`, absent for another logbook's record and with `[git] autocommit = false`; `triage` picks the newest valid proposal of this logbook (an invalid one warned and skipped, another logbook's skipped silently, a name that is not its id warned), `applied`; `decisions[].cases` without repeats; `index` twice at one clock writes the same bytes with every v2 field. `planned_link.rs`: `the_ledger_record_wins_over_an_edited_log`, `an_old_case_falls_back_to_its_log`; `idempotency.rs`: `a_state_reset_note_from_before_contract_2_still_counts` |
| `engine/tests/boot_hashes.rs` | WP-164 (AGENTS.md §6, S8): the config collector hashes `mkinitcpio.conf`, `mkinitcpio.conf.d/*`, `mkinitcpio.d/*`, `default/limine`, `limine-entry-tool.conf` and `limine-entry-tool.d/*` under `<guard>/etc` (the guarded `SELDON_ETC_DIR`), whatever `watchPaths` says. A drop-in added, changed (the same bytes extracted again are no change) and removed, `mkinitcpio.conf` and `default/limine` edited: one event each at the next capture, attention (`config`, `config-remove`); further captures write nothing. No line of the files is in the logbook, the state directory or `status --json`; `.pacnew`/`.pacsave` in a boot directory, `mkinitcpio.conf.pacnew`, a directory below a drop-in directory, `default/grub`, `pacman.conf`, `crypttab`, `shadow` and `<guard>/boot/limine.conf` are never named. A preset and a `limine-entry-tool.d` drop-in are watched; a `.pacnew` and its removal write nothing, the merge one `config-change`. Unreadable files (mode 0, skipped as root): add, an in-place write with the mtime put back, and removal are seen with `meta.hashBasis = "stat"`, no collector note. `[redaction] skipPaths` opts out (nothing named); removing the patterns lets the files enter the scope without events (`… 7 entered it`). No `/etc` at all: no note; a file that appears is one addition. `[drift] alwaysRedPaths` with an absolute pattern makes a drop-in change a crisis. A drop-in symlinked to a file under `/usr/` (round 3) is a `config-add` without `meta.matches`, attention `config` (no `system-link`; skipped without a readable file under `/usr/`). `attribution.rs` `a_boot_file_is_proven_by_its_absolute_path` (rule 1: `sudo tee`, `sudo sed -i`, `cp` into the directory, `rm -rf` of it prove; `cat`, another file, `mkinitcpio -P` do not). Unit tests: `collectors::config::tests::a_boot_file_is_hashed_by_content_or_metadata`, `boot_roots_are_the_listed_files`, `boot_files_and_pacman_leftovers` |
| `engine/tests/linked_folders.rs` | WP-168: one test per logbook folder the engine writes into (`decisions`, `work` and its three status folders, `journal` and a year folder, `ledger` and its views, `areas` and an area, `system`, `outputs`, `archive`, `.seldon`, `.seldon/imports`, `memory` and the other import folders; `drift explain|link`, `event --case`), each with a symbolic link to a copy outside the logbook and with a file in its place: exit 1, the reason names the folder, the logbook (through the link included) and its ledger unchanged. A `.seldon` file is no logbook (exit 3). `inbox` and the kit's `.claude` in the unit tests of `logbook::checked_dir` and `setup::copy_tree` |
| `engine/tests/linked_files.rs` | WP-171 (ADR-0049): one test per logbook writer of a file (a journal day, new and existing; a ledger month; a case file in place, moved from and moved to; `.seldon/active-case`; an area README; `decide accept`; `outputs/REBUILD.md`; a dossier file; `AGENTS.md` through `rules update`; the omarchy-agent import's report, marker, undo file, deviation file and a day; the task import's marker; `drift explain --area`), each with the file replaced by a link to a copy outside the logbook, a dangling link, a directory and a FIFO where they apply: exit 1, the reason names the file, the logbook (links not followed, FIFOs not opened) and what is outside it unchanged, a dangling link's target not made; a command still running after a minute fails the test (killed by its handle). Every refused file is also named by doctor's `layout` row as refused (`layout::misplaced`, the tie between the writers and `layout::written`). The views `STATUS.md`, `DECISIONS.md` and `ledger/*.md` through `status` and `index`: skipped, exit 0, the warning in human and `--json` output, `index.json` written, the view and what is outside unchanged, named by the row as not refused. `only_files_outside_the_logbook_are_written_through_a_link` counts every call of `write_atomic`/`write_generated` (and `_mode`/`_replace`) per source file against a list of files outside the logbook. `primitives`: the journal, the active case and an area README written by the library on their own. The capture's rules upgrade through a link (`tests/rules.rs`), doctor's `layout` row (`tests/doctor.rs`); the write primitives, `logbook::checked_file`, `layout::misplaced` and the kit copy in their unit tests |

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
that one), and the boot configuration under `$S/etc` (WP-164: point
`SELDON_ETC_DIR` at `/etc` to hash the real one; hashes only, AGENTS.md
§6).

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
#   History     N event(s) since …; M open drift item(s), M crisis  (dev host
#   2026-10-01: 1230 events, 22 items, all crises — WP-013 FINDINGS §2.2)
#               Omarchy-Agent kit: no kit at $S/data/seldon/harness/omarchy-agent; nothing copied …
rm -rf $S/logbook $S/state $S/config
$B --json init --non-interactive --path $S/logbook --since "$(date -d '-7 days' +%F)" --baseline
#   capture.baseline {"items": 22, "events": 1230, "reason": "before Seldon"}, openDrift 0
$B --json drift          # "openDrift": 0, "crisis": 0
$B --json capture --all  # "written": 0
git -C $S/logbook log --format=%s   # first capture and pre-Seldon baseline / init logbook
jq .logbook.git $S/state/seldon/index.json   # head = git rev-parse --short HEAD, dirty false
```

The interactive wizard needs a terminal; `script` provides one. Keys:
Enter takes the default, Space toggles a multi-select item, `y`/`n` answer a
confirmation. Export the scratch environment and `SELDON_TEST_GUARD`
*before* `script` (as above; `script` passes the environment on), pass
`--ask` for the full wizard and `--path` so the path step is skipped (its
default is `~/Seldon`), and stub
`omarchy` with `SELDON_OMARCHY` in case the theme hook is answered with yes:

```
export SELDON_OMARCHY=$S/omarchy-stub    # a script that only records "$*"
(sleep 1; for k in '\r' '\r' '\r' '\r' '\r' ' ' '\r' '\r' '\r'; do printf "$k"; sleep 0.4; done
 printf "$(date -d '-3 days' +%F)\r"; sleep 4; printf '\r'; sleep 3) \
  | script -qec "$B init --ask --path $S/logbook" /dev/null
# language, Obsidian, collectors, watched paths, more paths, agent setup
# (Space: claude-code; the kit item only with the kit), theme hook (no), git
# (yes), backfill date (Enter alone takes the date 90 days back), then
# after the capture: "The backfill opened N drift item(s) …" and "Dismiss
# them as "before Seldon"?" (Enter: yes)
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

**Memory bound (PLAN.md: RSS < 10 MB; 11 MB since 2026-10-07; 12 MB, measured, since WP-195).** The test runs the watcher on the
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
the test under `--profile bench` and prints the measurement; the peak must
stay under 12 MB (12 288 kB). The heap-growth bound above stays the real
limit; the peak also counts the binary's own file-backed pages, which grow
with the code (WP-175 traced +785 kB of `.text` and +368 kB of heap over
about ten WPs on `next`, no leak).
It is not part of `just check`; run it before the handover of any WP that
touches `engine/src/index/` or `engine/src/commands/watch.rs`. CI runs it
five times after `just bench` and writes the five lines to the run summary;
a run over the limit is a warning there, not a failure.

*How the limit is set (WP-195).* The limit is the highest peak measured
on the dev host, the test host and CI, plus a margin of at least twice
the spread between runs, rounded up to a whole MB. Measure with
`just check-rss` (one line `watch on ×10 (optimised): idle …, after
rebuild …, peak … kB; heap (RssAnon) … → … kB` per run), at least five
runs per place, on a quiet machine; record the numbers here. History: 10 MB
(PLAN.md), 11 MB (operator decision 2026-10-07: the 10 MB peak was
exceeded by 0.3–0.6 MB of the binary's own pages on the dev host), 12 MB
(WP-195, replacing operator decision E8 pending the operator's yes:
11 MB sat below `next`'s own peak on the dev host, 11.26–11.46 MB
in WP-175 and WP-165).

| Where | When, build | Peak, runs | Heap after the rebuild |
|---|---|---|---|
| dev host (Omarchy, 16 threads, kernel 7.2.5, rustc 1.98.1) | 2026-10-10, `next` fdaca081, bench profile | 11 348 to 11 632 kB, 8 runs, median 11 460 kB | 3 184 to 3 340 kB |
| test host | — | not run yet: measured after WP-195's merge (WP-195's brief allowed no network) | — |
| CI (`ubuntu-latest`, Arch container) | — | not run yet: the first CI run after the merge records it (the run summary) | — |

12 MB = 11 632 kB + 656 kB: more than twice the 284 kB spread of the dev
host's eight runs. The test host and CI rows are open: when they are
measured, the limit is set again by the rule above (lower, if all three
places stay well under it). To measure another binary, e.g. the musl release build with the
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

## What CI runs, and what only the dev host runs

CI (`.github/workflows/ci.yml`) runs `just check` in an `archlinux:base-devel`
container with `SELDON_SKIP_HOST_CHECKS=1`, on every pull request and every
push to `main` or `next`, then `just bench` and `just check-rss` (measured,
not gated; a run without any measurement line fails the step with an
error). The container comes from GHCR, a copy of Docker Hub's image with
the same digest that the `mirror` job makes once per digest, on a push to
`main` or `next` only; pull requests only pull by digest (WP-195;
packaging/README.md, "Pinned actions and image"): Docker Hub's anonymous
pull limit stopped CI before any step ran.

What CI runs for the plugin (WP-190):

- **Omarchy's plugin validator** on `plugin/`. The validator is 118 lines
  of bash and jq (`$OMARCHY_PATH/bin/omarchy-plugin-validate`) and needs no
  Omarchy install: without the omarchy CLI, `just plugin-validate` fetches
  it at the commit `packaging/omarchy-pin` names, over HTTPS, and refuses
  it unless its sha256 is the pinned one (`packaging/omarchy-validate.sh`).
  A broken manifest (a missing entry point, a kind without one, a symlink,
  an `omarchy.*` id) turns CI red.
- **`node tests/plugin/model.test.js`**: a broken `Model.js` turns CI red.
- **`node tests/plugin/model.bench.js`** with its budgets tripled
  (`SELDON_BENCH_BUDGET_SCALE=3`; a shared runner is slower and noisier
  than the dev host, and the dev host keeps the plain budgets). The graph's
  path counter is not a timing and is not scaled.
- **`bash tests/plugin/terminal-scripts.sh`** and
  **`bash tests/plugin/real-home-guard.test.sh`** (stubs, scratch homes).
- The release workflow's build job runs the same pinned validator on the
  exact plugin split (`git subtree split --prefix=plugin`, extracted with
  `git archive`) before anything is published; its `split` job recomputes
  the split on the `plugin` job's runner and stops the release on a
  mismatch, also in a dry run, and the `plugin` job pushes only that split
  (packaging/README.md, "The Omarchy pin").

What `SELDON_SKIP_HOST_CHECKS=1` still skips, with a notice and exit 0:

- **qmllint against the shell** and its token check need the installed
  shell tree (`$OMARCHY_PATH/shell`, default `/usr/share/omarchy/shell`)
  and Quickshell's QML modules (`/usr/lib/qt6/qml/Quickshell`).
- **The Quickshell harnesses** of `plugin-test` (`service-states.sh`,
  `desk-view.sh`, `bar-view.sh`, `ipc-restart.sh`) need `quickshell` and
  the installed shell (WP-191 brings them to CI against the same pin).

They run on the **dev host**: `just check` there runs them, and a missing
tool is an error, not a skip. The dev host and the test host stay the gate
for the plugin's behaviour; the pin mirrors the validator installed there
and never replaces the installed tree as the reference (AGENTS.md §1).
Never set `SELDON_SKIP_HOST_CHECKS` on the dev host.

### Dependabot pull requests

Dependabot (`.github/dependabot.yml`, WP-195) opens pull requests against
`next` (until 0.2.0, then `main`; security updates against `main`) once a
week: one with every action bump (the commit SHA and its `# vX.Y.Z`
comment), one with the engine's minor and patch crate updates, and one per
major crate update. CI runs on them like on any pull request; nothing
merges by itself. Review one like this:

- **Crates:** only crates AGENTS.md §7 allows, and what they pull in. A
  new crate in `engine/Cargo.lock` needs the one-line justification and the
  reviewer's approval like any other; `scripts/check-no-network.sh` (in
  `just check-packaging`) turns CI red on a network, TLS, async-runtime or
  DNS crate in the shipped graph, and `cargo tree -i <crate>` shows who
  pulls it in. Read the changelog of a major update.
- **Actions:** each action keeps one pin across the workflows
  (`tests/release/workflow-pins.test.sh`); read the action's release notes
  between the two versions, as for a pin refreshed by hand
  (packaging/README.md, "Pinned actions and image").
- **Not covered:** the build image's digest (Dependabot updates
  GitHub-repository actions only, not `container:` images) and the
  toolchain inside it; both stay manual (packaging/README.md).
- A Dependabot pull request, like every pull request, only pulls the
  image by digest; the copy to GHCR happens on a push to `main` or `next`.

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
  types. It also holds SPEC-PLUGIN §7's two house rules (WP-177), which need
  no shell and run with `--rules` in `plugin-test` everywhere: no
  `Color.muted` as the colour of a Text (nor through a colour property set
  to it), and `Util.alpha(…, <number>)` only where its `ALPHA_ALLOWED` lists
  the file and the number (the charts' and the graph's data colours);
  `tests/plugin/check-tokens.test.sh` proves each rule catches its
  violation.

## The session's runtime dir

`/run/user/<uid>` is a small tmpfs (3.2 GB on the dev host) that Hyprland,
Xwayland and every Wayland client write into. When it is full, the
compositor dies (2026-10-08: SIGBUS in Hyprland, the session lost). The
plugin harnesses filled it: they passed `${XDG_RUNTIME_DIR:-$work}` to
their Quickshells, which in a desktop session is the real dir, and
Quickshell leaves a `quickshell/by-id/<id>/` dir (lock and logs) behind
for every instance, even after a clean exit. About 34 700 runs in three
days filled it. Hence (WP-161):

- **Every Quickshell a test starts gets its own runtime dir:**
  `rt=$(mktemp -d /tmp/seldon-rt.XXXXXX)`, mode 700, removed by the
  script's EXIT trap, passed as `XDG_RUNTIME_DIR="$rt"`. It sits under
  `/tmp` and not under `$work` because `$work` follows `TMPDIR`, and the
  IPC socket `<rt>/quickshell/by-id/<id>/ipc.sock` must stay under 108
  bytes. Never `${XDG_RUNTIME_DIR:-…}`: in a session it is always set,
  and it is expanded before `env -i` clears anything.
- **Only remote preludes reach a live runtime dir:** `scripts/deploy-test-host.sh`
  and the remote side of `tests/integration/e2e.sh` drive the test host's
  live shell, and `tests/plugin/graph-live.sh` links the session's
  Wayland socket and Hyprland dir into its private dir; their lines carry
  `# live runtime dir: <reason>`, which `check-runtime-dir` requires.
- **The leak guard** (`tests/plugin/real-home-guard.sh`) lists the entries
  of `quickshell/by-id` in `/run/user/<uid>` and, when it is another dir,
  in the inherited `XDG_RUNTIME_DIR` before a harness runs, and each
  harness ends with one line per dir: the counts before and after, and a
  failure naming every new entry that no running process holds open (a
  harness's Quickshells have exited by then, so their leftovers are held
  by nobody). A new entry a running process holds is a live instance —
  another Quickshell app, a restarted shell — and only noted. The
  leftover of a concurrent run of an older harness fails it too; the line
  says which dir grew.
- **`check-runtime-space`** is the first dependency of `check` and a
  dependency of `plugin-test`; `just` runs a recipe once per invocation,
  so `just check` runs it once, at the start (not again right before the
  harnesses), and `just plugin-test` on its own runs it first. It prints `df -h /run/user/<uid>` and the `by-id` count, warns above
  50 % and refuses above 80 %. Above 50 %, look before cleaning up:
  `du -sh /run/user/<uid>/*` and `ls /run/user/<uid>/quickshell/by-id | wc -l`.
  The running Omarchy shell has its own `by-id` entry; never remove
  entries of a live instance (`quickshell list -a` lists them).
- **Less load (operator decision E29):** a WP's `just check` runs the four
  Quickshell harnesses only when their inputs changed against the merge
  base with `main` (`plugin/`, `tests/plugin/`, `schema/`, `fixtures/`,
  the `justfile`). They always run when `HEAD` is the merge base: on
  `main`, at a detached `main`, on a branch without its own commit, so
  the main check after a merge never skips them. Gates set
  `SELDON_FULL_CHECK=1` to run them whatever changed. No git or no merge
  base: they run. A skipped run prints `plugin-test: Quickshell harnesses
  skipped (…)`, and `deploy-test-host` refuses a check log with that line.
  On `next` and branches from it the base is still the merge base with
  `main`, and next's own changes under `plugin/` differ from it, so there
  the harnesses in effect always run.
- **Engine and cargo tests** inherit the session's `XDG_RUNTIME_DIR` (only
  Quickshell starts get a private one), which is harmless because the
  engine never reads it: no `*.rs` file under `engine/` names the
  variable, and `check-runtime-dir` fails if one does.

`just` itself writes its shebang recipe scripts under `XDG_RUNTIME_DIR`
(`just/just-*/`, removed after each recipe) unless `JUST_TEMPDIR` names
another dir; an agent's check sets both to private dirs.

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
strip text, the snapper banner, the Today view ("1 event today") and the
System sections with every field optional. The theme tones (WP-177,
SPEC-PLUGIN §7 "Text tones"): `textOn` and `deskTones` reach 4.7:1 (text)
and 3.2:1 (UI parts) on every surface, as mixes of the theme's own colours,
on the three themes of `fixtures/themes/roles.json` and on every theme
under `$OMARCHY_PATH/themes/` when it exists (read as `Color.qml` reads
`colors.toml`); a theme whose own foreground cannot reach a target is
printed as a `theme report`, not failed. `SELDON_TONE_REPORT=1` prints one
`tone-report` line per host theme with the contrast of Omarchy's default
focus border. The banners' terminal scripts
(WP-117) are pinned verbatim; each shows its command as Copy copies it and
runs it, bash parses each, and a hostile index (quotes, `$(…)`, `rm -rf`
in the snapper message and the contract version) changes none of them.
`bash tests/plugin/terminal-scripts.sh` runs every script inside the
presentation launcher's own `omarchy-show-logo; …; omarchy-show-done` line,
in a session of its own (`setsid`), with stub `sudo`, `curl`, `seldon`,
`omarchy` and `gum` (and the real gum, for its flags), scratch HOME: the
green line and the follow-up `seldon capture` or `seldon status` only on
success, the red line on a refused password, a failed download
(pipefail), a failed installer or an empty USER (no `sudo` call), one more
capture when the lock is held, "Read access granted" instead of
"recorded" when both captures fail, and "Done" after each; Ctrl+C (a stub
sends SIGINT to the process group and dies of it, or catches it and
exits 1, or it comes during the announce lines) gives the "Cancelled"
line as the last output, no follow-up, no command started after it, and
no "Done" (status 130). `terminalArgv` returns the launcher argv only for
one of the five scripts; a forged banner gets null. For the panel actions (WP-012): the case picker lists
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
`updated`), every part optional, all opening the fixed target `logbook`.
For Import tasks… (WP-102b): `importPathError` (empty, relative, `~user`,
not `.md`, a newline, U+202E, U+200B, a BOM, NUL, over 4096 characters);
`importArgs` (the path one argument after `--`, a shell-looking path kept
whole, the area a slug); `validateArgs` takes `import task --json
[--dry-run] [--area <slug>] -- <path>` and `plan show <caseId> --json` and
refuses the rest (no `--json`, flags out of order, `--include-done`, two
paths, a bad area or path, `omarchy-agent`); `importResult` (the dry run's
list, the import's ids — one that is no case id dropped —, the reasons in
words, the engine's refusal, not JSON); `caseShowArgs`/`caseShowResult`
(the whole Intent, `intent: null` withheld, a refusal); an imported case's
list mark, its Start without Enter and without a key (and no Drop on Enter
in its place), armed by click, `intentReviewed` only for this case's
finished, successful `plan show`; ImportForm and Work show engine text as
Text.PlainText only. Round 2: `BAD_PATH_CHARS` is exactly
`fixtures/bad-path-chars.txt` (every code point; the engine's
`import::tests::bad_path_char_is_the_shared_list` checks the same file);
`intentReviewed` false for a cut Intent or one with `hidden` characters,
and `reviewHint`'s words for each; `caseShowResult` `hidden`;
`importResult` with `droppedCharacters` and the `too-long` reason.
For bulk triage (WP-124b): `validateArgs` takes `agent ask triage|drift
<eventId>|case <caseId> --json`, `drift apply <proposalId> [--item
<eventId>] --json` and `drift discard <proposalId> --json`, and refuses
`propose`, two `--item`s, an `--actor`, free text and ids of the wrong
kind; `askArgs`, `applyArgs`, `discardArgs`; `triageButton` (open changes
and write access); `triagePath` (only `proposals/<id>.json` next to the
index); `parseProposal` (every schema part, the file off → null);
`triageView` (the bar's line, the state, regular and crisis items — the
index's crisis counts whatever the file says —, the authors, the marks,
the outcomes of this proposal's last run, applied ≠ done); `askResult`,
`applyResult` (done/skipped/refused, `markedApplied`, `gone`),
`discardResult`. Round 2: `parseProposal` against every rule of
`proposal.schema.json` (unknown properties at each level, `logbook`,
date-times, lengths, link/explain exclusions, item and ref counts) and
the 4 MiB limit; `triageSeen` (none, current, replaced, gone);
`evidenceAuthor` anchored at the start and with several authors;
`evidenceFlagged` for any `agent:` or `unknown` author; `itemOutcome`
with refused items; a static check that every `Text` of
`TriageDetail.qml` is `PlainText` and nothing is elided.
For the desk's sections 4–6 (WP-123): `deskFilter` (every word, any
field, case-insensitive); `decisionDetail` (Accept only while proposed
and marked as a write, nothing enabled for a malformed id, the notes);
`acceptArgs`, `acceptResult` and `validateArgs` for `decide accept
<ADR-NNNN> --json` (WP-135: the id checked, nothing else admitted; the
answer, `already`, refusals); `decisionCases` (the
sample's, null for an index without the field, titles and status from
the case lists, an unknown case by its id, non-text entries dropped); `systemTiles` (five tiles, big values
and leads on the sample, a failing collector's stripe, "—" and "Not in
the index" for an empty or sparse `system`); `memoryDetail` (file and
date). For the Prime Radiant (WP-030): the period ids and ←/→ wrapping;
`periodWindow` (inclusive days ending on the index's today,
across a leap day, *All* unbounded); `isoWeekMonday` (week 53 only in long
years); `seriesInPeriod` (heatmap and packages by date, a drift week that
touches the window, case spans that overlap it, open cases, broken rows
left out); `periodTable` on the sample (rows per slot for 30/90/365/All:
`30,2,5,3,17`, `90,3,5,3,18`, `365,3,5,3,18`, `366,3,5,3,18`, with the
count and detail lines) and without an index; `overlayGrid` in its three modes (exact
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
index; `aggregationCount` counts exactly the table's passes. ADR-0038
(WP-127): `driftRuleInfo` takes the index's `rule` before any `drift
show` answer and falls back to it only without the field (a rule out of
the schema's shape counts as none); `caseDetail` carries `intent`,
`result` and an "Imported from" row for the sample's C-2026-007 and
C-2026-001, nothing on an index without the fields or with non-strings;
`decisionDetail` shows the lead as `text`.
The graph (WP-125): `graphBuild` on the sample (67 nodes: 6 areas, 8
cases, 4 decisions, 47 changes, 2 crises; only change kinds; crises from
`drift[]`), its edges (event → case, case → area, decision → case, the
proposed case dashed; a contract-1 index without `decisions[].cases`
loses only those), the day index (event date, case created, decision
date, an area at its earliest neighbour, alone at day 0; an area only a
case names; a drift item older than the events), no index; folding on
`tests/plugin/graph-index.js`'s busy index (exactly 400 nodes, every
change a node or in one cluster, crises, areas, cases and decisions
never, no unfolded group bigger than a folded one, coarser levels for a
tighter cap) and `graphFold` level by level; `graphState` (deterministic,
positions kept by id, a new node beside its case, the cut kept
mid-replay), `graphSetCut` (visible = day ≤ cut, monotonic over every
day), `graphStep` (200 ticks then asleep, nothing overlaps, links
shorter than the mean, a pinned node held, its neighbours following),
Barnes–Hut within 5 % of the exact repulsion, `graphPick`, `graphInfo`,
`graphFit`, `graphShape`. Round 2: the step's path counters (exact
pairs for the sample, the quadtree for 400 nodes, exact again for a
replay's early days); case references that are prototype keys
(`constructor`, `__proto__`, `toString`, …) or name another node
(`ADR-0003`, `area:themes`) link nothing and throw nothing; 2000 more
areas give a still picture that never steps or wakes; the sleep test is
bounded (a layout that never sleeps fails after 250 ticks instead of
hanging).

`node tests/plugin/model.bench.js` times `periodTable` (what the service
does on every index write) on the sample, the sample ×10 and 7000 timeline
rows, in a plain function scope and in a vm sandbox (as the tests load
Model.js; slow global lookups, about 8× slower and load-sensitive, so only
reported), against the WP-030 cut. It fails when the fastest of 31 plain
runs on ×10 takes more than 10 ms (idle about 1.9 ms, about 4 ms with the
host fully loaded).
It also times one `graphStep` at 400 nodes (the cap, on the busy index
of `tests/plugin/graph-index.js`) and fails over 8 ms (fastest of 31
plain runs; about 0.25 ms under node — the shell's QV4 is about ten
times slower, which `desk-view.sh` and the live run below measure), and
reports `graphBuild` (the service's work per index, about 0.7 ms). It
also fails when the 400-node step ran exact pairs instead of the
quadtree (its path counter): under node that loss costs 0.3 ms and no
time gate sees it, under QV4 it costs 10 ms per tick.
`SELDON_BENCH_BUDGET_SCALE` multiplies both time budgets (a number ≥ 1;
CI sets 3, WP-190: on one dev-host core shared with two busy loops the
×10 median was about 6 ms, so a slow runner stays clear of 30 ms while a
regression of an order of magnitude still fails).

### 2. `Service.qml` in a private headless Quickshell

`bash tests/plugin/service-states.sh` starts `tests/plugin/harness/shell.qml`
with `quickshell -p` and `QT_QPA_PLATFORM=offscreen`, once per scenario. The
harness loads `plugin/Service.qml` the way the shell loads a third-party
service (no parent), prints a JSON snapshot (`status`, `pill`, `banner`,
`engine`, …) and quits. It is a separate Quickshell instance: it never talks
to the running omarchy-shell and writes only to a temp dir.

Scenarios: every status (fixture index, `PATH` without `seldon`,
`index-variants/not-initialised.json`, a missing path, broken JSON,
`SELDON_NOW` three hours after `generatedAt`, `invalid/index.contract-v3.json`
and the sample as contract 1 — the banner names both versions and the side
to update —, and the plugin of the `v0.1.3` tag (`git archive`; without the
tag this plugin set back to contract 1) against the contract-2 sample, WP-120),
a relative `SELDON_INDEX`, an index that appears after start, an atomic
replace (temp file + rename), an engine installed while running ("Check
again"), the live loop without the dev override (capture, then status
writes the index; calls never overlap), engine exit 3, the exact argv of
the banner fixes (fake `wl-copy` and terminal launcher record it), the
crisis strip text, the engine-missing banner urgent with an index and
accent without one (WP-117), `index-variants/snapper-degraded.json` with
the argv of its *Copy* (the plain grant) and *Grant* (the grant script),
its three actions and its one-sentence detail with the engine's message on
hover; the index replaced after *Grant*, as the script's capture does, and
the banner gone without a click; live, *Grant* changing nothing in the
panel, then *Check again* running the
same `capture` and `status` as *Capture now* (`["fix", action, banner]`
and `["snapshot"]` in `HARNESS_ACTIONS`): with snapper fixed the banner
is gone, still failing it stays with the new message on hover;
`XDG_STATE_HOME` (absolute and the
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
XDG dirs follow it), and every Quickshell gets a private
`XDG_RUNTIME_DIR` (see "The session's runtime dir"). Both scripts end with a check
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
reason. The same check fails when a new entry that no running process holds
appeared in the session's `quickshell/by-id` (WP-161). `tests/plugin/real-home-guard.test.sh` (part of `just plugin-test`)
proves both sides in scratch HOMEs, and the runtime check in scratch
runtime dirs.

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

### 3. The desk (`Desk.qml`) in a private headless Quickshell

`bash tests/plugin/desk-view.sh` runs the real desk against the real shell
components (ADR-0034; it replaced `panel-view.sh` and `overlay-view.sh` in
WP-121 — every scenario of those two and its successor are listed in
`tests/plugin/COVERAGE.md`). Quickshell serves `qs.*` from the config root
of the instance, so the script builds a temp root with copies of
`$OMARCHY_PATH/shell/Commons` and `shell/Ui` and starts
`tests/plugin/harness/desk.qml` as its `shell.qml`, against a copy of
`plugin/` whose `components/desk/DeskWindow.qml` (a layer-shell window,
which an offscreen instance cannot create) is replaced by
`tests/plugin/harness/DeskWindow.qml`, an Item filling the harness window.

The harness loads Service.qml and the pill as the shell does, with one
stand-in of the plugin's scoped facade that behaves like the shell's
overlay loader: `summon` creates Desk.qml bare and injects `shell`,
`manifest`, `service` before `open(payload)`; `hide` calls `close()` and
drops the item; `toggle` is hide when open, else summon. Its
`updateEntryInline` records every settings write and hands the entry back
to the pill after 30 ms, as the shell's reload of `shell.json` does
(`HARNESS_REFUSE` makes it refuse). Steps (`HARNESS_STEPS`, the header of
`harness/desk.qml` lists them): `summon`, `hide`, `toggle`, `pill:<left|
middle|right>`, `shim:<method>[:<arg>]` (the pill's `jax.seldon.panel`
handler), `call`, `section`, `select`, `width:<pct>` and `sidebar:<mode>`
(Omarchy's bar settings changing a key), `resize`, real keys (`key:
[Alt+]<Name>`, `text`, `type`, `keyDown`/`keyUp` to hold one), a held
key's auto-repeat (`keyRepeat:<Name|char>`: QtTest makes none, so the
harness hands an event object with `isAutoRepeat` to `keyPressed(event)`
of the key guard the focus is in — a field's, a form's, a `KeyButton`'s,
else the desk's; every report's `keyGuard` holds that guard's name and
call count, which a real `keyDown:` must raise, so the Keys handlers are
proven to route there; WP-173), `focusName:<objectName>`, `click`, `clickName`, `clickAt`, `drag`/
`release` (the width slider), `wheel:<objectName>:<delta>`, `hover`,
`settle`, `wait:<path>=<v>` (`^=` for a prefix), `pause:<ms>`, `shot`,
`view`; for the Prime Radiant (WP-123) `fresh[:<json>]` (drop the desk and
summon a new one, as the loader does from closed; the report's
`firstFrame` holds the aggregation passes and paints sampled on its first
swapped frames and the frame by which every chart has painted),
`hoverItem:<slot>:<i>` (the pointer onto item i of that chart,
`chart.locate`) and `leave`; for bulk triage (WP-124b) `service:<method>:<arg>`
(a Service method called directly, to reach a guard behind a disabled
button; its result in `call`) and `timedClickName:<objectName>` (a click;
`call` holds the milliseconds its handlers took); for the graph (WP-125) `graphPlay`,
`graphCut:<day>`, `graphHover:<id>` (the pointer onto that node) and
`graphDrag:<id|empty>:<dx>,<dy>` (press on a node, or on a point with
no node near, move, release; `call` holds the node's window point
before and after); `HARNESS_NO_PILL` runs without the pill (the
plugin not in the bar). The facade stand-in answers `false` when the
entry would not change, as `shell.qml` does. After each it prints
`Desk.view()` (with the current section's own `view()` as
`sectionView`), the facade's calls and writes, every visible text and
every text outside the window, the desk or its Prime Radiant slot.
Inside an item that clips (a list, a scrolled detail, a scrolled chart
grid) only the part of a text inside the clip counts: rows scrolled out
of a list are not on screen. Live cases run Service.qml without
`SELDON_INDEX` against the fake engine (`tests/plugin/fake-seldon`, in a
temp `HOME`) and compare its `argv.log` argument by argument with the
CONTRACT.md forms; the editor and terminal launchers are
`fake-recorder`s (`HARNESS_RECORD`).

Checks: the width at 50 / 67 / 75 / 100 % on 1366, 1920, 2560 and 3840 px
windows (the ADR-0034 §1 clamp, centred within a pixel); the sidebar's
icons under 960 px and with `deskSidebar` collapsed, the stacked layout
under 760 px, solo sections; the keyboard map (digits and `,` over the
nine targets, Alt+↑/↓ wrapping both ways, Tab doing nothing, `/` and the
search's Enter and Esc, the Esc order, closing through `hide`); Settings ›
Appearance (dragging previews and writes nothing, the release writes once
with every key of the entry, a preset once, the same preset not again,
the sidebar switch and the fold button, a refusal; a slider click at the
stored value writes nothing, with and without the key in the entry; three
wheel notches write once after the pause, a preset or Esc takes a pending
wheel value's place; no pill: nothing written, the change kept, the
sentence); open and close through
the shell, the pill and the shim (`tab work` lands on section 3, the
section remembered across a hide, `{"period":"30"}` on section 7); the
stacked Esc order; and the notices under the header with their fixes
(the restart notice and its one launch, the chip folding them, the rules
update live with doctor beside the queue, capture warnings and `c`). The
setup card (WP-119): from the fixtures, the snapshot step "1 of 3 steps to
go (optional)" (Grant opens the grant script, the watch starts, Not now
writes `setupSnapshots` once and card, chip and notice go, Settings ›
Capture's Offer again takes the key out and the card is back) and not
initialised "2 of 3" (Create logbook, `seldon init --defaults`, the chip
leading to Today's overview); live, the whole setup from no engine on
PATH (a stand-in terminal installs the fake engine, switches its mode as
`init` would and grants; the card moves 3 → 2 → 1 → done by its own
probe, index and capture, then the first-run card with the zero tile
muted; the three scripts launched once each, in order); the grant whose
own capture did not run, found by the card's capture
(`SELDON_SETUP_CAPTURE_MS`); an engine older than `engineMin` (the
logbook step waits for its notice; the urgent notice takes the chip); no
snapper (the `snapper-not-installed` variant) or the collector off (no
card, no notice); the `first-run` variant (the card, both tiles quiet);
a folder in use (the fake engine's exit 3 with
`FAKE_SELDON_UNINIT_REASON`: Choose a folder, which opens
`INIT_ASK_SCRIPT`); while only the optional step is open the sentence
stays. Sections 1–3 (WP-122; the 0.1 panel's scenarios for
these tabs, one to one): Today on the sample (state, tiles, NEEDS YOU, the
journal, yesterday in place, the overview and its case tiles, the sidebar
search) and live (the journal field with `--help 2` as one argument, a
blank note refused in the plugin, the case picker by keys, a refusal that
keeps the text, Open in editor, New case → `agent start --new`); the
Changelog on the sample (chips and counts, rows by class, the event
detail and its bar, `f`/`F`, groups and members, Enter opens the form and
Esc hides it, the shim's `filter` and `resolve`, Hide and Show, "+N
more"), a pacman transaction's packages (WP-137, `transactions`: the
09-19 interrupted `-Syu` marked in its rows and in the detail's callout,
the 09-18 mixed `-Syu` listing − + ↑ ↑ with old → new and its command,
the 09-27 downgrade group whose form lines give way to the list, a single
package without a list, no overflow at 100 % and 50 %; with `DESK_SHOTS`
`shot-tx-*` in the three themes; `model.test.js` adds failed and
unfinished, a word the contract does not know, a cut index, a clipped
command and the WP-141 hook on a synthetic note), the quiet surfaces (a crisis in the yellow zone, attention alone),
and live link / explain / dismiss, `--only` with a refusal in the plugin,
an already resolved re-run, a lock refusal with per-event drafts, `drift
show` from the leader and a member; bulk triage live (WP-124b: the
button and `agent ask triage`, the proposal row and detail with every
evidence text author first, Apply → done/done and the crisis skipped, the
crisis by its own button, a second Apply that skips all three, exact argv;
a refused launch and a refused item shown; Ask agent on an event and a
case and Discard (armed twice, the gone answer kept); no button with
nothing open; dev mode read-only with the marks for an agent's and an
unknown author's evidence; round 2: a newer proposal arriving while the
first is open — the bar says "Replaced …", Apply and Discard are off, the
service refuses the old id asked directly (`service:` step), nothing
reaches the engine, Review opens the new one; a 200 × 10 proposal of
256-character texts — items built only when shown and in the background,
the click under 200 ms, nothing outside its box at 1920, 960 and 700 px); Work on the sample (groups, the case
detail, the bar by status, dev mode's refusal, By agent, a reopen) and
live (the new-case sheet by keys, start → to verification → complete,
each armed then run, Open in editor, the engine's refusal, `x x`, hand to
agent and its refusal, a locked new case, Run and its refusal, Reopen and
`r`); the desk steps aside for what it opens (WP-156, section 8g; the
report's `service` read-out: `stepAsides`, the live `sessions`, the open
and plan results): a launch the engine answered closes the desk through
the facade (Hand to agent by key and by click, Focus, Run and New case,
every Open in editor, *Ask agent*, a notice's *Grant*), a refusal keeps it open with
the text; an active case an agent works on shows Focus in place of Hand
to agent ("agent working" on its row and in the bar); a stale desk heals
from the engine's "already working" and "no agent is working" refusals
(`FAKE_SELDON_SESSIONS_LATE`, `FAKE_SELDON_FOCUS_GONE`); an index change
while the desk is open asks for the sessions again (`aside-index`);
without Hyprland the engine tracks nothing and a second hand-off launches
(`FAKE_SELDON_NO_TRACKING`, `aside-nowindow`); `a` four times and `e`
three times send one call each, busy labels while in flight (the fake
holds each open until the step `touch:release-open`,
`FAKE_SELDON_HOLD_OPEN`, so "in flight" is a state, not a race); the same
open within 2 s sends nothing, after 2 s it is sent, and a failed open
does not start the 2 s (`aside-openfail`); an editor window the engine
focused (`FAKE_SELDON_OPEN_FOCUSED`) launches nothing. The fake engine
keeps the open agent windows in `$HOME/sessions` and logs `agent
sessions` (its own process) to `sessions.log`, not `argv.log`; before
init (WP-138) Today lists what `seldon preview --json` returns
(`FAKE_SELDON_PREVIEW`, the sample; its own process, logged to
`preview.log`, not `argv.log`): both groups, the card's sentence and
summary, and Set up Seldon opening the init terminal (`preview-uninit`);
a failed preview's error on the card and in the list (`preview-failed`,
`FAKE_SELDON_PREVIEW_EXIT`; `FAKE_SELDON_VERSION_DELAY` holds the probe so
the status turns notInitialised after the desk opened); none in dev mode
(`uninit`); Import tasks… (WP-102b) live against the fake engine (`import
task` and `plan show` there): the form, the dry run's list and skips, one
click imports, the first case selected with the list's line, its detail's
whole Intent (plain text, 5 lines, the provenance line, the source) and
no first-paragraph INTENT beside it, Enter twice does nothing, Start armed
by click and run, a second dry run all `already imported`, the argv with
the path as one argument after `--` and `plan show` only for the imported
case and before its Start; the form's own path check, the engine's
refusal in the form and the fields kept after Esc, a withheld Intent that
leaves Start off; dev mode: the index's first paragraph, "needs the
engine", no Start, the form shut, nothing outside the desk at 50 %;
round 2: two stray `trigger:workDetail:start` behind a disabled Start run
nothing (a harness verb that emits the detail's `actionTriggered`), a new
area turns Import off until its own dry run and the import carries
`--area`, a capture's new index asks `plan show` again with the last text
kept on screen and Start off until the answer, a marked (`hidden` 21) and
cut Intent keeps Start off with the hint and both notes
(`FAKE_SELDON_SHOW_HIDDEN`, `FAKE_SELDON_SHOW_TRUNCATED`); stage 2: the index rewritten while the first
`plan show` runs (`FAKE_SELDON_SHOW_TOUCH`) asks once more (three `plan
show` in all) and `Model.reaskAfter`'s cases; a
section change gives the keys back from a field and keeps its
draft; the Changelog's selection follows its event across an index update
(the acceptance's cursor stability); Capture now over a lock retry; one
count everywhere (chips = sidebar = header = the quiet line, a group once,
after a Hide too); the "why loud" callout from the index's `rule`
(ADR-0038: live and in dev mode, the argv only the start-up calls — no
`drift show` per click) and, on a copy of the sample without `rule`, from
the engine's rule (`drift show`, live; the fake names `always-red-paths`
for config, `always-red` for pacman, `attention-all` with
`FAKE_SELDON_ATTENTION_ALL`), a planned crisis, a group from a member; the
details (`details`, `details-bare`): an imported case's INTENT and
"Imported from", a completed case's RESULT, a decision's lead, and on an
index without the four fields none of them; free text with surrounding blanks in every
field (the argv keeps them); Open case for a case the index no longer
lists; key/values at 50 %; the
sticky bar (its scene position unchanged while the detail scrolls, in
Work and the Changelog); the stacked layout.
The Prime Radiant (WP-123, the old `overlay-view.sh` scenarios): periods with ←/→, h/l and chip clicks, the "←/→ period"
hint, 90 d on every entry, `setPeriod` from another section and an
unknown id that changes nothing, `hover` only while shown; entering
from closed aggregates nothing on the first frame, has painted nothing
then and every chart once by frame 2, a period switch repaints only the
charts with a period (`2,2,2,1,2,1`), a hover nothing, a resize each once;
hover read-outs from mouse moves and `call hover` with every malformed
argument; the grid at 2560×1440, 1.25-scaled outputs and
`QT_SCALE_FACTOR=1.25`, at 50 % (medium; the RiskDonut drops "all time"
from its centre there) and under 960 px (narrow, scrolling) with no text
outside its slot; not initialised; every index
variant. Decisions, System, Memory (WP-123, the old panel's scenarios
5–7 and 20–23): the sample's rows, details and sticky bars, the search
in all three (Decisions by title, System by lead and value, Memory by
title and path; the first `j` after a search hid the selection),
`select`, decision cases when the index carries them, the new-decision
form live against the fake engine (Enter arms, a change disarms, the
exact argv and editor paths of decide, `e`, Open in editor on
all three) and its refusals (title kept); Accept (WP-135,
`decisions-accept`): the first click arms it (*Confirm accept*, the
hint), a key, a click on another decision and a new index (a capture
from the pill, `decisions-accept-index`) disarm, the second click runs `decide
accept ADR-0004 --json` once and no `open`, the decision arrives
accepted without Accept; the engine's refusal and a held lock show in
place (`decisions-accept-refused`, `-locked`); in dev mode Accept is
disabled; System's five tiles with every
field optional and a failing collector, Memory, not initialised, the
stacked layout and label fit at 1366 and 3840 px. Every case ends with a
log free of warnings, `TypeError`s and binding loops. `DESK_SHOTS=<dir>`
also renders the desk in Tokyo Night, Kanagawa and Catppuccin Latte
(Today at 100 % and 50 %, Settings, the Changelog, Work, Decisions,
System, Memory, the Prime Radiant at 100 % and 50 % and with a hover,
the graph settled, with a hover, at 50 % and in a replay at day 12, not
initialised).

The graph (WP-125, section 11): on the sample it settles and sleeps
(200 ticks, the Timer off, no tick and no paint after), each reported
`tickMs` ≤ 8 and at most 2 of the ticks over it (`slowTicks` names
them). A case that misses runs once more and must pass then; if it
misses twice while the host's 1-minute load is at least half its cores,
the miss is printed as a `WARN` line and not counted, unless
`SELDON_PERF_STRICT=1` is set (the live run on the test host keeps every
budget strict). Why it retries: the
sample's ticks take 1–3 ms, but this host compiles other work packages
at the same time, and the harness's own polling (`wait:` builds the
desk's whole `view()` every 100 ms) makes the garbage collector run —
QV4's collections grew to 16 ms over a run here, and one that lands in a
paint shows as a slow tick. Without the polling a 200-tick settle ran no
collection at all (`QV4_MM_STATS=1` with `QT_LOGGING_RULES=
qt.qml.gc.allocatorStats=true`). A slower graph misses twice; the
strict "every tick" is the live run's (§3d). A switch
to another section stops the ticks at once and coming back resumes
them; a closed and reopened desk shows the settled layout from the
service without a tick; the service builds no graph before section 8
is shown, and none while two live captures rewrite the index with the
Prime Radiant shown — showing section 8 then builds once and keeps the
settled layout (the report's `graphBuilds`, `graphDirty`,
`graphNodes`); the replay from day 0 grows monotonically to all
67 nodes, `graphCut`, ←/→, Space and Esc; hover (the card, its line,
*Open case* into Work with the case selected after the pointer left the
node), `select` and Esc; a node dragged by 160,90 px lands there and
wakes the layout, which sleeps again; a pan and a wheel zoom only
repaint; `0` fits. On the busy index: 400 nodes, 295 changes folded into
99, the legend's *Folded*, a folded group's card, at most 5 of 200 ticks
over the budget (its ticks print). Not initialised, nothing to draw, and
50 % on 1366 (the right-edge crisis label goes left of its node) and a
700 px window without a text outside its box. 2000 more areas: a still
picture (2022 nodes) with its caption, no tick and no Timer after a cut
or a drag (strict), the dragged node exactly where it was put, and the
fastest of its three paints ≤ 8 ms (the same picture each time, so the
fastest is its cost, about 4 ms; a timing gate, once more on a miss).
The busy index's "at most 5 of 200 ticks over" runs once more on a miss
too.

### 3c. The pill (`BarWidget.qml`) in a private headless Quickshell

`bash tests/plugin/bar-view.sh` (WP-051) renders the real pill the way the
bar hosts it: `tests/plugin/harness/bar.qml` gives BarWidget.qml the
shell's own `PluginBarApi` facade, bound to the theme's bar colours and
font, in a strip one bar tall on the bar background, with Service.qml in
dev mode on the sample (2 active, 4 open drift, 2 crises: urgent tone) and
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
the scratch dir, rendered with `driftInBar` `all` so the digits stay;
the three tones must give three colours); the setting `driftInBar`
(ADR-0028 §4a, `HARNESS_SETTINGS` as the widget's `shell.json` entry):
none, `crisis` and an unknown value give `2 · 2`, `all` `2 · 4`, `none`
`2`, each urgent with the same tooltip and the service's read-out
following the widget; `none` with a crisis and no active case is the
urgent glyph alone; attention alone in the default mode is the glyph alone
in the bar foreground; plus the not-initialised variant (the glyph alone,
dimmed). Checks: file, box,
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

### 3d. The graph live on a session (WP-125)

`bash tests/plugin/graph-live.sh <index.json>` runs the real desk in its
real layer-shell window on the Hyprland session it is started in (read
only: `SELDON_INDEX`, a scratch config root, its own Quickshell; neither
the running shell nor `~/.config` is touched), opens section 8 and
prints `GRAPH-LIVE` lines after the layout settles, after a replay from
day 0, after a cut to the middle day and after a switch to Today: the
graph's read-out (ticks, `tickMs`, `tickMsMax`, `slowTicks`,
`stepMsMax`, `drawMs`, `paintMs`, the replay counts) and an event-loop
probe (a 1 ms Timer on the shell thread; its largest gap). Not in `just
check`: it needs a session and shows the desk (with the keyboard) for
about half a minute. Its Quickshell gets a private runtime dir like every
harness's (WP-161); the real window still needs the session's Wayland
socket and Hyprland dir, so that dir holds links to exactly those two.

Measured on the test host (2026-10-07, Omarchy 4.0.4, Quickshell 0.3.1,
6 cores, one 1920×1080 screen at scale 1.25, the desk 1528×830): on a
scratch logbook backfilled 90 days (ADR-0033; `--since`, `--baseline`;
2267 events, the index's newest 500, which fold one upgrade day of 355
changes into one node: 152 nodes) `tickMsMax` 3 ms over 659 ticks; on
the busy index of `graph-index.js` (400 nodes, 229 edges) 7 ms over 691
ticks (step ≤ 6 ms, drawing ≤ 2 ms), none over 8. Both replays grew
monotonically to every node; switching to Today stopped the ticks. The
probe's gaps of 16–20 ms are frames, not work: Qt's threaded render loop
holds the shell thread to the display's frames while a QML animation
runs (a bare layer-shell window with one `Behavior` shows the same, a
Timer-moved rectangle does not); the GUI thread's own share of a frame
(polish, sync) stayed at 0–1 ms (`QSG_RENDER_TIMING`). The first live
run found two things fixed since: the first tick ran interpreted (8 ms;
`graphWarm`), and the slider's knob animation ran through the whole
replay (266 such gaps; the knob now holds still while playing).

### 3e. An IPC exit with two pills (`ipc-restart.sh`)

`bash tests/plugin/ipc-restart.sh` (WP-162) starts the bar harness with
two BarWidget instances (`HARNESS_IPC_KILL`), waits until one owns
`jax.seldon.panel`, and ends the shell with `quickshell kill`, the way
`omarchy restart shell` does. Up to 0.1.3 the owner handed the target to
its sibling from `Component.onDestruction`; the sibling's IpcHandler then
registered with the dying engine generation and Quickshell 0.3.1
crashed. Cases: two drawn widgets and a centre placeholder next to a
drawn one, each with the owner created first and last (Qt tears the
newest down first; the shell's order is the owner first), and `three`
(round 2): the owner, a hidden placeholder and a survivor, where the
owner and the placeholder go at run time in one turn, both still listed,
and the survivor must own the target and receive IPC `open` (it
forwards the call to the desk) before the kill. Checks: no
widget becomes the owner after "Exiting due to IPC request" (the harness
logs every ownership change), the kill and the shell's exit status are
0, no crash report under the scratch HOME's `.cache/quickshell/crashes`,
nothing of the shell's session left running, a clean log, the real-home
guard.

The harness does not reach the SIGSEGV itself (the sibling's handler is
enabled during the teardown there too, but nothing registers); the
ownership check is the one that fails on the old code. The end-to-end
proof is the live restart with two monitors in the release steps
(VERSIONING.md, "Tag flow"). Each quickshell runs in its own session
(`setsid`) and its own runtime dir under `/tmp`, both removed afterwards;
the runtime-handover side (a monitor unplugged: the next widget takes the
target over, and IPC `open` reaches it) is `bar-view.sh` §4 and §5.

### 4. Runtime smoke test in the shell

From 0.2.0 (ADR-0034) the panel and the overlay are one surface, the
desk; the `jax.seldon.panel` commands below reach it through the shim
(SPEC-PLUGIN §8) until WP-126 rewrites this section for the desk.

**The desk's window on a live session** (any host with a Hyprland
session, the dev host included: it touches neither the running shell nor
`~/.config`): build a scratch config root as `desk-view.sh` does — copies
of `$OMARCHY_PATH/shell/Commons` and `Ui`, a copy of `plugin/` with the
**real** `DeskWindow.qml` — and a `shell.qml` that loads Service.qml and
Desk.qml with a stand-in facade, then run it with `SELDON_INDEX` set
(`quickshell -p <root>/shell.qml`, `WAYLAND_DISPLAY` and
`HYPRLAND_INSTANCE_SIGNATURE` of the session). `hyprctl layers -j` shows
the `jax-seldon-desk` surface (monitor, position below the bar, size) and
`Desk.view()` the card's geometry; `grim` shows the result. WP-121 did
this on both hosts (HANDOVER.md).

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
     `invalid/index.contract-v3.json` as they are, and delete the file for
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
   omarchy-shell shell summon jax.seldon '{"section":"radiant"}'   # the desk at the Prime Radiant
   omarchy-shell shell call jax.seldon view ""   # while it is open; sectionView: period, slots, geometry, charts
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

For a live test of `next` (the next minor version) the orchestrator
deploys `next` the same way after next's check and push (WP-155):

```bash
SELDON_TEST_HOST=<alias> just deploy-test-host --branch next <next check log>
```

Everything below says `main`; with `--branch next` read `next`: the
checkout is on `next`, HEAD equals `origin/next`, the check log is
next's, the build is `SELDON_BUILD=next.<short sha>`
(`<version>+next.<short sha>`), the marker says `build=next.<short sha>`
and the log line `mode: "next"`. One step comes first: while the host
does not run a next build yet (neither its plugin marker nor its engine
says next, so a retry after a first next deploy that failed after the
engine swap takes no second backup), the deploy copies, whichever
exist, `~/.config/omarchy/shell.json`, `~/.config/seldon`,
`~/.local/state/seldon`, the host's logbook (the path its installed
engine's `seldon doctor --json` names), the engine and the plugin dir to
`~/.local/state/seldon-dev/backup-before-next-<UTC stamp>/`
(`shell.json`, `config-seldon/`, `state-seldon/`, `logbook/`,
`seldon.engine`, `plugin-jax.seldon/`, as in the orchestrator's backup
by hand of 2026-10-07; links, such as a linked engine or `shell.json`,
are copied as links) before the engine swap, with a `RESTORE.txt` that
the summary prints too: first stop the watcher and put back the engine,
the logbook, `~/.config/seldon`, the state dir and `shell.json` (next's
copies move aside as `<path>.next`), then right away deploy main (or
`--release`), whose smoke `capture` then writes into the restored
logbook, not next's; then start the watcher again. Next writes state and ledger lines main cannot
read; this is the way back. An active
`seldon-watch.service` is stopped for the copy and started again (a
failed stop or start fails the deploy before the swap).

Refused before anything changes (exit 1): an engine on the host that
does not name a logbook (`seldon doctor --json`, `.logbook`; no engine
either), with the fix; a logbook that is not a directory strictly below
the host's home — `/`, the home, a parent of
it, a path outside it, a link out of it, a control character such as a
newline in the path — or that holds
`~/.local/state/seldon-dev` (the backup would copy itself); and a backup
larger than `SELDON_DEPLOY_BACKUP_MAX_MB` (default 1024 MiB; the message
names the size) or whose size the host's `du` does not tell. A logbook
the engine names but that does not exist yet has nothing to copy; the
dry run says "absent". An unsafe logbook is not even measured. The install
step checks the logbook path again right before the copy. A failed
backup stops the deploy before any other change (exit 2, logged with the
partial backup). The summary and the log line (`backup`) name the
backup; a deploy from next to next takes none. A main or `--release`
deploy onto a host that runs next warns and points to the newest
`RESTORE.txt`, to be followed before it (it does not restore anything).

The deploy test runs in a temp dir under the checkout's ignored
`target/`, never `/tmp`, and on its fake host `cp` and `du` refuse every
path outside that dir (`mv`, `rm`, `rsync`, `install` and `tar` too),
and switches make them fail (`cp_fail`: a source
path containing the named substring; `du_empty`) whatever the user, as
CI runs the test as root: on 2026-10-09 a hand mutant without the logbook
guard (WP-155) copied `/` into a test dir under `/tmp` and filled it.

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
   `backup`, `watch`, `restart`, `smoke`, `failures`) and prints a summary;
   for a main or next build its `commit` line has the full commit.

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

**Live test of a release.** The summary's `commit   <sha>` line is the
commit a release's live test ran on; the record of that test,
`packaging/acceptance/vX.Y.Z.json`, names it, and
`packaging/acceptance-check.sh` refuses a tag whose code changed after it
(VERSIONING.md, "Release acceptance record").

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
