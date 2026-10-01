WP-024 HANDOVER

Branch `wp/024-templates-wizard`, worktree `wt/WP-024`. Base: `main` at
`90c3dec`. `main` has since moved by `3d0037c` (CONTRACT/SPEC-PLUGIN only).
Not pushed, no PR.

Commits `90c3dec..HEAD`:

| Commit | What |
|---|---|
| `85535fe` | templates en/de |
| `419c2be` | `hook::merge_claude_hooks` split out of `hook install` |
| `d6b73e5` | wizard steps, tests |
| `b914645` | TESTING.md, hooks README |
| `204a814` | README section order (avoids a conflict with WP-022) |
| `191859f` | memory |

The handover commit follows. `just check` exits 0 at HEAD.

Dry-run merges (`git merge-tree`) with `main`, `wp/016`, `wp/022` and
`wp/023` are clean.
- `wp/013` conflicts in `memory/pitfalls.md`. The conflict is already
  there between `main` and `wp/013`; it is not from this WP.
- My own appends to `memory/*.md` may conflict with other WPs' appends.
  That is the usual append-at-end case: keep both.

## ⚠ Host incident: leftover files under the real home (needs the operator)

What happened:
- During a pty smoke test of the interactive wizard I ran
  `env HOME=<scratch> … script -qec "seldon init --path <scratch>/logbook"`.
- `HOME` did not reach the engine through `script`. The engine used the
  real home.

What it created on the dev host (2026-10-01 17:10, all from this one run):
- `~/.config/seldon/config.toml`. Its `logbook =` names
  `/tmp/claude-1000/…/scratchpad/pty3/logbook`.
- `~/.local/state/seldon/`, containing `cursors.json`, `index.json`,
  `lock`, `manifest.json` and `hooks/seldon-theme-set.sh`. All of them name
  the same scratch logbook.
- Both directories did not exist before. I checked at the earlier
  real-host run: `ls ~/.config/seldon` → "No such file or directory".

What did not happen:
- No `~/Seldon` was created.
- `~/.config/omarchy/hooks/theme-set.d/` holds only its old `.sample` file.
- The real `omarchy` never ran. The theme-hook call went to the recording
  stub (`SELDON_OMARCHY`). The stub's argv file shows
  `hook install theme-set /home/…/.local/state/seldon/hooks/seldon-theme-set.sh`
  and nothing else ran.

Cleanup is not done:
- My `rm -r ~/.config/seldon ~/.local/state/seldon` was **blocked by the
  guard**: "red zone: write under ~/.config outside the jax.seldon plugin
  dir".
- Per the standing rule, I stopped there and did not route around the block.
- I also left `~/.local/state/seldon` alone, because it was part of the
  blocked command.

Fix for the operator:
- First check that `~/.config/seldon/config.toml` names the `pty3`
  scratch logbook.
- Then run `rm -r ~/.config/seldon ~/.local/state/seldon`.
- Until then, a `jax.seldon` dev install would read the stray `index.json`
  (the scratch logbook).

Cause and prevention:
- The cause is in `memory/pitfalls.md` (WP-024).
- `docs/TESTING.md` now says to export the XDG variables before `script`
  and to probe the paths with `seldon --json doctor` first.

## Done

### Templates (`engine/templates/{en,de}/`), ADR-0007

English keys and headings in both languages; the prose follows the
language.

- **`AGENTS.md` for agents** (rewritten, both languages). Its sections:
  - "The engine is the only writer": the ledger is append-only, and
    `ledger/*.md`, `STATUS.md`, case frontmatter and folders, `.seldon/`
    and the fences are generated;
  - "Work in cases": `plan new|start|verify|done|drop`, `--actor`;
  - "Zones": red, yellow, green per ADR-0014/0019, with the snapshot rule;
  - "Journal and memory": `seldon log`, `memory/lessons.md`,
    `decide --no-edit`;
  - "Drift": `link|explain|dismiss`, never hide drift by editing;
  - "Hooks": `hook install claude-code` for Claude Code; for other agents
    `hook generic`, `session-start` and `session-stop --actor`;
  - "Never": secrets, red commands without a red case, rewriting history.
