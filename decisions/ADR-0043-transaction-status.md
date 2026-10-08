# ADR-0043 — A pacman transaction that did not complete says so: `meta.txStatus`

**Status:** accepted 2026-10-08 (operator, E21)
**Date:** 2026-10-08

> Adds one **optional** `meta` key to contract 2 under ADR-0035 §6,
> before 0.2.0 is tagged: nothing required is added, nothing is removed or
> changes meaning, `contractVersion` stays 2. Extends CONTRACT.md rule 9.
> Work package: WP-137 (a transaction's packages in the Changelog detail).

## Context

WP-137 shows a pacman transaction's packages in the Changelog detail
and marks a transaction that did not complete in the urgent style. The
index already has the rest: every package event carries the
transaction's `txId`, its versions (`meta.from`/`meta.to`,
`meta.version`) and the command (`meta.command`).

Whether the transaction completed is lost. pacman ends every transaction
it commits with one of three lines (libalpm's `alpm_trans_commit`):
`transaction completed`, `transaction failed` (removing or upgrading a
package failed), `transaction interrupted` (the user interrupted the
commit, which pacman finishes after the current package). The collector's
line table matched the three as one end marker. A transaction that has no
end line at all (pacman was killed, the machine lost power) was closed
by the next `transaction started` or by the end of the log, and was
written like a completed one.

An incomplete transaction is the state a user most needs to see after
an update goes wrong: packages are half-applied, hooks such as the
initramfs build may not have run.

## Decision

### 1. The key

`meta.txStatus` on **every package event of a pacman transaction that did
not complete**:

| Value | When |
|---|---|
| `failed` | pacman logged `transaction failed` |
| `interrupted` | pacman logged `transaction interrupted` (Ctrl-C, or the terminal closed: pacman finishes the current package) |
| `unfinished` | no end line: the next `transaction started` came first, or the log ends and pacman's `db.lck` is absent (pacman is gone) |

- **Absent** when the transaction completed, and on every line written
  before this ADR (the ledger is append-only; no backfill, `seldon
  rebuild` does not re-read the log). Absent therefore means "completed,
  or not known": the plugin never claims *completed*.
- A transaction pacman **still runs** (`db.lck` present) is held back as
  before (ADR-0013 §5) and gets no status until it ends. Exception: one
  still open at the end of a rotated `<log>.1` is emitted `unfinished`,
  since the cursor moves to the new file and would never read it again
  (WP-137 round 2; rare, Arch does not rotate pacman.log by default).
- A package line outside any transaction (logs before pacman logged
  transactions) has no `txId` and no status.
- Only on events with `source: pacman` **and** a `txId`. The engine
  validates that on write; the schema says so (an `if` on the key); the
  index drops a hand-edited `txStatus` anywhere else, and one with
  another word reads as none (read leniently, as `meta.risk`), so a hand
  edit never makes the index invalid.
- `seldon event --meta txStatus=…` is refused: the pacman collector is
  the only writer.
- The status is written once, when the transaction is emitted; dedupe and
  idempotency are unchanged (`(ts, kind, subject, version)`).
- The status is final: a later end line, a rotation or a removed lock
  never revises it.

### 2. What it does not change

- **Classes** (ADR-0028): an incomplete transaction is classed like a
  complete one. Whether "did not complete" should earn attention is a
  separate question for ADR-0028, not this ADR's (see Alternatives).
- **Groups and attribution** (ADR-0013, ADR-0014): unchanged; the status
  is the same on every member, so a group's leader carries it too.
- **WP-141** (files pacman left, `meta.transaction`, ADR-0042 pending):
  independent. A note carries no `txStatus`; the plugin finds its
  transaction's status through the package events of that `txId`.

### 3. Plugin

The Changelog row of an event whose transaction did not complete shows
the status as one word in the urgent colour ("interrupted", "failed",
"unfinished"); the event detail says it in an urgent callout and lists
the transaction's packages (WP-137). Without the key nothing is marked.

### 4. Contract

`contractVersion` stays **2** (ADR-0035 §6). `event.schema.json` gains
the key (enum, description, the `if`), CONTRACT.md rule 9 lists it,
fixtures show an interrupted transaction (09-19 in the story) and two
invalid lines (`event.tx-status-without-tx`,
`event.tx-status-other-word`).

## Consequences

- Size: at most about 25 bytes per pacman event of an incomplete
  transaction; nothing for the common case.
- A 0.2.0 plugin against an earlier v2 engine build: no mark, the
  package list and the command still show. A 0.1.x plugin meets the
  contract-2 mismatch banner, unchanged.
- An old transaction keeps no status after an upgrade of the engine; a
  new logbook's backfill (ADR-0033) reads the log afresh and writes it.
- After a crash `db.lck` usually stays; the transaction is held back and
  marked only once the lock is gone (WP-160).
- SPEC-ENGINE §4 (the collector), CONTRACT.md rule 9, SPEC-PLUGIN (the
  Changelog detail), the fixture README.

## Alternatives considered

- *A top-level `txStatus` beside `txId`:* `event.schema.json` has
  `additionalProperties: false`, so it is a schema change of the event
  shape; `meta` is where the collectors' conventional keys live.
- *Write `completed` too:* every pacman line would grow, and an absent
  key would still have to mean "not known" for the lines written before.
- *Only `failed` and `interrupted`:* a transaction without an end line is
  the worst case (pacman killed mid-commit) and would stay unmarked.
- *A separate event per transaction* (`kind` `transaction`): a new kind is
  a schema change, the Changelog would gain rows, and the status belongs
  to the package lines the user selects.
- *Raise an incomplete transaction to attention or crisis:* a class change
  under ADR-0028 (its harm test), beyond WP-137; the operator may want it
  as a follow-up.
