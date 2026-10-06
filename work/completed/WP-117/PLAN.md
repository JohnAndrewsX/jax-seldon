# WP-117 — plan

## What changes

1. **Terminal scripts (`plugin/Model.js`).** One constant script per
   command the panel can open in a terminal: `INSTALL_ENGINE_SCRIPT`,
   `UPDATE_ENGINE_SCRIPT`, `UPDATE_PLUGIN_SCRIPT`, `INIT_SCRIPT`,
   `SNAPPER_FIX_SCRIPT`. Each is built once, at load, from string
   literals only, in Omarchy's pattern (`omarchy-system-factory-reset`,
   `omarchy-update-confirm`, `omarchy-snapshot`): a bold `gum style` line
   (what), one plain line (why, password or not), the command indented,
   the command run, then a green (`--foreground 2`) result line or a red
   (`--foreground 1`) "Nothing changed" line. The presentation wrapper
   adds logo and "Done" and themes gum. The snapshot grant runs
   `seldon capture` after a successful grant, so the index changes and
   the banner goes by itself; the engine update runs `seldon status` so
   the index is rewritten by the new engine.
2. **Banners.** Each banner carries `command` (shown, copied) and
   `script` (the terminal). `Service.fix("terminal")` launches only a
   script from the fixed list (`Model.isTerminalScript`). One sentence
   per detail; buttons *Install*, *Create*, *Grant*, *Update*;
   engine-missing in the accent tone without an index, urgent with one;
   the snapper banner's engine message (and what the grant grants) move
   into the hover `full`. `SNAPPER_HINT` and `snapperHintIndex` go.
3. **Today.** "1 event today" (singular), other labels unchanged.
4. **Docs.** SPEC-PLUGIN §5 banner paragraph (and the §3 sentence that
   quotes the engine-missing text), plugin README *States* and
   *Security*.
5. **Tests.** model.test.js: every script pinned verbatim, contains its
   command verbatim, the same for any index (hostile text in the
   collector message), only literals between the quotes; banner texts,
   tones and buttons; the plural. service-states.sh/panel-view.sh/
   overlay-view.sh: the launcher's argv is the script, Copy the plain
   command, no hint, new titles and labels.

## Decisions (made here; the WP leaves them open)

- All five terminals get the pattern, not just the three the WP names:
  the goal says "every terminal the panel opens".
- Titles of the three setup banners are steps (*Install the engine*,
  *Create your logbook*, *Read snapshots (optional)*), as in the review's
  table; an engine that was there and is gone (index present) keeps a
  state title, *Seldon engine missing*, in the urgent tone.
- *Check again* stays on engine-missing, not-initialised, engine-too-old
  and snapper (0.1.4 keeps the three banners; auto re-probe is WP-119).
  The install/update result line therefore says "press Check again";
  init and the grant say "the panel updates by itself" because the index
  they write is picked up by the FileView.
- No `gum` fallback: A1 holds (Omarchy's bin calls it unguarded); without
  gum the command still runs, only the text lines are missing.
- The command runs in `(set -o pipefail; …)` so a failed `curl` in the
  install one-liner is a failure, not bash's 0.
