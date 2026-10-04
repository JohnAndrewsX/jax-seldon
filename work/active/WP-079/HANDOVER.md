# WP-079 HANDOVER

Branch `wp/079-review`, worktree `wt/WP-079`. Commits on top of `2c1d7ec`:

- `cf78e68` decisions: ADR-0026 snapper read grant, supersedes ADR-0011
- `f5a2d68` engine: snapper fix is a read grant on /.snapshots; doctor prints the revert of the old opt-in
- `e813f63` plugin: snapper banner offers the read grant on /.snapshots
- `fbc65b0` docs: snapper read grant and the revert of the old opt-in in SPEC-ENGINE, user guides (en), TESTING, packaging
- `d1808a8` docs(de): snapper read grant and the revert of the old opt-in, stamped at fbc65b0
- plus the commit with this handover and the pitfalls entry

## Done

1. **ADR-0026** (`decisions/ADR-0026-snapper-read-grant.md`, accepted,
   2026-10-04). It replaces ADR-0011 as a whole and restates the parts
   that stay: degraded by default, one printed command, never run by
   Seldon, the banner's *Run in terminal* / *Copy* / *Check again*. The fix
   is `sudo setfacl -m u:$USER:rx /.snapshots`, and the engine reads the
   info files (WP-060). ADR-0011 got `superseded by ADR-0026` in its status
   line plus a one-line pointer. DECISIONS.md: ADR-0011 is marked
   superseded, and there is a new ADR-0026 row.
2. **Engine constants.**
   - `SNAPPER_FIX` is the read grant.
   - `SNAPPER_FIX_GRANTS`: "The fix grants your user read access to the
     snapshot directory listing and the snapshot info files (files inside
     a snapshot keep their own permissions), nothing else: no snapshot
     creation, change or deletion."
   - `NO_PERMISSIONS`: "snapper: No permissions. This user can neither
     list the snapshots nor read the snapshot directory; `seldon doctor`
     prints the read grant."
   - The degraded doctor row now says "… until you grant your user read
     access to the snapshot directory once (ADR-0026)".
   - `SNAPPER_LIST_GRANTS` is unchanged. It is still true when listing
     works.
3. **Revert hint.**
   - `collectors/snapper.rs` has three new items:
     - `get_config_command` runs `--jsonout -c root get-config`. It shares
       one private builder with `list_command`: `LC_ALL=C`, no `LANGUAGE`.
     - `REVERT_OPT_IN` is `sudo snapper -c root set-config ALLOW_USERS=""`.
     - `lists_current_user` reads `USER`, else `LOGNAME`, and runs
       `get-config` only then. `allow_users` parses the flat JSON object;
       the names are split at blanks and matched whole.
   - `check_snapper` runs `lists_current_user` only after `snapper list`
     succeeded. When the user is listed, the row stays `ok`. Its message
     adds the old opt-in and that the revert empties `ALLOW_USERS`. Its
     fix is `REVERT_OPT_IN && SNAPPER_FIX`: the revert first, then the
     grant.
   - In `doctor.rs` I changed only the two constants and `check_snapper`
     (coordination with WP-081). The new items live in `snapper.rs`.
4. **init.** It prints the fix of any snapper row that has one:
   - degraded: `… # optional: snapshots in the timeline (ADR-0026)`
   - ok with the revert: `… # recommended: a read grant instead of the
     snapper opt-in (ADR-0026)`

   Without that, a listed user would read "revert it" in the Snapper line
   with no command anywhere.
5. **Plugin.**
   - `SNAPPER_FIX_COMMAND` and `SNAPPER_FIX_GRANTS` are the new line and
     text. The plugin text says "The command below grants …", worded the
     same as the engine's.
   - Comments in Model.js, Panel.qml and Service.qml now point to
     ADR-0026.
   - README: the States row and the five-constants list.
   - SPEC-PLUGIN: the banner paragraph.
