# WP-164 — plan

Branch `wp/164-boot-config-hashes` from `next` (dda8b6aa), merged into
`next`. AGENTS.md §6 (operator decision 2026-10-08, S8): hashes only of
`/etc/mkinitcpio.conf`, `/etc/mkinitcpio.conf.d/*`, `/etc/mkinitcpio.d/*`,
`/etc/default/limine`, `/etc/limine-entry-tool.conf`,
`/etc/limine-entry-tool.d/*` and `/boot/limine*.conf`.

## What the code does today

- The config collector (`collectors/config.rs`) walks `watchPaths` (all
  under `~` by default). No `/etc` source exists on `next`: WP-114
  (`pacman.conf`) is not built, WP-131 (printers) is parked unmerged.
  WP-131's branch is the pattern this WP follows (a fixed source beside
  `watchPaths`, stat hash when unreadable); nothing is cherry-picked.
- A file it cannot read is `skipped` outside the persistence paths: an
  unreadable file would be invisible.
- ADR-0028 §2 has no row for a config event outside `~`: the total row
  makes it attention (`config`; a removal `config-remove`).

## Measurement first (test host, read-only, no root; 2026-10-08)

- `/boot` is the ESP, `vfat` with `fmask=0077,dmask=0077`: the directory
  is `drwx------ root`. The user cannot list it nor `stat` a file in it
  (`Permission denied`): `/boot/limine*.conf` cannot be hashed, not even
  by its metadata. Besides, `limine-snapper-sync` (its watcher service is
  active; it reads `/etc/default/limine`) and `limine-entry-tool` (the
  kernel and mkinitcpio hooks) regenerate the Limine config on every
  snapshot and every kernel or initramfs build, i.e. on routine pacman
  transactions: a file rewritten without a configuration change (the
  WP-131 lesson). **Left out.** What the user configures for it is in
  `/etc/default/limine` and `/etc/limine-entry-tool.d/`, both hashed.
- Every `/etc` file is `0644 root:root`, readable: content hash.
  `/etc/mkinitcpio.d/` is empty on Omarchy 4 (no presets).
- Since install (2026-08-14): 97 pacman transactions, 4 kernel upgrades,
  3 limine and 4 `omarchy-settings` upgrades. The ctimes show writes only
  by the owning package's upgrade or a migration (`omarchy-settings`
  4.0.4 on 09-23 extracted its drop-ins; `limine-mkinitcpio-hook` 1.38 on
  09-14 its `limine-entry-tool.conf`; migration 1789325478 wrote
  `/etc/default/limine`). `/etc/mkinitcpio.conf` and the hand-made
  drop-ins keep their install ctime through all kernel upgrades.
  Omarchy's `bin/` writes none of the files; only install scripts,
  migrations and user-run tools (`omarchy-hibernation-setup`,
  `omarchy-provision-owner`) do. A package re-extracting the same bytes
  gives the same content hash: no event.
- `sudo` on the test host: blocked by the guard hook (reported, not
  routed around). The live edit of a drop-in needs root: the operator's
  step.

## Design

1. **Fixed source.** `Sources::etc_dir` (`SELDON_ETC_DIR`, default `/etc`;
   under `SELDON_TEST_GUARD` without the variable `<guard>/etc`, so no
   test reads the host's). WP-114 can reuse it. `boot_roots(etc)`: the
   three files and three directories. The collector adds them to its
   roots whatever `watchPaths` says (a user list is never widened,
   ADR-0028 §4d). Opt-out: `[redaction] skipPaths` (`/etc/mkinitcpio*`),
   `[collectors] config = false` for all.
2. **Only the listed files.** A directory root is read one level deep,
   its files only. pacman's `.pacnew`, `.pacsave`, `.pacorig` there are
   left out: the tools read only `*.conf` (or a preset), and WP-141's
   `pacman` note records each already (a second, attention item for the
   same file would only double it).
3. **Hash only; stat hash when unreadable.** Content SHA-256 (`hash_any`,
   up to 64 MiB); a file that cannot be opened or is larger gets
   `stat_hash` (size, mtime ns, ctime, inode; ADR-0037/WP-113) and its
   event `meta.hashBasis = "stat"`. The content goes into SHA-256 and
   nowhere else. The `FileStat` reuse is unchanged.
4. **Upgrade.** The first capture after the upgrade sees the boot roots
   as new scope: the files enter without events (`watch scope changed: …
   entered it`), WP-069's rule.
5. **Events and class.** `config-add|change|remove`, subject the absolute
   path. Class attention `config` (`config-remove`) by ADR-0028's total
   row; no new rule, no contract change. **Crisis:** the harm test holds
   for an unasked change (a wrong hooks line stops the next boot), but a
   crisis row is ADR material (ADR-0042: "widening the list is a new
   ADR"), and `omarchy-settings` upgrades rewrite its drop-ins inside a
   plain `omarchy update`: a crisis row would need an exception for a
   package's own extraction first. The user can opt in today with
   `[drift] alwaysRedPaths` (absolute patterns match). Recommendation in
   the handover.
6. **Open case.** Rule 1 links when an agent's command writes the path
   (`sudo tee /etc/mkinitcpio.conf.d/x.conf`, `sed -i … <path>`); rule 9
   when the active case's Plan names it.

## Steps (one commit each)

1. `Sources::etc_dir` + `SELDON_ETC_DIR`, guard default; test benches.
2. Collector: boot roots, one level, leftovers out, `system_hash`,
   `hashBasis = "stat"`. Unit tests.
3. Integration tests (`engine/tests/boot_hashes.rs`): baseline, add /
   change / remove in a drop-in, `mkinitcpio.conf` change, unreadable
   file (stat; skipped as root), nothing else under `/etc` read,
   `.pacnew` left out, idempotent second capture, upgrade enters scope
   without events, `skipPaths` opt-out, class attention, `alwaysRedPaths`
   opt-in to crisis, rule 1 link by an agent's `sudo tee`.
4. SPEC-ENGINE §2/§4/§5, guide 06 en/de, CHANGELOG, TESTING env var.
5. `just check` green, hand mutants, live run on the test host (scratch
   HOME and logbook, private runtime dir), HANDOVER.md.
