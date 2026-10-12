# WP-194 — handover (what Seldon assumes about Omarchy, checked offline)

Branch `wp/194-omarchy-assumptions` in `wt/WP-194`, from `next` 3bcc07dc.
Not merged.

## Done

1. **The ledger, `tests/omarchy/assumptions.json`.** 22 assumptions over
   17 installed files under `$OMARCHY_PATH`, measured on Omarchy 4.0.4-1
   on 2026-10-12. Each entry: `id`, `path` (relative to `$OMARCHY_PATH`),
   `sha256`, `omarchyVersion`, `measured`, `symbol` (in the Omarchy
   file), `pluginSymbol` (in the code of the plugin file; round 2, round 3),
   `plugin` (`plugin/<file>:<line>`, the line a hint since round 3), `relies` ("Seldon relies on … at
   `plugin/<file>:<line>`."), `remeasure` (the harness or live step to
   repeat). Every file was confirmed by a grep of what the plugin calls:

   | File | Assumptions (plugin line) |
   |---|---|
   | `shell/services/PluginShellApi.qml` | `updateEntryInline` (Desk.qml:527), `serviceFor` (BarWidget.qml:55), summon/hide/toggle (BarWidget.qml:75) |
   | `shell/shell.qml` | overlay routing (Desk.qml:175), loader injection (Desk.qml:66), reload without `clearComponentCache` (Service.qml:76), fresh public manifest (Service.qml:75), settings write (Desk.qml:527) and service lookup (BarWidget.qml:55) behind the facade (round 2) |
   | `shell/Ui/PluginBarApi.qml` | `moduleWidgets` for third parties (BarWidget.qml:138) |
   | `shell/plugins/bar/Bar.qml` | per-monitor instances plus the hidden centre placeholder (Model.js:4222) |
   | `shell/plugins/bar/BarModel.js` | `pickDrawnSlot` mirrored by `pickDrawnWidget` (Model.js:4231) |
   | `shell/Ui/BarWidget.qml` | `settings` / `setting()` from the shell.json entry (BarWidget.qml:46) |
   | `shell/Commons/Style.qml`, `Color.qml`, `Border.qml` | `Style.bar.iconCanvas` (BarWidget.qml:249), `Color.popups.text` (Desk.qml:135), `Border.surfaceSpec` (Desk.qml:706) |
   | `shell/Ui/BorderSurface.qml`, `Button.qml`, `ButtonGroup.qml`, `Dropdown.qml` | the desk card (Desk.qml:697), `focusable` (NewCaseSheet.qml:276), `changed` without assigning `value` (NewCaseSheet.qml:179), `popupOpen` (JournalField.qml:35) |
   | `bin/omarchy-plugin-update`, `bin/omarchy-restart-shell` | reload without new code (Service.qml:76), the restart argv (Model.js:4190) |
   | `bin/omarchy-plugin-validate` | the manifest rules (manifest.json:2); its hash equals `packaging/omarchy-pin` |
   | `shell/README.md` | the documented plugin contract (manifest.json:10) |

   The WP said "about ten"; the candidates plus the `Ui` controls the
   plugin really uses came to 17. `bin/omarchy-restart-shell` is not on
   the WP's candidate list but the update notice runs it. `Util.qml`,
   `TextField`, `PanelSlider`, `PanelToolTip`, `PointerMoveGate`,
   `CursorSurface` and `WidgetButton` are used but left out (no
   behaviour beyond the token check rests on them); easy to add.
   Nothing from Omarchy is copied: only paths, hashes and identifier
   names that are already in SPEC-PLUGIN and `memory/omarchy-shell.md`.

2. **`scripts/check-omarchy-assumptions.sh`, `just check-upstream
   [--gate]`.** Hashes the listed files read-only, offline, no writes.
   On a mismatch: one NOTICE per changed or missing file (short hashes,
   the version measured), then each of its assumptions with whether the
   file still names the symbol, the sentence and what to re-measure.
   Compares the installed validator with `packaging/omarchy-pin`
   ("CI validates with another validator"). Default exit 0 with the
   notice; `--gate` exit 1 on any mismatch **and on a missing tree**
   ("skipped … --gate refuses an unchecked tree"; a release gate that did
   not look is not a pass). Without `$OMARCHY_PATH/shell`: "skipped",
   never "ok". Exit 2 for usage, a malformed ledger, a path with `..` or
   absolute, jq missing.

