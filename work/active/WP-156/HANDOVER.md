# WP-156 — Handover: the desk steps aside for what it opens; one agent per case

Branch `wp/156-desk-steps-aside` from `next` (d747daa); merge into `next`.
Plan, the launcher facts and the decisions the WP left open:
[PLAN.md](PLAN.md).

## What was done

### Engine (`engine/src/sessions.rs` new; `commands/agent.rs`, `commands/open.rs`)

- **Sessions by their marker.** No Omarchy launcher reports a window or a
  pid that survives (`omarchy-launch-tui` runs `setsid`, which forks
  because the engine starts the launcher as a process-group leader; the
  app-id `org.omarchy.agent` is shared by every agent; the title is the
  agent's). What survives the whole chain is the environment the engine
  sets. `sessions::marked` reads `/proc/<pid>/environ` of the user's own
  processes and keeps those whose environment holds every key/value asked
  for; only those keys are compared or kept. A session of `agent start`
  is `SELDON_CASE=<ID>` **and** `SELDON_LOGBOOK=<this logbook>` (an id is
  unique per logbook only). No state file, nothing goes stale, no gap
  between the launch and the terminal.
- **`agent start <ID>` refuses** (exit 1, nothing launched, the active
  case untouched; checked under the lock after the status check) while a
  session lives: "an agent is already working on C-… (pid N); focus it
  with `seldon agent focus C-…`, or start another with `seldon agent
  start C-… --again`; nothing was launched". **`--again`** (conflicts with
  `--new`) skips it. `--new` is never refused. `agent ask` sets no
  `SELDON_CASE` and is no session.
- **`seldon agent focus <ID> [--json]`**: the session's window from
  `hyprctl clients -j` (a marked pid, else its nearest ancestor; class
  `org.omarchy.agent` first among several), focused as
  `omarchy-launch-or-focus` does — `hl.dsp.focus({ window = "address:…"
  })`, then `focuswindow address:…`; success is hyprctl's `ok`. The
  address must be `0x` + 1–16 hex digits before it reaches the Lua
  expression. Exit 1 with a fix for: unknown case, no session, no
  hyprctl, no window, a refused dispatch.
- **`seldon agent sessions [--json]`**: `{sessions: [{case, pids,
  actor}]}`, read-only, no lock.
