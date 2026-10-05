<!-- maintained by the orchestrator; regenerate the WP table from work/ -->
# STATUS.md — Seldon

**Release:** v0.1.0 on GitHub (2026-10-02); AUR pending registration.
**Phase:** 0 exited — **gate G3 passed 2026-10-02** (operator's run on
the dev host, `work/completed/PHASE-0-EXIT-transcript.md`). Phases 1–3
complete and live-verified (G2 passed), Phase 4 packaging and docs merged.
Open: WP-042 marketplace submission (after the operator's release setup),
WP-038 (engine attributes its own installs), WP-033 and WP-043 (operator
decisions).
**Contract version:** 1 (draft)
**Last updated:** 2026-10-05 (tick 105)

## Active work packages
| WP | Title | Role | Worker | Worktree | Since |
|---|---|---|---|---|---|
| WP-097 | redaction: remaining credential forms on command lines | Engine | `engine-097` (opus) | `wt/WP-097` · `wp/097-redact-forms` | 2026-10-05 |
| WP-099 | crash-proof seldon notes (state reset, snapper access) | Engine | `engine-099` (opus) | `wt/WP-099` · `wp/099-crash-proof-notes` | 2026-10-05 |
| WP-103 | config replay with redacted twins; SPEC §7 clause | Engine | `engine-103` (opus) | `wt/WP-103` · `wp/103-replay-twins` | 2026-10-05 |

## Queued (next up)

0.1.4 wave: wave 1 WP-087…090 active (file-disjoint, scoped with the
advisor: no ADR, no contract change); wave 2 WP-091 (snapper access
notes, after WP-088) and WP-092 (hook rebuild margin, after WP-087).
After 0.1.4: ADR-0027 (operator decision 2026-10-05: a case the user
started authorises the agent; the agent verifies and closes; only steps
that can make the machine unbootable need the user's go) → WP-100 (agent
rules v2 + migration) first, WP-096 (default actor, attended marker),
WP-101 (one-click start, snapshot and close path, reopen), WP-094 (skill),
WP-095 (Ask agent), WP-102 (import Markdown task files as cases); no
Omarchy upstream contribution before 1.0. Then the
contract v2 bundle for v0.2.0 (autocommit result in the index,
`meta.truncated`, state-loss event kind; ADR first). Waiting for the AUR
account: WP-033 (update-impact, option C) and WP-042 (marketplace
submission). (see `work/queued/`)

## Preparation done (2026-10-01)
- Kickoff checklist steps 1–3: repo + first commit, WP files, host verified
  (`memory/host.md`, `memory/omarchy-shell.md`, `memory/local.md`).
- Dev host toolchain: rustup + musl target, just, qmllint. Test host: cargo
  (glibc), qmllint. GitHub remote: `JohnAndrewsX/jax-seldon` (private).
- How to run the team: `docs/HERDR-SETUP.md`.

## Blocked
(none)

## Recently completed
- 2026-10-05 WP-093 redaction masks e-mail addresses (local part;
  the domain stays so a desktop entry stays recognisable); SSH remotes,
  `user@host`, versions, npm scopes and systemd units stay; non-ASCII
  text before an address and an address after `addr:` are handled;
  hook 4.1 ms (curl) / 3.6 ms (address) at 900 lines; Opus review, one
  round, Fable look; merged. **0.1.4 wave complete on main** (WP-087…093,
  WP-096, WP-098, WP-100).
- 2026-10-05 WP-100 agent rules v2 (ADR-0027) in the logbook templates
  en/de: act inside the Intent, preview line, attended by provenance,
  ask first only outside the Intent, for a destructive step without
  rollback, and for R3 (the whole resolved transaction against alwaysRed,
  never refreshing the sync db; a system upgrade is R3); own snapshot
  after case start; verify and close; user and area rules only add
  limits; child agents get their own actor. `seldon rules update`
  migrates (edited files archived, only the user's own lines kept under
  their heading); doctor `rules` row. Opus review, Fable stage 2, three
  rounds, Opus verification; merged.
- 2026-10-05 WP-096 `seldon agent start` gives the launched agent
  `SELDON_ACTOR=agent:<launcher>` and `SELDON_ATTENDED=1` (checked live
  through Omarchy's launch chain on the test host and the dev host);
  `log`, `plan`, `drift`, `event` and `hook generic` take the actor from
  `SELDON_ACTOR` when `--actor` is missing; an explicit `--actor` wins;
  Opus review, Fable look, one round; merged (ships with WP-100).
- 2026-10-05 WP-092 the agent hook is faster: globs compile on demand,
  a plain `hook claude-code` skips the full parser, the rebuild allocates
  less; the 900-line curl case 4.6 → 3.9 ms (0.9 ms headroom with WP-093);
  the 2.5 ms target was not met — the remaining lever (rebuild after the
  hook returns) is a design change, deferred; Opus review (byte-identical
  goldens, 46 M-pair glob property test); merged.
- 2026-10-05 first deploy of main to the test host (`just
  deploy-test-host`): it runs `0.1.3+main.a1e1a94`, smoke ok, panel
  checked.
- 2026-10-05 WP-098 the test host follows main: `just deploy-test-host`
  builds the static engine with the release's features and a
  `+main.<sha>` marker, installs engine and plugin on the test host only
  (host list plus a pinned machine-id, git-ignored), restarts the shell
  when the plugin changed and the session is unlocked, smoke-tests and
  logs each deploy; refuses unless the main check log names a commit
  with no engine/plugin/schema change since; `--release` goes back.
  Opus review, three fix rounds, Opus verification, Fable look; merged.
- 2026-10-05 WP-091 snapper changes between degraded and ok are
  recorded as notes (both directions); a collector not run in the
  capture that loses its state records its gap later (bare marked entry,
  read as no entry everywhere); doctor names a waiting baseline by its
  reason; the debug watch test bounds heap growth instead of mapped
  pages (the gate's RSS failure was debug-binary page noise, measured);
  downgrade note in VERSIONING; Opus review, Fable stage 2, two fix
  rounds, Sonnet verification; merged.
- 2026-10-05 WP-087 redaction: one quote-aware command context for the
  six option rules (never less than the plain reading), options given
  twice are masked each time, marker checks by binary search (long lines
  linear; 128 KB budgets in `just check-perf`); Opus review, Fable
  stage 2, one fix round, Opus verification, Fable look; merged.
  Remaining credential forms: WP-097.
- 2026-10-05 WP-090 the panel says "Restart the shell to finish the
  update" when the shell still runs plugin code from before an update
  (Quickshell 0.3.1 keeps compiled QML), one-shot *Restart shell* action;
  the version pair is checked in `just check-packaging` and the release;
  guide 11 and both READMEs tell users to restart after a plugin update;
  harness recorder appends in one write; live check on the test host
  (notice, real click, new shell in 1 s); Opus review, one fix round,
  Sonnet verification; merged.
- 2026-10-05 WP-088 a collector degraded in a state reset keeps a
  `pendingBaseline` mark (`cursors` or `logbook`) and records its gap on
  its first successful run; rule 8 catches up Seldon's own changes left
  open (dismissed rows keep their resolution; dated at max(now, event))
  so a clock moved back leaves no drift; Opus review, Fable stage 2, one
  fix round, Sonnet verification; merged.
- 2026-10-05 WP-089 Omarchy programs run by the engine get
  `OMARCHY_PATH=/usr/share/omarchy` when it is unset or empty (ssh, cron,
  scripts: the plugins collector no longer degrades; checked live on the
  test host, over ssh and with `env -i`); `~/.local/share/applications`
  is a default watch path with `mimeinfo.cache` excluded; guide 06 gives
  the line for existing configs; one review round; merged. E-mail
  redaction for desktop entry names follows as WP-093 in the same release.
- 2026-10-05 WP-086 Seldon's own plugin updates (update, enable, disable)
  and package upgrades are explained by rule 8, not drift; adding,
  installing, downgrading and removing stay drift (no provenance check);
  two review rounds; merged.
- 2026-10-05 WP-084 redaction covers proxy credentials, inline JSON secrets
  and api keys, cookie headers (cookie pairs only) and cookie options; ASCII
  word boundaries and joined triggers make long command lines 40× faster;
  hook perf case with a recorded curl line; two review rounds; merged.
- 2026-10-05 WP-085 the panel shows a capture's warnings (the state reset)
  in a neutral notice, full text in a bounded, wrapping tooltip; kept across
  failed or locked captures, cleared by a clean one; two review rounds;
  merged.
- 2026-10-05 WP-082 snapper keeps one date per snapshot across `snapper
  list` and the info files, also in the repeated hour and at its ends; the
  ledger dedupe knows both instants; the list shape checked on the dev
  host; two review rounds; merged.
- 2026-10-05 WP-083 doctor warns before the capture that would record a
  state reset, sharing capture's detection (`Collector::cursor_reads`); the
  fix names a `--path` logbook; two review rounds; merged.
- 2026-10-04 WP-081 a lost, corrupt or foreign state directory is recorded
  as a `state-reset` note (existing kind, no contract change), printed by
  capture and the session-stop hook, shown by doctor with an honest fix;
  two review rounds; merged.
- 2026-10-04 WP-080 release assets carry build-provenance attestations
  (pinned action, least-privilege build job); install.sh verifies them with
  `gh` against the release workflow and the tag, `--require-verified` and
  `--skip-provenance`; two review rounds, two dry runs; merged.
- 2026-10-04 WP-079 snapper access by a read grant on `/.snapshots`
  (ADR-0026 supersedes ADR-0011); doctor and init print the revert of the
  old opt-in with `SYNC_ACL=no`; two review rounds; merged.
- 2026-10-04 WP-078 plugin: callWarning first line, new-decision sheet busy
  refusal, Changelog Capture now replaces a pending retry, "Engine too old"
  rows in README and the troubleshooting guides (en, de), IPC owner prefers
  a drawn widget, harness clean_log; two review rounds; merged.
- 2026-10-04 WP-077 engine: a refused case save writes nothing (prepared
  before the ledger write in log, event, the agent hooks and drift link),
  validation errors and warnings name values and file names escaped, plan
  list warns of an unreadable case and lists the rest, the reference
  derive() clips as the engine (ADR-0025); two review rounds; merged.
- 2026-10-04 WP-076 index: long event and drift texts clipped at 256 bytes
  with a visible marker (ADR-0025), size warning at 1 MB, perf budgets at
  the stated scale in `just check-perf`, collector test benches on scratch
  paths only; merged after two review rounds.
- 2026-10-04 WP-073 collectors: plugins dedupe against the ledger, capture-time
  attribution window with the newest-known-event rule, verb-aware plugin
  proofs, capture on SELDON_NOW, snapper reused snapshot numbers, hooks
  ignore an empty watch path; merged after two review rounds.
- 2026-10-04 WP-075 import guards a case-less vault, reads non-UTF-8 names,
  accepts a BOM; dossier skips an unreadable system file and neutralises
  fence bodies idempotently; watch covers areas/ (two Opus rounds) — merged.
- 2026-10-04 WP-074 init: language setting, config saved before the layout
  and restored on a failed layout, --no-git kept across re-runs, theme hook
  installed and recorded under one lock (WP-052 folded in; two Opus rounds)
  — merged.
- 2026-10-04 WP-071 one shell parser for hook and attribution, pkexec/run0
  and sudo clusters, version/help probes, redirects and heredocs, floating
  words match no path below a skipPaths pattern (two Opus rounds, 14+
  mutants) — merged. **Wave 4 complete.**
- 2026-10-04 WP-070 doctor checks config, state files, patterns, ledger
  lines, fences, duplicate case ids and the last capture's collector state
  (read-only, proven); `index --check` exits 1 for a duplicate case id (two
  Opus rounds) — merged.
- 2026-10-04 WP-069 config collector: scope changes no longer record every
  file, default skipPaths (an empty list means the defaults), home-relative
  paths, hash reuse by size/mtime/ctime/inode (capture 79 → 11 ms on 2000
  files), events deduplicated against the ledger (two Opus rounds) — merged.
- 2026-10-03 WP-068 failed engine calls logged, a locked capture retried
  (bounded), busy sheets say so, engineMin enforced (Opus review APPROVE)
  — merged; small follow-ups queued as WP-078. **Wave 3 complete.**
- 2026-10-03 WP-072 actions and the build image pinned by commit and
  digest, cargo audit gates the release with a dated ignore list, pins
  and ignore tests in `just check`; positive dry run green, a probe branch
  with a vulnerable dev dependency failed at the audit step (Opus review
  APPROVE + round 2) — merged. Operator decisions still open: release
  signing, dependabot for the action pins.
- 2026-10-03 WP-065 fence values cannot end their fence, a damaged STATUS.md
  is left alone and reported, ledger months decoded line by line with bad
  lines skipped, actor and case checked on load (two Opus rounds) — merged.
- 2026-10-03 WP-066 hand-edited frontmatter stays valid on save (read back
  before write, refused unchanged otherwise), BOM and padded fences, case
  ids with control characters refused on load (Opus review APPROVE) —
  merged; follow-ups in WP-077 and WP-075.
- 2026-10-03 WP-067 tab changes give the keys back, the Changelog cursor
  follows its event, one widget registers the panel IPC target (Opus
  review APPROVE) — merged. Operator item: a live check of the IPC
  hand-over on a hot reload or a two-monitor session.
- 2026-10-03 WP-062 redaction: more token, option, header and URL forms;
  every text field redacted before it is written (journal, cases, tags);
  credential names mask any value, key names need a credential-shaped
  value; rules compiled once per process (three Opus rounds) — merged.
  **Wave 2 of the review plan complete.**
- 2026-10-03 WP-061 autocommit only in the logbook's own repository (ceiling,
  toplevel and gitdir checks), detached HEAD and failing commits visible,
  doctor reads only, import undo scoped to its own files with literal
  pathspecs (three Opus rounds) — merged.
- 2026-10-03 WP-063 hook recording scope from the project directory (default:
  only inside a logbook, `[hooks] scope = "all"` keeps the old behaviour),
  skipPaths on recorded command lines, already-recorded check under the
  lock, `plan show` delimits the case text (two Opus rounds) — merged.
- 2026-10-03 WP-064 atomic writes follow symlinks and keep modes, new files
  private, sync policy (durable data synced, rebuildable files not), run
  timeouts cover pipes, git stays in the engine's process group (four
  review rounds; a test-stub race fixed on the way) — merged.
- 2026-10-03 WP-060 snapshots read from the info files when listing is not
  permitted, an empty snapshot directory keeps snapper degraded, the banner
  names what the opt-in grants; the opt-in command itself waits for the
  operator's ADR decision (two Opus review rounds) — merged.
- 2026-10-03 WP-058 session-start context delimits logbook text as data,
  agent start prompt without logbook text, one-line agent notes (two Opus
  review rounds, stage 2 by the orchestrator) — merged.
- 2026-10-03 WP-057 agent hook reads the case under the lock (race tests),
  patient lock wait, session-stop runs every step, journal day without
  frontmatter, duplicate case id in `index --check` — merged (Opus review
  APPROVE, stage 2 by the orchestrator).
- 2026-10-03 WP-059 REBUILD.md checks and shell-quotes every generated
  command name and escapes control characters in displayed values (three
  Opus review rounds, 12+ mutants) — merged.
- 2026-10-03 WP-056 user guide and watcher README: uninstall with a kept
  logbook, state-directory backup and restore (Sonnet worker and review) —
  merged.
- 2026-10-03 WP-055 session-start context moved into its own module (pure
  move, golden output byte-identical; Haiku worker, Sonnet review) — merged.
- 2026-10-03 WP-054 snapper banner *Check again* runs a capture, hint
  after *Run in terminal*, fixes #2 — merged (Opus review APPROVE; at
  merge: SPEC §5 rows for the other banners aligned to the buttons,
  harness diff pipeline no longer aborts the run). Both issue fixes go
  into 0.1.2.