6. **Fixture.**
   - The `snapper-degraded` overlay in `scripts/validate-fixtures.py`
     carries the new `NO_PERMISSIONS`.
   - I regenerated it with `--write-index`. Only
     `index-variants/snapper-degraded.json` changed.
   - `engine/tests/index.rs` now uses the constant. It is still compared
     with the variant file.
7. **Docs.**
   - SPEC-ENGINE §3: an `ok` row has a fix only for the old opt-in.
   - SPEC-ENGINE §4: the read grant, `NO_PERMISSIONS`, the revert hint,
     `get-config` in the C locale.
   - The capture JSON line now points to ADR-0026.
   - User guides 01, 06 and 10 (en, de). Guide 10 has a new section,
     "doctor suggests reverting the snapper opt-in".
   - TESTING.md: the doctor and init rows, and manual step 3.
   - fixtures/README.md: the variant row.
   - CHANGELOG: Engine, Plugin, and "Packaging and docs".
   - The de pages are re-stamped at `fbc65b0` in their own commit.
8. **Tests.**
   - Every test that asserted the old command now asserts the new one:
     doctor.rs (four places), init.rs, index.rs, model.test.js,
     panel-view.sh case 4 and service-states.sh 14b.
   - New: `doctor::snapper_revert_hint_only_for_a_listed_user`.
     - The stub answers `get-config` only with `LC_ALL=C` and logs every
       call.
     - These get the revert: a listed user, by `USER` (alice) and by
       `LOGNAME`, and bob under `LANG=de_DE.UTF-8`.
     - These get no fix: a user not listed (carol), a partial name
       (alic), no user, a refused `get-config` (the `ALLOW_GROUPS` case)
       and a missing snapper.
     - The call log holds only `--jsonout list` and
       `--jsonout -c root get-config`.
   - New: `init::a_listed_user_gets_the_revert_step`.
   - New unit test: `get_config_command_runs_in_the_c_locale`, which also
     covers the `allow_users` parse.

## Mutants (each run alone; file restored from the pre-mutant text, `touch`ed, compared with `git show HEAD:`)

