---
language: {{language}}
machineId: {{machineId}}
---
# {{machineId}}

## Purpose
Wofür diese Maschine da ist, in ein paar Zeilen. Jeder Agent liest diese Datei.

## Must not happen here
- Keine Änderung in der Red Zone (Pakete, Dienste, `/etc`, Bootloader) ohne Case.
- Keine Änderungen im Omarchy-Paket unter `/usr/share/omarchy`; Anpassungen nur unter `~/.config`.

## Who works here
- Ich (`human`)
- Agenten, mit dem Namen, den sie in `--actor` angeben, z. B. `agent:claude-code`
