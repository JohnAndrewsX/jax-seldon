# WP-113 — handover

Branch `wp/113-collector-hashes`, from `next` at 38a9103 (`next` has not
moved since); merge into `next`. Plan and decisions D1–D10:
`work/active/WP-113/PLAN.md`. The amending ADR (round 2): `decisions/ADR-0037-adr-0028-amendment-toggles-and-links.md` (proposed); the round-1 note it replaced is removed.

## What was done

| Line item | Where | Result |
|---|---|---|
| Plugin trees (operator-approved, hashes only) | `collectors/plugins.rs` | Every listed third-party plugin's `~/.config/omarchy/plugins/<id>/` is hashed as one tree: all files at any size and content, `.git` and `[redaction] skipPaths` left out, links to files followed, any other link counted by its target string and not walked (Omarchy refuses links inside a plugin folder). A tree change is **one** `plugin-update` with `meta.hashFrom/hashTo` (with `from`/`to` too when the version moved; detail `files changed (sha256 <8> → <8>)` otherwise). A stat fingerprint per tree in the cursor skips the reading of an unchanged tree. Old cursor / new plugin: tree taken without an event; unreadable tree: last hash kept; first-party: no tree. Class: the existing `plugin-*` row (attention); `jax.seldon` is auto-explained by rule 8 (tested end to end). |
| Toggles directory (operator-approved, hashes only) | `config.rs` `DEFAULT_WATCH_PATHS`, `collectors/config.rs` | `~/.local/state/omarchy/toggles` is a default watch path; 0.1.4's default list is an "earlier default", so it gains the path at the next capture (ADR-0028 §4d mechanism). `omarchy-default` evidence maps `toggles/<app>/<rel>` ↔ `$OMARCHY_PATH/default/<app>/toggles/<rel>` (what `omarchy-hyprland-toggle on` copies): a flag turned on is routine, other Lua there attention. **Turning a flag off is a `config-remove` = quiet attention** under the current row; the capture already records the evidence on such a removal (amendment B makes it routine, a three-line classifier change). |
| `~/.ssh/authorized_keys` opt-in | `config.rs` `DEFAULT_ALWAYS_RED_PATHS` | Not a default watch path. Default `alwaysRedPaths` gains it, so adding it to `watchPaths` is the whole opt-in; a change is then a crisis `always-red-paths`, only the hash recorded. Documented in SPEC §2/§4 and guide 06 en/de (PLAN D7 said "doctor and SPEC": no doctor row was added — a hint for an unwatched opt-in would be noise on every machine). |
| Hook-path blind spots (WP-109 stage 1) | `collectors/config.rs` | Under the configured `alwaysRedPaths`: binary files and files over 1 MiB are hashed (streamed, `sys::Sha256`); a directory link at or below a persistence path is followed (loop guard on canonical targets, ≤ 1024 files below one link, the rest `skipped` and counted, a directory's files before its subdirectories). Each of the three cases yields its event and is a crisis (tested). A file that moves from `skipped` to hashed (the upgrade) is no event. |
| `system-link` narrowing (WP-109 stage 2) | — | **Not implemented**: changes an ADR-0028 row. Proposed as amendment A (row text, capture change, test sketch, one unchecked evidence item — see open questions). |
| Baseline hash list (WP-109 stage 2, "consider") | — | Considered, not built (D9): a baseline is no change, and Omarchy's shipped autostart entries would be crises at install. Proposal: a later `doctor` row naming persistence-path files no ledger event ever recorded. |
| yay/paru value options (WP-109 stage 2) | `pkgcmd.rs` `LONG_WITH_ARG`, `scripts/validate-fixtures.py` | yay's (`man 8 yay` on the dev host) and paru's options with a required value consume it; `yay -Syu --answerdiff None` is a plain full upgrade. Optional-value options (paru `--chroot`, `--localrepo`, `--sign`) only as `--opt=value`. |
| Fixture (0.2.0 form) | `fixtures/logbook/ledger/2026-09.*`, derived index files | 09-22: `weather-plus` edited in place → one tree `plugin-update`, explained (a quiet Changelog row). Placed before the 7-day window so `STATUS.md`, the session-start golden and `summary.events7d` stay as they are. |

## Decisions worth a second look

- **D1 — `plugin-update`, not `config-change`.** The 0.2.0 form's fixture
  line says "one `config-change` of a plugin tree"; ADR-0028 §8 WP-E and
  the WP goal say "one `plugin-update` on a tree-hash change". I followed
  the ADR: one collector gives one event for a `git pull` (version and
  tree), rule 8 keeps `jax.seldon`'s own updates and the dev install
  quiet, and the `plugin-*` row already classifies it. The fixture
  therefore carries a tree `plugin-update`.
- **D2 — attention, not routine.** "So the desk's Changelog shows it as
  routine" would be a row change (F4 kept in-place edits of unsandboxed
  code visible). The fixture event is explained, so it renders as an
  ordinary quiet row.
