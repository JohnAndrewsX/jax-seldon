# WP-117 — handover

Branch `wp/117-plugin-texts`, base `main` d919974. Commits: plan
(7d21927), implementation (8703b22), this handover.

## What was done

- **Terminal scripts** (`plugin/Model.js`, "Terminal scripts" section):
  five constants, `INSTALL_ENGINE_SCRIPT`, `UPDATE_ENGINE_SCRIPT`,
  `UPDATE_PLUGIN_SCRIPT`, `INIT_SCRIPT`, `SNAPPER_FIX_SCRIPT`. Each one
  is built once, at load time, from string literals by `terminalScript()`,
  following Omarchy's pattern (`omarchy-system-factory-reset`,
  `omarchy-update-confirm`, `omarchy-snapshot`):
  `gum style --bold 'Seldon: …'`; one paragraph (`--width 72`) saying
  what it does and whether it asks for a password; the command indented
  and single-quoted, exactly as *Copy* copies it; `if (set -o pipefail;
  <command>); then [follow-up;] <green line>; else <red line>; fi`. The
  wrapper (`omarchy-launch-floating-terminal-with-presentation`) adds the
  logo, the theme's gum env and "Done!". Every script ends on a gum line
  (exit 0), so "Done!" always follows.
  - Grant: after a successful grant it runs `seldon capture` (retries
    once after 3 s, for exit 4 / lock held, A6). Then it prints "Snapshots
    are now recorded. The panel updates by itself." On failure it prints
    "Nothing changed. Snapshots stay off; Seldon works without them."
  - Engine update: `seldon status` after success, so the new engine
    rewrites an index in an old contract version.
  - Install / update: "… In the Seldon panel, press Check again." (the
    engine is only probed on *Check again* in 0.1.4).
  - Init: "Your logbook is ready. The panel updates by itself." (init
    writes the index; the FileView's 5 s retry picks it up).
- **Service.fix**: *Copy* copies `banner.command`. *terminal* launches
  `banner.script`, but only if `Model.isTerminalScript(script)` (an
  exact match against the five constants). The `snapperHintIndex`
  plumbing, `SNAPPER_HINT` and the hint are gone.
- **Banners**: one sentence each. Buttons: *Install*, *Create*, *Grant*,
  *Update* (*Copy* and *Check again* stay).
  - Engine missing: without an index it is the setup step "Install the
    engine", accent tone. With an index it is "Seldon engine missing",
    urgent tone (`ctx.indexExists`, from `fileState` loaded/invalid).
  - Not initialised: "Create your logbook".
  - Snapper: "Read snapshots (optional)". The engine's message and
    `SNAPPER_FIX_GRANTS` move into the hover `full` text.
  - Engine too old and contract mismatch: shortened to one sentence each.
- **Today**: "1 event today" (singular), plural otherwise.
- **Docs**: SPEC-PLUGIN §5 (banner paragraph, new "Terminal scripts"
  paragraph, singular count). Outside §5, to keep the spec true:
  - §3: the engine-missing text no longer says "AUR package: coming soon".
  - §10: the launcher gets a constant script.

  Also updated: plugin README *States* and *Security* (the stale
  `yay -S jax-seldon` constant is removed from the list), TESTING.md
  (plugin test descriptions) and a CHANGELOG *Unreleased › Plugin* entry.

## Decisions (left open by the WP)

1. **All five terminals get the pattern**, not only the three the WP
   names. The goal says "every terminal the panel opens".
2. **Titles**: the three setup banners get step titles, as in the
   review's table. An engine that was there and is now gone gets a state
   title in the urgent tone. I did not use the review's
   "Install the engine" in red for that case.
3. **Check again stays** on engine-missing, not-initialised, too-old and
   snapper. 0.1.4 keeps the three banners; auto re-probe is WP-119. The
   install/update result lines therefore tell the user to press it.
   *Check again* on the snapper banner is still useful for a grant run
   outside the panel.
4. **No gum fallback** (A1): Omarchy's own bin calls gum unguarded.
   Without gum the command still runs and only the text lines are
   missing.
5. **pipefail subshell** around every command. Without it, a failed
   `curl -f` in `curl … | bash` would be reported as success (bash exits
   0 on empty input). A mutation probe confirms the test catches this.
6. **Plugin update result line does not claim an update**:
   `omarchy plugin update` exits 0 when the user answers "no" as well. It
   says "If the plugin was updated, the Seldon panel offers Restart shell
   to load it."
7. **`$USER` stays literal in the shown command** (single-quoted). This
   matches what *Copy* puts on the clipboard. It is expanded only where
   the command runs.

## Verification

- `flock /tmp/seldon-check.lock just check` → `check: ok` (exit 0):
  - model.test.js 101
  - terminal-scripts 43
  - service-states 330
  - panel-view 923
  - overlay-view 326
  - bar-view 194
  - plugin-validate ok
  - qmllint ok (29 files)
  - docs-check ok
- `omarchy plugin validate plugin/` and `qmllint` were run before each
  commit.
- New and changed tests:
  - model.test.js:
    - all five scripts pinned verbatim
    - each shows its command (single-quoted) and runs it
    - starts bold, has green and red lines, ends with `'; fi`
    - `bash -n` parses each one
    - a hostile index (`'; rm -rf ~; …$(reboot)` in the snapper message,
      `generatedAt`, the contract version and the engine version)
      changes no script
    - `isTerminalScript` refuses the bare command, near-misses and
      non-strings
    - banner texts, tones and buttons; the singular/plural
  - `tests/plugin/terminal-scripts.sh` (new; wired into
    `just plugin-test`):
    - runs each script inside the launcher's own line,
      `omarchy-show-logo; …; if (( $? != 130 )); then omarchy-show-done; fi`
    - uses stub sudo, curl, seldon, omarchy and gum, with a scratch HOME
    - covers success, refused password, curl 404, installer failure,
      lock held (two captures) and the follow-up calls only on success
    - runs once with the real gum to check its flags
  - Mutation probes (reverted afterwards):
    - dropping pipefail → 4 failures
    - dropping the grant's capture → 2 failures
  - service-states.sh:
    - engine missing with index: urgent "Seldon engine missing"
    - fresh home without index: accent "Install the engine"
    - the launcher receives the scripts verbatim for install, init,
      plugin update and grant; *Copy* receives the plain command
    - new case 14h: the index is replaced after *Grant* (as the
      script's capture does) → the banner goes with no click and no
      plugin engine call
    - live snapper: *Grant* changes nothing in the panel
  - panel-view.sh:
    - the snapper banner's sentence and buttons
    - hover shows the engine's message plus the grant text (and fits)
    - *Grant* sends the grant script to the launcher
    - no hint text
    - "1 event today" case
    - Create banner
  - overlay-view.sh: new title, still only *Copy*.
- Not run here: the live grant on the fresh test host (the
  orchestrator's). shellcheck is not installed on this host;
  `terminal-scripts.sh` was not shellchecked.

## Not done / for others

- `install.sh` and the `seldon init` wizard texts are WP-118. When
  install.sh gets its own announce line and context-aware next steps,
  the terminal will show both the plugin's "Seldon: install the engine"
  paragraph and install.sh's announce. They say the same thing in
  different words. WP-118 may want to drop its announce line when
  stdin is not a terminal, or keep both; this is a judgement call for
  the WP-118 dev and the reviewer.
- The user guides (`docs/user/*/10-troubleshooting.md` banner table, 01
  "Check again") still name *Install in terminal* / *Run in terminal* and
  the old titles. They are left for WP-118 (docs), as briefed. docs-check
  does not catch label drift.
- WP-118's list also names the plugin README *States/Troubleshooting*. I
  updated *States*. The "update the plugin first" troubleshooting line
  (UX review §4, F1) is release-notes/guide content and was left for
  WP-118.
- Auto re-probe after *Install*, the setup card and *Not now* are WP-119.

## Open questions

None blocking. For the stage-2 review (shell strings): the launcher still
runs the script through `bash -c` (Omarchy's wrapper). The only inputs
are the five literals; `Service.fix` cannot pass anything else.