- **German headings are now English** (`# Decisions`, `## Purpose`,
  `## Must not happen here`, `## Who works here`, `## History`, `# Packages`,
  `# Services`, `# Deviations from Omarchy defaults`). The `DECISIONS.md`
  table header is now `ID | Title | Status | Date`.
- **The `AGENTS.md` H1 is `# AGENTS.md` in both languages.** The tagline is
  now the first line of prose.
- **Other completions:**
  - `PROJECT.md` has a `## Purpose` section;
  - every area README points at `seldon plan list --area <a>` and at the
    optional `areas/<a>/AGENTS.md`;
  - `memory/lessons.md` tells you what a lesson holds;
  - the decision template has guidance comments under Decision and
    Consequences.
- **The case template is unchanged.** `log.rs` pins its ending with
  `## Result`.

### Wizard (`seldon init`)

New flags: `--harness omarchy-agent`, `--since TS` (YYYY-MM-DD at local
midnight, or RFC 3339), `--baseline` (requires `--since`), `--no-capture`
(conflicts with `--since`), `--theme-hook`.

The interactive order:

1. path
2. language
3. Obsidian
4. collectors
5. watched paths
6. harnesses (both listed; the kit entry says whether the template dir
   exists)
7. theme hook (yes/no, default no)
8. git
9. backfill (a note explains the drift consequence, then a date or empty)

After the layout, `init` does these in order:

1. **Harness setup.** It runs before the git step, so the files are in the
   first commit `seldon: init logbook`.
   - `omarchy-agent` runs first. It copies the template dir into
     `.claude/`, keeping modes, skipping symlinks and never overwriting a
     file.
   - Then `claude-code` runs WP-009's merge into `.claude/settings.json`.
     It merges next to a guard that the kit's `settings.json` may bring.
   - Without the kit, nothing is copied and `init` prints where it looked
     and what it would have copied.
2. **The init lock is released.**
3. **`capture --all`** runs with `--since` as the backfill window. It runs
   in-process with the logbook pinned, so `--logbook`/`SELDON_LOGBOOK`
   cannot redirect it.
4. **Baseline.**
   - Non-interactive: only with `--baseline`.
   - Interactive: when a backfill opened drift, `init` asks "The backfill
     opened N drift item(s) (M crisis) … Mark them as the pre-Seldon
     baseline?" (default yes).
   - `setup::baseline` takes every open drift event in `Built::open_drift`
     (also those past the 200 cap), oldest first. For each event not yet
     covered it calls `reconcile::select(…, only = false)` and then
     `reconcile::resolutions`.
   - Each line is `dismissed`, reason `pre-Seldon baseline`, actor `human`,
     `meta.txId` on fan-out.
   - Everything goes out in one `emit`, then the index is rebuilt.
5. **Theme hook, on opt-in only.**
   - `init` writes the embedded `engine/hooks/theme-set.sh` to
     `$XDG_STATE_HOME/seldon/hooks/seldon-theme-set.sh` (mode 755).
   - It then runs `<omarchy> hook install theme-set <that file>` once. The
     program comes from `Sources::from_env()`, so `SELDON_OMARCHY` or
     PATH.
   - When `~/.config/omarchy/hooks/theme-set.d/seldon-theme-set.sh` already
     exists, nothing runs.
   - A failure is reported with the manual command as a next step.
6. **Autocommit** `seldon: first capture` (or
   `… first capture and pre-Seldon baseline`).

Failures in steps 1–6 are reported and never fail `init`: the logbook
exists by then.

**Stale text fixed (WP-013 FINDINGS §3).**
- `--json` no longer prints `capture: {ran:false, reason:"no collectors…"}`.
- `capture` is now:
  - after a run, capture's own JSON plus `ran`, `since`, `openDrift`,
    `crisis`, `baseline` (`{reason, items, events}` or null) and `git`;
  - with `--no-capture`, `{ran:false, reason:"--no-capture"}`;
  - on failure, `{ran:false, error}`.
- The JSON also gained `harnessSetup` (per harness) and `themeHook`.
- Human output shows one line per step.
- Next steps no longer list `seldon capture --all` or
  `seldon hook install claude-code` once those steps have run. They add
  `seldon drift   # N open drift item(s) …` when drift is left open.

