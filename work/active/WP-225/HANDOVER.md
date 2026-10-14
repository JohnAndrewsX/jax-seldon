# WP-225 — Handover

Branch `wp/225-edge-files` (off `next`), worktree `wt/WP-225`. Not pushed.

## What was done

**B9 — Omarchy-written files (ADR-0059, proposed).** A new evidence mark
and routine rule `omarchy-written`: a `config-add`/`config-change` whose
new hash equals a content one of Omarchy's commands writes to a file
Omarchy ships no copy of. The candidates are built from the trusted
`$OMARCHY_PATH` alone (same trust rule as `omarchy-default`: root-owned,
not group/world-writable, every directory on the way), never from the
user's file; the collector compares hashes it already has.

| File | Candidates | Omarchy command |
|---|---|---|
| `~/.config/omarchy/language` | `<code>\n` per key of `default/i18n/locales.json` | `omarchy language <code>` |
| `~/.config/uwsm/env.d/50-omarchy-language` | Omarchy's two-line text with `export LANG=<locale>` per locale of `locales.json` | same |
| `~/.config/omarchy/defaults/agent` | `<agent>\n` per entry of `bin/omarchy-default-agent`'s `# omarchy:args=[…]` | `omarchy default agent`, migration 1786719479 |
| `~/.config/omarchy/defaults/dictation` | `<backend>\n` per `bin/omarchy-install-dictation-<backend>` | `omarchy dictation use`, migration 1791479273 |

The rule sits beside `omarchy-default`, before the persistence paths, so
the session locale (under `~/.config/uwsm/**`) is routine only with
exactly Omarchy's content; a line more is still a crisis. The shipped
`dictation` copy stays `omarchy-default`. `ROUTINE_RULES` gains
`omarchy-written` (14 ids); SPEC-ENGINE §4/§5, the user guide's lists,
`scripts/validate-fixtures.py` updated. No hashes are taken that were
not taken before; no schema change.

**B10 — crash mutes.** `omarchy crash mute` and `omarchy toggle crash
capture` go through `omarchy-toggle`: empty flags, already routine
`toggle-flag` both ways (ADR-0037). `omarchy_copies()` mapping
`crash-ignore/<name>` to a non-existent copy is harmless (no copy, no
mark; the empty-hash row decides), so it was left as it is.

**Attribution (B9, B10).** `attribution::omarchy_writes`: an agent's
`omarchy language <code>`, `omarchy crash mute [--] <program> [action]`
(by base name), `omarchy toggle crash capture`, `omarchy-toggle <flag>`,
`omarchy default agent <agent>`, `omarchy dictation use <backend>` (and
each as its script) prove the files they write; read-only forms prove
nothing.

**B15.** `SKILL_DIRS` gains `.gemini/config/skills`; docs (SPEC, user
guide en/de) list it.

**B13.** `fixtures/logs/pacman-pkg-add.log`: `pacman -S --noconfirm
--needed -- <pkgs>` (two names + a dependency; `extra/btop`), described
in `fixtures/README.md`. The parser already handled `--`.

**B12.** The terminal scripts (`plugin/Model.js`) now end with status 1
after their red result line (was 0). On Omarchy edge the wrapper then
shows "Failed (exit code 1)!" instead of "Done!"; an earlier Omarchy
still shows "Done!" for both. `tests/plugin/terminal-scripts.sh` runs
the edge wrapper (DONE / FAILED 1) and the stable wrapper (DONE always);
`model.test.js` pins the new ending. SPEC-PLUGIN and the getting-started
guide (en/de) describe both prompts. This goes one step beyond "update
docs and tests": without the status the edge prompt would say "Done!"
under a red failure line. Reviewer: veto it if the WP meant docs only.

CHANGELOG `[Unreleased]` Highlights: three bullets.

## Tests (acceptance)

- One test per path, routine and attributed:
  `collector_hashes::omarchy_written_files_are_routine` (the four files
  routine `omarchy-written`, the shipped dictation copy `omarchy-default`,
  two mutes and the capture switch `toggle-flag` on and off, no open
  item; a second capture writes nothing; unknown code/agent attention,
  a session file with an extra line crisis; an untrusted tree gives no
  mark) and `attribution::rules::omarchy_commands_prove_the_files_they_write`
  (21 command/path pairs); class table rows in `index/class.rs`.
- Skills: `skills::installs_only_into_skill_folders_that_exist_and_creates_none`
  covers `~/.gemini/config/skills`.
- B13: `collectors::pacman_reads_omarchy_pkg_adds_double_dash_form`
  (four installs, explicit flags, `parse_command` targets, second run 0).

## How it was verified

- Engine unit and integration tests, fixture validator, plugin model
  and terminal-script tests: **fixture**, dev host (Omarchy edge), all
  inside the full check below.
- `flock /tmp/seldon-check.lock env SELDON_FULL_CHECK=1 just check`
  (dev host, Omarchy edge, Qt 6.12): **not green.** Every step through
  `plugin-validate` passes (fmt, clippy, engine tests incl. `--features
  watch`, packaging, install, deploy, guard, runtime dir, perf dirs,
  assumptions, schema, docs). It stops at `qmllint`: two
  `id-shadows-member` warnings, `plugin/components/desk/EventDetail.qml:480`
  (`form`) and `plugin/components/desk/ListColumn.qml:109` (`top`) —
  WP-224's two QML ids, not touched here.
- `just plugin-test` run on its own (**headless**): terminal-scripts 78/78,
  real-home-guard 79/79, check-tokens 14/14; `service-states` 364 passed,
  2 failed (`sheets-busy`, `sheets-rearm`: `NewDecisionForm.qml:36`
  "Unable to assign [undefined] to QColor", `Tone.qml` "Cannot read
  property … of undefined"). The same two fail on a clean checkout of
  `next` (cca07cfd, same host, same harness): they predate this branch,
  which changes no QML (only `plugin/Model.js`). Not fixed here; they need
  their own WP or belong with WP-224's edge work.
- `just check-rss` (required: `engine/src/index/` changed): peak 11524 kB
  < 12 MB, **fixture**. `just check-perf`: all budgets met, **fixture**.
- Omarchy's real scripts were read (not run) on the dev host to take the
  exact texts: `omarchy-language`, `omarchy-crash-mute`, `omarchy-toggle`,
  `omarchy-toggle-crash-capture`, `omarchy-default-agent`,
  `omarchy-dictation-use`, `omarchy-show-done`, the presentation launcher,
  migrations 1786719479 and 1791479273.
- A real `omarchy language`, crash mute or terminal step against a
  logbook: **not run** (dev host runs released builds only; no test-host
  run in this WP). **desktop**, **test host**: not run.

## Open questions

1. ADR-0059 is **proposed**; the code depends on it. The number assumes
   WP-227 takes 0056–0058; DECISIONS.md and CHANGELOG rows will conflict
   textually with other branches (append-only, trivial).
2. Omarchy assumptions for WP-224's ledger (I stayed out of it): the
   session file's text, the `# omarchy:args=` line of
   `omarchy-default-agent`, the `omarchy-install-dictation-*` names,
   `locales.json`'s shape, `omarchy-show-done <code>`.
4. The edge-only `service-states` failures above (pre-existing on `next`).
3. `engine/src/config.rs` changed only at `ROUTINE_RULES`, a few lines
   above WP-224's always-red family; expect a nearby-hunk merge.
