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
