# Pitfalls — things that cost time, so the next agent does not repeat them

Append-only. One bullet per pitfall: what happened, how to avoid it.

## 2026-10-01 · WP-002 (Schema Keeper)

- **The red-zone guard matches substrings.** `scripts/guard.sh` blocked a
  read-only `grep -rln … /usr/share/omarchy` because `-rln` contains `ln`
  followed by ` /usr/`. Write flags separately (`grep -r -l -n`) or avoid
  `ln`, `rm`, `cp`, `mv`, `tee` substrings before a path under `/etc`,
  `/usr`, `/var`, `~/.config`. The bare word `pacman` anywhere in a Bash
  command is blocked too — including inside a heredoc of prose or JSON
  (`/var/log/pacman.log` is fine, `grep pacman file` is not). Do not work
  around the guard; rephrase, or write the file with the Write tool.
- **Fixture content that looks like a command trips the guard** when written
  through a Bash heredoc (a hook payload with `sed -i … ~/.config/…`, notes
  mentioning package commands). Use the Write/Edit tools or a script file.
- **Frontmatter is not YAML-safe-loaded.** PyYAML turns `created: 2026-10-01`
  into a `date`, which fails `type: string`; serde in the engine reads it as a
  string. The fixture validator uses a strict flat parser (strings, ints,
  bools, `[a, b]`, empty = null) and rejects anything else. Keep case,
  journal, decision and memory frontmatter flat.
- **The sample index is derived, not hand-edited.** After changing anything
  under `fixtures/logbook/`, run `bash scripts/validate-fixtures.sh --write-index`
  and review the diff; hand edits to `fixtures/index.sample.json` are reported
  as "not derivable from the logbook".
- **Every JSON file under `fixtures/` needs a schema mapping** in
  `scripts/validate-fixtures.py` (`collect_instances`); an unmapped file fails
  the run. Must-fail fixtures go to `fixtures/invalid/<schema>.<name>.json`.
- **Collector events cannot know the case.** Only hooks know actor and active
  case; a human package install during an active case is drift (with a
  proposal), and an agent's Edit/Write tool call is invisible to the Bash
  hook. The fixtures model both; WP-006 should decide whether the hook also
  reads Edit/Write payloads (`tool_input.file_path`).
- **`omarchy plugin catalog` does not carry versions** (see host.md); do not
  build `plugin-update` on it.

## 2026-10-01 · WP-014 (Schema Keeper)

- **The guard also blocks a `python3 -c` or heredoc that contains the
  package-manager word**, e.g. a schema edit whose replacement text mentions
  `pacman` in a description. Put such edit scripts in a file under the
  scratchpad (Write tool) and run the file.
- **`meta.command` is logged unquoted.** pacman writes `Running '<argv joined
  by spaces>'`, so `--overwrite /usr/share/omarchy/*` appears bare. Split on
  whitespace, not with a shell lexer (an apostrophe in an argument would make
  `shlex` throw), and know which options take an argument word: every
  `omarchy update` runs `-Syu --noconfirm --overwrite /usr/share/omarchy/*`,
  and a parser that treats `/usr/share/omarchy/*` as a package name turns every
  routine update red (ADR-0013 §3). Unknown options must err towards red.
- **A direct `-Syu` is blocked on Omarchy** by `00-omarchy-update-guard.hook`
  (`omarchy-update-pacman-guard`) unless `OMARCHY_ALLOW_DIRECT_PACMAN=1` is set;
  the environment is not logged. Most routine upgrades therefore arrive via
  `omarchy update`, which also yields a keyring reinstall (named package →
  red) and, on a version change, the red `omarchy update` event.
