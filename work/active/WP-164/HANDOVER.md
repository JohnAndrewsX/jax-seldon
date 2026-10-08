# WP-164 — handover

Branch `wp/164-boot-config-hashes` from `next` (dda8b6aa). Not pushed.
Plan: `work/active/WP-164/PLAN.md`.

## What was done

- **Source.** `Sources::etc_dir` (`SELDON_ETC_DIR`, default `/etc`; under
  `SELDON_TEST_GUARD` without the variable `<guard>/etc`), every test
  bench spelled out. WP-114 (`pacman.conf`) can reuse it.
- **Collector** (`collectors/config.rs`). `boot_roots(etc)`:
  `mkinitcpio.conf`, `mkinitcpio.conf.d/`, `mkinitcpio.d/`,
  `default/limine`, `limine-entry-tool.conf`, `limine-entry-tool.d/`,
  added to the roots whatever `watchPaths` says (one a watch path covers
  is walked there). A directory among them is read one level deep, files
  only. `PACMAN_LEFTOVERS` (`.pacnew`, `.pacsave`, `.pacorig`) in a boot
  directory are not hashed. `system_hash`: content SHA-256 (any size up
  to 64 MiB), else `stat_hash` with `meta.hashBasis = "stat"` (also when
  the stored hash is reused). The content never leaves SHA-256.
- **Class.** Attention `config` / `config-remove` by ADR-0028 §2's total
  row; no new rule, no class code change, no contract change.
- **Tests.** `engine/tests/boot_hashes.rs` (7 tests: add / change /
  remove / same bytes re-extracted / main file and `default/limine`,
  hash only and nothing beside the listed files, presets and `.pacnew`
  merges, unreadable → stat, scope entry without events and
  `skipPaths` opt-out, no `/etc` at all, `alwaysRedPaths` opt-in to
  crisis), `attribution.rs` `a_boot_file_is_proven_by_its_absolute_path`
  (rule 1 by `sudo tee|sed -i|cp|rm`, not by `cat` or `mkinitcpio -P`),
  three unit tests in `collectors::config`.
- **Docs.** SPEC-ENGINE §2 (manifest row), §4 config (the boot
  paragraph, the measurement, class, crisis answer, opt-in/opt-out),
  §5 rule 4 (the attention list names the boot files); TESTING (row,
  `SELDON_ETC_DIR` in the scratch-environment note); guide 06 en/de;
  CHANGELOG.

## Measurement (test host, 2026-10-08, read-only, no root)

- `/boot`: the ESP, `vfat` `fmask=0077,dmask=0077`, `drwx------ root`.
  The user cannot list it nor `stat` `/boot/limine*.conf` (`Permission
  denied`): not even a stat hash is possible. And `limine-snapper-sync`
  (watcher service active; it reads `/etc/default/limine`) and
  `limine-entry-tool` (kernel / mkinitcpio hooks) regenerate the Limine
  config on every snapshot and every kernel or initramfs build: rewritten
  on routine transactions without a configuration change. **Left out**,
  as the WP says for such a file; SPEC §4 and guide 06 say so.
- `/etc`: every listed file `0644 root:root` (content hash).
  `mkinitcpio.d/` empty. Since install (2026-08-14): 97 transactions, 4
  kernel, 3 Limine, 4 `omarchy-settings` upgrades; ctimes show writes only
  by the owning package's upgrade (`omarchy-settings` 4.0.4 on 09-23,
  `limine-mkinitcpio-hook` 1.38 on 09-14) and migration 1789325478
  (`default/limine`, 09-23). `mkinitcpio.conf` and the hand-made drop-ins
  keep their install ctime through all four kernel upgrades. Omarchy's
  `bin/` writes none of the files (`grep` of the installed tree); install
  scripts, migrations and user-run tools (`omarchy hibernation setup`,
  `omarchy-provision-owner`) do. **No file rewritten without a change**;
  and a re-extraction of the same bytes gives the same content hash.
