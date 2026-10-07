# WP-113 — handover

Branch `wp/113-collector-hashes`, from `next` at 38a9103 (`next` has not
moved since); merge into `next`. Plan and decisions D1–D10:
`work/active/WP-113/PLAN.md`. Proposed ADR-0028 amendment (items A, B;
C recorded): `work/active/WP-113/ADR-0028-AMENDMENT-PROPOSED.md`.

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
