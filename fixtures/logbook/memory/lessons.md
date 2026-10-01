---
type: memory
topic: lessons
updated: 2026-10-01
---
# Lessons

Jeder Agent liest diese Datei beim Sitzungsstart.

## `omarchy pkg add` statt yay direkt
Dann ist der Paketname im Befehl und das Ledger erkennt explizit installierte Pakete sauber.

## Theme-Overrides nie im Omarchy-Repo
`/usr/share/omarchy` gehört dem Paket; das nächste Update überschreibt alles.

## Hyprland reload nach bindings.conf
Ohne `hyprctl reload` wirken neue Bindings erst nach dem nächsten Login.
