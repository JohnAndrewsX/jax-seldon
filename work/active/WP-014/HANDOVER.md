# WP-014 HANDOVER

Branch `wp/014-contract-followups`, worktree `wt/WP-014`. Not pushed, no PR.
Commits: `2d99a0d` schema · `354e620` collector logs · `6a7de7a` derivation +
ledger + sample index + variants · `97ab7ce` README + pitfalls · this handover.
Every commit passes `bash scripts/validate-fixtures.sh` on its own (checked from
`git archive` exports).

## Done
- **`schema/index.schema.json`** (contractVersion stays 1):
  - `$defs/drift` gains optional `txId` (string, minLength 1) and `members`
    (integer ≥ 2). They are present **together and only on groups**, and
    `members` requires `source: pacman` (`allOf` if/then). The descriptions say
    that a group's ts/source/kind/subject/detail/actor/proposedCase are the
    leader's, that `eventId` is the leader's id (ADR-0008 unchanged), and that
    the zone of a pacman item is computed (ADR-0013 §3).
  - `series.timeline[].ts`/`end` descriptions state the two forms (ADR-0012
    §12): `YYYY-MM-DD` for case spans, RFC 3339 date-time for release, snapshot
    and crisis; `end` is for case spans only.
  - `events`, `drift` and `series.drift` descriptions name `resolutionDetail`,
    grouping and the per-item counting.
- **`schema/event.schema.json`**:
  - `resolutionDetail` (index only, ADR-0012 §11). It is defined here, next to
    `resolution`, because `index.events` items are `$ref: event.schema.json` with
    `additionalProperties: false`. This follows the `case.path`/`case.steps`
    precedent. A rule keeps it out of ledger lines: `resolutionDetail` requires
    `resolution`, and `kind` must not be `resolution`. New must-fail fixture
    `invalid/event.resolution-detail-in-ledger.json`. The validator also
    rejects it in any ledger line.
  - `meta.enabled` (boolean; plugin-add/-enable/-disable, §15) and
    `meta.txId` (string; **only on `kind: resolution`**, enforced by if/then;
    ADR-0013 §4) are declared, and the conventional-keys description lists both.
- **Ledger + logs**: the open caseless full upgrade is appended to
  `ledger/2026-09.jsonl` at 2026-09-30 21:41, after the file's last line, so
  the month stays chronological:
  `firefox`, `libinput`, `noto-fonts` · `upgrade` · `explicit: false` ·
  `actor: system` · `zone: red` (event zone, ADR-0014 §2) · `txId
  tx-20260930T214115` · `meta.command "pacman -Syu"`. ULIDs follow the fixture
  convention (time part = ts, random part = hash). All existing ids are
  unchanged. The same transaction block is in `logs/pacman.log` and
  `logs/pacman-rotation/pacman.log`, so "the log produces exactly the ledger's
  pacman events" stays true. The baseline offset 6129 is unchanged, and complete
  lines now end at 11159 (README updated). `ledger/2026-09.md` (generated view)
  has the three lines.
- **`index.sample.json`** (regenerated with `--write-index`): `drift` has
  **exactly 4 items**: tokyo-night (yellow), ollama.service (red, crisis),
  ollama (red, crisis), and the group `01M3SXBQVR7AW8PJQC1YXDCQ14` (firefox
  leader, `zone: yellow`, `crisis: false`, `txId: tx-20260930T214115`,
  `members: 3`). The three existing rows are byte-identical. `summary.openDrift`
  4, `crisis` 2. Seven folded events carry `resolutionDetail`. `events` has 58
  items, `series.drift` W40 opened 6.
- **`scripts/validate-fixtures.py`**:
  - Grouping per txId (ADR-0013 §1). Leader = lowest-id explicit member, else
    lowest-id member.
  - Routine class and zone (§3). Routine means: kind `upgrade`/`reinstall`,
    `explicit` is false, and `meta.command` is split on whitespace (pacman logs
    argv unquoted). It must be `pacman`, the sync op only, with `-u`/`--sysupgrade`
    and no package word. Options that take an argument consume it (`--overwrite`,
    `--config`, `-b`, `-r`, `--ask`, …). An unknown option is assumed to take
    none, so in doubt the item is red.
  - The `alwaysRed` default is the fnmatch globs `linux*`, `systemd`, `glibc`,
    `hyprland`, `omarchy`, `quickshell`.
  - `series.drift` counts items (a txId opens once, in the week of its earliest
    line) and resolution writes (same `meta.txId` + `ts` + `actor` = one).
  - `resolutionDetail` is folded onto the target. `meta.txId` on a resolution
    must equal the target's `txId`.
  - The proposal regex now implements ADR-0012 §13 literally: word chars
    `[A-Za-z0-9._+-]` on both sides. The fixture's proposals are unchanged.
  - `index-variants/*` are now **generated as RFC 6902 overlays** (`VARIANTS`:
    `test`/`add`/`replace`/`remove`) on the sample. I checked before
    regenerating: the overlays reproduce the old variant files byte for byte. A
    variant file that differs from sample + overlay, or that has no overlay,
    fails the check. `--write-index` rewrites the variants too.
  - **18 mutation self-checks on every run** (in memory, about 0.1 s; whole
    script 0.24 s). Each mutation turns the group red/crisis: a member
    explicit; subject `linux`, `linux-firmware` (glob) or `quickshell`; kind
    `install`; `-Syu ollama`; `-Syu -- ollama`; `-Sy`; an unknown option plus a
    word; `yay -Syu`. These stay yellow: `reinstall`, `-S -u --needed`, `--sync
    --sysupgrade --refresh`, `--overwrite /usr/share/omarchy/*`, `-Syur /mnt`.
    A `--only` resolution leaves a group of 2. A fan-out resolution removes the
    group and counts once in `series.drift`. I made sure the self-checks can
    fail: with the `alwaysRed` list cut down to `linux`, two checks fail, and
    with no arg-taking options, the `--overwrite` check fails.