**`--non-interactive` defaults** (in `--help`):
- flags win, then the existing config, then:
  - the path is `~/Seldon`;
  - the language comes from the locale;
  - all collectors are on and the watched paths are the defaults;
  - the harnesses are those in the config (none on a fresh machine);
  - git is on;
  - the first capture runs from now on, with no backfill and no baseline;
  - no Obsidian and no theme hook.

### Docs

- **`engine/hooks/README.md`:**
  - the theme hook's real install path and name;
  - a new section on the Omarchy-Agent kit (template dir, layout, copy
    rules, order, no-kit behaviour);
  - Claude Code hooks installed by `init`.
- **`docs/TESTING.md`:**
  - the `init.rs` rows;
  - why the shared helper passes `--no-capture`;
  - the real-host wizard run with scratch XDG dirs;
  - the pty recipe, with the `HOME` warning.

### Tests

**Lib: 87 → 90.** New `setup::tests`:
- `--since` parsing;
- the embedded script;
- `copy_tree` keeps existing files and modes and skips symlinks.

**`tests/init.rs`: 15 → 28.** 13 new tests in `setup::`:
- `init_runs_the_first_capture`:
  - all six collectors ran;
  - no stale text;
  - `cursors.json` and `index.json` exist;
  - a second capture writes 0.
- `no_capture_skips_it_and_keeps_the_next_step`.
- `backfill_opens_drift_and_the_baseline_dismisses_it`, using the
  fixture `pacman.log` with `--since 2026-08-01`:
  - without the baseline, open drift > 0 and the `seldon drift` step is
    listed;
  - with it, the baseline items equal the earlier open count, and
    `drift --json` reports 0/0;
  - there is one dismissed resolution per member, each with a unique
    `refersTo`, actor human and source seldon;
  - git log is `first capture and pre-Seldon baseline` /
    `init logbook`, and the tree is clean.
- `since_baseline_and_no_capture_are_checked_first`: exit 1, the flag
  named in the error, nothing created.
- `claude_code_hooks_are_installed_idempotently`:
  - three hooks in place;
  - a later `seldon hook install claude-code` adds 0, finds 3 present and
    leaves the bytes unchanged;
  - the settings are in the first commit.
- `omarchy_agent_kit_is_copied_and_claude_code_merged_into_it`:
  - copied list;
  - the guard is still executable;
  - `PreToolUse` = [guard, seldon].
- `omarchy_agent_without_a_kit_says_what_it_would_do`.
- Theme hook, against a recording `omarchy` stub on PATH:
  - `theme_hook_is_installed_once_on_opt_in`: exactly one
    `hook install theme-set <state>/hooks/seldon-theme-set.sh`; the script
    is byte-equal to `engine/hooks/theme-set.sh`, mode 755;
  - `theme_hook_is_never_installed_without_opt_in`: no `hook` call, no
    script written;
  - `theme_hook_failure_is_reported_with_the_fix`;
  - `theme_hook_already_there_is_not_installed_again`.
- `templates_have_english_keys_and_headings_in_every_language`:
  - the en and de skeletons are equal;
  - the skeleton is the frontmatter keys, `#` headings, fence names and
    table headers of all 19 template files;
  - both equal the new golden file `tests/golden/init-skeleton.txt`.
- `templates_are_written_as_rendered_with_prose_per_language`:
  - each written file equals `render(template)`;
  - de says "Logbuch", en says "logbook";
  - the required `AGENTS.md` rules are present;
  - prose differs per language.

**Existing tests adjusted:**
- `init::german_templates_and_obsidian` now expects the installed
  `.claude/settings.json` instead of the next step.
- `common::Env::init_logbook_at`, `collectors_user.rs::Capture::new` and
  `idempotency.rs::Cli::new` pass `--no-capture`. These tests build the
  machine state after `init` and need their own first capture as the
  baseline. Without the flag, `attribution.rs::end_to_end`,
  `collectors_user::capture::emits_the_expected_events_once` and two
  idempotency tests failed (`config-add` instead of the silent first
  state).
- WP-003's round-trip tests are untouched and green.

## Not done

- **SPEC-ENGINE edits.** These are the orchestrator's; see the list below.
- **No command to redo a step later.** The kit copy, the theme hook and the
  baseline have no standalone command, such as `seldon hook install
  omarchy-agent|theme-set` or `seldon drift dismiss --all --reason …`.
  The README gives the manual kit copy.
