WP-013 HANDOVER

Branch `wp/013-integration`, worktree `wt/WP-013`, **rebased onto `main`
at `f54e382`** (WP-020…WP-041 merged). Not pushed, no PR. 16 commits
`main..HEAD`:
- the original WP: `5a367c6` script · `439580a` justfile · `a413964`
  TESTING · `eaf635c` findings · `328d66e` memory · `f46df44` handover;
- the review follow-up: `227eba2` · `dd6f000` · `f3bf013`;
- the live session: `ae2dd01` · `cd51151` · `27bf86c` · `a1080d7` ·
  `f99d772` · `6be6455` · `7a0730a`, and this handover.

The commit ids quoted in the sections below are from before the rebase.

## Live session after the unlock (orchestrator's resume brief)

**1. Rebase.**
- `git rebase main` had one conflict, in `memory/pitfalls.md`, where both
  sides had appended. I kept both, main's sections first.
- Main's engine changed one thing the script relied on: `seldon init` now
  runs the first capture (WP-024). A later `capture --since` is then
  ignored (`sinceIgnored`).
- The script therefore backfills with `init --since <now − 7 d>`, and the
  idempotency check is now "`capture --all` after `init` writes 0"
  (`ae2dd01`; TESTING updated in `cd51151`).

**2. E2E on the test host, two runs back to back.** Both exited 0 (47/47):
- the lock was checked before every key and the restart;
- the restart ran only after the reloads had settled;
- no new crash report;
- the host was restored each time.

`just e2e --engine-only` exits 0 (24/24), and `just check` on the
rebased branch exits 0. Logs:
`live/e2e-full-run1.log`, `live/e2e-full-run2.log`,
`live/e2e-engine-only.log`. The host name is redacted to `<test-host>`.

**3. Pending live steps of WP-020/021/022/023/030/031.**
- Setup: a scratch logbook `~/Seldon-e2e`, built from `fixtures/logbook/`
  (never a real one), the static engine, and the plugin from `main`.
- 31 of the steps pass. Details and the table: `live/RESULTS.md`.
- Highlights:
  - all panel keys (digits 1–6, h/l and ←/→, Tab/Shift-Tab, n, e, c, +,
    x, a, d, Esc, two-press arming with disarm on a move);
  - the Work chain start → verify → done, drop, and Start agent (with a
    harmless `/usr/bin/true` launcher, so no real agent was launched);
  - Decisions `d`, and the Memory open;
  - the drift sheet: Explain, Link, and Dismiss of a group, with the
    ledger `resolution`/`refersTo` lines and the pill and strip following;
  - the overlay:
    - every chart's `paints` = 1 on open, `paintMs` 0–2;
    - `call hover`, period keys 1–4, ←/→ and h/l;
    - a real pointer hover through `ydotool`, which repaints nothing;
    - Esc, a scrim click, and the pill's middle click;
  - the shell log of the live instance is clean of jax.seldon warnings
    and errors.
- `seldon open --editor` (WP-012 Decision 1) is fixed on main: the editor
  stays open.

**4. Results.**
- `work/active/WP-013/live/`: `RESULTS.md` (step, WP, result, note), the
  logs, and `screenshots/osaka-jade-*.png`. That is 11 shrunk PNGs: six
  tabs, the drift sheet, the new-case and new-decision sheets, the
  overlay and the pill.
- Live vs headless mismatches: FINDINGS §5.
  - **`aggregations.overlay` is 23 live and 0 headless** on the same
    index. Plugin follow-up.
  - The heatmap probe point `0.9,0.5` from the handovers lands on the
    empty part of the slot (the grid fills only ~15 %). This is a layout
    and docs question; it is the same live and headless.
  - Everything else agrees.
- **Nothing needs an engine fix.** Plugin follow-ups:
  - the aggregation counter (FINDINGS §5.1);
  - the heatmap's use of its slot (FINDINGS §5.2).

**5. Test host left as found.**
- Theme Osaka Jade (never changed, see Decision 1).
- `~/Seldon-e2e`, `~/.local/bin/seldon`, `~/.local/state/seldon`,
  `~/.config/seldon`, `~/.cache/seldon-e2e` and `/tmp/seldon-wp013` are
  absent, and the `ls -A ~` listing equals the one before the session.
- The service shows `engineMissing`, and the session is unlocked.
- Crash reports: still 4.
- The plugin folder holds the fresh copy of `main` (hash `644fd01ae2c8`).
- No editor windows I opened are left.
- `/tmp/seldon-unlock.sh` is untouched.
- Two things were already there before the session and are not mine:
  the second `quickshell` process (the crash-report window of 15:13) and
  the Navbar Cat layer.

