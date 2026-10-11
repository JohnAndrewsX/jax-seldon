# WP-204 — handover

Branch `wp/204-work-totals` from `next` 1e872c28. Plugin only: no
engine, schema, fixture or contract change.

## What was done

- `plugin/Model.js`:
  - `wholeCases(index)` (new): `{queued, active}` from
    `summary.queuedCases` / `summary.activeCases`, 0 when absent.
  - `workColumns`: each column's `total` is the sum over its groups of
    max(listed, summary count). Queued uses `queuedCases`. Active is
    max(listed active, `activeCases`) + listed verification, because
    `summary.activeCases` counts `active` only (`engine/src/index/build.rs`,
    `groups.active.len()`). Completed has no summary count and stays as
    it was, so the *By agent* filter's "COMPLETED 1 / 2" is unchanged.
    `columnHeader` already prints "N / total" when they differ:
    "QUEUED 50 / 450".
  - `deskWork` also returns `whole: wholeCases(index)`. `workView` raises
    each group's total to max(listed, whole), giving "QUEUED · 50 / 450"
    and "ACTIVE · 2 / 40". The headers are unchanged when the list is
    whole. A summary that is lower than the list never lowers the count.
- `docs/SPEC-PLUGIN.md` (Work): one sentence on the whole-count header.
- `tests/plugin/model.test.js`: test "WP-204: …". It builds a cut index
  from the sample (50 queued listed, `queuedCases` 450, `activeCases`
  40) and checks the following:
  - `workColumns` headers: "QUEUED 50 / 450", "ACTIVE 3 / 41",
    "COMPLETED 2". With *By agent*: "COMPLETED 1 / 2".
  - `workView` labels: "ACTIVE · 2 / 40", "VERIFICATION · 1",
    "QUEUED · 50 / 450", "COMPLETED · 2".
  - Search against the whole: "QUEUED · 1 / 450".
  - A summary below the list keeps "QUEUED · 50".
  - No summary falls back to the list length.
  - The whole sample keeps its plain counts.

  The shipped fixture lists every case, so the short-list case lives
  only in this test, as the WP allows.

## Note for the reviewer

The WP names `Model.workColumns`, but the Desk (`sections/Work.qml`)
renders its group headers from `Model.workView(Model.deskWork(index))`.
No QML calls `workColumns` or `columnHeader` any more; only
`model.test.js` does. I changed both so they agree. Without the
`workView` change the visible headers would still read "QUEUED · 50".

## Not done

- An empty listed group (for example, every active row cut on rung 21)
  shows no header in the Desk, so it shows no "0 / 40" either. The Desk
  draws a header only above a row. This is outside this WP.
- `wipStatus` still counts the listed `cases.active` (ADR-0053 §5 row).
  Not part of this WP.

## How it was verified

- **fixture**: `node tests/plugin/model.test.js`, 219 passed, including
  the new WP-204 test against the sample fixture and the cut index
  built from it.
- **headless**: `SELDON_FULL_CHECK=1 just check` on the dev host at
  abdcf939 (the code of this handover; only this file was added after
  it): `check: ok`, exit 0. Results:
  - desk-view 2242/0. Its `work #1` still finds "QUEUED · 3" on the
    sample, so the whole list keeps the plain count.
  - service-states 366/0, bar-view 195/0, ipc-restart 43/0,
    terminal-scripts 73/0.
  - qmllint ok (52 files), plugin-validate ok, tokens ok, plaintext ok,
    docs-check ok (514 links).
  - Every harness reported "no leftover" in the quickshell by-id
    directory (328 before and after). The runtime dir was 2 % full
    before and after.
  - desk-view logged that the real state directory was changed by the
    live engine, not by the harness. The harness judges this itself.
  - TMPDIR, JUST_TEMPDIR and the cargo target were under the private
    gates folder.
- **test host**: shellcheck through the orchestrator kit's wrapper,
  inside the same `just check` run.
- **not run**: CI; a live desktop with a logbook over the caps.
