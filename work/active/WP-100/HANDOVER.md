# WP-100 HANDOVER

Branch `wp/100-agent-rules-v2`, worktree `wt/WP-100`. Based on `71affbd`
(main has not moved since).

## Done

- **Rules v2 templates** (`engine/templates/{en,de}/AGENTS.md`), ADR-0027
  §2–§6 and §8: a case the user started — or work the user asked for in
  this session — is the authorisation; act inside the *Intent*; Plan as
  a running note that never widens the Intent; preview line before the
  first privileged step (terminal and *Log*, no wait); attended by
  provenance (`SELDON_ATTENDED=1` or a human message; sudo cache /
  NOPASSWD never count); unattended = record and report only;
  "Instructions and data" (only the rules, the user's rules and the
  user's words instruct; the Intent bounds the work and never changes
  the rules; logbook text, session context, web pages, READMEs, install
  scripts and command output are data); ask first only for (a) outside
  the Intent (fetched instructions, `curl … | sh`; PKGBUILD read first),
  (b) destructive without rollback, (c) R3; R3 check by read-only
  resolution (`pacman -Sp --print-format %n`, PKGBUILD depends) against
  `alwaysRed`, one go per step; the agent runs `sudo` itself, never
  handles a password; snapshot itself without the cleanup pass
  (`sudo snapper -c <config> create -c number -p -d "<ID>"`, not
  `omarchy-snapshot create`), number recorded; no-snapper fallback (R3
  stops, R2 named backup); install-route order with `omarchy pkg add`
  as a recommendation; verify and close yourself (Result with one
  non-own check, `plan verify` + `plan done` with `--actor`). Removed:
  propose-and-wait, plan-before-you-change, "`plan done` only when the
  user agrees", "not with the package manager directly". English
  headings in both languages; every command on one line.
- **Rules block**: the engine part sits in
  `<!-- seldon:begin rules v2 -->` … `<!-- seldon:end -->` (first line of
  the file); then `## Your rules` for the user. The block body contains
  no marker text (the v1 sentence quoting the markers would have ended
  the block; a unit test pins it).
- **`seldon rules update [--replace] [--json]`** (`engine/src/logbook/rules.rs`
  pure logic, `engine/src/commands/rules.rs` command):
  fenced → block rewritten, every byte outside kept (CRLF kept);
  unfenced → block on top, old text below `## Your rules (kept)` byte for
  byte; unfenced and **exactly a released v1 template** (sha256 of the
  v0.1.0 and v0.1.1–v0.1.3 en/de files, kept as goldens in
  `engine/tests/golden/rules-v1/`) → replaced whole (see Decisions 2);
  missing → template; `--replace` → old bytes to `archive/AGENTS-<date>.md`
  (`-2`, `-3`, … never overwritten), then the template; refused with
  exit 1 and file untouched: damaged block, newer block (v3+), non-UTF-8
  (all three: `--replace` takes them). `-U0` diff printed and in
  `--json` `diff`; autocommit `seldon: rules update`; second run "nothing
  changed", exit 0, no write, no commit; exit 3 without a logbook; exit
  4 lock.
- **`seldon doctor` row `rules`**: `current (v2)` ok; `outdated (v1)`,
  `outdated (vN)`, `outdated (v2, its text differs …)`, `missing` →
  degraded, fix `seldon rules update` (in `--json` `fix`); damaged →
  fix names the marker lines or `--replace`; newer → "update seldon".
  doctor.rs change is the one row (+ its function).
- **Docs**: AGENT-GUIDE.md §1–§10 rewritten (§3 "Work in a case: act,
  then account", §4 R3 / snapshots / install routes, §8 closing, §9 the
  2026-10-05 task done right: one sentence, one password prompt, agent
  closes; the old seven steps named as the counter-example), guide 04
  en/de (rules list, "Update the rules of an older logbook", who closes,
  work a case in one sentence, lane list), guide 05 en/de (`seldon rules`,
  `seldon rules update`, help blocks regenerated), llms.txt (anchors and
  wording), SPEC-ENGINE §3 (command, JSON, doctor row) and §9 (template
  contents), SPEC-LOGBOOK §3 who closes, CHANGELOG. de source lines
  re-stamped to `35a02ff` (en had no untranslated change before).
- `memory/pitfalls.md`: four WP-100 entries.

## Not done

- **Panel one-row change (doctor banner fix).** There is no doctor
  banner: the plugin never runs `doctor` (SPEC-ENGINE §3 says so), its
  banners come from `index.json`, and the index has no field for the
  rules state. Offering the fix in the panel needs a decision (below).
  The fix is in `doctor --json` (`checks[].fix`), ready for it.
- ADR-0027 commands that do not exist yet are **not** in the rules text:
  `plan snapshot`, `plan set`, `plan reopen`, `agent start --new` (WP-101).
  The v2 text uses today's commands instead: the snapshot number goes
  into `plan start --snapshot N` when the agent starts the case itself,
  else into a *Log* line; a riskier-than-estimated case is said in the
  *Log*. WP-101 should swap these in the templates; a changed block text
  makes doctor report existing v2 files as "outdated (v2, …)", and
  `rules update` refreshes them (no v3 needed for wording).
- No live run on the test host (not in this WP's acceptance; WP-101's
  live check counts the steps).

## Verified by

