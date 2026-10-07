# WP-118 — plan

## What changes

1. **Wizard (`engine/src/commands/init.rs`).** Every prompt a named
   constant, each short enough not to wrap in the presentation terminal
   (≤ 60 columns; a test pins the texts and the width).
   - `BACKFILL_NOTE` without the red pill and crises (ADR-0028): older
     changes are mostly routine history, the rest can be marked as the
     pre-Seldon baseline in one step. The baseline question shrinks to
     one count line and "Mark them as the pre-Seldon baseline?".
   - Theme hook: a note line (what the hook does, the Omarchy command)
     printed above a short question "Record theme switches instantly?".
   - Harnesses: built by one function from the kit directory. The kit
     item exists only when the kit directory does, labelled "Omarchy-Agent
     kit (private template)"; "Claude Code hooks (user-wide)"; "Seldon
     agent skill (into the agent skill folders that exist)".
   - Summary: five aligned rows (Logbook, Recording, Agents, History,
     Snapshots), then either "Done. …" or "Next steps:" with only what
     is left to do (a failed harness, git, capture or dossier, open
     drift, the optional snapshot grant). No machine id, file count,
     skipPaths hint, hook counts, fence count or ADR numbers; no
     `seldon doctor` on a clean run. `--json` keeps every key.
2. **`install.sh`.** An announce line before the download (what, where,
   as your user, how it is checked). "Next steps" from three read-only
   tests: no `seldon init` when `config.toml` exists, no plugin add line
   when `~/.config/omarchy/plugins/jax.seldon` exists, "nothing else to
   do" when nothing is left; the update/remove line after a blank line.
3. **Docs en/de.** Guide 01 step 2 (table, backfill text, summary block);
   guide 10 banner table (WP-117's titles and buttons); guide 11 "plugin
   0.1.0 users update the plugin first", "Omarchy shows the diff and asks
   — answer yes", "update the plugin when you update the engine";
   the release-notes template's standing line; CHANGELOG.
4. **Tests.** `engine/tests/init.rs`: the summary on a clean run and
   with work left, the kit item hidden without the kit and shown with it
   (unit test on the item builder), the prompt texts and widths.
   `tests/install/install.test.sh`: announce line, next steps for a fresh
   home, with a config, with the plugin, with both. shellcheck runs there
   already (CI installs it).

## Decisions

See HANDOVER.md, "Decisions".
