# WP-089 HANDOVER

Branch `wp/089-environment`, worktree `wt/WP-089`. Commits on top of
`d52e96d`:

- `1a30c0b` engine: default OMARCHY_PATH for Omarchy's programs
- `4402549` engine: watch desktop entries, exclude mimeinfo.cache
- `cfc0265` engine: tests for OMARCHY_PATH and desktop entries
- `4ee9ecc` docs: OMARCHY_PATH default and desktop entries
- `8dc80d4` memory: WP-089 pitfalls
- this handover

## Done

(a) OMARCHY_PATH
- `sys.rs`: `OMARCHY_PATH_DEFAULT` (`/usr/share/omarchy`, the value
  query.rs used before), `omarchy_path()` (the engine's value, or the
  default when unset or empty) and `omarchy_command(program, args)`. The
  last one builds the `Command` and adds `OMARCHY_PATH=<default>` through
  `Command::env` only when the engine's own value is unset or empty. A set
  value is passed on unchanged, like PATH. No `set_var` anywhere. Both
  functions share one rule (`omarchy_path_to_set`).
- Routed through it:
  - plugins.rs `list` (`omarchy plugin list --json`, which dossier uses
    too);
  - plugins.rs `catalog` (`omarchy plugin catalog`, which also needs the
    variable because it walks `$OMARCHY_PATH/shell/plugins`);
  - omarchy.rs `current_version` (`omarchy-version`, which dossier uses
    too; the package-query fallback stays plain);
  - the doctor omarchy probe;
  - query.rs: the package-list default now comes from
    `sys::omarchy_path()`.
- `catalog` lost its unused `ctx` parameter (it called `ctx.run`; Ctx
  lives in collectors/mod.rs, WP-088's file, so it calls `sys` directly
  now).
- `seldon watch` runs no collectors: it only rebuilds `index.json`
  (commands/watch.rs header). No change was needed there.

(b) Desktop entries
- config.rs: `~/.local/share/applications` is the sixth entry of
  `DEFAULT_WATCH_PATHS`.
- collectors/config.rs: `MIME_CACHE` (`.local/share/applications/mimeinfo.cache`
  relative to `$HOME`) is added to `excluded` next to `Plugins::dir`, so
  it works the same way as the plugins dir. It is never hashed, never
  listed as skipped, and shows up in `scope.exclude`. It is excluded
  wherever the watch paths reach it, including under a user-added
  `~/.local/share`. The module header names it.
- A path that enters the scope records no addition. This is existing
  WP-069 rescope behaviour, now proven for this path. A `config.toml`
  without `watchPaths` picks up the new default. The next capture writes
  0 events and says `watch scope changed: 0 file(s) left it, 2 entered
  it`. A later edit is a `config-change`.

Docs
- SPEC-ENGINE §4: an omarchy bullet sentence on the OMARCHY_PATH
  default; the config bullet's default list and its exclusion; §2
  manifest wording ("excluded folders and files").
- Guide 06 (en/de): the example and the defaults list include the path,
  plus the `mimeinfo.cache` bullet. The new paragraph says that a
  `config.toml` the wizard wrote keeps its list, gives the line to add,
  and says that files already there record no addition. The `plugins`
  bullet now says ssh/cron works while the shell runs.
- CHANGELOG `[Unreleased] ### Engine`: two lines.

## Not done

- The live ssh check on the test host is the orchestrator's (brief).
  Suggested command. Scratch XDG dirs and a throw-away logbook, no
  `OMARCHY_PATH` in the environment. `<host>` is the alias from
  memory/local.md, `<bin>` is the WP-089 build on the host:

  ```
  ssh <host> 'd=$(mktemp -d); env -u OMARCHY_PATH HOME=$d XDG_CONFIG_HOME=$d/.config \
    XDG_STATE_HOME=$d/.local/state XDG_DATA_HOME=$d/.local/share \
    XDG_RUNTIME_DIR=/run/user/$(id -u) <bin> init --non-interactive --no-capture --path $d/lb \
    && env -u OMARCHY_PATH HOME=$d XDG_CONFIG_HOME=$d/.config XDG_STATE_HOME=$d/.local/state \
    XDG_DATA_HOME=$d/.local/share XDG_RUNTIME_DIR=/run/user/$(id -u) \
    <bin> capture --source plugins --json'
  ```

  Expected: the `plugins` collector has `ok: true`. With 0.1.3 the same
  call says "omarchy plugin list --json: OMARCHY_PATH is not set (it
  needs the running Omarchy shell)". `HOME` in a scratch dir means
  `~/.config/omarchy/plugins` is empty there. That is fine for this check,
  which is about the list call, not about versions.
- Cron without `XDG_RUNTIME_DIR`: `omarchy-shell` also needs it to reach
  the shell (`qs ipc`). An ssh login gets it from pam_systemd; a cron
  job may not. In that case `plugin list` still degrades, now with a
  different message. The WP only asks for OMARCHY_PATH; I did not default
  `XDG_RUNTIME_DIR`.
