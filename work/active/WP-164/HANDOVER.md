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

## Crisis: does it apply?

The harm test holds for an unasked change (a wrong `HOOKS` line stops
the next boot). I did **not** add a crisis row: ADR-0042 says widening
its list is a new ADR, and an `omarchy-settings` upgrade inside a plain
`omarchy update` rewrites its drop-ins (09-23 on the test host), so a
crisis row first needs an exception for a package's own extraction (no
evidence mechanism exists for that today: the config event has no
`txId`, and the extraction's mtime is the package's build time).
Recommendation: attention now; if the operator wants red, an ADR that
makes a boot-file change a crisis **unless** a pacman transaction of the
file's owning package ran in the same capture window (owner known from
ADR-0042's evidence: `omarchy-settings`, `mkinitcpio`,
`limine-mkinitcpio-hook`). Until then a user opts in with `[drift]
alwaysRedPaths` (absolute patterns work; tested).

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
  host (below), build dir deleted afterwards.

## Not done — decisions needed

1. **Live acceptance on the test host** ("an edit of a drop-in gives one
   event at the next capture and nothing else changes it"): not run.
   - The guard hook blocked `sudo -n true` on the test host (a probe
     whether I could edit `/etc` there). Reported, not routed around.
     Editing a drop-in needs root, so that step is the operator's.
   - The guard hook then blocked the scratch run itself (`fail closed …
     the command name is computed`: I called the binary through a shell
     variable). Per the guard rule I stopped and did not redo it in
     another wording. Nothing ran on the test host; only the binary was
     copied to `/tmp/seldon-wp164` there (delete it by that path when
     done).
   - Proposed run (scratch `HOME`, state and logbook under `/tmp/s164`,
     private runtime dir `/tmp/r164`, `SELDON_ETC_DIR=/etc`, `capture
     --source config` only; the engine reads hashes of the listed files
     and nothing else): baseline, a second capture (0 events), then the
     operator runs e.g. `echo '# seldon live test' | sudo tee
     /etc/mkinitcpio.conf.d/zz-seldon-test.conf`, capture (expect one
     `config-add`), `sudo rm` the file, capture (one `config-remove`),
     then captures over a while (0 events), `rm -r /tmp/s164 /tmp/r164`.
     Needs: the orchestrator's go for the run written with the literal
     binary path (or a guard rule for it), and the operator's two `sudo`
     steps. The test-host case C-2026-005 is not touched (scratch
     logbook).
2. **Crisis row** (above): operator decision / ADR.
3. **ADR-0042's detail text** ("Seldon does not read /etc, so it cannot
   tell whether that happened since") stays true for the content, but for
   the boot files a merge now shows as a `config-change`. The text is the
   ADR's; I left it and the WP-141 CHANGELOG line as they are.