**Incident (please read `live/INCIDENT.md`).**
- While I was driving the drift sheet, a pointer click closed the Seldon
  panel. The next `wtype` text went into the operator's foot terminal on
  the test host, where bash ran it: `command not found: Live`, exit 127,
  plus two empty Returns.
- Nothing else ran. The line is now in that shell's history; I left the
  history alone.
- From then on, every key went out only while a Seldon surface reported
  `opened: true`, and text only into a field reporting `editing: true`.
  The guard refused keys correctly twice afterwards. This rule is in
  `memory/pitfalls.md`.

**Not done in this session:**
- the Tokyo Night and Catppuccin Latte screenshots (Decision 1);
- the WP-040 `makepkg` build on the test host (the guard blocks package
  builds);
- the WP-024 theme hook install (`omarchy hook install` is guard-blocked);
- the real default agent launch of WP-022 (deliberately replaced by a
  harmless launcher);
- `plugin/preview.png` (WP-041) is not mine to change. The live pill and
  panel shots are in `live/screenshots/`.

## Review follow-up (SEND BACK → fixed)

Commits `06470d5` (script) and `5b7eeb2` (TESTING), on top of `d69e981`.

1. **Same-host guard (blocking).**
   - Right after the first `rsh true`, the script compares the remote
     `/etc/machine-id` (falling back to `hostname`) with the local one. It
     dies when they are equal or when the remote answer is empty.
   - So `SELDON_TEST_HOST=localhost`, or an alias for the dev host, never
     reaches the copy, the init or the restart.
   - Verified with a fake `ssh` on `PATH` that runs the command on this
     machine: the run made exactly two ssh calls (`true` and the id),
     then gave "FAIL SELDON_TEST_HOST=localhost is this machine (same
     machine id)…" and exit 1.
2. **Unknown enabled flag.**
   - `plugin_enabled` asks `listPlugins` up to five times and returns
     `true`, `false` or empty (unknown).
   - The restore disables the plugin only when the flag was found `false`
     for certain and is `true` now.
3. **`found.env` before the mv-aside loop.**
   - The backup now writes `found.env` first: whether the plugin dir
     existed, the flag, and the list of Seldon paths that were absent.
     Only then does it move the existing paths aside.
   - The restore (`put_back`) removes a path only when it was absent or
     was moved aside. A found path that had not been moved yet is kept,
     with a "kept ~/…" line.
   - Verified offline: I extracted the remote functions into a scratch
     `HOME` with stubbed `omarchy-shell`, `quickshell` and `omarchy`.
     Four scenarios:
     - **A:** an original `~/.config/seldon` is moved aside, the run
       writes all four paths, and the restore leaves only the original;
     - **B:** a run killed inside the loop (`found.env` written,
       `~/.config/seldon` not moved): the original is kept, and the
       other three paths are removed;
     - **C:** `listPlugins` answers empty at the backup and the plugin is
       enabled now: no `omarchy plugin disable`;
     - **D:** the plugin was found disabled and is enabled now: exactly
       one `omarchy plugin disable jax.seldon`.
4. **`SELDON_E2E_SINCE_DAYS`** must match `^[0-9]+$`; otherwise the script
   says why and exits 1 before the build. TESTING.md now also says that a
   killed run on a locked host keeps its leftovers until the operator
   unlocks the session: the lock check comes before the recovery restore.
5. **The duplicate assertion** after the plugin's capture is renamed to
   "index after the plugin's capture still has the note in today". The
   panel check is the separate count comparison.

**Re-verified:**
- `just e2e --engine-only` exits 0 (23 passed), with no temp dir left;
- `SELDON_E2E_SINCE_DAYS=abc … --engine-only` exits 1 with the message.

**Full run: pending unlock.** As instructed, I have not used ssh to reach
the test host since the review. The two consecutive full runs happen after
the orchestrator says the host is unlocked and stay-awake is set.

## Done