- 2026-10-03 WP-053 snapper runs in the C locale: one helper builds every
  snapper command with `LC_ALL=C`, localized-stub test, fixes #1 — merged
  (Opus review APPROVE, stage 2 by the orchestrator). Follow-ups noted:
  `C.UTF-8` for non-ASCII snapshot descriptions; doctor should honour
  `SELDON_SNAPPER` like the collector.
- 2026-10-02 WP-051 Prime Radiant assets: `assets/` of record (round-3
  masks, round-2 rest), plugin bar glyph tinted by the theme, panel
  header mark, eight state pictograms, timeline markers, README heroes,
  user guide wording; check 4 offscreen within 1 px — merged after two
  Opus review rounds. Live bar check on the test host: pending, operator.
- 2026-10-02 WP-049 CLI polish: `seldon completions`, `seldon mangen`
  (man page in the package, the release tarball and install.sh),
  every --help reviewed, `hook uninstall claude-code` and
  `init --remove-theme-hook` recorded as own writes, stdout write errors
  exit 2, hook install/uninstall under the lock — merged after two
  Opus review rounds and three release dry runs.
- 2026-10-02 WP-046 Root README along GitHub best practice (badges, hero,
  why, quick start, tour, docs table), developer content moved to
  docs/DEVELOPMENT.md, plugin README on the same skeleton with absolute
  links for the split repo, docs-check covers the front pages — merged.
