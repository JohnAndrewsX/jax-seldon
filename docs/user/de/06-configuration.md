# Konfiguration

<!-- source: en/06-configuration.md @ b69de2f -->

Diese Seite beschreibt alles, was du einstellen kannst: die
`config.toml` der Engine mit Collectors, beobachteten Pfaden, Schwärzung,
Drift-Liste und Agent-Launcher, danach die Einstellungen des Plugins und
die Tastenbelegung.

## Die Konfigurationsdatei

Die Engine liest `~/.config/seldon/config.toml`. `seldon init` schreibt
sie, und du kannst sie mit jedem Editor ändern. Eine Änderung wirkt ab
dem nächsten `seldon`-Befehl; nichts muss neu gestartet werden.

Bevor du sie änderst:

- `seldon init` schreibt die Datei neu. Es behält Schlüssel, die es nicht
  kennt, verwirft aber Kommentare und ordnet die Schlüssel neu. Halte
  Notizen woanders fest.
- Eine Datei, die kein gültiges TOML ist, stoppt jeden Befehl mit Exit 1.
  Der Fehler nennt die Datei und das Problem.

Für eine andere Datei bei einem einzelnen Befehl gibst du
`--config <FILE>` an oder setzt `SELDON_CONFIG`.

## Ein vollständiges Beispiel

So sieht die Datei aus, die der Assistent schreibt, wenn du seine
Vorgaben nimmst und Claude Code anhakst. Den Abschnitt `[agent]` lässt
die Datei weg, solange er die Vorgabe enthält; hier steht er, damit du
den Schlüssel siehst. In deiner Zeile `logbook` steht dein eigener Pfad.

```toml
harnesses = ["claude-code"]
language = "de"
logbook = "/home/you/Seldon"
watchPaths = ["~/.config/hypr", "~/.config/omarchy", "~/.config/waybar", "~/.bashrc", "~/.zshrc"]

[collectors]
config = true
omarchy = true
pacman = true
plugins = true
snapper = true
theme = true

[drift]
alwaysRed = ["linux*", "systemd", "glibc", "hyprland", "omarchy", "quickshell"]

[git]
autocommit = true

[redaction]
patterns = []
skipPaths = []

[agent]
launcher = ["omarchy", "agent", "prompt", "{prompt}"]
```

## Schlüssel

