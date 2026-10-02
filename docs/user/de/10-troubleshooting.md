# Fehlersuche

<!-- source: en/10-troubleshooting.md @ 2382ac3 -->

Diese Seite hilft, wenn etwas falsch aussieht: Sie beginnt mit
`seldon doctor`, geht dann durch die Banner des Panels, die Exit-Codes
der Engine und die häufigsten Probleme.

## Mit doctor anfangen

```sh
seldon doctor
```

Es prüft sechs Dinge und nennt für jedes, das nicht `ok` ist, eine
Abhilfe:

| Prüfung | `ok` heißt | Wenn sie nicht ok ist |
|---|---|---|
| `engine` | die Engine läuft; ihre Version und ihr Vertrag | (wenn du `doctor` ausführen kannst, ist das ok) |
| `config` | `~/.config/seldon/config.toml` wurde gelesen | `degraded`: noch keine Konfiguration, Vorgaben in Gebrauch; Abhilfe `seldon init`. `error`: die Datei ist kein gültiges TOML |
| `logbook` | das Logbuch existiert; Maschine, Sprache, Zahlen | `error`: an diesem Pfad nicht angelegt; die Abhilfe nennt den Befehl `seldon init` |
| `omarchy` | `omarchy-version` hat geantwortet | der Omarchy-Collector kann die Version nicht lesen |
| `snapper` | Snapshots lassen sich auflisten | `degraded`: dein Benutzer darf keine Snapshots auflisten; siehe [Snapshots werden nicht aufgezeichnet](#snapshots-werden-nicht-aufgezeichnet) |
| `git` | git ist da; das Logbuch ist ein Repository | git fehlt, oder das Logbuch ist kein Repository; dann ist Autocommit aus |

`doctor --json` gibt dasselbe als JSON aus. Das Plugin liest es, um sein
Banner zu wählen.

## Banner im Panel

Wenn etwas nicht stimmt, zeigt das Panel oben ein Banner mit einem
Knopf, der es behebt.

| Banner | Ursache | Abhilfe |
|---|---|---|
| Seldon engine not installed | das Plugin kann `seldon` nicht starten | *Install in terminal* startet den GitHub-Installer in einem Terminal, das du siehst; oder du installierst selbst ([Erste Schritte](01-getting-started.md#schritt-1-die-engine-installieren)), dann *Check again* |
| Logbook not initialised | es gibt noch kein Logbuch | *Run in terminal* startet `seldon init` |
| No index yet / Index unreadable | `~/.local/state/seldon/index.json` fehlt oder ist kaputt | *Build index* startet `seldon status` |
| Index is stale | der Index ist älter als zwei Stunden | *Capture now* |
| Index format mismatch | Plugin und Engine sprechen verschiedene Versionen des Index | das ältere aktualisieren. Plugin: `omarchy plugin update jax.seldon`. Engine: *Update in terminal* führt den Installer noch einmal aus (bis es das AUR-Paket gibt; siehe [Aktualisieren und entfernen](11-update-and-uninstall.md)) |
| Snapshots not readable | Snapper weist deinen Benutzer ab | *Run in terminal* startet die einmalige Snapper-Abhilfe; dort tippst du dein Passwort |

Das Plugin sucht die Engine beim Start der Shell und wenn du *Check
again* drückst. Hast du die Engine installiert, drück *Check again* oder
starte die Shell mit `omarchy-restart-shell` neu.

Ein Index veraltet, wenn zwei Stunden lang keine Erfassung lief, etwa
nachdem die Maschine geschlafen hat. Die nächste planmäßige Erfassung
behebt das von selbst.

## Exit-Codes

Jeder `seldon`-Befehl endet mit einem dieser Codes:

| Code | Bedeutung | Was zu tun ist |
|---|---|---|
| 0 | ok | |
| 1 | Benutzerfehler: ein falsches Argument, ein unbekannter Case oder ein unbekanntes Ereignis, ein Schritt, den der Case nicht gehen kann | die Meldung lesen; `seldon <command> --help` |
| 2 | Fehler der Engine | den Befehl noch einmal mit `--json` ausführen und die Meldung für einen Fehlerbericht aufheben |
| 3 | das Logbuch ist nicht angelegt | `seldon init`, oder `--logbook` und `SELDON_LOGBOOK` prüfen |
| 4 | ein anderes `seldon` hält die Sperre | kurz warten und noch einmal ausführen |

## Häufige Probleme

### „another seldon process holds the lock“

Es schreibt immer nur ein `seldon` zur Zeit. Vielleicht läuft gerade eine
Erfassung des Plugins oder der Hook eines Agenten. Warte eine Sekunde
und versuche es noch einmal. Löst sich die Sperre nie, such mit
`pgrep -a seldon` nach einem hängenden Prozess. Die Sperre ist
`~/.local/state/seldon/lock`; sie wird frei, wenn der Prozess endet, du
löschst sie also nie von Hand.

### Snapshots werden nicht aufgezeichnet

`seldon doctor` meldet `snapper degraded: No permissions`. Omarchy
erlaubt deinem Benutzer nicht, Snapshots aufzulisten. Seldon
funktioniert ohne sie; die Zeitleiste hat dann keine Snapshot-Marken. Um
es zu erlauben, führst du einmal aus:

```sh
sudo snapper -c root set-config ALLOW_USERS=$USER SYNC_ACL=yes
```

Das ändert die Snapper-Konfiguration von root. Seldon führt es nie für
dich aus.

### Eine Änderung erscheint nicht

- Führ `seldon capture` aus und sieh dir die Zeilen pro Collector an. Ein
  Collector mit 0 ist vielleicht in `config.toml` ausgeschaltet oder
  `degraded`.
- Der Collector sieht eine Konfigurationsdatei nur, wenn sie unter einem
  beobachteten Pfad liegt, nicht größer als 1 MiB ist, keine Binärdatei
  ist und nicht in `skipPaths` steht
  ([Konfiguration](06-configuration.md#beobachtete-pfade)).
- Eine Paket-Transaktion, die noch läuft, zeichnet der Collector auf,
  sobald sie endet.
- Änderungen durch ein Programm, das die Hooks nicht sehen (eine
  grafische Oberfläche, ein Skript), erscheinen bei der nächsten
  Erfassung, als Drift ohne Namen eines Agenten.

### Das Panel zeigt alte Daten

Das Plugin liest den Index, und die Engine schreibt ihn nach jedem
Befehl neu. Hast du Dateien in einem Editor geändert, holt der Index das
mit dem nächsten Befehl oder der nächsten Erfassung nach. Drück `c` im
Panel oder führ `seldon status` aus. Damit Änderungen sofort erscheinen,
nimm den [Watcher](11-update-and-uninstall.md#der-optionale-watcher).

Sagt eine Aktion im Panel, der Index sei hinter deinem Logbuch, hast du
das Logbuch in der Zwischenzeit woanders geändert. Erfasse, dann
versuche es noch einmal.

### Viel Drift nach einer Nacherfassung

Eine Nacherfassung zeichnet ältere Änderungen auf, und keine davon hat
einen Case. Markiere sie als Baseline: Verwirf sie mit einem Grund. Eine
Gruppe von Paketen ist ein Eintrag, also sind das meist nur eine Handvoll
Befehle:

```sh
seldon drift
seldon drift dismiss <EVENT> -- "pre-Seldon baseline"
```

### Drift, die ich nicht kenne

```sh
seldon drift show <EVENT> --json
```

zeigt das Ereignis mit jedem Mitglied seiner Gruppe, den Actor und bei
Paketen den Befehl, der lief. Sieh im Journal und in den Notizen der
Agenten von diesem Tag nach. Weißt du es dann immer noch nicht, lass sie
offen und schreib eine Notiz ins Journal. Eine falsche Erklärung führt
dich später in die Irre.

### Die Befehle eines Agenten werden nicht aufgezeichnet

- Claude Code: Hast du es im Ordner des Logbuchs gestartet und dem
  Ordner vertraut? Gibt es `.claude/settings.json`?
  `seldon hook install claude-code` ergänzt, was fehlt.
- Ist ein Case aktiv? Grüne Befehle werden nur dann aufgezeichnet.
- Befehle in `xargs`, `find -exec` oder `python -c` liest der Hook nicht.
- Andere Agenten rufen `seldon hook generic` selbst auf
  ([Mit Agenten arbeiten](04-working-with-agents.md#andere-agenten)).

### `seldon agent start` verweigert, oder nichts öffnet sich

- Der Case muss aktiv sein: erst `seldon plan start <ID>`.
- Einen Launcher, der eine Shell ist oder kein `{prompt}` hat, lehnt die
  Engine ab; die Meldung sagt, warum.
- Erscheint das Fenster des Agenten nicht, lies
  `~/.local/state/seldon/agent-launch.log`.

### Der Theme-Collector ist degraded

Er liest `~/.local/state/omarchy/current/theme.name`. Omarchy schreibt
diese Datei, wenn es ein Theme setzt. Setz einmal irgendein Theme, und
die nächste Erfassung ist ok.

### Der Plugins-Collector ist degraded

Er fragt die laufende Omarchy-Shell. Eine Erfassung von einem TTY, über
SSH oder während die Shell neu startet, erreicht sie nicht. Die nächste
Erfassung in deiner Desktop-Sitzung holt das nach.

## Logs

| Was | Wo |
|---|---|
| Warnungen des Plugins | `journalctl --user -t omarchy-shell` |
| Fehler des Agent-Launchers | `~/.local/state/seldon/agent-launch.log` |
| der Watcher, wenn du ihn nutzt | `journalctl --user -u seldon-watch` |
| was das Plugin sieht | `omarchy-shell jax.seldon.service status` und `omarchy-shell jax.seldon.panel view` (JSON) |

## Einen Fehler melden

Eröffne ein Issue unter <https://github.com/JohnAndrewsX/jax-seldon/issues>
mit dem Befehl, der Ausgabe von `seldon --version` und `seldon doctor` und
der Meldung desselben Befehls mit `--json`. Ersetze vor dem Posten deinen
Benutzernamen, den Namen deiner Maschine und private Pfade; die Ausgabe
kann sie enthalten. Bitte schreib das Issue auf Englisch.

---

Zurück: [Import aus omarchy-agent](09-import-from-omarchy-agent.md) · [Übersicht](README.md) · Weiter: [Aktualisieren und entfernen](11-update-and-uninstall.md)