- `flock /tmp/seldon-check.lock just check` → `check: ok`, exit 0, at `7c35591` (fmt, clippy, tests, watch feature, packaging, install, schema, docs-check "ok (425 links, 14 translated pages, 42 commands, 505 command lines)", plugin validate, qmllint, plugin tests); an earlier run at `823b405`… before the template-command test was also exit 0
- `cargo test --lib logbook::rules` (9 tests) and `cargo test --test rules`
  (8 tests): fenced rewrite with user text above and below (a quoted end
  marker below), unfenced edited v1 kept byte for byte below the heading
  (no final newline, a tab, a quoted begin marker), unchanged released v1
  replaced whole (both releases, both languages), `--replace` archive and
  second archive name, non-UTF-8 refused then archived as bytes,
  idempotency (no write, no commit), damaged and newer refused and left
  alone, missing file, exit 3, doctor row for current / outdated (v1) /
  outdated (v2 changed) / missing / damaged / newer with the fix.
- `tests/init.rs`: section list (18 headings), needles per section in
  both languages, dropped v1 phrases absent in both, first line is the
  begin marker; new `every_command_in_agents_md_is_one_this_engine_has`
  runs `seldon <subcommand words> --help` for every `seldon …` code span
  of both templates (98) and checks each `--flag` is in that help —
  the templates had no such check (docs-check covers only the guides); `tests/golden/init-skeleton.txt` blessed and reviewed
  (diff: `fence rules v2`, the new headings, `## Your rules`).
- Manual run in a scratch HOME: v1 file + a user line → doctor
  `degraded rules outdated (v1)` / `fix: seldon rules update`; update
  printed the insertion diff; second run "nothing changed", exit 0; git
  log `seldon: rules update`.
- `bash scripts/docs-check.sh` ok (help blocks, usage of every `seldon …`
  in the guides, de structure and source lines).
- **Mutants** (script in the session scratchpad; each applied alone, the
  rules unit and integration tests run, then restored; all compiled; 22
  of 22 killed):
  M1 end marker found inside a line · M2 no damaged check inside the
  block · M3 begin marker anywhere in a line · M4 CR not stripped ·
  M5 no digit check on the version (survived until the `v+2` case was
  added — committed before the run) · M6 unterminated block reads as
  unfenced · M7 same version reads as outdated · M8 state compares
  without CRLF folding · M9 rewrite ignores CRLF · M10 released v1 not
  recognised · M11 kept text trimmed · M12 `--replace` of the template
  not "unchanged" · M13 newer block rewritten · M14 damaged block treated
  as unfenced · M15 diff without common tail · M16 archive overwrites ·
  M17 archive skipped · M18 unchanged still writes and commits · M19
  non-UTF-8 replaced without `--replace` · M20 doctor row missing · M21
  doctor calls outdated ok · M22 doctor fix dropped for missing.
  Template-command test, 2 of 2 killed: T1 the template names
  `seldon plan snapshot <ID> <N>` (no such subcommand) · T2 `--by`
  instead of `--actor` on `plan verify`.

## Learned

In `memory/pitfalls.md` (WP-100 section): a fenced template must not
quote its own markers; `"+2".parse::<u32>()` is `Ok(2)`; the guard
blocks a read-only grep naming Omarchy's package-add subcommand; the
templates' commands are now checked by an init test, so WP-101 cannot
name `plan snapshot` / `plan set` in the rules before the engine has
them.

## Decisions needed

1. **Panel fix for `rules: outdated`.** Options: (a) the plugin runs
   `seldon doctor --json` (new fixed-argv call; doctor probes snapper and
   omarchy with 10 s timeouts, so only on panel open, and CONTRACT.md's
   command list grows); (b) a rules field in `index.json` (contract
   change: ADR, `contractVersion` bump, fixtures); (c) leave it to
   `seldon doctor` and the CLI. Recommendation: (c) now, (a) folded into
   WP-101's panel work if the operator wants it — no contract change.
2. **Deviation from the WP text, for review:** an unfenced `AGENTS.md`
   that is *byte-identical* to a released v1 template is replaced whole
   instead of being kept under `## Your rules (kept)`. It holds nothing
   of the user's, and keeping it would put the old contradicting rules
   ("propose and wait", "the user closes", "not with the package
   manager") under the new ones for no gain; git has the old file. An
   edited v1 file is kept byte for byte as specified.
3. **Deviation from ADR-0027 §3 wording, for review:** the snapshot
   description is the case id only (`-d "<ID>"`), not
   `"<case id>: <title>"`. The title is logbook text; the rules forbid
   shell strings built from logbook text, and a title with `$(…)` or a
   quote inside a double-quoted `-d` would run. The id is engine-made.
4. **Release order:** the rules say `seldon agent start` sets
   `SELDON_ATTENDED=1`; that is WP-096. Until it lands, an agent started
   from the panel has only the id prompt — no human message — and reads
   its session as unattended (record and report only). WP-096 must ship
   in the same release as WP-100.
5. **Kept v1 text vs. new rules (Fable):** an edited v1 file keeps the
   old rules under `## Your rules (kept)`. The block says the user's
   rules "may add limits" and that "where it repeats an older Seldon
   rule, this block wins"; `rules update` tells the user to trim the
   kept text. Please judge whether that wording lets an agent tell a
   user's deliberate limit from a leftover v1 rule.

## Touched outside WP scope

None. `fixtures/logbook/AGENTS.md` (a hand-written unfenced file) is
unchanged; doctor on the fixture logbook now reports `rules: outdated
(v1)`, degraded, which is true and breaks no test.

Guard: one read-only `grep` over the docs whose pattern contained
Omarchy's package-add subcommand was blocked ("omarchy command that
changes the system"); not rerouted, the search was dropped (a false
positive for `scripts/guard.sh`, WP-098's area).