- 2026-10-02 WP-050 Advisory warning on `plan start` without a snapshot
  for R2/R3, alwaysRed aligned with ADR-0023 (login path added, kernels
  only), engine fills the decisions index, doctor paths, wizard hint,
  damaged-fence safety in dossier and import — merged after one review
  round; the CLI reference was regenerated on main (cross-track drift
  caught by docs-check).
- 2026-10-02 WP-045 User documentation: 13 pages in English and German
  under docs/user/, STYLE.md with glossary, `docs-check` (links,
  translation parity, CLI reference vs --help) in `just check` — merged
  after one review round (literal replay of Getting Started passed; 15
  skeptical-reader fixes applied in both languages).
- 2026-10-02 WP-044 `install.sh` (verified download of the release asset,
  `--uninstall`, `--force` for self-built binaries, 106 hermetic checks),
  fourth release asset, "AUR: coming soon" in both READMEs and the
  banner, the one-click fix runs the installer during the AUR pause
  (ADR-0024) — merged after one review round; branch dry run green.
- 2026-10-02 WP-048 CONTRIBUTING, SECURITY (private reporting on both
  repos), Code of Conduct, issue forms, PR template, topics, release
  notes from the CHANGELOG section (checked in the build job), weekly
  `audit.yml`, VERSIONING.md — merged after one review round.