3. **`just check` runs it** through a new step `check-assumptions`:
   `bash -n`, shellcheck when installed, `tests/omarchy/assumptions.test.sh`,
   then the check on the installed tree in notice mode.
   `plugin-validate`'s existing pin notice stays.

4. **`tests/omarchy/assumptions.test.sh`** (fixture, runs in CI too):
   the ledger lint (fields, 64-hex, relative paths, date, unique ids, one
   hash per file, the sentence form, **each cited plugin line still holds
   its symbol**, the validator hash = the pin) with seven mutants; the
   check on a synthetic tree (ok; changed file → notice naming both of its
   assumptions, exit 0; `--gate` → exit 1; a lost symbol; a missing file;
   a new validator → its assumption and the pin notice; pin-only
   difference; skipped without a tree, `--gate` refuses it; usage and
   ledger errors exit 2; the tree unchanged afterwards). Where Omarchy is
   installed: a scratch copy (in `TMPDIR`) of the ledger's 17 files checks
   exactly as the installed tree does, and one appended line in the copy
   of `PluginShellApi.qml` produces the notice naming
   `facade-settings-write` at `plugin/Desk.qml:527`, `--gate` refuses.
   Every check process runs with its own `HOME` and a private
   `XDG_RUNTIME_DIR` (0700, removed afterwards).

   Consequence for plugin WPs: moving a cited line (editing `Desk.qml`
   above line 527, say) fails `check-assumptions` until the ledger cites
   the new line. That keeps the sentences true; it costs a one-line
   ledger edit now and then.

5. **Docs.** `docs/TESTING.md`: a row in the `just check` table and the
   section "Omarchy assumptions" (one paragraph: what, notice vs gate,
   skipped in CI, re-measure and update in the same commit, never
   silenced, open items into the acceptance record's `limitations`).
   `memory/omarchy-shell.md`: a dated "Assumptions" header pointing to
   the ledger. `packaging/README.md`, "The Omarchy pin": refreshing the
   pin updates the ledger's validator entry in the same commit.
   `CHANGELOG.md` [Unreleased] › Docs: one entry.

## Not done

- **WP-126 and WP-188** already name WP-194's `--gate` before their live
  checks (`work/queued/WP-126.md:103`, `WP-188.md:80`); I did not edit
  them. Round 2 (orchestrator, Q3): they run it from the deployed clone,
  see "Round 2".
- No network check, no cron or scheduled job, no check on push (out of
  scope).
- No ADR (the WP says none).

## How it was verified

| Check | Where |
|---|---|
| `SELDON_FULL_CHECK=1 just check` on the final commit | headless (dev host; `CARGO_TARGET_DIR`, `TMPDIR`, `JUST_TEMPDIR` under the private `gates/`; shellcheck through the E80 wrapper) — result below |
| `tests/omarchy/assumptions.test.sh`, synthetic tree and lint mutants | fixture |
| scratch copy of the dev host's installed files, one changed → notice, `--gate` refuses | fixture (copy of the desktop's tree in `TMPDIR`) |
| missing `$OMARCHY_PATH` → "skipped" | fixture; CI: not run here (no push), the same code path the fixture runs |
| ledger = dev host's installed tree (`just check-upstream --gate`, exit 0, 17 files ok) | desktop (read-only) |
| ledger = test host's installed tree (`check-omarchy-assumptions.sh --gate` from a temp dir over ssh, `omarchy version` 4.0.4-1, 17 files ok, exit 0) | test host |
| shellcheck of the new script and test | test host (E80 wrapper) |

