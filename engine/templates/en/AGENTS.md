# AGENTS.md — rules for every agent on this machine

Read `STATUS.md` and `memory/lessons.md` first.

1. Work on one active case at a time. No case? Propose one with `seldon plan new "<title>" --zone <z> --risk <r>`; do not start without it.
2. Red zone (packages, services, `/etc`, boot loader): snapshot first, and a case with a rollback plan.
3. Install packages with `omarchy pkg add`, not with `pacman`/`yay` directly.
4. Write to the journal: `seldon log "<text>" --case <ID> --actor agent:<name>`.
5. What you learn goes into `memory/`, not into the chat.
6. Never write secrets into the logbook.

Area rules: `areas/<area>/AGENTS.md`.