- **The interactive path has no automated test.** It was smoke-tested in a
  pty (below); dialoguer needs a terminal.
- **The theme hook never ran against the real `omarchy`.** That would be a
  red-zone write on the dev host, and the guard blocks it. The test host
  or the operator can verify it.
- **The `DECISIONS.md` table and the `system/*.md` fences have no producer
  yet** (dossier / decisions index). The templates describe the spec ("the
  table between the fences is generated by Seldon").
- **`capture.git` can over-report.** It says `committed: true` with
  "seldon: first capture" even when nothing was staged. This is the same
  `autocommit` semantics as `drift`/`log`, and minor.
- **The fixture logbook still uses the old German headings.**
  `fixtures/logbook/` has `# Entscheidungen` and `## Was hier nicht passieren
  soll`. No test ties it to the templates. WP-016 owns `fixtures/` if
  alignment is wanted.

## Verified by

- `just check`: fmt, clippy `-D warnings`, all engine tests, schema
  validation, plugin validate, qmllint, plugin tests (419 panel-view) →
  `check: ok`.
- **Real host, scratch XDG dirs** (`XDG_CONFIG_HOME`/`XDG_STATE_HOME`/
  `XDG_DATA_HOME` under the scratchpad, `--path` there; recipe in
  TESTING.md).
  - `init --language de --harness claude-code --harness omarchy-agent
    --since <today−7d>` printed: `First capture: 1230 event(s) since
    2026-09-24T00:00:00+02:00; degraded: snapper …; 22 open drift item(s),
    22 crisis`, plus `Harness omarchy-agent: no kit at …; nothing copied …`
    and `Harness claude-code: .claude/settings.json: 3 hook(s) added`. It
    took 0.69 s.
  - With `--baseline`: `capture.baseline {"items":22,"events":1230}`,
    `openDrift 0`, and `drift --json` 0/0. A following `capture --all`
    wrote 0. Git log is `seldon: first capture and pre-Seldon baseline`,
    `seldon: init logbook`.
  - `ls ~/.config/seldon` afterwards → absent. That was before the pty
    incident above.
- **pty smoke run** (`script`, `--path` given, `SELDON_OMARCHY` stub; XDG
  exported). It went through all prompts. With a 3-day backfill the
  wizard asked "The backfill opened 22 drift item(s) (22 crisis) … Mark
  them as the pre-Seldon baseline?"; yes → `22 drift item(s) (1230
  event(s)) dismissed …; 0 open drift item(s)`.
  - A third run, answering yes to the theme hook, is the one that leaked
    to the real home (incident above).
  - That run showed the hook being called once with the state-dir script.

## Learned (in `memory/`)

- **rust-notes:**
  - one command calling another in-process (pin the logbook, release the
    flock);
  - bulk resolutions through `reconcile`;
  - `include_str!` outside `src/`;
  - `copy_tree` and symlinks/modes;
  - clap `requires`/`conflicts_with`;
  - dialoguer `validate_with` typing;
  - the skeleton snapshot.
- **pitfalls:**
  - `HOME` through `script`, the incident;
  - `--no-capture` for tests that build state after `init`;
  - `pacman.log` ending at 17:04 on the test day;
  - backfill size, which doubles the ledger;
  - EPIPE panic with `| head`.

## Decisions needed

1. **The host leftovers above.** The operator should remove
   `~/.config/seldon` and `~/.local/state/seldon` (the guard blocked me).
2. **Baseline granularity.** The WP says "one `dismissed` resolution per
   open item". I wrote one line per open *member*, the same fan-out as
   `seldon drift dismiss` (ADR-0013 §4, `meta.txId` on groups). A
   one-line-per-item reading would leave members open. Please confirm.
3. **Installed hook name.** It is `seldon-theme-set.sh`, not WP-005's
   `theme-set.sh`. `omarchy hook install` keeps the base name and `cp`s
   over an existing file, so `theme-set.sh` could replace a user's own
   hook. Please confirm.
4. **Kit template dir.** It is `${XDG_DATA_HOME:-~/.local/share}/seldon/harness/omarchy-agent/`,
   overridable with `SELDON_OMARCHY_AGENT_KIT`. Packaging (WP-040) might
   add `/usr/share/seldon/harness/omarchy-agent/` as a fallback. Please
   confirm.
5. **`--since` without `--baseline`.** Non-interactively it leaves the
   drift open (an explicit opt-in, and the next steps say `seldon drift`).
   The alternative is baseline-on-by-default with `--since`. The
   interactive default is yes.
6. **English headings in German logbooks.** These are as the WP asks. They
   change how a German `PROJECT.md`/`DECISIONS.md` looks, and the fixture
   logbook (WP-016) still has the German ones.

## SPEC-ENGINE edits for the orchestrator

- **§3, `init` synopsis:**
  `seldon init [--path DIR] [--non-interactive] [--language de|en] [--obsidian]
  [--harness claude-code|omarchy-agent]… [--git/--no-git] [--since TS [--baseline]]
  [--no-capture] [--theme-hook]`.
  - `--since` takes YYYY-MM-DD (local midnight) or RFC 3339.
  - `--baseline` requires `--since`; `--no-capture` conflicts with it.
- **§3, add the `init --json` shape:**
  `{logbook, config, machineId, language, files, obsidian, collectors,
  watchPaths, harnesses, harnessSetup:{<name>:{…}}, git, snapper,
  capture:{ran, since, written, files, collectors, sinceIgnored, openDrift,
  crisis, baseline:{reason, items, events}|null, git} | {ran:false,
  reason|error}, themeHook:{requested, installed, already?, script?, hook?,
  error?, fix?}, nextSteps}`.
- **§9, step list:**
  - The wizard asks: path → language → Obsidian → collectors → watched
    paths → harnesses → theme hook → git → backfill.
  - Then `init` runs: layout + harness files → git init + first commit →
    `capture --all [--since]` → pre-Seldon baseline (flag, or asked with
    the item count) → theme hook (opt-in) → commit `seldon: first
    capture[ and pre-Seldon baseline]` → next steps.
  - A failure after the layout is reported, never fatal.
- **§9, `--non-interactive` defaults:** the list under "Wizard" above.
- **§9, baseline:**
  - one `dismissed` resolution per open drift member, reason `pre-Seldon
    baseline`, actor `human`, `meta.txId` on groups;
  - one ledger write, every open item (not capped);
  - the events stay.
- **§9, harnesses:**
  - `claude-code` = §8's merge into `<logbook>/.claude/settings.json`;
  - `omarchy-agent` copies `${XDG_DATA_HOME:-~/.local/share}/seldon/harness/omarchy-agent/`
    (or `$SELDON_OMARCHY_AGENT_KIT`) into `<logbook>/.claude/`:
    - existing files are kept, symlinks skipped, modes kept;
    - the kit runs before claude-code;
    - without the dir, `init` reports what it would copy.
- **§9, theme hook:**
  - opt-in only;
  - the script goes to `$XDG_STATE_HOME/seldon/hooks/seldon-theme-set.sh`;
  - `omarchy hook install theme-set <script>` → `~/.config/omarchy/hooks/theme-set.d/seldon-theme-set.sh`;
  - skipped when that file exists;
  - a failure gives the manual command as a next step.
- **§9, templates paragraph:**
  - add `STATUS.md` and `.seldon/templates/{case,decision}.md`;
  - "English keys and headings in every language, prose per language
    (ADR-0007); `engine/tests/golden/init-skeleton.txt` pins the
    skeleton".
- **§8, last sentences of the hooks paragraph:** "`seldon init --harness
  claude-code` runs the same merge into the new logbook (in its first
  commit)".

## Touched outside WP scope

- **`engine/src/commands/hook.rs`:** the merge was split into
  `pub fn merge_claude_hooks` (+ `Merged`). `hook install` behaves the
  same, and its tests are unchanged and green.
- **`engine/src/config.rs`:** `HARNESSES` gained `omarchy-agent`.
- **`engine/src/main.rs`:** the init flags.
- **`engine/src/commands/mod.rs`:** `pub mod setup`.
- **`engine/tests/common/mod.rs`, `collectors_user.rs`, `idempotency.rs`:**
  `--no-capture` on their `init` calls, as explained under Tests.
- **The host:** the leftover `~/.config/seldon` and `~/.local/state/seldon`
  described at the top.
