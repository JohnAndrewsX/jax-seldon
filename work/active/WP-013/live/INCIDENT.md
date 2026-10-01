# Incident during the live session (2026-10-01, about 23:15 CEST)

In the WP-021 drift sheet I wanted to switch the action to *Dismiss*.

1. A pointer click (`hyprctl dispatch movecursor` + `ydotool click`) at the
   button's position, read off a screenshot, did not select the button. It
   closed the Seldon panel.
2. The next step, `wtype -- "Live smoke: dismissed, test host package"` and
   two Returns, went to the focused client window: the operator's foot
   terminal on the test host.
3. bash ran the text as a command line: `bash: command not found: Live`,
   exit 127. The two Returns gave empty prompts. Nothing else ran, but the
   line is now in that shell's history. I left the history alone (it is the
   operator's file).

Cause: the session helper checked only the lock before typing, not that a
Seldon surface held the keyboard.

Fix (from then on):
- every key goes out only while the panel or the overlay reports
  `opened: true`;
- text goes out only while the target field reports `editing: true`;
- no more pointer clicks inside the panel.

The same guard belongs in any future live smoke script.
