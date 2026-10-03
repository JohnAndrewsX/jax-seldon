# WP-074 HANDOVER

Branch `wp/074-review`, worktree `wt/WP-074`, based on `e6ef88a` (main has
not moved since then). No PR, no push.

## Done

- **F-542 (language).** `Config.language` is `Option<Language>`
  (`skip_serializing_if = "Option::is_none"`), so a `config.toml` without
  the key, for example one with only `[redaction] patterns`, leaves the
  language to the locale: `--language` > config (when it has the key) >
  locale > `en`. `init` writes the chosen language into the file.
  **Choice: doc change, not "the key works after init".** Why: the
  language belongs to the logbook. It is stored in `.seldon/logbook.toml`
  and moves with the logbook to another machine, while `config.toml` is
  per machine. The templates `init` writes are already in that language,
  so a later switch would mix languages. Making the key work would also
  mean changing every prose writer (status, plan, journal, dossier,
  rebuild, import), and several of those files belong to the WPs running
  in parallel. 06-configuration (en + de) now says that the key sets the
  language of a *new* logbook and that an existing logbook keeps its own.
  SPEC-ENGINE §2 row and §9 precedence say the same.
- **F-543 (config before layout).** `init` saves `config.toml` after its
  checks and before `layout::create`. A save that fails writes nothing
  into the logbook folder, so the same `init` runs once the folder is
  writable again. `layout::create` writes the marker
  `.seldon/logbook.toml` last, so a layout that stops half-way is not
  taken for a logbook (by `init`, `doctor` or any command).
- **F-545 (`--no-git`).** `init` writes `config.git.autocommit =
  choices.git`: `false` for `--no-git` or "no" in the wizard, `true` when
  git is chosen. `doctor`'s git check is then `ok` ("not a repository;
  autocommit off"), without a fix. 06-configuration (en + de) documents
  it, including how to start using git later.
- **F-546 / WP-052 (theme hook under the lock).** New
  `setup::theme_hook_step(dirs, config, omarchy)`: it takes the state
  lock first, then writes the script, runs `omarchy hook install`,
  records the copy in `owned.json`, and only then drops the lock.
  `install_theme_hook` now takes `&Lock`, so a caller must hold the lock.
  `setup::record_own_writes` (which took a second lock after the write)
  is removed; `record_own_writes_under` is the only way to record. While
  another `seldon` holds the lock, the step writes nothing (no script, no
  hook, no record), reports "another seldon process holds the lock …;
  nothing was written" and offers no fix. `ThemeHook::fix()` now offers
  the manual `omarchy hook install` command only when the script exists.
  Nothing needs a commit: the hook and `owned.json` are outside the
  logbook. SPEC-ENGINE §9 has the sentence.
- CHANGELOG `[Unreleased]` › Engine: four bullets (appended).
- Commits: `0a9e829` (F-542), `1d0d119` (F-543, F-545), `bb71b9e`
  (F-546/WP-052), `1957e15` (spec + en guide), `e7f09f6` (de guide,
  stamped `@ 1957e15`), `18071be` (changelog), plus this handover.

## Tests (each one fails on a mutant, see "Verified by")

| Finding | Test |
|---|---|
| F-542 | `tests/init.rs` `init::a_config_without_language_leaves_it_to_the_locale`: a config with only `[redaction]`, `LANG=de_DE.UTF-8` → `de` logbook, `language = "de"` written, patterns kept; `language = "en"` in the config → `en` under the same locale. Unit: `config::tests::a_missing_language_key_is_no_value` |
| F-543 | `init::a_config_that_cannot_be_saved_leaves_nothing_init_refuses`: config folder 0555 → exit 2, logbook folder absent or empty, no config; 0755 → the same `init` exits 0 and the config names the logbook (skips itself when the mode is ignored, as for root). `init::a_layout_that_stops_half_way_is_no_logbook`: a folder where `DECISIONS.md` goes → `layout::create` fails, `is_initialised` false |
| F-545 | `init::no_git_is_stored_and_doctor_reads_it_as_chosen`: `--no-git` → `[git] autocommit = false`; `doctor --path` git check `ok`, no `fix` |
| F-546 / WP-052 | `tests/own_writes.rs` `a_capture_during_the_theme_hook_install_finds_the_lock_held`: the `omarchy` stub copies the hook and then runs a real `seldon --json capture --all`, as the plugin timer could. That capture must exit 4; then `ownWrites == [hook]`, the next capture explains the `config-add`, drift 0. `a_held_lock_stops_init_with_the_theme_hook_before_any_write`: the lock is held → exit 4, JSON error code 4, the whole temp tree byte-identical, no `owned.json`, no hook, `omarchy` never ran. Unit: `commands::setup::tests::the_theme_hook_step_writes_nothing_while_the_lock_is_held`: lock held → no script, no hook, no `owned.json`, error names the lock, no fix |

## Verified by

Mutant runs (scratchpad script; each file restored with `cp` + `utime`
and checked by content; the binary rebuilt afterwards). Assertion lines
only:

