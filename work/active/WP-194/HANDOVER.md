# WP-194 — handover (what Seldon assumes about Omarchy, checked offline)

Branch `wp/194-omarchy-assumptions` in `wt/WP-194`, from `next` 3bcc07dc.
Not merged.

## Done

1. **The ledger, `tests/omarchy/assumptions.json`.** 22 assumptions over
   17 installed files under `$OMARCHY_PATH`, measured on Omarchy 4.0.4-1
   on 2026-10-12. Each entry: `id`, `path` (relative to `$OMARCHY_PATH`),
   `sha256`, `omarchyVersion`, `measured`, `symbol` (in the Omarchy
   file), `pluginSymbol` (on the plugin line; round 2), `plugin`
   (`plugin/<file>:<line>`), `relies` ("Seldon relies on … at
   `plugin/<file>:<line>`."), `remeasure` (the harness or live step to
   repeat). Every file was confirmed by a grep of what the plugin calls:

   | File | Assumptions (plugin line) |
   |---|---|
   | `shell/services/PluginShellApi.qml` | `updateEntryInline` (Desk.qml:527), `serviceFor` (BarWidget.qml:55), summon/hide/toggle (BarWidget.qml:75) |
   | `shell/shell.qml` | overlay routing (Desk.qml:175), loader injection (Desk.qml:66), reload without `clearComponentCache` (Service.qml:76), fresh public manifest (Service.qml:75), settings write (Desk.qml:527) and service lookup (BarWidget.qml:55) behind the facade (round 2) |
   | `shell/Ui/PluginBarApi.qml` | `moduleWidgets` for third parties (BarWidget.qml:138) |
   | `shell/plugins/bar/Bar.qml` | per-monitor instances plus the hidden centre placeholder (Model.js:4129) |
   | `shell/plugins/bar/BarModel.js` | `pickDrawnSlot` mirrored by `pickDrawnWidget` (Model.js:4138) |
   | `shell/Ui/BarWidget.qml` | `settings` / `setting()` from the shell.json entry (BarWidget.qml:46) |
   | `shell/Commons/Style.qml`, `Color.qml`, `Border.qml` | `Style.bar.iconCanvas` (BarWidget.qml:249), `Color.popups.text` (Desk.qml:135), `Border.surfaceSpec` (Desk.qml:706) |
   | `shell/Ui/BorderSurface.qml`, `Button.qml`, `ButtonGroup.qml`, `Dropdown.qml` | the desk card (Desk.qml:697), `focusable` (NewCaseSheet.qml:276), `changed` without assigning `value` (NewCaseSheet.qml:179), `popupOpen` (JournalField.qml:35) |
   | `bin/omarchy-plugin-update`, `bin/omarchy-restart-shell` | reload without new code (Service.qml:76), the restart argv (Model.js:4097) |
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