| Schlüssel | Vorgabe | Bedeutung |
|---|---|---|
| `logbook` | `~/Seldon` | der Ordner des Logbuchs |
| `language` | aus deiner Locale | `en` oder `de`: die Sprache der Texte, die die Engine ins Logbuch schreibt (Journal-Zeilen, `STATUS.md`) |
| `watchPaths` | siehe [Beobachtete Pfade](#beobachtete-pfade) | Dateien und Ordner, die der Config-Collector beobachtet |
| `harnesses` | `[]` | die Agenten-Harnesses, die du im Assistenten gewählt hast; zur Information |
| `[collectors]` | alle `true` | welche Collectors eine Erfassung startet |
| `[git] autocommit` | `true` | das Logbuch nach jedem schreibenden Befehl committen |
| `[redaction] patterns` | `[]` | deine eigenen Muster für Geheimnisse, siehe [Schwärzung](#schwärzung) |
| `[redaction] skipPaths` | `[]` | Dateien, die die Engine nie öffnet oder nennt |
| `[drift] alwaysRed` | sechs Namen | Pakete, deren Upgrade immer eine Krise ist, siehe [Drift](#drift) |
| `[agent] launcher` | `omarchy agent prompt` | was `seldon agent start` startet, siehe [Agent-Launcher](#agent-launcher) |
| `[agent.launchers]` | keine | weitere Launcher mit Namen |

Den Pfad des Logbuchs nimmt die Engine in dieser Reihenfolge:
`--logbook`, `SELDON_LOGBOOK`, `logbook` aus der Konfiguration,
`~/Seldon`.

## Collectors

| Collector | Liest | Zeichnet auf |
|---|---|---|
| `snapper` | `snapper --jsonout list` | angelegte und gelöschte Snapshots |
| `pacman` | `/var/log/pacman.log` | installierte, entfernte, aktualisierte und zurückgestufte Pakete, nach Transaktion gruppiert |
| `omarchy` | `omarchy-version` | Wechsel der Omarchy-Version |
| `plugins` | `omarchy plugin list --json` | Shell-Plugins: hinzugefügt, entfernt, aktiviert, deaktiviert, aktualisiert |
| `theme` | `~/.local/state/omarchy/current/theme.name` | Theme-Wechsel |
| `config` | die Dateien unter `watchPaths` | Dateien: hinzugefügt, geändert, entfernt |

Jeder Collector liest nur. Mit `false` schaltest du einen aus;
`seldon capture --source <name>` startet ihn trotzdem bei Bedarf.

Ein Collector, der seine Quelle nicht lesen kann, ist `degraded`: Die
Erfassung läuft weiter, und `seldon doctor` nennt die Abhilfe. Zwei Fälle
sind normal:

- `snapper` braucht deinen Benutzer in `ALLOW_USERS` der
  Snapper-Konfiguration. Omarchy setzt das nicht. Die Abhilfe ist
  `sudo snapper -c root set-config ALLOW_USERS=$USER SYNC_ACL=yes`, die
  du selbst ausführst, oder gar nicht. `ALLOW_USERS` kennt keine
  Nur-Lese-Stufe: Dein Benutzer kann danach auch Snapshots von root ohne
  Passwort anlegen, ändern und löschen.
- `plugins` fragt die laufende Omarchy-Shell. Erfasst du von einem TTY
  ohne Desktop-Sitzung, ist er für diese Erfassung `degraded`.

## Beobachtete Pfade

`watchPaths` listet Dateien und Ordner. Der Config-Collector bildet bei
jeder Erfassung den Hash jeder Datei darunter und zeichnet auf, was
hinzugekommen, geändert oder entfernt ist. Er zeichnet den Pfad und zwei
kurze Hashes auf, nie den Inhalt.

Vorgaben: `~/.config/hypr`, `~/.config/omarchy`, `~/.config/waybar`,
`~/.bashrc`, `~/.zshrc`. Fehlende Pfade überspringt der Collector. Ergänze
eigene, zum Beispiel:

```toml
watchPaths = ["~/.config/hypr", "~/.config/omarchy", "~/.config/waybar", "~/.bashrc", "~/.zshrc", "~/.config/nvim", "~/.config/systemd/user"]
```

Immer ausgenommen:

- `~/.config/omarchy/plugins/` (das deckt der Plugins-Collector ab);
- `.git`-Ordner und Ordner, die über einen Symlink erreicht werden;
- Binärdateien und Dateien über 1 MiB (als übersprungen gelistet, ohne
  Hash);
- alles in `[redaction] skipPaths`.

Unit-Dateien unter `~/.config/systemd/` gehören zur roten Zone. Alles
andere hier ist gelb.

Fügst du einen Pfad hinzu, zeichnet die nächste Erfassung jede Datei
darin als hinzugefügt auf, und jede öffnet als Drift. Verwirf sie, zum
Beispiel mit
`seldon drift dismiss <EVENT> -- "beobachte jetzt ~/.config/nvim"`.
Nimm einen Ordner mit vielen Dateien nur auf, wenn du jede einzelne
verfolgen willst.

## Schwärzung

Bevor die Engine ein Ereignis schreibt, entfernt sie Geheimnisse aus der
Befehlszeile und dem Detailtext. Ein geschwärzter Wert lautet
`‹redacted›`. Die eingebauten Regeln erfassen:

- `--password`, `token=`, `Authorization:` und ihre Werte;
- AWS-Zugangsschlüssel (`AKIA…`), GitHub-Tokens (`ghp_…`), API-Schlüssel
  (`sk-…`);
- das Passwort nach `-p` bei `mysql`, `psql` und `smbclient`;
- Benutzer und Passwort in einer URL (`https://user:secret@host`).

Die Regeln schwärzen lieber zu viel als zu wenig. Eigene Regeln trägst
du als reguläre Ausdrücke ein; jeder ersetzt seinen ganzen Treffer:

```toml
[redaction]
patterns = ["MYAPP_KEY=\\S+", "xoxb-[0-9A-Za-z-]+"]
```

Ein ungültiges Muster ist ein Fehler (Exit 1): Seldon schreibt lieber gar
nicht, als etwas preiszugeben.

`skipPaths` nennt Dateien, die der Config-Collector und die Hooks nie
öffnen, hashen oder nennen:

```toml
[redaction]
skipPaths = ["~/.config/hypr/secrets.lua", "*.key", "**/tokens/**"]
```

| Muster | Trifft |
|---|---|
| mit `/`, etwa `~/.config/app/secret.conf` | genau diesen Pfad, oder diesen Ordner mit allem darin |
| ein relativer Pfad, etwa `app/secret.conf` | diesen Pfad unter jedem Ordner |
| ein Name ohne `/`, etwa `*.key` | jede Datei und jeden Ordner mit diesem Namen |

`*` und `?` bleiben innerhalb eines Pfadteils; `**` geht über Teile
hinweg.

Die Schwärzung hilft, aber sie fängt nur, was sie erkennt. Schreib nie
ein Geheimnis in eine Notiz, einen Case oder eine Befehlszeile, wenn du
es vermeiden kannst.

## Drift

`[drift] alwaysRed` listet Paketnamen, deren Routine-Upgrade trotzdem
eine Krise ist. Ein Routine-Upgrade ist eine Transaktion, die bei einem
vollständigen System-Upgrade nur Pakete aktualisiert (`omarchy update`
macht eins). Es öffnet gelbe Drift und löst darum keinen Alarm aus. Ein
Paket auf dieser Liste macht die ganze Transaktion rot.

```toml
[drift]
alwaysRed = ["linux*", "systemd", "glibc", "hyprland", "omarchy", "quickshell", "nvidia*"]
```

`*` passt auf jedes Ende. Behalte die Vorgaben: Diese Pakete können
Boot, Login oder die Shell kaputtmachen.

## Agent-Launcher

`seldon agent start` (und *Start agent* im Panel) startet den Launcher.
Die Vorgabe startet Omarchys Standard-Agenten in einem eigenen
Terminalfenster:

```toml
[agent]
launcher = ["omarchy", "agent", "prompt", "{prompt}"]
```

Ein Launcher ist eine Liste: das Programm, dann seine Argumente. Regeln:

- Genau ein Element ist `{prompt}`. Die Engine ersetzt es durch den
  Prompt, als ein einziges Argument.
- Das Programm ist ein Name in deinem `PATH` (ohne `/`) oder ein
  absoluter Pfad.
- Vor `{prompt}` lehnt die Engine Programme ab, die ihre Argumente
  bekanntermaßen als Code ausführen: Shells (`bash`, `sh`, `zsh`, `fish`
  und andere), Interpreter (`python`, `perl`, `node` und andere),
  Programme, die einen String an eine Shell geben (`script`, `watch`,
  `flock`, `su`, `ssh`, `tmux`, `screen`, `xargs` und andere), `eval`,
  `hyprctl`, die Omarchy-Starter, die Shell-Strings bauen, `env -S` und
  `sudo -s`/`-i`. Ein Versionsanhang ändert den Namen nicht
  (`python3.12` ist `python`). Die Prüfung geht nach dem Programmnamen:
  eine Heuristik, keine Sandbox.
- Der Prompt nennt den Case und das Logbuch und enthält keinen Text aus
  dem Logbuch. Er ist ein Kommandozeilenargument und in der Prozessliste
  (`ps`) sichtbar, solange der Agent läuft.

Weitere Launcher gehören unter `[agent.launchers]`; du wählst einen mit
`seldon agent start <ID> --launcher <NAME>`. Der Name `omarchy` erreicht
immer die eingebaute Vorgabe.

```toml
[agent]
launcher = ["alacritty", "-e", "claude", "{prompt}"]

[agent.launchers]
codex = ["alacritty", "-e", "codex", "{prompt}"]
```

Der Launcher startet losgelöst im Ordner des Logbuchs. Seine
Fehlerausgabe landet in `~/.local/state/seldon/agent-launch.log`.

## Git

Mit `autocommit = true` committet jeder schreibende Befehl das Logbuch,
mit einer Nachricht wie `seldon: C-2026-004 active`. Der Commit nimmt den
ganzen Ordner, also gehen Änderungen, die du seit dem letzten Befehl im
Editor gemacht hast, mit. Die Geschichte ist dein Backup und dein
Rückgängig.

Mit `false` oder mit `--no-commit` an einem Befehl schreibt die Engine
die Dateien und überlässt dir das Committen. Seldon pusht nie.

## Einstellungen des Plugins

Du änderst sie in Omarchys Einstellungen (Setup, Plugins, Seldon) oder
mit `omarchy bar set`:

| Schlüssel | Vorgabe | Bedeutung |
|---|---|---|
| `captureIntervalMin` | `15` | Minuten zwischen zwei Erfassungen, solange die Shell läuft (5 bis 120) |
| `wipLimit` | `3` | aktive Cases, mit denen der Tab Work vergleicht (1 bis 20); es warnt, blockiert nie |

```sh
omarchy bar set jax.seldon captureIntervalMin 30 --json
```

Die Pill verschieben: `omarchy bar move jax.seldon --section right`.

## Eine Taste für den Prime Radiant

Das Plugin richtet keine Tastenbelegung ein. Füg diese Zeile zu deinen
Hyprland-Bindings hinzu, um den Prime Radiant mit Super+Shift+S zu
öffnen:

```text
o.bind("SUPER + SHIFT + S", "Seldon", "omarchy-shell shell toggle jax.seldon")
```

Das Panel hat ein eigenes Ziel. Um es über eine Taste oder ein Skript
umzuschalten: `omarchy-shell jax.seldon.panel toggle`.

---

Zurück: [Befehlsreferenz](05-cli-reference.md) · [Übersicht](README.md) · Weiter: [Das Logbuch](07-the-logbook.md)