| # | Claim | Mutant | Killed by |
|---|---|---|---|
| M1 | doctor prints the read grant | `SNAPPER_FIX` = old `set-config` line | doctor `green_after_init…`, `human_output…`, `snapper_revert…` |
| M2 | the grants text | old wording | doctor `green_after_init_with_snapper_degraded` |
| M3 | `NO_PERMISSIONS` points at the grant | old message | index `snapper_degraded_equals_the_variant` |
| M4a | revert only for a listed user | `lists_current_user` always true (when a user is known) | doctor `snapper_revert…` (carol), init `a_listed_user…` |
| M4b | revert for a listed user | always false | doctor `snapper_revert…`, init `a_listed_user…` |
| M5 | a name is matched whole | substring match on stdout | doctor `snapper_revert…` (alic) |
| M6 | `get-config` in the C locale | `get_config_command` without `LC_ALL`/`LANGUAGE` | doctor `snapper_revert…` (stub's German branch); alone in the unit test `get_config_command_runs_in_the_c_locale` |
| M7 | revert first, then grant | order swapped | doctor `snapper_revert…`, init `a_listed_user…` |
| M8 | init prints the revert step | init back to "degraded only" | init `a_listed_user_gets_the_revert_step` |
| M9 | `LOGNAME` fallback | only `USER` | doctor `snapper_revert…` |
| M10 | doctor runs only read-only queries | an extra `set-config` call before `get-config` | doctor `snapper_revert…` (call log) |
| M11 | plugin command | old `SNAPPER_FIX_COMMAND` | model.test `snapperBanner…`, `plugin/README.md States…` |
| M12 | plugin grants text | old wording | model.test `snapperBanner…` |

After the last mutant I rebuilt the engine (`cargo build`) and confirmed
that the tree is clean.

## How verified

- `cargo fmt`, `cargo clippy --all-targets -D warnings` and
  `cargo test --no-fail-fast` all pass (31 binaries).
- `node tests/plugin/model.test.js`: 87 passed.
- `tests/plugin/service-states.sh`: 275 passed.
- `tests/plugin/panel-view.sh`: 743 passed.
- `scripts/docs-check.sh`: 0. `packaging/check-srcinfo.sh`: ok.
- `just check`: see the section below.
- `grep -rn ALLOW_USERS engine/src plugin docs` shows only the revert
  hint:
  - `snapper.rs` (`REVERT_OPT_IN`, `lists_current_user`, `allow_users`
    and their tests)
  - the doctor message
  - SPEC-ENGINE §4
  - the guide 10 section (en, de)
  - the TESTING rows that describe the new tests

  Elsewhere `ALLOW_USERS` is only in the ADRs, in CHANGELOG history and
  STATUS.md, in `work/` and `memory/`, and in
  `schema/external/snapper-list.schema.json` (see open questions).
- I ran no `sudo`, no `setfacl` and no `snapper set-config`. On the host I
  ran two read-only commands to learn the formats:
  - `snapper --jsonout -c root get-config` prints a flat JSON object of
    strings.
  - `stat` and `getfacl` on `/.snapshots`: `root:root 0750`, the snapshot
    directories are `0755`, and `info.xml` is `0644`.

  Nothing from the host is in the repo; the tests use `alice`, `bob` and
  `carol`.

## just check

(filled in below after the run)

## Not done

- doctor.rs line 6 (module doc) still says "ADR-0011". It is outside the
  snapper check, so I left it for after the WP-081 merge.
- `schema/external/snapper-list.schema.json` says in its description
  "Not verified on the dev host (needs ALLOW_USERS, ADR-0011)". It is a
  third-party format, I did not touch `schema/`, and the sentence is
  history. Follow-up: the operator's host can list today, so the shape
  could be verified and that sentence dropped.
- `fixtures/README.md` line 205 ("not verified on the dev host
  (ADR-0011)") is the same history; I left it.

## Open questions

1. **"nothing else" vs. snapshot contents.** The grant `u:$USER:rx` on
   `/.snapshots` opens the way into every snapshot. Inside, each file
   keeps its own mode, so the user can read in an old snapshot what was
   readable to them when it was taken. A file made private later stays
   readable there. Before the grant, `0750` hid all of it. I kept the
   WP's wording and added "(files inside a snapshot keep their own
   permissions)". ADR-0026 states this under Consequences, and guide 10
   says it in plain words. Is that enough, or should the operator decide
   on a narrower grant? Reading `<n>/info.xml` needs `x` on `/.snapshots`,
   and that opens `<n>/snapshot` too, so a narrower grant would need a
   different design, for example a copy of the info files made by root.
2. **The revert empties `ALLOW_USERS`.** The command comes from the WP.
   On a config that also lists other users, it drops them. The doctor
   message says "this empties ALLOW_USERS", and guide 10 says to add
   others back. WP-060 had suggested "remove only this user". I did not
   build that: the printed command would then contain names from the
   config.
3. **An `ok` row with a `fix`** is new in doctor's JSON (SPEC-ENGINE §3
   now says so). The plugin does not read doctor, so nothing breaks.
   Flagging it for the reviewer.

## Touched outside the WP's input list

- `docs/user/{en,de}/01-getting-started.md` (sample doctor and init
  output)
- `docs/SPEC-PLUGIN.md` (banner paragraph)
- `docs/TESTING.md`
- `fixtures/README.md`
- `scripts/validate-fixtures.py` and `fixtures/index-variants/snapper-degraded.json`
  (the overlay)
- `packaging/PKGBUILD` and `.SRCINFO` (optdepends: "needs read access to
  /.snapshots")
- Comment-only ADR pointers: `engine/src/collectors/mod.rs`,
  `engine/src/commands/capture.rs`, `engine/tests/common/mod.rs`,
  `engine/tests/collectors.rs`, `plugin/Panel.qml`, `plugin/Service.qml`
- Merge risk with WP-081:
  - `docs/TESTING.md` line 73 (the doctor row is one long line)
  - the CHANGELOG section ends
