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
- **Tests.** `engine/tests/boot_hashes.rs` (6 tests: add / change /
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

## Round 2

Stage 1 (Opus) approved with notes N1–N3
(`review-0.1.1/handovers/WP-164-review-1.md` in the private folder).

- **N1** (mutant M5 survived): `collectors_user.rs`
  `config::a_failed_write_keeps_the_stat_basis_of_a_boot_file`. An
  unreadable drop-in (mode 0, mtime an hour back so its stats are
  stored) is added; the ledger write of its `config-add` "fails" (events
  dropped, cursor not saved); the retry reuses the stored hash and the
  re-reported `config-add` still carries `meta.hashBasis = "stat"`, same
  `hashTo`; a further capture is quiet. Skipped as root. M5 (`hashBasis`
  only when the hash was computed fresh) applied by hand: this test fails
  (`tests/collectors_user.rs:2147`); restored.
- **N2**: `docs/user/de/06-configuration.md` source line at `83ec84fb`,
  the English page's commit after N3. `docs-check` no longer warns for
  page 06. Its one remaining warning (`de/01-getting-started.md` at
  9daa7a3) is on `next` already (fd7b741d), not from this WP.
- **N3**: SPEC-ENGINE §4 and guide 06 en/de: a symlink in a boot
  directory is followed to its file and recorded under the link's own
  path, hash only; SPEC also says a link into `/usr/` carries
  `system-link` (routine) and that directory links, FIFOs, sockets and
  devices are not opened. The handover's test count is fixed: 6 tests in
  `boot_hashes.rs`.
- Unchanged by decision: the two `sudo` operator steps stay pending (they
  gate the release, not the merge); crisis-unless-owner-window is a later
  ADR (WP-170); ADR-0042's text stays.

**Check:** `XDG_RUNTIME_DIR=/tmp/r164b` (0700, removed afterwards),
`SELDON_FULL_CHECK=1`, `JUST_TEMPDIR=<scratch>`, `CARGO_TARGET_DIR`
unset (the worktree's `engine/target`, on disk), `flock
/tmp/seldon-check.lock just check`: **`check: ok`, exit 0**, 2428 passed,
0 failed; no Quickshell harness skipped; `/run/user/1000` 2 % before.

## Round 3

Stage 2 (Fable) sent back finding 1
(`review-0.1.1/handovers/WP-164-stage2-fable.md`); `next` merged first
(13c992cc, brings WP-170's queue file).

- **Finding 1 (blocking), fixed:** `evidence()` grants `system-link` only
  to a subject under the home directory (`key` starts with `~/`). A boot
  file linked into `/usr/` is a `config-add`/`config-change` without
  `meta.matches`, attention `config`, like any other boot change (ADR-0037
  §2 gives the mark to two home paths only). Test
  `boot_hashes.rs` `a_boot_symlink_into_usr_is_no_system_link` (a drop-in
  linked to the first readable of `/usr/share/zoneinfo/UTC`,
  `/usr/lib/os-release`, `/usr/share/licenses/glibc/LICENSE`; skipped
  without one): one `config-add`, hash only, no `matches`, class
  attention `config`, quiet afterwards. With the `~/` check taken out the
  test fails; restored. SPEC §4: the boot paragraph says a link there
  carries no evidence mark, and the evidence-marks sentence limits
  `system-link` to the home directory; guide 06 en/de say a link to a
  shipped file counts like any other change; the de source line is at
  `51759fb3`.
- **Finding 2, done:** `schema/event.schema.json` `subject` description:
  "path (~-relative; absolute for the boot files under /etc, WP-164)".
  Description only, no contract bump.
- **Finding 4, done — as an index variant, not in the sample logbook:**
  `fixtures/index-variants/boot-config.json` (overlay in
  `scripts/validate-fixtures.py`, README table row): a `config-change` of
  `/etc/mkinitcpio.conf.d/omarchy_hooks.conf` and a `config-add` of
  `/etc/mkinitcpio.conf.d/99-omarchy-provisioning-key.conf` with
  `meta.hashBasis: "stat"`, inserted by time on 09-23, both open
  attention drift items, `summary.openDrift: 8`. I first added them to
  the sample logbook: that moved every count and list the plugin tests
  and harnesses walk (7 model tests failed at once: counts, 83 events,
  periods, "+244 more"), so I reverted it and followed the existing
  index-only pattern (`case-reopened`, `drift-explained-case`). New model
  test `WP-164: boot-file events with absolute subjects under /etc`
  (parse, counts, the two rows by absolute subject, `lastSegment`); the
  desk harness renders the variant in every chart (`radiant-variant-
  boot-config`). `validate-fixtures`: ok, 10 variants.
- Finding 3 (`PACNEW_HINT` wording) is WP-170's, not done here.

**Check:** `XDG_RUNTIME_DIR=/tmp/r164c` (0700, removed afterwards),
`SELDON_FULL_CHECK=1`, `JUST_TEMPDIR=<scratch>`, `CARGO_TARGET_DIR`
unset (the worktree's `engine/target`, on disk), `flock
/tmp/seldon-check.lock just check`: **`check: ok`, exit 0**, 2430 passed,
0 failed, model tests 185 passed; `/run/user/1000` 2 % before. docs-check's
one warning (`de/01-getting-started.md`) is still `next`'s.

## Live result (orchestrator, 2026-10-09, fresh test host, 0.1.4+next.4dfb9bdf)

The operator ran both steps on the freshly installed test host.

1. `tee` of `/etc/mkinitcpio.conf.d/zz-seldon-test.conf` (0644, 19 bytes)
   → exactly one `config-add`, zone yellow (open drift, attention),
   `meta.hashTo` 18bc6aaa…ef431b, equal to `sha256sum` of the file; no
   `hashBasis`; the next capture wrote 0 events. PASS.
2. `rm` of the file → exactly one `config-remove`, zone yellow,
   `meta.hashFrom` equal to step 1's `hashTo`; two further captures wrote
   0 events. PASS.

The first captures after the deploy seeded the boot-file baseline
silently (0 events). Both items were then explained in one completed case.
