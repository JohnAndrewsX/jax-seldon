# WP-227 — packet for the advisor's look (before "proposed")

Branch `wp/227-app-adrs` (off `app`). Docs only. Read in this order;
each is short except CONCEPT and PLAN, where only the new parts matter.

1. `decisions/ADR-0056-seldon-is-an-omarchy-app.md` — option A as decided
   (three artefacts, one package; one job; four views; EASY | PRO retired;
   the pill; the desk ends with 0.5.0; install route; templates; §9 the
   exceptions table; §10 precedence; §11 the superseded rows).
2. `decisions/ADR-0057-ui-strings-through-catalogs.md` — catalogs per
   surface, `en`+`de` in 0.5.0, 48 in 0.6.0, the language file is display.
3. `decisions/ADR-0058-seldon-open-is-the-app.md` — `open`/`edit`/alias,
   `skill`, exit codes 0–4, `changes --around`.
4. `DECISIONS.md` — three rows and "superseded in part by ADR-0056/0057
   (proposed)" on ADR-0001, 0002, 0004, 0007, 0016, 0022, 0024, 0034,
   0045; ADR-0041 "stands, `open --editor` becomes `edit --editor`".
5. `docs/ROADMAP.md` (one page, the decision's §2 table), `docs/APP-PARITY.md`
   (every desk-view scenario → app or drop), `docs/PLAN.md` phases 5–7.
6. `docs/AGENTS-PROPOSAL.md` (rows for the operator), `docs/CONCEPT.md` v2,
   notes at the top of SPEC-PLUGIN, its §4, §5, §8, SPEC-ENGINE §1.1 and §3,
   the omarchy-ux skill §1.

## Where the text goes beyond the decision (please rule)

- **ADR-0058 §1, `seldon open --json`:** the decision says "exec". AGENTS.md
  §7 asks `--json` of every command, and an exec cannot print. Written: bare
  form execs; with `--json` the engine starts the app detached and prints
  `{launched, program}`, as `edit --editor` already does.
- **ADR-0058 §1, single instance:** a second `seldon open` raises the open
  window (mechanism left to the app).
- **ADR-0058 §6, `changes --around`:** defaults 2 h / 30 min (from WP-229),
  oldest first, each event with kind, subject, actor, case and class; reads
  the ledger, needs no index; its JSON becomes a CONTRACT.md row only when a
  plugin or the app runs it.
- **ADR-0057 §4:** `Model.js` returns keys and arguments, the view
  translates; a bare-literal check joins the linters (rule left to a WP).
- **APP-PARITY drops:** scenario 1 (width), 4 (Settings › Appearance; no
  settings view unless a setting survives — marked "proposed drop"), 5's
  shell routes, middle click and shim, 7c (shell restart notice, stays with
  the pill), 8g's stepping aside, 9a's IPC. Everything else maps to a view.
- **AGENTS-PROPOSAL §1** (project line) is added beyond the decision's list
  of §2/§3/§4/§6/§7, marked as such.
- **PLAN M4** reworded: "public release (0.1.0 …; the 1.0.0 it once named is
  M7)"; M5–M7 added.
- **ADR-0056 §11** also lists ADR-0041 as standing with its command moved,
  and names ADR-0003/0005/0006/0009/0028/0051/0053/0055 as unchanged.

## Not done here (by design)

No AGENTS.md, PROJECT.md, STATUS.md, VERSIONING.md or CONTRACT.md edit
(CONTRACT.md's `edit` row is WP-229's; VERSIONING's app row comes with
WP-230 or the 0.5.0 release). `docs/seldon-concept.html` still shows
CONCEPT v1. No new WP numbers: phase 5–7 rows without one say "—".
