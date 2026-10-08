# WP-137 — Plan: a transaction's packages in the Changelog detail

Branch `wp/137-transaction-detail` from `next` (aaf7a0a); merges into
`next`, never `main`.

## What the index has, and what it lacks

Checked against `engine/src/collectors/pacman.rs` and the sample index:

- **Has:** every package event of a transaction carries the top-level
  `txId`, its versions (`meta.from`/`meta.to` for upgrade and downgrade,
  `meta.version` for install, remove, reinstall; `detail` "a → b") and
  `meta.command` (the `[PACMAN] Running` line). The package list, the
  versions and the command are therefore a plugin-only change.
- **Lacks:** whether the transaction completed. The line table matches
  `transaction (?:completed|failed|interrupted)` as one `TxEnd`, and a
  transaction closed by the next `transaction started` or by the end of
  the log looks the same as one that completed.

Contract question raised before any field was added (2026-10-08); the
orchestrator answered: **`meta.txStatus`**, under ADR-0035 §6
(`contractVersion` stays 2), ADR-0043 as *proposed* for the operator,
engine + schema + fixtures + plugin in this WP, no mark when the field is
absent. Keep it separable from WP-141 (`meta.transaction`, pending
ADR-0042).

## Build

1. **ADR-0043** (proposed): `meta.txStatus` — `failed` | `interrupted`
   (pacman's words) | `unfinished` (no end line: the next transaction
   started, or the log ended while pacman was no longer running), on every
   package event of a transaction that did not complete; absent when it
   completed, and on every line written before this build (append-only,
   no backfill). Pacman events with a `txId` only; the index drops it
   anywhere else; `seldon event --meta txStatus=…` is refused.
2. **Engine:** `Line::TxEnd` keeps its end; `Tx.status`; `parse` sets it
   (end line, next start, end of log without `db.lck`); `Tx::events`
   writes it; `Meta.tx_status` (typed, read leniently like `risk`);
   `index::build::clipped` keeps it only on pacman events with a `txId`;
   `commands/event.rs` refuses it. Unit tests for the three ends and for
   the held-back transaction (no status while pacman runs).
3. **Schema/contract:** `event.schema.json` meta `txStatus` (enum, an
   `if` tying it to `source: pacman` with a `txId`), the conventional keys
   list, CONTRACT.md rule 9, SPEC-ENGINE §4, DECISIONS.md.
4. **Fixtures:** in `logs/pacman.log` (and the rotation log) two new
   transactions — **09-18** a plain `pacman -Syu` with a `:: Replace`
   (− pulseaudio, + pipewire-pulse, ↑ pipewire, ↑ wireplumber; routine,
   `sysupgrade`) and **09-19** a `pacman -Syu` interrupted after two
   upgrades (`meta.txStatus: interrupted`; routine); the **09-27**
   downgrade group already exists. Ledger lines, the `.md` view, the
   index (`validate-fixtures.py --write-index`), variants, goldens,
   README counts and offsets follow. `validate-fixtures.py` checks the
   new key's place.
5. **Plugin** (`Model.js`, `EventDetail.qml`, `ListRow.qml`/`GroupedRow.qml`,
   `Changelog.qml`, Today's crisis list):
   - `deskChangelog` indexes events by `txId` once per index; a row
     carries `txStatus` (only a known value on a pacman event with a
     `txId`), the row shows it as an urgent word in its meta line;
   - `transactionDetail`: the packages (↑ upgraded, ↓ downgraded,
     + installed, − removed, ↻ reinstalled; old → new or the version),
     the command, the status, a count summary, whether the index may
     have cut it (CONTRACT.md rule 4), the selected event marked;
   - `EventDetail`: kv rows *Command* and *Transaction*, an urgent
     callout for an incomplete transaction, the package list; the
     DriftForm's member list stays only when the index lacks members;
   - WP-141 hook: an event whose `meta.transaction` names a txId (a file
     pacman left) gets the same *Transaction* row, and a transaction's
     detail counts "left N files" from such events. Unit-tested on a
     synthetic event only; no fixture row (WP-141's).
6. **Tests:** `model.test.js` (mixed, interrupted, downgrade, absent
   status, malformed status, cut by the cap, the WP-141 hook);
   `desk-view.sh` cases for the three fixture transactions (row mark,
   detail, the three themes at 50 % and 100 % with `DESK_SHOTS`);
   counts that follow the fixture.
7. **Docs:** SPEC-PLUGIN (Changelog detail), CHANGELOG, COVERAGE.md.

## Decisions (left open by the WP)

The final list, with reasons, is in [HANDOVER.md](HANDOVER.md) "Decisions".
