WP-013 HANDOVER

Branch `wp/013-integration`, worktree `wt/WP-013`, on `main` at `78aa1ff`.
Not pushed, no PR. Commits `main..HEAD`:
- `9798998` script;
- `cc007e8` justfile;
- `0fbce86` TESTING;
- `ff956d3` findings;
- `f5e1e97` memory;
- then this handover.

The restore refactor named under "Not done" is part of `9798998`.

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

Settled at the review:
- `SELDON_TEST_HOST`, default `test`; the operator adds the alias;
- the `+(members−1)` badge (plugin side);
- `init --json` (an engine follow-up);
- the guard false positive.

Open:

1. **Re-running the full variant.** It is pending the operator's unlock
   and stay-awake on the test host. After that: run
   `SELDON_TEST_HOST=<alias> just e2e` twice. This exercises the lock
   checks, the restore rules and the same-host guard (which passes there)
   live.

## Touched outside WP scope

- `memory/pitfalls.md` (append, as the brief asked). Nothing under
  `engine/`, `plugin/`, `schema/` or `fixtures/`.