- **Adding pacman events to the fixture ledger is a three-file change**: the
  ledger line(s), the same block in `fixtures/logs/pacman.log` *and*
  `fixtures/logs/pacman-rotation/pacman.log`, plus the byte offsets in
  `fixtures/README.md` (the collector tests rely on "the log produces exactly
  the ledger's pacman events"). Keep month files chronological when appending:
  pick a `ts` after the file's last line, and give the ULID that time part.
- **`index-variants/` are generated** from the sample plus an overlay
  (`VARIANTS` in `scripts/validate-fixtures.py`); a hand edit is reported. Add
  a banner state by adding an overlay, then `--write-index`.
- **ADR-0012 §13's token rule makes `.` a word character**, so a Plan line
  ending in `zed.` does not propose `zed`. The previous script regex allowed a
  trailing sentence period; the fixture's proposals are the same under both,
  so the change was invisible in the diff. Write Plan items without trailing
  punctuation after a package name, or raise it with the orchestrator.
- **Correction (same day): ADR-0015 §4 supersedes the item above.** A final
  `.` not followed by a word character is punctuation, so `Install zed.`
  proposes `zed` again; `zed.conf` does not. A rule change that leaves the
  sample index identical needs an end-to-end self-check, not just a clean diff.
## 2026-10-01 · WP-003 (Engine)

- **Running the engine by hand writes config and state.** `seldon init`
  writes `~/.config/seldon/config.toml` (red zone on the dev host) and takes
  `~/.local/state/seldon/lock`. Before any manual run, export
  `XDG_CONFIG_HOME` and `XDG_STATE_HOME` to temp dirs; the integration tests
  get the same effect from a temp `HOME`. The WP-003 acceptance line
  (`seldon init --non-interactive --path /tmp/seldon-wp003`) needs this too.
- **Filtering PATH directories does not hide host binaries.** `/usr/bin`
  holds `snapper` and `git` together, so dropping it from PATH drops git as
  well. Tests set PATH to a stub directory only, with a link to the host's
  git (`engine/tests/common/mod.rs`).
- **Real host names leak through `init` output.** The machine id is
  `<hostname>-<4 hex>`, so `seldon init` / `doctor` output names the
  machine. Redact it before pasting output into handovers or commits
  (AGENTS.md §8).
- **Scripted edits and `cargo fmt` make the editor's file state stale.**
  After a python/sed edit or `cargo fmt`, Read the file again before using
  the Edit tool.

## 2026-10-01 · WP-004 (Engine)

- **The guard blocks the package-manager word even inside code comments and
  memory text in a heredoc or `python3 -` edit.** Edits to the package-log
  collector, its tests, or notes about it need the Edit/Write tools, or a
  script file written with Write. Env names like `SELDON_PACMAN_LOG` pass.
  The lowercase word between spaces does not.
- **Count fixture events; do not trust memory.** The fixture ledger has
  **12** package-log lines (source `pacman`), not 13. Count them in the file
  before you write an expected number.
- **Capture-time events need the fixture's `now`.** The omarchy `update` and
  `snapshot-delete` events carry capture time. Run the collector at the `ts`
  the fixture gives them. The golden comparison then needs no ts
  normalisation.
- **The ADR-0014 10-minute window alone cannot produce the fixture.** The
  10-01 `omarchy update` event (09:21:00) is 10:58 after the hook command. It
  inherits from the package upgrade of `omarchy` to the same version instead.
- **A Claude Code PostToolUse hook fires after the command has finished.**
  The hook event's `ts` then lies *after* the package lines it caused. The
  ADR-0014 rule ("the command precedes") would then never match. The fixture
  has the command first. WP-009 must record the start time, or the ADR needs
  an amendment.
- **In-process tests that fork stub programs flake in parallel.** A forked
  child holds every inherited descriptor until it execs, so two races appear:
  - a stub written a moment ago fails with `ETXTBSY` ("text file busy");
  - a flock that was just released still looks held.

  `sys::run` retries `ETXTBSY`. The test bench takes its lock once. Do not
  re-acquire locks per step in a test, and loop the suite (e.g. 50 runs)
  after adding stub-heavy tests.
- **Running `seldon capture` against the real host needs redirected XDG dirs
  and a throw-away logbook.** Otherwise it writes
  `~/.local/state/seldon/cursors.json`. Reading the system log, `snapper` and
  `omarchy-version` is fine (read-only).