- `$OMARCHY_PATH/bin` is not added to PATH. Not needed on a package
  install: `/usr/bin/omarchy*` exist, and the `omarchy` dispatcher execs
  its own bin dir.
- `omarchy agent prompt` (agent launcher) and `omarchy-launch-editor`
  (`open`) are not routed. They are launches from the plugin (desktop
  session), in files outside this WP.
- The hook's `watchPaths` handling (hook.rs) has no exclusion list, for
  the plugins dir as well. An agent's Edit/Write of `mimeinfo.cache`
  would be recorded as a watched write (yellow). That stays as it is,
  same as for the plugins dir.

## Verified by

- `flock /tmp/seldon-check.lock just check`: exit 0, `check: ok`
  (bar-view 143 passed, 0 failed; real-home guard untouched). Ran on
  `4ee9ecc`. `8dc80d4` and this file only touch memory/ and work/.
- `cargo test --no-fail-fast`: all binaries green. `cargo clippy
  --all-targets -- -D warnings` and `cargo fmt` are clean.
- New file `engine/tests/environment.rs` (9 tests, separate from WP-088's
  capture/reset tests). The stubs `omarchy` and `omarchy-version` fail
  with "OMARCHY_PATH is not set" when the variable is empty, and log the
  value they got:
  - `omarchy_path::the_plugins_collector_runs_without_omarchy_path`
    (unset and empty; list and catalog both get `/usr/share/omarchy`)
  - `omarchy_path::a_set_omarchy_path_is_passed_on_unchanged`
  - `omarchy_path::the_omarchy_collector_reads_its_version_without_omarchy_path`
  - `omarchy_path::the_doctor_probe_runs_omarchy_version_without_omarchy_path`
  - `omarchy_path::the_dossier_queries_omarchy_without_omarchy_path`
  - `omarchy_path::the_dossier_reads_the_package_lists_below_omarchy_path`
  - `desktop_entries::the_defaults_watch_the_desktop_entries` (and
    `init` writes them)
  - `desktop_entries::a_desktop_entry_is_recorded_and_the_mime_cache_is_not`
  - `desktop_entries::the_desktop_entries_entering_the_scope_are_no_additions`
  - plus the unit test `sys::tests::omarchy_path_is_set_only_when_unset_or_empty`.
- Flake loop: `cargo test --test environment` 30×, 0 failures.
- Mutants (script: apply the mutant, run the named tests, restore with a
  fresh mtime; baseline green after the last one). All killed:
  - M1 `omarchy_command` never sets the variable → plugins, omarchy,
    doctor, dossier tests
  - M2 an empty value is trusted (`is_none()`) → plugins test (unset/empty
    loop)
  - M2u the same mutant → `sys::tests::omarchy_path_is_set_only_when_unset_or_empty`
  - M3 a set value is overridden → `a_set_omarchy_path_is_passed_on_unchanged`
  - M4 `plugin list` via plain `sys::run` → plugins and dossier tests
  - M5 `plugin catalog` via a plain Command → plugins test
  - M6 `omarchy-version` (collector) via a plain Command → omarchy
    collector test
  - M7 doctor probe via plain `sys::run` → doctor test
  - M8 query.rs package lists hard-code `/usr/share/omarchy/install` →
    package-lists test
  - M9 `omarchy_path()` always returns the default → package-lists test
  - M10 the path removed from `DEFAULT_WATCH_PATHS` → all three
    desktop_entries tests
  - M11 the `MIME_CACHE` exclusion removed → mime-cache test
  - M12 rescope no longer takes entering files (`base.files.extend`
    dropped) → scope-entry test

## Learned

Recorded in memory/pitfalls.md (§ WP-089):

- `plugin catalog` needs OMARCHY_PATH too.
- `omarchy-version` prints `dev` for any non-default OMARCHY_PATH.
- `omarchy-shell` also needs XDG_RUNTIME_DIR.
- manifest.json is flat (`current` is flattened).
- `dossier --section` takes group names, not fence names.
- A new exclusion re-scopes every manifest once, silently.
- Desktop entry names are user-chosen.

## Decisions needed

- Not blocking: desktop entry names become event subjects. A user-named
  web app can carry personal data in its file name; the dev host has
  one with an e-mail address. The SPEC-ENGINE §7 redaction rules have no
  e-mail rule, and redact.rs is outside this WP. `skipPaths` lets a user
  exclude such a file. Accept as is, or open a follow-up (an e-mail
  redaction rule, or a guide 06 note)? I did not decide this.

## Touched outside WP scope

- `CHANGELOG.md` and `docs/SPEC-ENGINE.md` (both listed under outputs).
- `memory/pitfalls.md` (Learned).
- No other files. capture.rs, reconcile.rs, collectors/mod.rs and
  redact.rs are untouched. The guard hook blocked nothing.
