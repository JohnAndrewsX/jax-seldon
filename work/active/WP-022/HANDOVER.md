WP-022 HANDOVER

Branch `wp/022-start-agent`, worktree `wt/WP-022`, based on `main` at
`90c3dec`. Not pushed, no PR. Commits in `main..HEAD`:
`2ae28b3` engine · `7c6de25` hooks README · `11df0c2` plugin ·
`d3ea6ae` TESTING · `779bd52` memory · then this handover.

## Done

**Engine: `seldon agent start <caseId> [--launcher NAME] [--json]`**
(`engine/src/commands/agent.rs`; `main.rs`: one variant and one arm;
`commands/mod.rs`: `pub mod agent`)
- The case must be `active`. A queued case gets exit 1 and the message
  "C-… is queued; start it first: `seldon plan start C-…`".
  Verification, completed and dropped cases get exit 1 too ("has status
  …"). Unknown or malformed ids: exit 1. No logbook: exit 3. Lock held:
  exit 4.
- Order of work: check the launcher; take the lock; check the case; write
  `.seldon/active-case`; build the prompt; launch. If the session-start
  block or the launch fails, the previous active case is restored (or the
  file is removed if there was none). No ledger event and no autocommit:
  `.seldon/active-case` is the only write.
- Prompt: `Work case C-… in the Seldon logbook at <path>; every mutating
  command is recorded.`, a blank line, then `hook::session_start(ctx)`,
  which already shows the case as active.
- Launcher: an argv list from `config.toml`:
  - `[agent] launcher` is the default;
  - `[agent.launchers] NAME = [...]` adds named ones for `--launcher NAME`;
  - `omarchy` is the built-in launcher, available even when the default
    is overridden;
  - an unknown name gets exit 1, and the message lists the known names.
- Launcher rules, checked before any write:
  - the list is not empty;
  - the first element is a program name without `/`, or an absolute
    path, and is not `{prompt}`;
  - exactly one element is exactly `{prompt}`. It becomes the prompt as
    one argument. A partial use such as `--p={prompt}` is refused;
  - no shell (`sh bash zsh dash ksh mksh fish nu xonsh eval`) may appear
    before `{prompt}`, and none of the three Omarchy launchers that build
    a `bash -c` string (see Decisions 2).
- Launch: reuses WP-008's `launch_detached`, now
  `launch_detached(Command, stderr)` (`open.rs` passes `Stdio::null()`):
  - stdin and stdout are null;
  - stderr goes to `~/.local/state/seldon/agent-launch.log`, a file, not
    a pipe. A launcher that exits non-zero within the 200 ms grace is
    reported with its last stderr lines;
  - the launcher gets its own process group, and nothing waits for it;
  - cwd is the logbook root and `SELDON_LOGBOOK` is set to it. When a
    non-default config was used, `SELDON_CONFIG` is set too.
- `--json` returns `{launched, launcher, program, argv (with the
  "{prompt}" placeholder, never the prompt), case, cwd,
  previousActiveCase}`. The human output is "Agent started on C-… with
  launcher `default` (omarchy) in ~/…".
- Config: `AgentConfig` in `config.rs` (`launcher`, `launchers`) and
  `DEFAULT_AGENT_LAUNCHER`. The section is skipped on save while it is the
  default, so `seldon init`'s config.toml is unchanged and WP-024's init
  code is untouched. The `save_keeps_unknown_keys` unit test used
  `[agent] launcher = "x"` (a string); it now uses an array.

**Plugin**
- `Model.js`:
  - the `agent` action ("Start agent", `twice: true`), active cases only,
    never the first action;
  - `agentArgs(id)` and `agentResult(...)`;
  - `validateArgs` accepts exactly `agent start <caseId> --json`;
  - `workCase` carries `agents`, and `caseAgents()` gives "agent:
    claude-code".
- `Service.qml`:
  - `startAgent(caseId)` writes to `planResult` with `action: "agent"`,
    so it uses the Work tab's result line and the one-at-a-time rule;
  - `setResult` and `runnerDone` have the `agent` branch;
  - `agent` errors stay in place and never reach `lastError` (the
    banner).
- `WorkTab.qml`:
  - `runAction` sends `agent` to `startAgent`;
  - `clickAction` arms `twice` actions;
  - key `a` arms or runs Start agent. Enter still arms the first action.
- `CaseCard.qml`:
  - the hint "Start agent on C-…? Press a again or click Confirm start
    agent.";
  - the "Confirm start agent" label while armed, and a tooltip;
  - the "agent: <name>" line from the case's `agents`.
- Buttons are disabled when `canWrite` is false. In dev mode the hint is
  the write blocker, and neither the key nor a click arms the action.
- Test harness:
  - `tests/plugin/fake-seldon` answers `agent start` with the engine's
    checks, messages and JSON. `FAKE_SELDON_NO_LAUNCHER` simulates a
    missing launcher;
  - the harness `shell.qml` gets the `["agent", id]` action;
  - new scenarios: service-states #28 (agent, agent-queued,
    agent-nolauncher, agent-devmode) and panel-view #10b `work-agent`,
    plus dev-mode gating steps added to #9 `work`;
  - the "Verify,Drop,Open" expectations are now "Verify,Start
    agent,Drop,Open".