- **`open --editor` (no terminal, the plugin's path)** sets
  `SELDON_OPEN=<path>` on the editor launcher. A second open of the same
  path while that editor lives focuses its window (`editor: {launched:
  false, focused: true, address, pid}`), or, while Hyprland lists no
  window yet, starts nothing (`{launched: false, running: true, pid}`);
  without hyprctl it launches as before. `decide`'s editor follows. The
  terminal path is unchanged.

### Plugin

- **The desk steps aside.** `Service.stepAside()` closes the desk through
  the facade (`Desk.dismiss`, as Esc) after a *successful* answer of
  `agent start` (also `--new`), `agent focus`, `open --editor` (launched,
  focused or opening; also a new decision's editor) and `agent ask`
  (`Model.opensWindow`; ready for WP-124b); the notices' *Run in terminal*
  closes it at once (a detached launch has no answer — Omarchy's menus
  close before they launch, too). A refusal keeps the desk open with the
  text. The selection is remembered for the next open.
- **One agent per case.** `Service.agentSessions` from `agent sessions
  --json` in its own process beside the queue (like `doctor`): on desk
  open, after every `agent` answer, on index change while open, every
  15 s while open. An active case with a session (`Model.withSession`)
  shows **Focus** (`agent focus <id> --json`, key `a`, runs at once, Enter
  still never takes it) in place of *Hand to agent*, "agent working" in
  the bar's meta and on its row, and the Agent row "working now ·
  agent:default · pid N". A stale desk heals: the "already working"
  refusal and the "no agent is working" refusal both ask again.
- **No double launch.** Busy labels while a call is in flight
  (*Starting…*, *Focusing…*, *Opening…*, disabled); the service refuses an
  open while one is pending, and the same target within 2 s of a
  successful open (`Model.OPEN_REPEAT_MS`), for every *Open in editor* on
  the desk.
- `Model.validateArgs`: `agent focus <caseId> --json`, `agent sessions
  --json`. `--again` stays CLI-only.

### Docs

SPEC-ENGINE §3 (synopsis and JSON: `--again`, `focus`, `sessions`, the
marker, `open`'s focus), CONTRACT.md argv table (two commands; no schema
change, `contractVersion` unchanged, fixtures untouched), SPEC-PLUGIN §5
(stepping aside) and §5.4 (Focus, busy, the 2 s rule), KEYBINDINGS (`a`),
TESTING (section 8g, the fake's knobs), CHANGELOG (Engine, Plugin); the
user guides' CLI reference (`docs/user/{en,de}/05-cli-reference.md`): a
paragraph on one agent per case and the generated help blocks of `agent`,
`agent start`, `agent focus`, `agent sessions`; the German page's source
line moved to the new English page.

## Decisions (PLAN.md D1–D8)

- **D1** tracking by the environment marker in `/proc`, not by the
  launcher pid (gone at once) or window app-id/title (shared / the
  agent's). No state file.
- **D2** the marker matches case *and* logbook.
- **D3** focus lives in the engine (`agent focus`), so the CLI refusal can
  name a command and the plugin keeps "fixed argv to seldon" as its only
  way to act.
- **D4** the desk hides after the engine's success, not before the call:
  a refusal must stay readable. *Run in terminal* has no answer and hides
  at once.
- **D5** the desk offers no `--again`. **D6** Focus runs at once (it
  writes nothing). **D7** the editor's own marker `SELDON_OPEN` (focus the
  first nvim instead of a second one on the same swap file); the 2 s rule
  is the floor for editors the marker cannot follow (a GUI editor that
  hands off to a running instance). **D8** `agent ask` takes the same
  step-aside path.
- The desk passes no window to focus: no launcher reports an address; the
  live check shows Hyprland focuses the new window once the overlay is
  gone (below).

## How it was verified

- `cargo fmt`, `cargo clippy --all-targets -D warnings` clean.
- Engine: `tests/one_agent_per_case.rs` (7 tests, real stub processes that
  stay alive, killed by their own process group): refusal text, one
  launch, active case untouched, `sessions` listing, `--again`, end of
  session → start again; `--again --new` refused; `--new` never refused;
  another logbook's session with the same id does not count; `ask` is no
  session; `focus` with a stub `hyprctl` (Lua form, legacy fallback, no
  hyprctl, no window, a non-hex address never dispatched, session gone);
  `open` twice (no hyprctl → launch, window not yet there → running,
  window → focused, another file → launch). Unit tests in `sessions.rs`
  (marker match, first entry wins, `stat` with `)` in the name, window by
  pid or ancestor, address check). Full `cargo test` green; no stub
  process left behind.
- Plugin: `model.test.js` 149 (6 new: argv forms, sessions parser, focus
  and open answers, `opensWindow`, the 2 s rule, Focus/withSession);
  `omarchy plugin validate plugin/` and `just qmllint` (46 files, 0
  warnings) before each commit.
- Harness `desk-view.sh` section 8g (`aside`, `aside-stale`, `aside-gone`,
  `aside-double`, `aside-focused`): launched → desk hidden through the
  facade (`a a`, click + Confirm, Focus); Focus in place of Hand to agent
  with "agent working" on the row and in the bar; stale both ways; `a`×4
  and `e`×3 → one call each, busy labels; the same open within 2 s sends
  nothing, after 2 s it does; Today's `e`×3 → one open (the service's
  guard). Existing cases that kept acting after a launch now summon the
  desk again (today-live, today-new, work-live, work-agent, work-run,
  decisions-live — the latter also waits 2.1 s before each repeated open),
  and the snapper notice's *Run in terminal* steps aside. `service-states.sh`:
  the back-to-back opens wait for each other. Fake engine: `$HOME/sessions`,
  `agent sessions` beside the queue (`sessions.log`), `agent focus`,
  `FAKE_SELDON_NO_SESSION`, `_SESSIONS_LATE`, `_FOCUS_GONE`, `_OPEN_FOCUSED`.
- **Mutants** (scratch copy, own target dir; a trimmed desk-view with
  section 8g only): refusal removed, logbook key dropped from the marker,
  legacy dispatch removed, editor focus removed, `--again` ignored,
  `stepAside` a no-op, the 2 s rule removed, `withSession` not applied,
  no sessions refresh after an agent answer, stepping aside on a refusal
  — all caught. The service's one-open-at-a-time guard first **survived**
  (Work's own button guard hid it); Today's `e`×3 case added, now caught.
- **Live check on the test host** (authorised; a private `quickshell -p`
  with the real layer-shell `DeskWindow` and a stand-in facade, this
  branch's release engine, its own config/state/logbook under a scratch
  dir in `~/.cache`; the installed engine, plugin and shell untouched;
  the operator's two open windows on workspaces 1 and 10 untouched; work
  done on empty workspaces 2–4, workspace 2 restored at the end; scratch
  removed, my three processes ended by PID). The launcher was a stand-in
  `["omarchy-launch-tui", "--app-id=org.omarchy.agent", "tail", "-f",
  "/dev/null", "{prompt}"]` — the same last hop as `omarchy agent prompt`
  (omarchy-launch-tui → setsid → uwsm-app → Alacritty) without starting a
  real AI agent on the host. Results:
  1. Desk open (layer `jax-seldon-desk` 1536×838 at y 26) → `a a` on
     C-2026-001 → desk layer gone, `stepAsides` 1, **the active window is
     the new `org.omarchy.agent` terminal** on the current workspace,
     focused (screenshot checked by eye). Its pid is the session the
     engine reports; `/proc/<pid>/environ` holds `SELDON_CASE` and
     `SELDON_LOGBOOK`.
  2. From empty workspace 3: the desk shows **Focus · To verification ·
     Drop · Open in editor**, "agent working · C-2026-001 · R1", the row
     "agent working", "Agent: working now · agent:default · pid N"
     (screenshot). `seldon agent start C-2026-001` on the CLI: exit 1 with
     the focus hint. `a` → desk gone, Hyprland switched to workspace 2,
     the agent terminal active.
  3. *Open in editor* on C-2026-002 from workspace 3 → nvim
     (`org.omarchy.nvim`) in front, desk gone; from workspace 4 the same
     again → no second editor, the first focused (same pid, workspace 3).
- `flock /tmp/seldon-check.lock just check` — see "Final check".

## Not done / for later

- The desk does not offer `--again` (D5); the CLI has it.
- `agent ask` is not run by the plugin yet (WP-124b); its answer will
  step aside through `Model.opensWindow` without further work.
- Today's active-case tiles do not show "agent working" (Work's row,
  bar and detail do).
- The live check ran the stand-in launcher, not a real `omarchy agent
  prompt` agent (see above); WP-126's live sweep can repeat step 1 with
  the host's default agent on a throwaway case.

## Security (for stage 2)

- **New read of `/proc/<pid>/environ`** (engine, `agent start|focus|
  sessions`, `open --editor`): the kernel allows it for the user's own
  processes only; others fail and are skipped. The engine compares only
  `SELDON_CASE`, `SELDON_LOGBOOK`, `SELDON_OPEN` and reads
  `SELDON_ACTOR`; nothing else of any environment is kept, printed or
  logged. Not a collector; nothing is recorded.
- **New `hyprctl` runs** (engine): `clients -j` (read) and two fixed
  dispatch forms. The Lua form embeds the address, which must match `0x`
  + 1–16 hex digits first (unit test with an injection attempt; the
  integration test proves a malformed address never reaches a dispatch).
  No shell, fixed argv, 3 s timeout. The launcher's `hyprctl` refusal
  (CODE_RUNNERS) is unaffected: this is not a launcher.
- **New env var on the editor launch**: `SELDON_OPEN=<path>` (a logbook
  path, inherited by what the user starts from that editor; harmless).
- Plugin: two new fixed argv forms, validated (`CASE_ID`); no shell
  strings; the sessions answer is parsed defensively (ids and actors
  checked, pids integers).
- **Guard block**: on the test host a read-only session-lock query
  (`loginctl show-session … -p LockedHint`) was blocked as a "service or
  boot command". I dropped the check and did not try it another way; the
  live check did not need it (the screenshots show an unlocked session).

## Open questions

None blocking. For the orchestrator: CONTRACT.md's argv table gains two
read-only/UI commands (`agent focus`, `agent sessions`) without a schema or
`contractVersion` change, as WP-101 and WP-124 added theirs; say if that
needs an ADR line.

## Final check

Round 1 on 5bf77b4: exit 1 at `docs-check` (the CLI reference had no
help blocks for `agent focus`/`agent sessions`, and `agent`/`agent start`
had changed); everything before it passed (fmt, clippy, engine tests with
and without `watch`, packaging, install, deploy, schema). Fixed in 7a8d0a8
and 1b91d62.

`flock /tmp/seldon-check.lock just check` on **1b91d62: exit 0** (`check:
ok`; docs-check ok, qmllint ok 46 files, model.test.js 149,
real-home-guard 11, service-states 328, desk-view 1542, bar-view 194).
Only this handover changed after that commit.
