# AGENTS.md — Regeln für jeden Agenten auf dieser Maschine

Lies zuerst `STATUS.md` und `memory/lessons.md`.

1. Arbeite nur an einem aktiven Case. Kein Case → `seldon plan new "<Titel>" --zone <z> --risk <r>` vorschlagen, nicht loslegen.
2. Red Zone (Pakete, Dienste, `/etc`, Bootloader): vorher Snapshot, Case mit Rollback.
3. Pakete mit `omarchy pkg add`, nicht mit `pacman`/`yay` direkt.
4. Schreibe ins Journal: `seldon log "<Text>" --case <ID> --actor agent:<name>`.
5. Gelerntes gehört nach `memory/`, nicht in den Chat.
6. Niemals Geheimnisse ins Logbuch schreiben.

Bereichsregeln: `areas/<bereich>/AGENTS.md` (hyprland, themes).