- `fixtures/README.md`: story row 09-30, result counts, a "rules the fixture
  check implements" list (resolutionDetail, the **token rule of §13** with its
  trailing-period consequence, grouping, routine/alwaysRed, series counting,
  self-checks), the overlay mechanism, and assumption bullets now citing
  ADR-0013/0014. `memory/pitfalls.md`: WP-014 section appended.

## Not done
- **jsonschema / check-jsonschema backends are still untested.** Neither is
  installed, and installing is red zone. Only the builtin backend ran. The new
  keywords (`not`, `const` inside `if/then`, nested `required` in `if`) are
  plain Draft 2020-12. CI should run `--validator jsonschema` once.
- No resolved group in the fixture. The fan-out path is covered only by the
  in-memory self-check, not by ledger lines. Adding one would mean a second
  transaction and more events, which is outside the brief.
- `invalid/index.contract-v2.json` is still a hand-written full file (a
  not-initialised index with version 2). It is not an index variant, so I left
  it as it is.
- docs/ is not mine. `docs/CONTRACT.md` lists `seldon drift link|explain|dismiss
  <eventId>` without the `--only` flag of ADR-0013 §4. That is a one-line
  follow-up for the orchestrator.

## Verified by
```
$ bash scripts/validate-fixtures.sh
validate-fixtures: ok — 94 instances (94 incl. 8 expected failures), 67 ledger events traced to index.sample.json, 2 variants, 18 drift self-checks; backend builtin
$ just check            # fmt, clippy, cargo test, schema-validate, plugin validate, qmllint
qmllint: ok (3 files)
check: ok               # exit 0
```
- **Acceptance mutation test**, run on the real ledger file and reverted
  (backup + restore, sha256 identical afterwards, validator green again):
  1. libinput (a non-leader member) set to `explicit: true`. The validator
     reports `/drift/3/zone yellow → red`, `/drift/3/crisis false → true`,
     `/summary/crisis 2 → 3` and a new timeline crisis marker. The leader
     changes to the explicit member (eventId/subject/ts/detail diffs), as
     ADR-0013 §1 requires.
  2. libinput's subject set to `linux`. Same zone/crisis/summary/timeline diffs,
     and the leader stays firefox.
- **`invalid/` reasons**, printed with the builtin backend: bad status (enum),
  unknown key `owner`, bad actor pattern, linked without `case`, naive
  timestamp, resolution without `refersTo`, contractVersion 2, and the new one,
  `resolutionDetail` without `resolution`. Each fails for its documented reason
  and nothing else.

## Learned
In `memory/pitfalls.md` (WP-014 section):
- `meta.command` is logged unquoted, so split it on whitespace and know the
  options that take an argument. Every `omarchy update` runs `-Syu --overwrite
  /usr/share/omarchy/*`, and a naive parser makes all routine updates red.
- On Omarchy, `00-omarchy-update-guard.hook` aborts a direct `-Syu` unless
  `OMARCHY_ALLOW_DIRECT_PACMAN=1` is set (read from the installed hook, read
  only). Real routine upgrades mostly come via `omarchy update`.
- Adding pacman events to the fixture touches three files plus the README byte
  offsets.
- The guard also blocks `python3 -c` and heredocs that contain the
  package-manager word. Write such scripts as files.

## Decisions needed
None of these block the work. Each one is implemented with the recommended
default, which WP-007/WP-008 can follow as is:
1. **proposedCase of a group = the leader's proposal only.** ADR-0013 doesn't
   say. If any member's proposal counted, a whole `-Syu` would be proposed for
   (and linking would fan out to) a case whose Plan names one upgraded package.
   *Recommend: keep the leader's.*
2. **`txId`/`members` only on groups.** Single pacman items carry neither, so
   the existing rows stay unchanged and the plugin has one test (`members`
   present). ADR-0013 §2 only says `members` is omitted. *Recommend: keep it
   (the schema enforces both-or-neither).*
3. **The zone is computed for every pacman item**, single-member transactions
   included. A lone caseless `-Syu` that upgrades one non-alwaysRed package
   is yellow. This is my reading of §3 "the item's zone is computed". Please
   confirm for WP-008.
4. **ADR-0012 §13 taken literally** changes two edge cases compared with the
   old script regex. A trailing sentence period (`zed.`) no longer matches,
   because `.` is a word character. `extra/zed` now matches `zed`, because `/`
   is not a word character. Neither case occurs in the fixture. If the
   trailing period should match, ADR-0012 needs a successor (e.g. "a final `.`
   not followed by a word character is punctuation").
5. **`series.drift` conventions**: a transaction opens in the week of its
   earliest line; one group write = same `meta.txId` + `ts` + `actor`. Please
   confirm or amend before WP-007's golden test.

## Touched outside WP scope
None. Changes are limited to `schema/`, `fixtures/` (incl. `logs/`),
`scripts/validate-fixtures.py`, `memory/pitfalls.md` (appended) and this file.
No engine/, plugin/, docs/, decisions/ or justfile changes.