```
=== MUTANT: F-542 config language ignored when absent (Option collapsed to en)
    (defaults: .or(existing.map(|c| c.language.unwrap_or_default())))
    test init::a_config_without_language_leaves_it_to_the_locale ... FAILED
    panicked at tests/init.rs:270:9: assertion `left == right` failed  left: En  right: De
=== MUTANT: F-543 config saved after the layout
    test init::a_config_that_cannot_be_saved_leaves_nothing_init_refuses ... FAILED
    panicked at tests/init.rs:307:9   (assert "nothing written into the logbook folder":
                                       the failed first init left the marked layout)
=== MUTANT: F-543 marker written first
    test init::a_layout_that_stops_half_way_is_no_logbook ... FAILED
    assertion failed: !Logbook::is_initialised(&root)
=== MUTANT: F-545 --no-git choice not stored (line removed)
    test init::no_git_is_stored_and_doctor_reads_it_as_chosen ... FAILED
    assertion `left == right` failed  left: Some(true)  right: Some(false)
=== MUTANT: F-546 lock after write (theme_hook_step: install under a dummy
    lock, drop it, then take the state lock for the record — the shape of
    the old init: install outside the lock, record under a new one)
    test commands::setup::tests::the_theme_hook_step_writes_nothing_while_the_lock_is_held ... FAILED
    panicked at src/commands/setup.rs:661:9: the script was written
    test a_held_lock_stops_init_with_the_theme_hook_before_any_write ... ok
    test a_capture_during_the_theme_hook_install_finds_the_lock_held ... FAILED
    assertion `left == right` failed: the capture during the install:
      {... "explainedOwn":0, "files":["ledger/2026-10.jsonl"], "written":1}
      left: "0"  right: "4"
```

In the last mutant the capture during the install wrote one unexplained
`config-add`, which is the race the finding describes. The held-lock test
passes on that mutant on purpose: it proves "exit 4, nothing written" at
init's start, where `init` takes its first lock. A lock held from outside
cannot reach the theme hook step, so the race test and the unit test are
the ones that catch the mutant (see "Learned").

- `cargo test --no-fail-fast` before the commits: 567 passed, 0 failed.
- `just check` once at the end: **exit 0** (fmt, clippy `-D warnings`,
  tests, watch feature, packaging, install script, schema
  `validate-fixtures: ok`, `docs-check: ok (393 links, 14 translated
  pages …)`, `plugin-validate: ok`, qmllint, plugin tests).
- No manual engine run: every scenario is a CLI test under `common::Env`
  (temp `HOME`, `SELDON_TEST_GUARD`), with the theme hook written only
  into the temp home. Nothing touched the real `~/Seldon`,
  `~/.config/seldon` or `~/.config/omarchy`.
- Removed my own leftovers `/tmp/seldon-theme-hook-<pid>` (two, from the
  mutant runs).

## Not done

- `docs/TESTING.md` rows for `tests/init.rs` and `tests/own_writes.rs`
  do not list the new tests. The file is not in this WP's list; the
  orchestrator can add a line or hand it to the next docs WP.
- If `layout::create` itself fails (disk full) after the config was
  saved, `config.toml` names the new, unmarked folder and `init` refuses
  it as "not empty" until it is removed. The previous config is not
  restored (rare; the WP asked for the save order or the marker order,
  and both are done).
- No way to switch an existing logbook's language (see Decisions 2).

## Learned (appended to memory/pitfalls.md)

- A lock held by the test cannot reach a late step of `init`. Race the
  step from the inside instead: a stub runs a real `seldon capture`, with
  the test's `PATH` saved before the stub's `PATH=/usr/bin:/bin`.
- `#[serde(default)]` turns a missing key into the field's default. A key
  whose absence must fall back to something else has to be an `Option`.
- "Save the config first" and "write the marker last" fix different
  failures. Only the first lets the same `init` run again.
- A unit test that panics leaves its temp folder behind; clean up your
  own after mutant runs.

## Decisions needed

1. **Lock held at the theme hook step → `init` exits 0, not 4.** At that
   point the logbook, config, git and first capture are done, and §9
   says "a failure after the layout is reported, never fatal". So the
   step is reported as failed ("…holds the lock…; nothing was written",
   no fix), and the rest of the report stays. "Exit 4 with nothing
   written" applies where `init` takes its first lock (tested). If the
   orchestrator wants exit 4 there too (full report, exit code 4), that
   is a one-line change to `Output`'s code.
2. **Switching an existing logbook's language.** The guide now says the
   key only affects a new logbook. A user who wants to switch has no
   documented way: hand-editing `.seldon/logbook.toml` conflicts with
   07-the-logbook ("`.seldon/` — the engine only"). Possible follow-up:
   a small command, or a documented exception. Not urgent.
3. **Choosing git writes `autocommit = true`**, also over an existing
   `false`, so the stored value always matches the init choice. Keeping
   an existing `false` would let one old `--no-git` silently turn off
   commits for every later logbook. Say so if the other rule is wanted.

## Touched outside WP scope

None. Files: `engine/src/commands/{init,setup}.rs`, `engine/src/config.rs`
(`language` only), `engine/src/logbook/layout.rs` (marker order only),
`engine/tests/{init,own_writes}.rs`, `docs/SPEC-ENGINE.md` (§2
config.toml row: `language`; §9 paragraph), `docs/user/{en,de}/06-configuration.md`,
`CHANGELOG.md` (appended), `memory/pitfalls.md` (appended), this file.
`engine/tests/common/mod.rs` unchanged (the tree helper is local to
`own_writes.rs`).
