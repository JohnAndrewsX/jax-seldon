# WP-004 foundation — what landed, and how WP-005 / WP-006 use it

Branch `wp/004-collectors-core`. These are the first commits on it. They build,
pass clippy `-D warnings`, and pass every existing test. The pacman, snapper and
omarchy collectors are still stubs in these commits; the real ones follow on
the same branch.

## Files

| File | What it is |
|---|---|
| `engine/src/model/event.rs` | `Event`, typed after `schema/event.schema.json`, plus `Source`, `Kind`, `Resolution`, `Meta`, `zone_for` and `Event::validate` |
| `engine/src/redact.rs` | `Redactor`: the SPEC-ENGINE §7 rules plus `[redaction] patterns` |
| `engine/src/ledger.rs` | `Ledger`: append (under the lock), read a month, read all, read a time range |
| `engine/src/collectors/mod.rs` | the `Collector` trait, `Outcome`, `REGISTRY` (6 collectors in run order), `Ctx`, `Sources` (env overrides), `Cursors` (`cursors.json`), `Tz` |
| `engine/src/collectors/{plugins,theme,config}.rs` | stubs for WP-005. Each returns `ok: true` and "not implemented yet (WP-005)" |
| `engine/src/commands/capture.rs` | `seldon capture [--source a,b \| --all] [--since TS]` with `--json` and `--quiet` |
| `engine/src/main.rs` | one variant `Capture` and one dispatch arm |
| `engine/Cargo.toml` | `ulid` 3 (`serde`) and `regex` 1, both on the allowed list; `jsonschema` (dev, `default-features = false`), allowed for tests |

## Writing events (WP-006: log, event, plan, decide, drift resolutions)

```rust
use seldon::model::event::{Event, Kind, Meta, Source, Resolution};
use seldon::ledger::Ledger;
use seldon::redact::Redactor;
use seldon::logbook::lock;

let lock = lock::acquire(&ctx.dirs.lock_file())?;                  // exit 4 if held
let ledger = Ledger::new(&logbook, Redactor::with_patterns(&config.redaction.patterns)?);
let e = Event::new(now, Source::Manual, Kind::Note, "journal")    // actor system, zone per ADR-0014 §2
    .detail(text)
    .actor("human")
    .case(Some("C-2026-004".into()));
let written = ledger.append(&lock, vec![e])?;                     // returns the events with ids
```

- **Ids.** `Ledger::append` assigns the ULIDs. Any `id` you set is overwritten.
  Within one call the ids increase in input order. Write a drift group's
  resolutions in one call (ADR-0013 §4).
- **Redaction.** `append` redacts `detail` and `meta.command`, and cuts
  `detail` to 4096 characters. Do not redact twice. Paths in `skipPaths`
  (ADR-0014 §4) are the hook's job.
- **Validation.** `append` checks every event against the schema rules
  (`Event::validate`): ULID, actor pattern, case id, subject length, and
  `resolution` needs `refersTo` + `resolution` (and `case` when linked).
  `correction` needs `refersTo`. `meta.txId` is allowed only on a
  resolution. If one event is invalid, nothing is written (exit 2).
- **Zone.** `Event::new` sets the zone with `zone_for(source, subject)`:
  - pacman, omarchy, and config under `~/.config/systemd/` → red;
  - other config, theme and plugins → yellow;
  - everything else → none.

  A hook `command` event takes the zone of what the command would produce.
  Set `e.zone = zone_for(Source::Pacman, …)` yourself.
- **`ts`.** `DateTime<FixedOffset>`. It is written as RFC 3339 and keeps its
  offset (`+00:00`, never `Z`). The month file is chosen from `ts` in its own
  offset.
- **Meta.** Use the typed keys: `command`, `version`, `from`, `to`, `hashFrom`,
  `hashTo`, `type` (`snapshot_type`), `cleanup`, `pairOf`, `enabled`, `txId`.
  Anything else goes in `meta.extra`, which holds scalars only. Key order
  in the output matches the fixtures. All 67 lines of
  `fixtures/logbook/ledger/*.jsonl` round-trip byte for byte.
- **Reading.**
  - `ledger.read_all()` reads every event.
  - `ledger.read_month("2026-10")` returns `MonthFile { events, bad_lines }`. A malformed line is skipped and its number is reported.
  - `ledger.read_range(from, to)` reads only the months that can hold the range.

## Writing a collector (WP-005: plugins, theme, config)

Replace the stub in `collectors/<name>.rs`:

```rust
impl Collector for Theme {
    fn name(&self) -> &'static str { "theme" }
    fn collect(&self, ctx: &Ctx, cursor: Option<&Value>) -> Outcome {
        let prev: Option<MyCursor> = typed_cursor(cursor);   // None = first run → baseline
        // … read the system (ctx.run(program, &args) for programs; fixed argv) …
        Outcome::ok(events, to_cursor(&MyCursor { … }))
        // or Outcome::degraded("omarchy shell not running", Some(fix)) — no events, cursor kept
    }
}
```

- **No cursor.** This means a baseline: emit nothing that happened before
  `ctx.baseline`. The baseline is the logbook's `created` time, or
  `capture --since`. A diff-based collector (theme, plugins, config) emits
  nothing and only saves its snapshot.
- **The collector does not write.** `capture` appends all events of one run
  in one pass, sorted by `ts`. Only then does it save `cursors.json`, so a
  failed write never moves a cursor.
- **`ctx.earlier`** holds the events of collectors that ran earlier in the
  same capture. **`ctx.ledger`** is available for lookups.
- **Host paths and programs.** Add a field to `Sources` (`collectors/mod.rs`),
  with a `SELDON_*` env override for tests. `Sources::from_env` is read once
  per process.
- **Status.** `cursors.json` stores `ok`, `message`, `fix`, `lastRun` and
  `events` for each collector. That feeds `index.state.collectors` (WP-007).
- **Cursors are per logbook.** If `cursors.json` belongs to another logbook,
  every collector takes a fresh baseline.

## `seldon capture`

```
seldon capture [--source snapper,pacman,… | --all] [--since RFC3339] [--json] [--quiet]
```

- **Selection.**
  - No flag, or `--all`: every collector enabled in `config.toml [collectors]`.
  - `--source`: exactly the named collectors, even disabled ones.
  - An unknown name is exit 1.
- **Errors.**
  - Exit 3 if the logbook is not initialised.
  - Exit 4 if the lock is held.
  - A degraded collector is **not** an error: it shows up as `ok: false`, with `message` and `fix`.
- **JSON output:**

  ```json
  {"ok": true, "logbook": "/path", "written": 13, "files": ["ledger/2026-09.jsonl", "ledger/2026-10.jsonl"],
   "collectors": [{"name": "snapper", "enabled": true, "ran": true, "ok": false, "events": 0,
                   "message": "No permissions.", "fix": "sudo snapper -c root set-config ALLOW_USERS=$USER SYNC_ACL=yes"}, …]}
  ```

- **Not done by capture yet:**
  - reconciliation (drift, WP-008);
  - rebuilding the index (WP-007);
  - the git commit (WP-006 owns the logbook commit helper — call it after `ledger.append`);
  - the generated `ledger/YYYY-MM.md` view (index/status track).

## Other

- `sys::random_hex` now uses ULID randomness. The misleading
  `Instant::now().elapsed()` term is gone (the nit from the WP-003 review).