**Gate:** `SELDON_FULL_CHECK=1 just check` exited 0 (`check: ok`) on
the tree of 46db4fbb (the code and docs commit; only this handover
follows it, under `work/`). Log in the private
`gates/check-dev-194.log`: cargo tests 2893 passed, 0 failed;
check-packaging ok; install 229/0; deploy-test-host 326/0; runtime-dir
45/0; assumptions.test ok (32 checks); check-upstream ok (17 files);
docs-check ok; plugin-validate ok; qmllint ok (52 files);
terminal-scripts 73/0; real-home-guard 40/0; check-tokens 14/0;
service-states 366/0; desk-view 2243/0; bar-view 195/0; ipc-restart
43/0 (no leftover in the session's `quickshell/by-id`). That run had no
shellcheck on `PATH`; afterwards `just check-assumptions` on 46db4fbb
with the E80 wrapper on `PATH` ran shellcheck on the test host for the
new script and test and exited 0. The scratch `TMPDIR` is empty
afterwards.

## Open questions

- Should `check-upstream --gate` also refuse when only the pin differs
  (today: yes, it refuses)? I think yes: at a release, CI validating with
  another validator than the test host is exactly what should be
  re-measured.
- `--gate` on a missing tree refuses (exit 1) while the notice mode says
  "skipped" with exit 0. The WP says "skipped, never passed"; I read a
  gate that could not look as a refusal. Reviewer to confirm.

## Round 2 (review 1: APPROVE WITH FIXES, findings 1–5)

Orchestrator decisions: fix findings 1–5 here; Q1 `--gate` keeps
refusing a pin-only difference; Q3 the test-host gate runs from the
deployed clone.

1. **`symbol` split (finding 1).** Each entry now has `symbol` (an
   identifier found in the Omarchy file; the check greps it there) and
   `pluginSymbol` (the plugin-line lint). The seven plugin-only spellings
   became Omarchy identifiers: `summon`, `isBarWidgetPanelPlugin`,
   `publicPluginManifest`, `popups`, `borderSpec`, `signal changed`,
   `secure`. New lint (the test's installed-tree part, hosts with
   Omarchy): wherever an installed file hashes to the ledger, each of its
   symbols is found in it, with a mutant (`Button.qml` given a symbol it
   lacks). Measured: all 24 symbols found in their 4.0.4-1 files.
2. **`shell/shell.qml` entries (finding 2).** `shell-settings-write`
   (`updateEntryInline`, Desk.qml:527) and `shell-service-lookup`
   (`serviceFor`, BarWidget.qml:55): 24 assumptions over 17 files. Test
   row: one appended line in a scratch copy of `shell.qml` alone names
   both (and `shell-overlay-routing`, "1 of 17 files differ"), names no
   `facade-*` and never "no longer names"; the `PluginShellApi.qml` row
   now also asserts it does not name `shell-settings-write`.
3. **Unreadable pin (finding 3).** When the installed validator exists
   and `omarchy-validate.sh --print-pin` fails or gives no 64-hex hash:
   "NOTICE: cannot read packaging/omarchy-pin, so the installed validator
   was not compared with the one CI fetches"; notice mode exits 0 (never
   "ok"); `--gate` exits 1 ("refused (--gate): every file matches the
   ledger, but packaging/omarchy-pin could not be read, …"). Test rows: a
   missing pin (notice and gate), a malformed pin (gate), a missing pin
   plus a changed file (both named), the pin back (ok), and no installed
   validator (no pin needed, ok). Reproduced the reviewer's command on
   the real tree: now rc=1 with that line.
4. **Code lines, distinctive symbols (finding 4).** No citation is a
   comment any more: `shell-reload-keeps-code` and `plugin-update-reload`
   cite `Service.qml:76` (`restartShellNotice`), `bar-instances`
   `Model.js:4129` (`isDrawnWidget`), `bar-drawn-slot` `Model.js:4138`
   (`pickDrawnWidget`). Generic symbols replaced: `service` →
   `item.service`, `focusable` → `Keys.onReturnPressed` (plugin side
   `focusable: true`), `onChanged` → `signal changed` (plugin side
   `onChanged: function(v)`). New lint: a cited line that starts with
   `//`, `/*`, `*` or `#` fails, with a mutant that finds a comment line
   of `Model.js` at run time.
5. **Dead `PIN` default (finding 5)** removed; the lint reads the real
   pin.

Q3: `docs/TESTING.md` ("Omarchy assumptions") and the justfile say the
release gate runs `bash scripts/check-omarchy-assumptions.sh --gate` in
the deployed clone of the release commit on the test host, never from
loose copied files. Observed today (test host, read-only `ls`): there is
no clone of this repository in the home there and no `just`; WP-126 and
WP-188 have to put the clone there before the gate. My round-1 test-host
run (the four files in a temp dir) stays valid as evidence that the
hashes match; it is not the release-gate form.

### How round 2 was verified

| Check | Where |
|---|---|
| `tests/omarchy/assumptions.test.sh`: 43 checks (lint and mutants, synthetic tree, pin rows) | fixture |
| symbols found in their installed files; shell.qml-only and PluginShellApi-only changes in a scratch copy | fixture (copy of the desktop's tree in `TMPDIR`), desktop (read-only) |
| `check-upstream --gate` on the installed tree, ok; with a missing pin, refused rc=1 | desktop (read-only) |
| shellcheck of the script and the test | test host (E80 wrapper) |
| ledger hashes = test host's tree | test host (round 1; no hash changed in round 2) |
| `SELDON_FULL_CHECK=1 just check` | headless — result below |

**Round-2 gate:** `SELDON_FULL_CHECK=1 just check` exited 0 (`check:
ok`) on 3d59ce7a, this time with the E80 shellcheck wrapper on `PATH`,
so every shellcheck inside the check ran on the test host. Log in the
private `gates/check-dev-194-r2.log`: cargo tests 2893 passed, 0 failed;
check-packaging ok; install 230/0; deploy-test-host 327/0; runtime-dir
45/0; assumptions.test ok (43 checks); check-upstream ok (17 files);
docs-check ok; plugin-validate ok; qmllint ok (52 files);
terminal-scripts 73/0; real-home-guard 40/0; check-tokens 14/0;
service-states 366/0; desk-view 2243/0; bar-view 195/0; ipc-restart
43/0. Only this handover follows 3d59ce7a. The scratch `TMPDIR` is empty
afterwards.

## Round 3 (found at merge: a moved plugin line failed `just check`)

The orchestrator's merge into `next` failed the gate: WP-204 moved
`Model.js` lines, and `bar-instances` cited `plugin/Model.js:4129`,
which no longer held `isDrawnWidget`. Citing by exact line made every
plugin edit above a cited line fail `just check` for every other WP.

1. **A citation is the plugin file and its `pluginSymbol`.** The lint
   now greps the `pluginSymbol` in the whole cited file and fails only
   when it is gone from the file ("'X' is gone from plugin/F (cited at
   line N)") or appears only in comments ("'X' is only in comments of
   plugin/F"): lines starting with `//`, `/*`, `*` or `#` and trailing
   ` // …` comments do not count. A `/* … */` span is not parsed (round
   4: the wording narrowed to this). The line number stays in `plugin` and
   in the sentence as a hint for the reader and in the messages, never as
   a pass or fail condition. The round-2 strength stays: every `symbol`
   is found in its Omarchy file where the hash matches (installed-tree
   part), comment-only citations fail.
2. **Merged `next` 23fd3105** (WP-204, WP-207, the 0.2.0 AUR bump) into
   the branch: 34fc4815, no conflicts. The three moved hints were
   updated to the current code lines: `bar-instances` Model.js:4141,
   `bar-drawn-slot` Model.js:4150, `restart-shell` Model.js:4109.
3. **Tests (fixture).** Replacing the round-2 "moved line" and "comment
   line" mutants:
   - a ledger whose `restart-shell` hint is `Model.js:1`: lint clean;
   - a scratch copy of the cited plugin files with 40 lines inserted
     above everything (all 24 hints off by 40): lint clean;
   - `pickDrawnWidget` deleted from the scratch `Model.js`: "is gone";
   - `pickDrawnWidget` only in a `//` line, a trailing `// …` and a
     starred block (every line starting with `/*` or `*`): "is only in
     comments", and nothing else named.
   Mutants of the lint itself, each restored afterwards (fixture): the
   comments branch disabled, trailing comments counted as code, and the
   old exact-line check put back; each was caught (the last by both
   moved-line rows).

`docs/TESTING.md` (the `just check` row and "Omarchy assumptions") and
the ledger's `about` say the line is a hint and when the check fails.

### How round 3 was verified

| Check | Where |
|---|---|
| `tests/omarchy/assumptions.test.sh` on the merged tree: 45 checks, including the moved-line rows | fixture; installed-tree part on a scratch copy of the desktop's tree |
| three mutants of the new lint, all caught | fixture |
| shellcheck of the script and the test | test host (E80 wrapper) |
| `SELDON_FULL_CHECK=1 just check` under `flock /tmp/seldon-check.lock` on the merged tree | headless — result below |

**Round-3 gate:** `SELDON_FULL_CHECK=1 just check` under `flock
/tmp/seldon-check.lock`, with the E80 shellcheck wrapper on `PATH`,
exited 0 (`check: ok`) on 9b0c09b3, the merge of `next` 23fd3105 plus
the round-3 fix. Log in the private `gates/check-dev-194-r3.log`: cargo
tests 2893 passed, 0 failed; check-packaging ok; install 230/0;
deploy-test-host 327/0; runtime-dir 45/0; assumptions.test ok (45
checks, among them both moved-line rows); check-upstream ok (17 files);
docs-check ok; plugin-validate ok; qmllint ok (52 files);
terminal-scripts 73/0; real-home-guard 40/0; check-tokens 14/0;
service-states 366/0; desk-view 2243/0; bar-view 195/0; ipc-restart
43/0. Only this paragraph follows 9b0c09b3. The scratch `TMPDIR` is
empty afterwards.

## Round 4 (round-3 re-check: APPROVE, two low findings)

Orchestrator decisions: narrow the wording to what the code does, and
warn (not fail) on a stale line hint. Docs and test script only.

1. **Wording (finding 1).** `docs/TESTING.md`, the test's comments and
   this handover now say what the lint counts as a comment: `//` line
   comments, trailing ` // …` comments, and lines that start with `/*`
   or `*`. A real `/* … */` span is not parsed: an unstarred line inside
   one, or an inline `/* … */` on a code line, counts as code. The
   plugin has no `/* … */` comment today. The test row's name says
   "lines starting with /* or *" instead of "block".
2. **Stale hint warning (finding 2).** When the `pluginSymbol` is in the
   code of its file but the hint line no longer holds it, the lint
   prints `warn <id>: plugin/F:N does not hold '<symbol>' (now at line
   M); refresh the hint in tests/omarchy/assumptions.json` on stderr.
   It is never a problem, so `just check` stays green and the line shows
   in its output. Test rows (fixture): the `Model.js:1` hint gives
   exactly that one warning with the line `RESTART_SHELL_ARGV` is at
   now; the 40-lines-inserted copy gives one warning per stale hint
   (24) and no problem. Mutant (fixture, restored afterwards): the
   warning removed → both rows fail. Today's ledger gives no warning.

### How round 4 was verified

| Check | Where |
|---|---|
| `tests/omarchy/assumptions.test.sh`: 46 checks, no `warn` line for today's ledger | fixture; installed-tree part on a scratch copy of the desktop's tree |
| the warning mutant, caught by two rows | fixture |
| shellcheck of the test | test host (E80 wrapper) |
| `just docs-check` | headless — result below |
| `SELDON_FULL_CHECK=1 just check` | not run (the orchestrator gates this round) |

**Round-4 checks:** on d6ac9940, `bash tests/omarchy/assumptions.test.sh`
ok (46 checks, no `warn` line) and `just docs-check` ok (514 links), with
`TMPDIR` and `JUST_TEMPDIR` under the private `gates/` (empty afterwards).
Only this handover follows d6ac9940.

## Round 5 (found at merge: the warning row asserted fresh real hints)

The orchestrator's merge into `next` 50bbd290 (WP-201, WP-206 moved
`Model.js` lines) failed the gate: the round-4 row "lint warns" ran on
the real ledger and expected exactly one warning, so the three real
hints that WP-201 and WP-206 had made stale turned the warning back into
a failure for every other WP.

1. **The real ledger only warns.** The lint of the real ledger prints a
   `warn` line per stale hint on stderr and never fails any check. No
   row asserts anything about how fresh the real ledger's hints are.
2. **The warning rows run on a scratch copy** of the cited plugin files
   with a ledger refreshed against that copy (each stale hint moved to
   the first code line holding its `pluginSymbol`), so they depend only
   on what the test controls:
   - the refreshed ledger: no problem, no warning;
   - its `restart-shell` hint set to `Model.js:1`: exactly one warning,
     with the line `RESTART_SHELL_ARGV` is at now, no problem;
   - 40 lines inserted above everything: one warning per stale hint
     (24), no problem;
   - the real ledger on the same moved copy: no problem (warnings only).
   The lint calls on mutant ledgers and on the "gone" and "comments"
   copies send their warnings to `/dev/null`; only the real ledger's
   lint shows them in `just check`.
3. **Merged `next` 50bbd290** into the branch (d35b8bfa, no conflicts).
   The test fix (65151f56) was committed while the ledger still had the
   three stale hints: `tests/omarchy/assumptions.test.sh` passed there
   with three `warn` lines and no failure, the state the gate saw.
   Then the hints were refreshed as a normal ledger update (81386632):
   `bar-instances` Model.js:4222, `bar-drawn-slot` Model.js:4231,
   `restart-shell` Model.js:4190; the test passes with no warning.
4. Mutants (fixture, restored afterwards): the warning sent to stdout
   (a failure again) → the real-ledger row and four scratch rows fail;
   the warning removed → the "warns" and "moved lines" rows fail.

`docs/TESTING.md` says a stale hint in the real ledger never fails any
check and that the warning rows run on a refreshed scratch ledger.

### How round 5 was verified

| Check | Where |
|---|---|
| `tests/omarchy/assumptions.test.sh` on 65151f56 (stale hints): ok, three `warn` lines | fixture; installed-tree part on a scratch copy of the desktop's tree |
| the same after the refresh: ok, no `warn` line; 47 checks | fixture; installed-tree part on a scratch copy of the desktop's tree |
| a cited line moved in a scratch copy of `plugin/`: passes, refreshed and real ledger | fixture |
| two warning mutants, each caught | fixture |
| shellcheck of the test | test host (E80 wrapper) |
| `just docs-check` | headless, ok (514 links) |
| `SELDON_FULL_CHECK=1 just check` under `flock /tmp/seldon-check.lock` | headless — result below |

**Round-5 gate: not green.** `SELDON_FULL_CHECK=1 just check` under
`flock /tmp/seldon-check.lock` (E80 wrapper on `PATH`) ran twice on
03513b6a and exited 1 both times, each time on the plugin harness's
real-path guard, not on a WP-194 check. The guard saw the real
`~/.local/state/seldon` rewritten during the run: `cursors.json`,
`index.json`, `recent-config.json` and the directory got new mtimes,
with sizes unchanged, 896 s and 900 s apart. In the first run desk-view
failed (2336 passed, 1 failed); in the second, service-states (365
passed, 1 failed). The cadence looks like a periodic job of the engine
installed on this machine writing its own state, which no harness
process here touches (each has its own `HOME`). This is not confirmed:
I did not inspect the real directory or the host's jobs (red zone).
Everything before the plugin harness passed in both runs (private
`gates/check-dev-194-r5.log` and `-r5b.log`): cargo tests 2893 passed,
0 failed; check-packaging ok; install 230/0; deploy-test-host 327/0;
runtime-dir 45/0; assumptions.test ok; check-upstream ok (17 files);
docs-check ok; plugin-validate ok; qmllint ok (53 files);
terminal-scripts 73/0; real-home-guard 40/0; check-tokens 14/0; and in
the first run service-states 366/0. Not run in either run: bar-view and
ipc-restart, and in the first run the rest of desk-view after the
guard. The scratch `TMPDIR` is empty afterwards.

## Paused (wind-down before the account switch)

- Done: merged `next` 71f070b8 (WP-212, the real-home guard fix) into
  the branch as f4a58c25, no conflicts. `bash
  tests/omarchy/assumptions.test.sh` on f4a58c25: ok, no `warn` line
  (fixture; installed-tree part on a scratch copy of the desktop's
  tree).
- Stopped at: `SELDON_FULL_CHECK=1 just check` under `flock
  /tmp/seldon-check.lock` on f4a58c25. It waited about 50 minutes for
  the lock behind other gates and never started (its log holds only
  `exit=143`). I stopped it while it was still queued, because it could
  not finish within the wind-down window. Not run: the full check on
  f4a58c25.
- Next: run that full check on f4a58c25 (or its successor with only
  this handover added) under the lock, with the same environment as
  round 3. Record the result here, then reply "WP-194 ready <sha>".

## Resumed: round-5 gate on next 71f070b8

**Round-5 gate: green.** `SELDON_FULL_CHECK=1 just check` under `flock
/tmp/seldon-check.lock`, with the E80 shellcheck wrapper on `PATH`,
exited 0 (`check: ok`) on b485d94a. That is f4a58c25 (the merge of
`next` 71f070b8 with WP-212's guard fix) plus only the paused-handover
note. Log in the private `gates/check-dev-194-r5d.log`: cargo tests 2893
passed, 0 failed; check-packaging ok; install 230/0; deploy-test-host
327/0; runtime-dir 45/0; assumptions.test ok (no `warn` line);
check-upstream ok (17 files); docs-check ok; plugin-validate ok; qmllint
ok (53 files); terminal-scripts 73/0; real-home-guard 71/0; check-tokens
14/0; service-states 366/0; desk-view 2337/0 (no drift-only or
today-live failure); bar-view 195/0; ipc-restart 43/0 (headless). The
two earlier round-5 runs that the real-path guard stopped are covered by
WP-212. Only this paragraph follows b485d94a. The scratch `TMPDIR` is
empty afterwards.

## Round 6 (round-5 re-check: APPROVE WITH FIXES, two findings)

1. **No real plugin text in the stale-hint row (finding 1).** The row
   took its expected line from `grep -nF 'RESTART_SHELL_ARGV = [...]'`
   on the copy of the real `Model.js`. A rename in another WP made that
   grep fail, and under `set -euo pipefail` the script ended with rc=1
   and no FAIL line. Now the expected line comes from the lint's own
   rule: the first code line of the scratch copy that holds the ledger's
   `restart-shell` `pluginSymbol`. If none is found, the row prints a
   FAIL line ("no code line of plugin/Model.js holds …") and the script
   goes on. The two other `at=` lookups (in the lint's warning and in
   `fresh`) end in `|| true` too, so a failed lookup never ends the
   script silently.
2. **The facade row reads the ledger (finding 2).** The expected
   `at plugin/Desk.qml:527.` and the re-measure text are taken from the
   ledger's `facade-settings-write` entry with jq. A missing entry is a
   FAIL line.

### How round 6 was verified

| Check | Where |
|---|---|
| `tests/omarchy/assumptions.test.sh` on d2d4183c: ok, 47 checks, no `warn` line | fixture; installed-tree part on a scratch copy of the desktop's tree |
| rename: `RESTART_SHELL_ARGV` → `RESTART_ARGV` (3 uses) in a scratch copy of the tree; the new test ok (rc 0, the stale-hint row passes); the round-5 test on the same copy ended with rc=1 and no FAIL line (the review's P4) | fixture |
| hint refresh: a line inserted above `Desk.qml:527` and the ledger's two `Desk.qml:527` hints moved to 528 in a scratch copy; the test ok, the facade row passes | fixture (installed-tree part) |
| failed lookup: the `omarchy-restart-shell` code line commented out in a scratch copy; six FAIL lines and the summary "6 failure(s)", rc=1, no silent stop | fixture |
| shellcheck of the test | test host (E80 wrapper) |
| `SELDON_FULL_CHECK=1 just check` | not run (the orchestrator gates this round) |

Each scratch copy came from `git archive` of d2d4183c in the private
`gates/` TMPDIR and was removed afterwards. Only this handover follows
d2d4183c.