- **D6 — toggle off stays attention until amendment B.** I did not
  change the classifier for removals: it is a row change. If B is not
  accepted before 0.2.0, every *off* of an Omarchy toggle is one quiet
  drift item (never in the bar).

## Capture cost (`just check-perf`, bench profile, tmpfs, dev host)

Synthetic home: 115 files under the earlier default paths; items added
one by one. Before = `next` at 38a9103 with the same home (blobs were
skipped there).

| Measure | Before | After |
|---|---|---|
| config, warm (stat cache), full home | 1.01 ms | 1.34 ms |
| config, cold (no manifest), full home | 2.04 ms | 25.1 ms |
| ↳ earlier default paths only (warm / cold) | — | 1.11 / 2.17 ms |
| ↳ + toggles directory, 6 files | — | 1.14 / 2.22 ms |
| ↳ + 4 MiB binary and 2 MiB script in a hook directory | — | 1.24 / 24.6 ms |
| ↳ + `authorized_keys` (opt-in) | — | 1.34 / 25.1 ms |
| plugins, warm (stub `omarchy` process ≈ 20 ms of it), 8 trees × 42 files incl. a 2 MiB image | 20.6 ms | 22.1 ms |
| plugins, cold trees (no fingerprint) | 20.6 ms | 88.7 ms |

Cold costs are paid once per change (the stat caches keep warm runs at
≈ +0.3 ms config, +1.5 ms plugins); `sys::Sha256` (no crate allowed)
runs at ≈ 250 MB/s. The rest of `check-perf`: index ×10 median 9.8 ms,
×150 median 64.7 ms (first attempt 106 ms on a busy host — other agents
were compiling — the second attempt passed, as the recipe allows),
`status` at the stated scale 45.9 ms, hooks ≤ 4.6 ms, redaction within
budget. Exit 0.

## How it was verified

- New tests: `engine/tests/collector_hashes.rs` (CLI, temp HOME: the
  three blind spots → crises, hashes only, idempotent; linked hook dir,
  loop and budget; skipped → hashed without an event; toggles evidence
  on/off; `authorized_keys` opt-in; plugin trees classified, rule 8 for
  `jax.seldon`), `collectors_user.rs` plugins (in-place edit = one
  update, version + tree = one event, what a tree holds, old cursor,
  unreadable tree, lost cursor save repeats nothing), `pkgcmd.rs`
  (helper options), `sys.rs` (stream = one shot), `capture_cost.rs`
  (ignored, `just check-perf`).
- Changed expectations: `drift_classes.rs` upgrade test (0.1.4's list is
  now an earlier default); fixture counts 83 → 85 ledger lines, ×10 830 →
  850, stated scale 10 292 → 10 540 (SPEC §6, TESTING.md, justfile,
  `scale.rs`, `tests/index.rs`), the `drift-explained-case` overlay index
  67 → 68, plugin harness pins (74 events, 9 folded resolutions, 13/8
  drift opened/resolved, 69 events and 15 active days in 30 days) plus
  one assertion for the new row. No plugin code changed.
- `flock /tmp/seldon-check.lock just check`: **exit 0** (`check: ok`; fmt, clippy incl. `watch`, all engine test binaries, packaging, install, deploy, schema-validate, docs-check, `omarchy plugin validate`, `qmllint: ok (49 files)`, plugin harness incl. `model.test.js` 107 passed). Run at d049dcf (only this handover file followed).
- Not run: a live check on the test host (toggles through Omarchy's
  menu, a real plugin edited in place). Tests run as a user here; the
  "unreadable tree" assertion only runs for a non-root user (CI is root:
  root reads it anyway, the test skips that half).

## Overlaps