**Docs**
- `engine/hooks/README.md`: a new section covering the command, the
  config, the rules, names and fallbacks.
- `plugin/README.md`: the actions table, the Start agent paragraph and
  the `a` key.
- `docs/TESTING.md`: the engine table row, the isolation note, the layer 2
  and layer 3 paragraphs, and the manual smoke step for an unlocked test
  host.

## Not done

- **The real launcher on the test host:** skipped. It is locked:
  `ssh pbbau-lnx-ea env OMARCHY_PATH=/usr/share/omarchy
  XDG_RUNTIME_DIR=/run/user/1000 omarchy-shell lock status` →
  `"locked":true`. The manual step is in TESTING.md layer 4, step 5
  ("Start agent (WP-022) …"). It also needs a default agent set
  (`omarchy default agent`).
- `CONTRACT.md`, `SPEC-ENGINE.md` §2/§3 and `SPEC-PLUGIN.md` §5 (the new
  command, the `[agent]` config keys, `agent-launch.log` in the state dir,
  the `a` key) are not edited; per the WP, the orchestrator edits docs on
  handover. Text to copy is in `engine/hooks/README.md` and
  `plugin/README.md`.
- No theme sweep and no screenshots: the session is locked, and the only
  new visuals are a button, a hint line and a caption line drawn with
  existing tokens.

## Verified by

- `just check` → `check: ok`, exit 0:
  - fmt and clippy clean;
  - every engine suite passes, including `tests/agent.rs` (8) and the
    unit tests in `commands/agent.rs` (3);
  - plugin-validate ok, tokens ok, qmllint ok (15 files);
  - `model.test.js: 48 passed`, `service-states: 166 passed, 0 failed`,
    `panel-view: 448 passed, 0 failed`.
- The first `just check` run failed only on the real-home guard. The cause
  was another worker (see Decisions 3). A rerun with the paths stable was
  green.
- Acceptance, engine (isolated `HOME` in my scratchpad, stub `omarchy`
  recording NUL-separated argv, debug build):
  `seldon agent start C-2026-001 --json` → `{"launched":true,"launcher":
  "default","program":"omarchy",…}`, exit 0, `real 0m0.013s`. One line in
  the call log, and `3 argv elements`: `agent`, `prompt`, and the prompt
  starting "Work case C-2026-001 in the Seldon logbook at …".
  `tests/agent.rs` checks the same, plus a title with quotes, `$(touch
  pwned)` and backticks that stays text in the one element, under 1 s.
- Queued case → `{"error":{"code":1,"message":"C-2026-002 is queued;
  start it first: `seldon plan start C-2026-002`"}}`, exit 1.
- Missing launcher → exit 1, "launcher `default`: `omarchy` not found; set
  `[agent] launcher` in config.toml", and the active case is restored
  (`tests/agent.rs`).
- Acceptance, panel: `work-agent` runs the real `Panel.qml` against the
  fake engine. Key `a` twice on C-2026-003 sends exactly `agent start
  C-2026-003 --json`, and a click plus Confirm on C-2026-004 sends
  `agent start C-2026-004 --json` (exact argv log). The result line shows
  "Agent started on C-2026-003 · launcher default (omarchy)". A malformed
  id never reaches the engine (service-states `agent`).