- 2026-10-02 WP-047 `llms.txt`, `docs/AGENT-GUIDE.md`, the logbook
  `AGENTS.md` rules in en and de (no CLAUDE.md), README pointers;
  ADR-0023 (agent verifies, human closes; risk scale) — merged after one
  review round.
- 2026-10-02 WP-043 `seldon import omarchy-agent <vault> [--apply]`: cases,
  journal, knowledge and deviations mapped, ids renumbered on collision
  with references rewritten, redaction, dry-run report, marker-guarded
  apply — merged after one review round; the operator's real vault dry
  run: 36 cases (7 renumbered), 41 sessions, 20 knowledge sections, 25
  deviation rows, 0 errors. The `--apply` on `~/Seldon` is the operator's.
- 2026-10-02 WP-039 Panel 460 wide, tabs sized to their labels, chips
  wrap, Changelog header keeps the sort word, fit assertions at scale
  1.0/1.25, real-home guard recognises the operator's live engine,
  preview refreshed — merged (operator's live observation).
- 2026-10-02 WP-038 Engine attributes its own installs (owned.json →
  `explained` resolution on the next capture; actor stays `system`,
  source `seldon`), Phase 0 exit procedure for the shipped wizard,
  fresh-machine smoke list — merged.
- 2026-10-02 WP-037 Overlay live findings: memoised empty period view
  (no aggregation when the shell injects `service` after creation), the
  harness now creates the overlay like the shell and the old code fails
  its assertion with the live number (23), probe docs, Tokyo Night /
  Catppuccin Latte / Osaka Jade live screenshots, theme restored — merged;
  live `call view`: overlay 0, every chart paints 1. The Phase 1–3 live
  sweeps are complete.
- 2026-10-02 WP-013 Engine ↔ plugin e2e on the test host: two full runs
  47/47 after the unlock (gate G2), the live checklist of WP-020…031
  (keys, arming, drift and decisions flows, overlay paint counters,
  pill), FINDINGS with one real mismatch (overlay aggregates once on
  open in the live shell because the shell injects `service` after
  creation — WP-037) — merged. Incident: a `wtype` text ran in the test
  host's terminal after a click closed the panel (exit 127, harmless);
  rule added to ORCHESTRATION §11.
- 2026-10-02 WP-040 AUR package `jax-seldon` (PKGBUILD with the `watch`
  feature, `.SRCINFO`, user unit installed not enabled), release workflow
  (tag → musl asset + checksums, PKGBUILD bump, AUR push and plugin
  subtree split skipped without secrets, `workflow_dispatch` dry run),
  packaging/README.md with the operator's one-time setup — merged after
  one review round; ADR-0022 accepted. Test-host `makepkg` and the first
  real workflow run wait for the operator (guard exception, secrets).
- 2026-10-02 WP-041 Plugin README along the marketplace template,
  preview.png from offscreen renders (no bar pill until a live shot),
  Security section cross-checked against the plugin's three non-engine
  calls, KEYBINDINGS.md, plugin/LICENSE — merged after one review round;
  CONTRACT.md probe form `seldon --version --json`; SPEC-PLUGIN §10
  names wl-copy and the terminal launcher.
- 2026-10-02 WP-036 Dossier follow-ups: `omarchy-base` vs `user` origin
  from Omarchy's package lists (read-only), "Before the logbook" lists
  only the user's packages plus a base-count line, `packages.explicit`
  fence in the init templates and the fixture, empty deviation case
  cells filled from later cased events — merged; 111 dossier-related
  tests; real-host read-only run (counts only).
- 2026-10-02 WP-031 Prime Radiant charts (Heatmap, Series, DriftBars,
  RiskDonut, Timeline) and The Plan as the sixth slot; one-pass period
  table; hover read-outs via IPC; first-frame rule proven in the harness
  (per-file aggregation counters) — merged after one review round; 72
  node + 312 overlay checks; offscreen frame profile 4–7 ms; live profile
  pending the test host (operator item).
- 2026-10-02 WP-035 `seldon dossier` (eight generated fences in
  system/*.md from read-only queries, new `packages.explicit`, user text
  kept byte for byte, config redaction over host strings), "Before the
  logbook" group in REBUILD.md, `init` runs it once — merged after one
  review round; one real-host read-only run (counts only).
- 2026-10-02 WP-034 `seldon watch` behind the `watch` feature (notify
  only under the feature, debounce, generated-file filters, lock retry,
  one rebuild at start, clean SIGTERM/SIGINT), systemd user unit template
  and README (never installed by the engine), `check-rss` — merged after
  one review round; 8 watch tests; RSS 7.9 MB musl / 9.1 MB bench on the
  ×10 fixture.
- 2026-10-02 WP-032 `seldon rebuild` → outputs/REBUILD.md (seven sections,
  AUR rule from the command, open drift in place, dismissed changes
  "deliberately not reproduced", fence `rebuild`, only-on-change write)
  — merged after one small fix round; 329 engine tests; golden
  byte-identical on the fixture. Gap found by the review: pre-logbook
  packages are only counted → WP-035.
- 2026-10-02 WP-030 Prime Radiant overlay skeleton (five-slot 12-column
  grid with reflow, period selector 30/90/365/All, keyboard, precomputed
  period table in the service, overlay harness at 1080p/1440p/1.25 scale)
  — merged; 62 node + 180 overlay checks; live keys/screenshots on the
  test host pending the unlock.
- 2026-10-02 WP-024 Logbook templates en/de (English keys and headings,
  prose per language), logbook AGENTS.md, wizard first capture with
  `--since` backfill and pre-Seldon baseline, Claude Code / Omarchy-Agent
  harness installs, theme hook opt-in, `SELDON_TEST_GUARD` — merged; 308
  engine tests. **Phase 2 is code-complete** (live sweeps pending the
  test host unlock).
- 2026-10-01 WP-022 `seldon agent start` (config-driven launcher argv,
  `{prompt}` as one argument, detached, shell-string launchers refused)
  and the *Start agent* card action (key `a`) — merged after one review
  round; 548 panel checks on the rebased branch.
- 2026-10-01 WP-023 Decisions and Memory tabs (digits 4 and 6, new-decision
  sheet, six-tab panel) — merged; 53/171/519 checks; real-engine offscreen
  run on the test host; live sweep pending the unlock.
- 2026-10-01 WP-016 Fixture additions (three drift index variants,
  PreToolUse/Edit/Write hook payloads, regenerated fixture STATUS.md) —
  merged; validator 109 instances, 8 variants.
- 2026-10-01 WP-021 Drift sheet (link / explain / dismiss, group fan-out
  with `--only`, crisis strip → first crisis, "+N more", IPC `resolve`) —
  merged; 47/148/419 checks; real-engine run in a private offscreen shell
  on the test host; live sweep pending the unlock.
- 2026-10-01 WP-020 Work tab (three columns, case card with two-press
  arming, new-case sheet, `wipLimit`, badge `+(members−1)`) — merged; 261
  panel checks; real-engine smoke via IPC on the test host; live key smoke
  and theme sweep pending the test host unlock.
- 2026-10-01 WP-008 Reconciliation and `drift` commands (list/show/link/
  explain/dismiss with group fan-out and `--only`), `after_capture` case
  bookkeeping, ADR-0021 folding, detached editor launch — merged; 290+
  engine tests. **Phase 0 is code-complete.**
- 2026-10-01 WP-009 Hooks (Claude Code PreToolUse, generic, session
  start/stop, `hook install`), shared command parser `pkgcmd.rs`, shared
  attribution pass, green zone per ADR-0019 — merged after one review
  round; 277 engine tests; hook under 2 ms in release.
- 2026-10-01 WP-012 Panel actions: QuickEntry, Capture now, Open in
  editor; harness HOME isolation and real-dir guard — merged; smoke on the
  test host with the real engine found the `open --editor` launcher bug
  (fix in WP-008).
- 2026-10-01 WP-007 Index builder, `index`/`status`, ledger views,
  STATUS.md, drift cap (ADR-0020), fast rebuild — merged; golden test
  zero differences against the fixture; ×10 build 4.7 ms, ×150 78 ms.
- 2026-10-01 WP-015 Fixture corrections — merged; verification step for
  C-2026-001, green `tee` event, pre/post snapshot pair, three index
  variants, case walker and index-time checks (101 instances, 71 events).
- 2026-10-01 WP-011 Panel tabs Today/Changelog/System, crisis strip,
  snapper banner, keyboard per §5, panel harness (86 checks), event-driven
  test waits, three-theme sweep on the test host — merged.
- 2026-10-01 WP-005 Collectors plugins/theme/config, SHA-256 in `sys`,
  theme-set hook script — merged; ADR-0018 applied; 21 collector tests;
  real-host read-only run: 38 plugins, 25 config files, second run 0.
- 2026-10-01 WP-006 `log`, `event`, `plan`, `decide`, `open`; case state
  machine; journal; `--config`; `--` forms — merged; 130 engine tests;
  ledger-first plan steps; specs amended (SPEC-ENGINE §3, SPEC-LOGBOOK §3).
- 2026-10-01 WP-004 Collectors pacman/snapper/omarchy + event model, ledger,
  redaction, collector registry, `capture` — merged after two review rounds
  (attribution per ADR-0017, Running-line rewind, greedy userinfo
  redaction, capture-time dedupe); 109 engine tests; real-host read-only
  capture 1230 events, second run 0.
- 2026-10-01 WP-010 Plugin skeleton — merged; Service state machine, pill,
  banners, headless harness (48 checks), token check; smoke-tested on the
  test host; ADR-0016 (install via AUR helper); plugin test expectations
  updated to the 4-item drift fixture at merge.
- 2026-10-01 WP-003 Engine core — merged; config, logbook layout, lossless
  frontmatter, typed models, `init` wizard, `doctor`; 71 tests; specs
  amended (SPEC-ENGINE §2 §3 §9, SPEC-LOGBOOK §6).
- 2026-10-01 WP-014 Contract v1 follow-ups — merged; drift group fields,
  `resolutionDetail`, open `-Syu` group in the sample index (4 drift items),
  token rule per ADR-0015; `just check` green on main.
- 2026-10-01 WP-002 Contract and fixtures — merged `wp/002-contract-fixtures`;
  `just check` incl. schema-validate green on main; ADR-0012 accepted with
  review edits; ADR-0013 (drift grouping, debate) and ADR-0014 (attribution,
  zones) decided; follow-ups in WP-014.
- 2026-10-01 WP-001 Repo scaffold, CI, toolchain — merged `wp/001-scaffold`;
  `just check` green on main; handover in `work/completed/WP-001/`.

## Decided 2026-10-01
- Default logbook path `~/Seldon`, wizard options → ADR-0010.
- Binary name `seldon` with `jax-seldon` symlink → ADR-0001 confirmed.
- Monorepo `jax-seldon`, installable plugin published as
  `jax-seldon-plugin` via subtree split → ADR-0009.
- Snapper collector degraded by default, user opts in with one command
  → ADR-0011.

## Decided 2026-10-05
- ADR-0027 (act, then account): a case the user started authorises the
  agent to act; privileged commands in an attended session; the agent
  takes its own snapshot; install route as the software documents it; the
  agent verifies and closes (spot checks are optional, reopen in one
  click); manual cases and hand-off stay first-class; steps that can make
  the machine unbootable (R3, alwaysRed hits) need the user's explicit go.
  Supersedes ADR-0023 §1 in part.
- The test host follows main (WP-098); productive machines get releases only.
- No Omarchy upstream contribution before 1.0.

## Decided 2026-10-04 (review follow-ups)
- Snapper access by a read grant (`setfacl … rx /.snapshots`) instead of
  the `ALLOW_USERS` opt-in → ADR-0026 supersedes ADR-0011 (WP-079); the
  operator reverts the opt-in on their machines after the read grant.
- Release provenance: GitHub artifact attestations now, verified by
  install.sh when `gh` is present, `--require-verified` optional; minisign
  later if needed (WP-080).
- State-directory loss: ledger note of an existing kind + capture line +
  doctor check, no contract change (WP-081).
- Autocommit result in `index.json`, `meta.truncated` and a dedicated
  state-loss event kind are bundled into one contract v2 ADR for v0.2.0.
- Global hook installs serve only sessions inside the logbook by default
  (`[hooks] scope = "logbook"`): confirmed.
- v0.1.3 released 2026-10-05 (0.1.3 wave, run 37289208720); v0.1.2 released 2026-10-04 (tag on the operator's go, run 37236761814:
  build, release, aur (no-op), bump, plugin all green; tarball and install.sh
  attestations verified with `gh attestation verify`; plugin repository
  tagged). Next: the contract v2 bundle (v0.2.0) and the 0.1.3 follow-ups.

## Open questions for the operator
- **Release v0.1.3 published 2026-10-05** (run 37289208720: build,
  release, aur (no-op), bump, plugin all green; tarball and install.sh
  attestations verified; plugin repository at `v0.1.3`). Update your
  machines with the installer one-liner and the plugin update command.
- **Release v0.1.2 published 2026-10-04:** GitHub release with install.sh,
  the musl tarball, the source tarball and SHA256SUMS, all attested;
  PKGBUILD/.SRCINFO bumped on main (395ad08); plugin repository at
  `v0.1.2`. Your two follow-ups: the `v*` tag ruleset and the snapper
  revert on your machines (`seldon doctor` prints the line).
- **Tag ruleset:** the repository has no ruleset protecting `v*` tags.
  Since WP-080 the tag is the trust anchor of the provenance check
  (`--source-ref refs/tags/<tag>`); a ruleset that limits tag creation and
  deletion to you closes that gap. Also: after the read grant is live on
  your machines, revert the old snapper opt-in with the line `seldon
  doctor` prints.
- **Review of v0.1.1 processed (2026-10-04):** 22 work packages
  (WP-055…076) merged, follow-ups WP-077/078 merged; the six decisions
  are taken (see *Decided 2026-10-04*). Still yours: a `scripts/guard.sh`
  false positive on path words inside command text (seen four times);
  dependabot for the pinned actions; the plugin-repo README push
  ("push plugin main"); the live bar check on the test host.
- **Test host:** unlocked 21:04 UTC, stay-awake on; G2 passed. The
  helper `/tmp/seldon-unlock.sh` there is yours. An incident line
  (`Live smoke: dismissed, test host package`, command not found) sits
  only in the memory of the open foot bash on the test host; `history -d`
  before closing it if you care. `scripts/guard.sh` comments cite
  "HERDR-SETUP.md §5" for the theme-sweep exception, which that doc does
  not mention — a comment fix in your file.
- **ssh alias `test`**: docs and the e2e script default to `ssh test`; this
  dev host has no such alias. Add `Host test` to `~/.ssh/config` (name in
  `memory/local.md`) or keep passing `SELDON_TEST_HOST=<alias>`.
- **Phase 0 exit done** (G3): real logbook at `~/Seldon`, snapper
  enabled by you (ADR-0011), one case worked by Claude Code, STATUS.md
  rendered. Finding F1 (the engine's own theme hook file shows up as
  drift) → WP-038 queued.
- **Guard updated on your decision (2026-10-02):** makepkg whitelist over
  ssh (two exact forms), service rule at the command position, comment
  fix; 74-row test table green.
- **Vault import ready (WP-043):** run the dry run yourself and read the
  report before applying — see the orchestrator's message; `--apply`
  writes to `~/Seldon` and is yours to run.
- **ADR-0024 flip list (when the AUR package is live):** plugin
  `INSTALL_ENGINE_COMMAND` and `ENGINE_MISSING_DETAIL`, the contract-
  mismatch fix (`UPDATE_ENGINE_COMMAND`, the installer since v0.1.1); README.md: the bold
  sentence in Quick start step 1, the `> [!NOTE]` v0.1.0 block, "Engine
  from the AUR" under "Install options"; plugin/README: step 1 first
  sentence, the NOTE block, "Once the AUR package is live", the States
  row and the Security constants; docs/user 01 install order.
- **Release v0.1.1 published 2026-10-03** (tag on the operator's go; GitHub
  release with install.sh, the musl tarball, the source tarball and
  SHA256SUMS; PKGBUILD/.SRCINFO bumped on main; plugin repository at
  `v0.1.1`; AUR skipped, no account yet). The operator's end-to-end test of
  install.sh on the laptop is next. Preparation record: (WP-038 self-attribution, WP-039
  panel width, WP-043 import, WP-050 advisory warning + alwaysRed, WP-049
  completions/man page/removal commands, update command → installer):
  version 0.1.1 in `engine/Cargo.toml` and `plugin/manifest.json`,
  CHANGELOG section, dry run on `main`; the tag is the operator's
  (`git tag -a v0.1.1 -m "Seldon 0.1.1" && git push origin v0.1.1`).
  WP-051 (Prime Radiant assets) is part of 0.1.1 on the operator's word
  and is merged; the dry run on `main` runs next, then the tag. Deferred to the next release:
  the panel surfacing `plan start` warnings.
  Earlier proposal text: say "v0.1.1 vorbereiten" and the
  orchestrator prepares CHANGELOG, version bump and dry run, then asks
  for the tag.
- **Release v0.1.0 published 2026-10-02:** repo public, GitHub release
  with the three assets, `jax-seldon-plugin` filled (branch `main` + tag
  `v0.1.0`), PKGBUILD/.SRCINFO bumped on `main`. **AUR still open:**
  registration on aur.archlinux.org is temporarily closed; when it
  reopens, create the account, tell the orchestrator, and it generates a
  new key, sets `AUR_SSH_PRIVATE_KEY` and pushes `packaging/` by hand
  (no new tag needed). Until then users install the engine from the
  release asset.
- **ADR-0022** (AUR package builds against glibc; the static musl binary
  is the GitHub release asset) reads AGENTS.md §7 without changing it —
  accepted by the orchestrator, veto possible.
- **Live sweeps done**, the QSG frame-timing profile too (next item); `call view` paint counters (1 per chart, 0–2 ms) are the
  live record.
- **Live frame profile of the Prime Radiant (WP-031): done 2026-10-05**
  on the test host without changing Hyprland's environment
  (`work/completed/WP-031/LIVE-PROFILE-2026-10-05.md`): period switches
  ≤ 2 ms render / ≤ 8 ms polish at every index size; the first frame of
  a newly opened overlay 23–26 ms, independent of the data (new-surface
  uploads, not aggregation: 0).
- **FYI, veto possible:** the Prime Radiant's default period is 90 days
  (30/90/365/All available; resets to 90 d on every open). 365 d would
  leave the drift bars and timeline sparse on every logbook younger than
  a year. Say so if you prefer 365 d.
- **FYI, veto possible:** generated logbooks use English headings in every
  language (`# Decisions`, `## Purpose`, `## History` …) with German prose
  (ADR-0007, WP-024). The fixture logbook still has German headings; the
  schema track aligns it.
- **AGENTS.md §7 exit codes** say 0/1/2/3; SPEC-ENGINE §3 and the code add
  `4 lock held`. AGENTS.md needs a one-line fix on operator instruction.
- **`scripts/guard.sh` false positives** (operator-owned): the "write under
  /etc, /usr" rule fires when such a path is a read-only *source* argument;
  the package-manager rule fires on any command text that merely mentions
  the package manager (e.g. a heredoc writing a CI file). Both are
  trade-offs for the operator; the WP-001 review has concrete proposals.
- **`serde_yaml` is deprecated upstream** (0.9.34+deprecated). It works and
  the YAML surface is flat, but AUR reviewers may flag it. Before WP-040,
  AGENTS.md §7 should allow a maintained fork with the same API
  (`serde_yaml_ng`); one-commit swap. Operator instruction needed for the
  AGENTS.md line. `rust-version` was raised to 1.89 for `File::try_lock`
  (both hosts have 1.98) — orchestrator approved.
- **AGENTS.md §7** lists `index.json` among the generated files that carry
  the `<!-- generated by seldon; do not edit -->` header; a closed-schema
  JSON file cannot carry one. One-word fix (strike `index.json`) on
  operator instruction.
- **CI may fetch `omarchy-plugin-validate`** from a pinned Omarchy tag with
  a checksum (network in CI, not red zone). FYI; scheduled unless vetoed.
- **Omarchy is a package install, not a git checkout.** Version comes from
  `omarchy-version`; WP-033 (update-impact from release notes) needs
  another source. Decide in Phase 3.
- **Sample index data.** The event list in `fixtures/index.sample.json` is
  partly generated filler (e.g. `pacman upgrade` with subject `110`,
  `snapper snapshot` with a config path). WP-002 rewrites it from the
  fixture logbook.