- **WP-120** (contract v2, fixture v2): this WP edits
  `fixtures/logbook/ledger/2026-09.{jsonl,md}`, every derived index file,
  `fixtures/README.md`, the overlay index in `scripts/validate-fixtures.py`
  and the harness pins in `tests/plugin/model.test.js`. All of it is one
  commit (`d049dcf`), last; on a conflict, re-insert the two ledger lines
  and the view lines and run `python3 scripts/validate-fixtures.py
  --write-index`. Schema: untouched. The schema's description of the
  conventional meta keys says `hashFrom/hashTo (config-*)`; it could add
  `plugin-update` — cosmetic, left to WP-120.
- **WP-114** (`/etc/pacman.conf` hashes): no file in common except
  possibly `config.rs` defaults; not touched by me beyond
  `DEFAULT_WATCH_PATHS`/`EARLIER_DEFAULT_WATCH_PATHS`/
  `DEFAULT_ALWAYS_RED_PATHS`.

## Open questions

1. **Amendment A** (narrow `system-link`) and **B** (toggle off routine):
   orchestrator → operator. B is cheap and makes the toggles line item
   fully quiet; recommended before 0.2.0.
2. Amendment A's evidence: which targets `--user enable` of a packaged
   unit and Omarchy's install scripts create. My read-only `grep` over
   `$OMARCHY_PATH/{migrations,install,bin}` was **blocked by the guard
   hook** (the pattern named the service manager's command; "service or
   boot command"). Reported, not routed around; WP-D on the test host
   can answer it.
3. Attribution: an agent's in-place edit of a plugin file (Edit tool)
   yields a tree `plugin-update` that is not attributed to the agent's
   case (attribution proves `plugin-update` only by `omarchy plugin
   update`). A follow-up could attribute hook-recorded file writes under
   the plugins directory.

## Round 2

Brief: `WP-113-round-2-brief.md` with the stage-1 packet `WP-113-review-1.md` (SEND BACK: B1–B3, N1–N7); D1/D2 agreed. Round-2 commits: `ebecb59` … HEAD.

| Item | Fix | Tests (new or changed) | Mutants |
|---|---|---|---|
| **B1** link walk | One walk-wide set of directories walked (device + inode): a link to one of them is not followed, and below a link no directory is walked twice (outside links the tree has no loops of its own). One walk-wide budget, `LINKED_ENTRIES` = 4096 entries (files, directories, links) read below links. A link whose walk runs out of it is **cut off**: what was read below it is dropped, the files the manifest had below it keep their last hashes, and the link itself becomes one manifest entry (hash of `CUT_OFF`): event detail `linked directory cut off: …`, `meta.cutOff: true`, any evidence mark dropped. **Class: crisis** — the entry lies on its persistence path (the existing `alwaysRedPaths` row), and a persistence directory nobody can see into is the harm test's case: decoys must not hide a payload. When it fits again: `linked directory watched in full again` (a removal, attention) plus the real changes made meanwhile. | `collector_hashes.rs`: `a_back_link_to_an_ancestor_is_not_followed` (no outer link), `a_two_link_cycle_is_walked_once` (A→B→A outside HOME), `below_links_no_directory_is_walked_twice`, `a_diamond_of_links_is_linear` (the reviewer's probe: 2 links/level, 17 levels, < 1 s, manifest < 64 KiB; measured ≈ 40 ms debug), `a_link_past_the_budget_is_cut_off_and_a_crisis` (4096 decoys + a payload + a change to a known file: one crisis on the link, idempotent, manifest bounded; decoys gone → payload add + the change + the link's removal) | guard on link targets off → 4 tests fail (back-link, cycle, diamond, budget); guard on directories below links off → `below_links_no_directory_is_walked_twice` fails (both halves of the guard caught, cf. M1a/M1b) |
| **B2** menu toggles | ADR-0037 §1 (proposed, one amending ADR with A and C): under `~/.local/state/omarchy/toggles/` an event whose content (`hashTo`, removal `hashFrom`) is empty is routine `toggle-flag`; a removal with `omarchy-default` evidence is routine `omarchy-default`; anything else there stays attention. `toggle-flag` joins `[drift] routine` defaults; the fixture validator ports the rule. Verified on the dev host: `omarchy-toggle` `touch`es / `rm`s the flag. `work/active/WP-113/ADR-0028-AMENDMENT-PROPOSED.md` removed; DECISIONS.md row added (ADR-0028's row notes the proposed amendment). | `toggles_are_routine_both_ways` (menu flags on and off, Hyprland copy on and off, foreign Lua attention, a flag that gains content attention; only those are open items; idempotent) | M3d (toggle-off mark dropped) → fails `toggles_are_routine_both_ways` (mark assertion) |
| **B3** unreadable plugin entry | An entry that cannot be read (file not openable, directory not listable) is the line `<rel> NUL unreadable <sha256 of size, mtime, mode>` (directory `<rel>/`), so the tree changes once and the rest is still hashed; `partial: true` in the cursor and the event meta; the collector's message counts unreadable entries. Only an unreadable plugin directory keeps the last hash. | `collectors_user.rs`: `an_unreadable_entry_does_not_freeze_the_tree` (the reviewer's `chmod 000` + QML edit → one event, `partial`; further edit → one event; idempotent; unreadable directory; readable again; unreadable plugin directory keeps the hash). The old `…_kept_when_unreadable` became `a_tree_is_taken_without_an_event`. Skipped as root (CI): root opens every file. | — |
| **N2** caps | Where every file is hashed (persistence paths, plugin trees) a file over `STAT_HASH_ABOVE` = 64 MiB is not read: SHA-256 of `stat <size> <mtime ns> <inode>`, `meta.hashBasis = "stat"` (config event; plugin event when the tree has such a file). Plugin trees: sorted walk, `TREE_ENTRIES` = 10 000 entries, cut off past them (`NUL cut` line, `partial`, counted). Omarchy copies over 64 MiB are never evidence. | `a_huge_hook_is_hashed_by_its_metadata` (2 GiB sparse hook < 2 s, `hashBasis`, the fingerprint recomputed), `a_tree_is_capped_and_huge_files_count_by_metadata` (2 GiB sparse plugin file; a touch is one event with `hashBasis`; 10 000-entry cap: `partial`, message, an edit inside the cut shows, past it not) | — |
| **N3** (review N4) links into Seldon | No link — to a file or a directory — whose canonical target lies in the logbook, the state directory or `~/.config/seldon` is followed; counted in the message. | `a_link_into_the_logbook_is_not_followed` (dir link to `ledger/`, dir link to the state dir, file link to `STATUS.md`: four captures, 0 events) | — |
| **N4** (review N5) stat-cache test | — | `the_tree_fingerprint_is_reused_but_not_fooled`: mtimes set into the past → the cursor keeps the fingerprint; an edit with the same size and the mtime put back → one event | M3c (fingerprint always reused) → fails this test |
| **N1** unreadable round trip | Done (small): an unreadable file under a persistence path keeps its last hash (no stat entry), so the content it has when readable again is compared; a file never read is `skipped` as before; counted in the message. | `an_unreadable_round_trip_is_seen` (`chmod 200`, edit, `chmod 755` → one `config-change`, crisis). Skipped as root. | — |
| **N5** (review N6) paru | SPEC §5 says paru's option list is unchecked (paru not installed on the dev host). | — | — |
| **N7 / C** | In ADR-0037 §3 with A (§2, decided there, implemented by a follow-up WP after the evidence check) and B (§1). The ADR names the four ADR-0028 §2 rows it changes. | — | — |

**Not merged:** per the brief, this branch waits for the operator's acceptance of ADR-0037.

**Capture cost after round 2** (`just check-perf`, exit 0): config warm 1.11 / 1.15 / 1.26 / 1.33 ms and cold 2.16 / 2.20 / 25.7 / 24.7 ms (earlier defaults / + toggles / + hook blobs / + `authorized_keys`); plugins warm 22.8 ms (+0.7 ms: every file is opened once to tell an unreadable one), cold trees 89.2 ms. Index ×10 9.1 ms, ×150 68.3 ms (first attempt 115 ms on a busy host, second within budget, as the recipe allows), `status` 44.1 ms, hooks ≤ 4.4 ms.

**Gate:** `flock /tmp/seldon-check.lock just check` → **exit 0** at `03ce1b5` (`check: ok`; 84 test binaries ok, `qmllint: ok (49 files)`, `model.test.js` 107 passed, plugin harness ok); only this handover followed (log `check-wp113-r2.log`).

**Process note (review §5 Q6):** my logs this round have unique names (`perf-wp113-r2.log`, `check-wp113-r2.log`); round 1 used `scratchpad/check.log` in this session's own scratchpad.

**Open:** ADR-0037 acceptance (operator); §2's evidence item (WP-D, test host) and its implementation; the agent-edit attribution follow-up (round 1, open question 3).

## Round 3

Brief: `WP-113-round-3-brief.md` (Fable stage 2: SEND BACK for a small round; B2, B3, N2 and N4 closed; Fable advises accepting ADR-0037 with the wording edits). Round-3 commits: `50ba134` … HEAD. Every fix was reverted alone once and its test failed (mutant column).

| Item | Fix | Test | Mutant (fix reverted) |
|---|---|---|---|
| **R1** cut leaked out of its link | `Walker::follow` clears `follow.cut` after recording the cut, so the walk above the link and the later watch paths go on. | `a_cut_stays_in_its_link` (Fable's layout: theme Lua, `hooks/zz.d`, waybar, a user unit; `post-update.d` → 4097 decoys → exactly one event, the link's add; idempotent). `a_link_past_the_budget_is_cut_off_and_a_crisis` unchanged and green (clearing the cut gives only its expected events). | fails `a_cut_stays_in_its_link` |
| **R2** stat hash without ctime | `stat_hash` = `stat <len> <mtime_ns> <ctime> <ctime_nsec> <ino>`; the plugin trees' `unreadable_hash` gets the change time the same way (a chmod round trip is a tree change). Test helper `seldon_stat_hash` updated. | `a_huge_hook_is_hashed_by_its_metadata` and `an_unreadable_entry_does_not_freeze_the_tree` gain the `touch -d` case (write in place, size and mtime restored → one event; the plugin case on a `chmod 200` file, `partial`) | config: fails `a_huge_hook_is_hashed_by_its_metadata`; plugins: fails `an_unreadable_entry_does_not_freeze_the_tree` |
| **R3** link to an ancestor of Seldon's dirs | Below a link, a non-link directory that leads into the logbook, the state dir or `~/.config/seldon` is not walked (`scan.own += 1`). | `a_link_to_an_ancestor_of_seldons_dirs_stays_out_of_them` (`post-update.d` → `~/.local`: four captures, the message `1 link(s) into Seldon's own files not followed` each time, no `/seldon/` subject, idempotent) | fails that test |
| **R4** `authorized_keys2` | `~/.ssh/authorized_keys2` in `DEFAULT_ALWAYS_RED_PATHS` and the fixture validator; SPEC §2/§4, guide 06 en/de ("add both lines"), concepts en/de, CHANGELOG. | `authorized_keys_is_a_crisis_once_watched` opts in both and checks the second file → crisis | — |
| **N-a** every toggles file hashed | Files under `~/.local/state/omarchy/toggles/` are hashed whatever they hold (`hash_any`, the stat hash above 64 MiB). The general attention paths stay a follow-up. | `every_file_in_the_toggles_directory_is_hashed` (a binary and a 2 MiB Lua in `toggles/hypr` → attention `config`) | fails that test |
| **ADR-0037 wording** | §3 title and text name both files and "adding both to `watchPaths`"; Consequences: stat hash with the change time ("a `touch` — and an in-place write, whose change time no user can reset — changes it") and every toggle-folder file hashed; §2 evidence gap adds `/etc/systemd/user/`; the §1 consequence stands without a qualifier; DECISIONS.md row follows. | docs-check ok | — |

**Capture cost after round 3** (`just check-perf`, exit 0, `perf-wp113-r3.log`): config warm 1.13 / 1.14 / 1.21 / 1.35 ms and cold 2.14 / 2.15 / 24.2 / 23.7 ms (earlier defaults / + toggles / + hook blobs / + `authorized_keys`); plugins warm 22.5 ms, cold trees 87.0 ms; index ×10 4.6 ms, ×150 56.1 ms (first attempt), `status` 44.2 ms; hooks and redaction within budget.

**Gate:** `flock /tmp/seldon-check.lock just check` → **exit 0** at `cad7ff3` (`check: ok`; 84 test binaries ok, `qmllint: ok (49 files)`, `model.test.js` 107 passed, plugin harness ok); only this handover followed (log `check-wp113-r3.log`).

**Open (follow-ups, from the brief):**
- A `doctor` row "N linked directories cut off" (N-c).
- Hashing every file on attention paths in general (`~/.config/hypr/**`, themes), with a cost line for the operator.
- `omarchy-toggle-input-device`'s non-empty `<kind>-disabled-name` flag (N-b): measure it on the test host before widening ADR-0037 §1.
- A stat line with the change time for unreadable persistence files that only the hook can read (N1 rest).
- From earlier rounds: ADR-0037 acceptance (operator; merging into `next` waits for it), §2's evidence check on the test host and its implementation, attributing an agent's in-place plugin edit to its case.