- **`tests/integration/e2e.sh`.** `--engine-only` runs on the dev host; the
  default is the full run on the test host. `-h` prints the usage.
  - **Build:** the static musl engine (it checks that the binary is
    static).
  - **Engine steps, both variants:**
    - `--version`, and `contract-version` equal to the manifest's;
    - `init --non-interactive --path ~/Seldon-e2e`;
    - `capture --all --since <now − 7 d>`, with ≥ 1 package event;
    - a second `capture`, which must write 0;
    - `plan new`, then `plan start`;
    - `log --case <id> -- <note with quotes and $(…)>`, which must arrive
      verbatim;
    - `status`, giving `state.status` `ok`;
    - `index --check`, giving `valid: true`;
    - `doctor`, giving `ok: true`.

    Then index.json must have the contract version, one active case, the
    note in `today.entries`, and ≥ 1 package event.
  - **`--engine-only`:**
    - `HOME`, `XDG_CONFIG_HOME` and `XDG_STATE_HOME` live in a temp dir;
    - it then runs `plugin/Service.qml` in the existing headless harness
      (dev mode) against the index the engine just wrote. Status must be
      `ok`, the pill must equal the index counts, and the log must be
      clean;
    - it ends with `real-home-guard.sh`;
    - it never touches the running shell, and it never enables the
      plugin.
  - **Full run, after the engine steps on the test host:**
    1. rsync the plugin (`--checksum`), validate it, and enable it if it
       was disabled;
    2. wait for the hot reloads to settle, then `omarchy-restart-shell`;
    3. wait until `jax.seldon.service status` is `ok`, after the plugin's
       own start-up capture;
    4. check the pill text and tone (`panel pill` and the service) against
       `summary`;
    5. check Today's count against `today.entries`, and the Changelog rows
       with and without the pacman filter;
    6. QuickEntry with `wtype`: `n`, a note that starts like an option,
       Enter. The result must say "Saved", and after the FileView refresh
       Today has one more entry; the note is in the index and in the
       ledger;
    7. the log of the shell instance: the pid comes from `quickshell list
       -a -j`, the log must contain "Configuration Loaded", and no WARN or
       ERROR line may name jax.seldon;
    8. no new crash report under `~/.cache/quickshell/crashes`.
  - **Restore (full run):**
    - Before the run it takes a fingerprint of the test host. It covers
      the four Seldon paths, the plugin dir hash and its enabled flag, the
      theme, the number of `quickshell` processes and of crash reports.
    - It moves existing Seldon paths aside into `~/.cache/seldon-e2e/` and
      copies the plugin dir there.
    - At the end, or on any failure through the EXIT trap, it restores in
      this order: the engine binary, the plugin dir, the enabled flag,
      then a restart (only when the plugin code differs from what the
      shell runs; otherwise `service refresh`), a wait for the service to
      go idle, and then the state, config and logbook dirs.
    - It compares the fingerprint, and waits for the service status it
      found (`engineMissing`).
    - A run that was killed is restored at the start of the next one.
  - **Locked session:** the full run refuses to start while the test
    host's session is locked. It re-checks the lock before the restart and
    before any `wtype`.
  - **Artifacts:** `SELDON_E2E_OUT=<dir>` keeps the index, the status, the
    pill and the views for the findings. They name the host and are not
    for the repository.
- **`justfile`:** a recipe `e2e *args`, host only. It skips with a notice
  under `SELDON_SKIP_HOST_CHECKS`, and it is not part of `check`.
- **`docs/TESTING.md`:** the "Integration" section (steps, restore,
  locked session, artifacts), plus `just e2e` among the other recipes.
- **`work/active/WP-013/FINDINGS.md`:** no engine↔plugin display
  mismatch. It lists:
  - every compared surface, with its command and an index excerpt;
  - two reading questions;
  - one engine self-inconsistency;
  - three test-host findings.

## Not done

- **I could not re-run the full variant after the last script changes**,
  because the test host's session is locked (Decision 1). These changes
  are:
  - the lock checks (their refusal path *is* verified: the run stops
    before the backup and changes nothing);
  - the trailing `wtype -k Escape` dropped (the IPC `close` remains);
  - the restore verification moved into a function that the EXIT trap
    also runs.

  Everything else in the full run passed four times, three of them after
  the crash hardening.
- **No screenshots and no theme sweep.** The WP asks for neither.
- **I did not close the Quickshell crash-report window myself**
  (FINDINGS §4.1). A later shell restart closed it; the test host has one
  `quickshell` process again.

## Verified by