- **Disclosure: the real `omarchy` CLI ran once on the dev host.** In the
  manual demo I removed the stub, but `PATH` still had `/usr/bin`, so
  `agent start` ran the host's real `omarchy agent prompt <prompt>` once.
  The dev host has no default agent, so it exited 1 at once with "Choose
  default agent with: omarchy default agent <name>". The engine reported
  exactly that (exit 1, message relayed). Nothing was launched, and
  nothing outside the demo HOME was written.

## Learned (in memory/)

- `rust-notes.md`:
  - a file as stderr for a detached child;
  - `skip_serializing_if` for a new config section;
  - NUL-separated argv recording in `/bin/sh` stubs;
  - a slice pattern for the "exactly once" rule.
- `pitfalls.md`:
  - `omarchy agent prompt --inline` needs a terminal;
  - the Omarchy launchers that build `bash -c` strings;
  - `omarchy-shell lock status` over ssh needs `OMARCHY_PATH` and
    `XDG_RUNTIME_DIR`;
  - the real-home guard also catches other workers;
  - card actions other than the first need their own key and hint;
  - a new verb must be added to `runnerDone`'s in-place-error list.

## Decisions needed

1. **Default launcher without `--inline` (a deviation from the brief).**
   The brief says `["omarchy","agent","prompt","--inline","{prompt}"]`.
   `/usr/share/omarchy/bin/omarchy-agent` ends with `exec
   "${command[@]}"` when `--inline` is given, which runs the agent in the
   caller's terminal. Without it, it runs `omarchy-launch-tui
   --app-id=org.omarchy.agent …`, which opens a window. `agent start`
   always launches detached with null stdio (the plugin has no terminal),
   so `--inline` would start the agent with no terminal at all. I used
   `["omarchy","agent","prompt","{prompt}"]`. Please confirm, or tell me
   to switch it back.
2. **`omarchy-launch-floating-terminal-with-presentation` is refused**,
   although the WP lists it as an alternative. It builds `cmd="$*"` and
   runs `bash -c "…; $cmd; …"`, so the prompt (logbook text) would run as
   shell code (AGENTS.md §8). `omarchy-launch-or-focus` and
   `omarchy-launch-terminal-tmux` are refused for the same reason. The
   documented safe alternative is `["omarchy-launch-tui",
   "--app-id=org.omarchy.agent", "<agent>", …, "{prompt}"]`.
3. **Another worker wrote the operator's real Seldon paths.** At
   17:10:37 something wrote `~/.config/seldon/config.toml` (`logbook =`
   a path in WP-024's scratchpad `…/wt-WP-024/…/scratchpad/pty3/logbook`)
   and `~/.local/state/seldon/{cursors.json,index.json,manifest.json,lock,
   hooks/seldon-theme-set.sh}`. `seldon-theme-set.sh` exists only in
   `wt/WP-024/engine/src/commands/setup.rs`, so this looks like WP-024's
   pty test of `seldon init` run with the real `HOME`. I did not touch or
   remove these files (not mine, not my zone). Until they are cleaned up,
   any `seldon` call on the dev host without `--config` uses that config.
   Please have WP-024 checked and the files removed.
4. Smaller choices, for the record:
   - Start agent is refused on `verification` cases ("an active case's
     card");
   - Start agent writes no ledger event and no case Log line (the WP asks
     for neither);
   - the plugin key is `a`. Enter is taken by the first action (Verify).

## Touched outside WP scope

- `plugin/components/WorkTab.qml` (beyond the brief's "CaseCard.qml, one
  Service function, one Model helper"). The tab owns arming and the engine
  call, so `runAction`, `clickAction` and `textKey` needed one branch
  each. WP-023 does not touch WorkTab.
- `Model.js` got more than one helper: `agentArgs`, `agentResult`,
  `caseAgents`, the `ACTIONS` entry and the `validateArgs` case. All
  additive and next to the WP-020/021 code; none in the decision or
  memory helpers WP-023 works on.
- `engine/src/commands/open.rs`: the `launch_detached` signature (reuse,
  as the brief asks), and `engine/src/config.rs` (`[agent]`).
- `tests/plugin/harness/shell.qml`: one action line.