- The test host has `mkinitcpio.conf.d/omarchy_hooks.conf.pacnew` (from
  `omarchy-settings` 4.0.0, 08-17): the reason the leftovers are left out
  (WP-141's note already lists it).

## Crisis: does it apply? (open question)

Left out of this WP (orchestrator, 2026-10-09). ADR-0042 already makes a
`.pacnew` beside a boot file a crisis; a change of the boot file itself
stays attention here. The harm test holds for an unasked change (a wrong
`HOOKS` line stops the next boot), but an `omarchy-settings` upgrade
inside a plain `omarchy update` rewrites its drop-ins (09-23 on the test
host), so a crisis row would first need an exception for a package's
own extraction (the config event has no `txId`; the extraction's mtime
is the package's build time). Open question for the operator / a later
ADR: a boot-file change as crisis **unless** a transaction of the file's
owning package (`omarchy-settings`, `mkinitcpio`,
`limine-mkinitcpio-hook`, per ADR-0042's evidence) ran in the same
capture window. Until then a user opts in with `[drift] alwaysRedPaths`
(absolute patterns work; tested).

## Verification

- `XDG_RUNTIME_DIR=<private> SELDON_FULL_CHECK=1 JUST_TEMPDIR=<scratch>
  flock /tmp/seldon-check.lock just check`: **`check: ok`, exit 0**
  (2426 tests passed, 0 failed; docs-check ok). A first run died with
  exit 101 when the per-user quota of the `/tmp` tmpfs (`usrquota`) ran
  full mid-test (the log stopped mid-line; the harness reported
  `EDQUOT`): my 1 GB musl target dir in the scratchpad, on top of other
  sessions' scratchpads (WP-159 and WP-166 hold about 5.5 GB each, not
  touched). I deleted my own target dir; the second run passed.
- Hand mutants (`collectors::config` unit tests + `boot_hashes`), all
  killed: leftovers kept; boot directories walked deep; no stat hash
  (`hash_file`); boot roots not added; no `hashBasis`; roots only (no
  files in a root directory); leftover check outside boot directories;
  leftover suffix by `contains`.
- Release build (static musl) of this branch: built, copied to the test
  host (below), build dir deleted afterwards. The build dir was in the
  session scratchpad under `/tmp` (a RAM tmpfs): a mistake, cargo
  targets belong in the worktree's `engine/target` or a private gates
  target.

## Live measurement on the test host (passive, 2026-10-09)

The release build of this branch (`/tmp/seldon-wp164`, literal path),
`env -i` with scratch `HOME`, `XDG_CONFIG_HOME`, `XDG_STATE_HOME`,
`XDG_DATA_HOME` under `/tmp/s164`, `SELDON_TEST_GUARD=/tmp/s164`, a
private runtime dir `/tmp/r164` (0700), `SELDON_ETC_DIR=/etc`. Scratch
logbook `/tmp/s164/lb` (`init --non-interactive --no-capture`); only
`capture --source config` ran. C-2026-005 and the host's own logbook were
not touched; `/run/user/1000` was 10 % full before.

- 00:30 baseline: `ok`, 0 events. The manifest holds exactly the ten
  boot files the host has: `/etc/default/limine`,
  `/etc/limine-entry-tool.conf`, `limine-entry-tool.d/` (4 files),
  `/etc/mkinitcpio.conf`, `mkinitcpio.conf.d/` (3 files); the existing
  `omarchy_hooks.conf.pacnew` is left out; `skipped` empty (every file
  readable, content hashes); `mkinitcpio.d/` empty.
- 00:50 second capture: `ok`, 0 events, the ledger still empty; the
  ctimes of all ten files are as at 00:30.
- No pacman transaction in the window (the last one 2026-10-06), so the
  "after an update" capture is **pending**: there was no update to
  measure. The earlier passive evidence (97 transactions, ctimes; see
  Measurement) stands for it.
- Cleanup: `/tmp/seldon-wp164`, `/tmp/s164`, `/tmp/r164` deleted on the
  test host by their paths.

## Operator steps, pending

Agents cannot type the `sudo` password (orchestrator, 2026-10-09). With
a build of this branch (or the release that carries it) capturing on the
test host, the operator runs:

1. `echo '# seldon live test' | sudo tee /etc/mkinitcpio.conf.d/zz-seldon-test.conf`
   → the next `seldon capture` writes exactly one event: `config-add`,
   subject `/etc/mkinitcpio.conf.d/zz-seldon-test.conf`, `meta.hashTo` a
   SHA-256, no `hashBasis` (the file is 0644, readable), class attention
   (`config`); a further capture writes nothing.
2. `sudo rm /etc/mkinitcpio.conf.d/zz-seldon-test.conf`
   → the next capture writes exactly one `config-remove` of that subject,
   `meta.hashFrom` equal to step 1's `hashTo`, class attention
   (`config-remove`); a further capture writes nothing.

Neither file is read by `mkinitcpio` until the next initramfs build, and
step 2 removes it before one runs.

## Open

1. Crisis for boot-file changes (above): operator / later ADR.
2. ADR-0042's detail text ("Seldon does not read /etc, so it cannot tell
   whether that happened since") stays as it is (orchestrator,
   2026-10-09); so does the WP-141 CHANGELOG line. For the boot files a
   `pacdiff` merge now shows as a `config-change`.
3. The post-update capture of the live measurement (no update came).

## Guard blocks (reported, not routed around)

- `sudo -n true` on the test host (a probe whether I could edit `/etc`).
- The first scratch run (`fail closed … the command name is computed`:
  the binary was called through a shell variable). Rerun with the
  literal path on the orchestrator's go.