```
$ just e2e --engine-only                                  → exit 0, 23 passed (run twice in a row; again after the last edit, see below)
$ SELDON_E2E_SINCE_DAYS=0 tests/integration/e2e.sh --engine-only
                                                          → exit 1, 2 failed (no package events), temp dir removed  [negative control]
$ SELDON_SKIP_HOST_CHECKS=1 just e2e                      → "e2e: skipped (…)", exit 0
$ SELDON_TEST_HOST=<alias> just e2e                       → exit 0, 43 passed (first version)
  after the crash hardening:                              → exit 0, 45 passed · exit 0, 45 passed (twice in a row)
                                                          → exit 0, 45 passed (with SELDON_E2E_OUT, for FINDINGS)
$ timeout -s KILL 12 tests/integration/e2e.sh            → killed after the backup (leftovers on the test host)
$ SELDON_TEST_HOST=<alias> just e2e                       → "a previous run did not finish; restoring what it found",
                                                            then FAIL omarchy-restart-shell (session locked, Decision 1);
                                                            the EXIT-trap restore ran, all paths absent again
$ SELDON_TEST_HOST=<alias> just e2e (locked)              → exit 1 before the backup: "the test host's session is locked"
$ just check                                              → exit 0 ("check: ok"; 17 cargo test suites ok, plugin-test ok)
$ just e2e --engine-only (after the last script edit)     → exit 0, 23 passed
```

The assertions the WP demands held in every passing full run:
- pill = index counts: `⟡ 1 · 2` from `activeCases 1, openDrift 2`, tone
  `urgent` (crisis 1);
- Today has the logged note (and, after the QuickEntry, that note too);
- ≥ 1 package event on the test host: 4 in the 7-day window;
- `state.status == "ok"` in the plugin after `status`;
- the log of the shell instance has no WARN or ERROR line naming
  jax.seldon. The positive control holds ("Configuration Loaded" present).

**Hosts restored. This is what I checked after the last run:**
- **Test host:**
  - `~/Seldon-e2e`, `~/.local/bin/seldon`, `~/.local/state/seldon`,
    `~/.config/seldon` and `~/.cache/seldon-e2e` are absent, as found;
  - the plugin dir hash is the one found (`f3e55bb59b11`), and the plugin
    is enabled, as found;
  - the service status is `engineMissing`, as found;
  - the theme is Osaka Jade, as found;
  - there is 1 `quickshell` process;
  - there are 3 crash reports: the one from the first run is new (FINDINGS
    §4.1), the other two are from August;
  - **the session is locked**; it was unlocked when I started.
- **Dev host:**
  - the real `~/.local/state/seldon` and `~/.config/seldon` are unchanged
    (`real-home-guard` in every engine-only run);
  - no `/tmp/seldon-e2e.*` is left;
  - the plugin was not copied to the dev install and not enabled.

## Learned (appended to memory/pitfalls.md, "WP-013")

- The `test` ssh alias does not exist here; pass the real name through
  `SELDON_TEST_HOST`.
- A fresh logbook records no history without `--since`, and a backfill
  turns package changes into drift and crises.
- `pgrep -x quickshell` also matches crash-report windows. Use `quickshell
  list -a -j`, plus a positive log line.
- A shell restart during the rsync hot-reload storm crashed the shell.
  Rsync with `--checksum`, settle first, and count the crash reports.
- A locked session blocks the restart and would receive the keys; check
  `omarchy-shell lock status` first.
- A killed run's remote command keeps running.
- Remote scripts go as a base64 argument with `ssh -n`, not on stdin.
- The guard reads `> 0` in a jq filter as a redirection.

## Decisions needed

1. **Guard block: `omarchy theme set` over ssh.**
   - My command was `ssh -n <test-host> '… omarchy theme set "Tokyo
     Night"'; echo "theme set exit $?"`. The trailing `; echo` takes it
     outside the guard's ssh-only exception, so it was blocked as a
     system change.
   - Per §11 and the memory rule, I did not retry it in another form. The
     theme stayed Osaka Jade.
   - The shots a parallel call took under the name `tokyo-night` showed
     Osaka Jade, and I deleted them.
   - Decide whether I (or the next live session) may run the bare form
     the exception allows, to finish the Tokyo Night and Catppuccin Latte
     shots. The steps are scripted (`shots <theme>` in the session
     helper); each theme takes about 1 min.
2. **Plugin follow-ups** from FINDINGS §5:
   - the `aggregations.overlay` counter (23 live vs 0 headless);
   - the heatmap filling only ~15 % of its slot, and the docs' probe
     point outside the grid.
3. **WP-040 and WP-024 live items** need an operator exception to the
   guard, or a stand-in: CI for the package build, a manual run by the
   operator for the theme hook.
4. **The operator's shell history on the test host** has the line from
   the incident (`Live smoke: dismissed, test host package`). Remove it
   if wanted; I did not touch it.

## Touched outside WP scope

- `memory/pitfalls.md` (append, as the brief asked). Nothing under
  `engine/`, `plugin/`, `schema/` or `fixtures/`.
