# WP-053 HANDOVER

```
WP-053 HANDOVER
Done:
- engine/src/collectors/snapper.rs: `list_command(program)` is the one
  place a snapper command is built: `program --jsonout list` with
  `LC_ALL=C` and `LANGUAGE` removed. `run_list(program, timeout)` runs it;
  `is_no_permissions(stderr)` is the single English match.
- The collector (`run_list(&ctx.sources.snapper, RUN_TIMEOUT)`) and
  `doctor::check_snapper` (`run_list("snapper", PROBE_TIMEOUT)`) use it;
  `init` gets it through `doctor::check_snapper` (its "Snapper:" line and
  the `set-config` hint) and through the first capture (collector).
  A grep finds no other place that spawns snapper.
- engine/src/sys.rs: `run` is split into `run` + `run_command(Command,
  timeout)` (same stdio, ETXTBSY retry, timeout), so a caller can add
  environment. No change in behaviour for the other callers.
- Tests: `common::Snapper::NoPermissionsLocalized`, a stub that prints
  `Keine Berechtigungen.` unless `LC_ALL` is `C`, then `No permissions.`.
  `tests/doctor.rs::doctor::snapper_permission_error_is_recognised_in_any_locale`
  runs `init --non-interactive` and `doctor` with `LANG=de_DE.UTF-8` and
  `LANGUAGE=de` and asserts: init prints `Snapper: degraded — No
  permissions.` and `<SNAPPER_FIX>   # optional: snapshots in the timeline
  (ADR-0011)`; `cursors.json` → `collectors.snapper` is `ok: false`,
  `message == NO_PERMISSIONS`, `fix == SNAPPER_FIX`; doctor prints
  `fix: <SNAPPER_FIX>`; "Keine" appears nowhere. Unit test
  `collectors::snapper::tests::list_command_runs_in_the_c_locale` pins the
  argv `--jsonout list`, `LC_ALL=C` and the removed `LANGUAGE`.
- Docs: SPEC-ENGINE §4 snapper bullet ("snapper is run with `LC_ALL=C`
  (and without `LANGUAGE`); its messages are matched in English …");
  CHANGELOG [Unreleased] `### Engine` line ending "(fixes #1)";
  TESTING.md (doctor.rs row, the stub list under Isolation).
- memory/pitfalls.md: one bullet (below, Learned).
Not done:
- Nothing from the WP's outputs. The doctor probe still runs the bare
  `snapper` (not `SELDON_SNAPPER`), as before; out of scope.
Verified by:
- `just check` → exit 0, "check: ok" (cargo tests: 855 passed, 0 failed;
  clippy -D warnings, fmt, watch feature, packaging, install, schema,
  docs-check, plugin validate, qmllint, plugin-test all green).
- `--jsonout` paths unchanged: the argv is still exactly
  `["--jsonout", "list"]` (unit test above); parsing untouched.
- Host, read-only: `snapper --jsonout list` and `LC_ALL=C snapper
  --jsonout list` give byte-identical output on this machine (9 entries,
  no non-ASCII description). Nothing else ran against the host snapper.
- Mutant: `.env("LC_ALL", "C")` deleted from `list_command`, then
  `cargo test --test doctor snapper_permission` (file restored after):

      test doctor::snapper_permission_error_is_recognised_in_any_locale ... FAILED
      thread '…' panicked at tests/doctor.rs:112:9:
      Logbook created at /tmp/seldon-test-env-…/logbook (machine <machine>, language de, 31 files).
      Config: ~/.config/seldon/config.toml
      Git: repository initialised, first commit "seldon: init logbook"
      Snapper: degraded — snapper failed: exit 1: Keine Berechtigungen.
      First capture: 0 event(s); degraded: snapper, plugins, theme (see seldon doctor); …
      Next steps:
        seldon doctor
      test result: FAILED. 0 passed; 1 failed

  That is the issue's symptom verbatim: no hint, the raw German message.
  With the helper restored the test passes.
Learned: Host programs translate their stderr; the test `Env` sets
  `LANG=C`, which hides it. Match stderr only under `LC_ALL=C`; a locale
  test overrides `LANG` on the command (memory/pitfalls.md).
Decisions needed: none. One note for the reviewer: `LC_ALL=C` (as the
  WP says) rather than `C.UTF-8`. snapper's JSON was identical here, but
  this host has no snapshot with a non-ASCII description, so that case is
  untested (making one is red zone). If it matters, `C.UTF-8` also gives
  English messages and exists on Arch's glibc.
Touched outside WP scope: none (engine/, docs/, CHANGELOG.md, memory/,
  this file; nothing in plugin/). The CHANGELOG [Unreleased] section may
  meet WP-054's plugin line at merge; both are additive.
```
