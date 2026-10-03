# Mit Agenten arbeiten

<!-- source: en/04-working-with-agents.md @ 04e3c6a -->

Diese Seite zeigt, wie ein KI-Agent einen Case bearbeitet, während
Seldon aufzeichnet, was er tut: Claude Code, Omarchys Standard-Agent und
jeder andere Agent. Sie behandelt die Hooks, den aktiven Case,
`seldon agent start` und wie du das Ergebnis prüfst.

## Was Seldon tut und was nicht

Seldon zeichnet auf. Hooks melden der Engine, welche Befehle ein Agent
ausführt, und die Engine schreibt sie mit dem Namen des Agenten und dem
aktiven Case ins Ledger. Danach siehst du die Änderungen des Agenten
neben dem Plan, den er bekommen hat.

Seldon bewacht nichts. Ein Hook hält nie einen Befehl an, fragt nie nach
einer Erlaubnis und ändert nie, was der Agent tut. Wenn du Grenzen
willst, setz sie in den Berechtigungen deines Agenten. Seldon zeigt dir
hinterher, ob der Agent sich an den Plan gehalten hat.

## Die Regeln, die Agenten lesen

`seldon init` schreibt `AGENTS.md` in dein Logbuch, in der Sprache des
Logbuchs. Die Datei sagt jedem Agenten, wie er dort arbeitet:

- zuerst `PROJECT.md`, `memory/lessons.md` und `STATUS.md` lesen;
- die Maschine nur in einem aktiven Case ändern, und erst, nachdem du dem
  Case zugestimmt hast;
- den Plan in den Case schreiben, bevor er etwas ändert;
- den Case in die Prüfung schieben, wenn er fertig ist, und das
  Abschließen dir überlassen;
- festhalten, was er gelernt hat, in `memory/`;
- nie das Ledger, erzeugte Dateien oder Felder der Engine ändern.

Die Datei gehört dir. Ergänze Regeln für deine Maschine; Agenten folgen
der Datei im Logbuch. Regeln für einen Bereich gehören nach
`areas/<bereich>/AGENTS.md`. Die Langfassung der Regeln ist der
[Agenten-Leitfaden](../../AGENT-GUIDE.md) des Projekts (Englisch).

## Wer einen Case abschließt

Ein Agent führt `seldon plan verify` aus, wenn seine eigenen Prüfungen
bestanden sind, und hört dort auf. Du prüfst das Ergebnis und führst
`seldon plan done` aus oder drückst *Done* im Tab Work. Soll der Agent
den Case selbst abschließen, schreib das in den *Plan* des Case, etwa
„nach der Prüfung abschließen“. Die Engine setzt diese Regel nicht
durch; das Ledger zeigt, wer welchen Case abgeschlossen hat.

## Claude Code

### Einrichten

Claude Code braucht drei Hooks in `.claude/settings.json` des Logbuchs.
Hast du im Assistenten „Claude Code hooks“ gewählt, sind sie schon da.
Sonst installierst du sie einmal:

```sh
seldon hook install claude-code
```

```text
~/Seldon/.claude/settings.json: installed the Seldon hooks.
  added    PreToolUse (Bash|Edit|Write|MultiEdit): seldon hook claude-code
  added    SessionStart: seldon hook session-start
  added    SessionEnd: seldon hook session-stop
```

Der Befehl lässt jeden anderen Hook in der Datei stehen. Ein zweiter
Aufruf ändert nichts.

Im Kontext, den `SessionStart` ausgibt, beginnt jede Zeile aus deinem
Logbuch mit `>`, unter einem Hinweis, dass diese Zeilen Daten sind und
keine Anweisungen. Text in einer Notiz oder einem Case kann sich deshalb
nicht als Teil von Seldons eigener Struktur ausgeben. Das macht den
Kontext für den Agenten klarer; es garantiert nicht, dass der Agent
ignoriert, was der Text sagt.

Seldon selbst sendet nichts über das Netz. Der Agent schon: Er sendet,
was er liest, an seinen Modellanbieter, auch diesen Kontext, die Dateien,
die er öffnet, und die Ausgabe seiner Befehle. Die Schwärzung gilt für
aufgezeichnete Befehle, nicht für den Text von Notizen, Cases und
`memory/` (siehe [Konfiguration](06-configuration.md#schwärzung)). Halte
Geheimnisse aus dem Logbuch heraus.

| Hook | Führt aus | Wirkung |
|---|---|---|
| `PreToolUse` | `seldon hook claude-code` | zeichnet jeden ändernden Befehl und jede Dateiänderung auf, mit Agent und aktivem Case |
| `SessionStart` | `seldon hook session-start` | gibt dem Agenten den Status, den aktiven Case mit seinem Plan, die letzten Journal-Zeilen und die Überschriften der Lektionen, jede Zeile aus dem Logbuch als Zitat |
| `SessionEnd` | `seldon hook session-stop` | schreibt „session ended; N events recorded“ ins Journal, erfasst, baut `STATUS.md` neu und committet |

### Einen Case bearbeiten

1. Leg den Case an und starte ihn, im Panel oder im Terminal:

   ```sh
   seldon plan new --zone yellow --risk R1 --area hyprland -- "Größere Abstände zwischen Fenstern"
   seldon plan start C-2026-003
   ```

2. Starte Claude Code im Ordner des Logbuchs:

   ```sh
   cd ~/Seldon && claude
   ```

   Bestätige die Frage, ob du dem Ordner vertraust. Die Hooks liegen in
   `.claude/settings.json` des Ordners, und Claude Code führt sie nur in
   einem vertrauten Ordner aus.

3. Gib ihm die Aufgabe, zum Beispiel: „Bearbeite Case C-2026-003. Schreib
   zuerst den Plan in den Case. Schieb ihn in die Prüfung, wenn du fertig
   bist.“

4. Der Agent liest `AGENTS.md`, füllt *Intent* und *Plan* des Case,
   erledigt die Arbeit und führt `seldon plan verify C-2026-003` aus.

5. Beende Claude Code mit `/exit`. Der Hook `SessionEnd` fügt eine
   Journal-Zeile hinzu, erfasst und committet.

6. Prüf das Ergebnis (siehe [Prüfen, was der Agent getan hat](#prüfen-was-der-agent-getan-hat))
   und schließ den Case ab:

   ```sh
   seldon plan done C-2026-003
   ```

Die Hooks laufen nur, wenn Claude Code im Ordner des Logbuchs startet. Du
kannst sie auch in deine Benutzereinstellungen installieren, dann führt
Claude Code sie in jedem Ordner aus:
`seldon hook install claude-code --settings ~/.claude/settings.json`.
Seldon zeichnet trotzdem nur Befehle von Sitzungen im Ordner des Logbuchs
oder darunter auf und gibt nur dort seinen Kontext aus; in anderen
Projekten tun die Hooks nichts, und `hook install` weist darauf hin.
Damit jede Claude-Code-Sitzung auf dieser Maschine in dein Logbuch
schreibt, in jedem Projekt, mit dem aktiven Case, setzt du in
`~/.config/seldon/config.toml`:

```toml
[hooks]
scope = "all"
```

Dann werden rote und gelbe Befehle in jeder Sitzung aufgezeichnet, grüne
nur, solange ein Case aktiv ist.

## Der aktive Case

Der Case, den du zuletzt gestartet hast, ist der aktive Case. Seine ID
steht in `.seldon/active-case` im Logbuch. Die Hooks lesen sie, also
landet jeder Befehl eines Agenten bei diesem Case, egal wo der Agent
arbeitet.

- `seldon plan start <ID>` macht einen Case aktiv.
- `seldon plan done` und `seldon plan drop` leeren die Markierung, wenn
  sie diesen Case nennt.
- Die Engine erlaubt mehrere aktive Cases, aber nur der zuletzt
  gestartete bekommt die Befehle der Agenten. Arbeite an einem Case zur
  Zeit.

Ohne aktiven Case zeichnen Hooks rote und gelbe Befehle trotzdem auf,
aber ohne Case. Die Änderungen, die diese Befehle bewirken, erscheinen
dann als Drift, mit dem Namen des Agenten. Grüne Befehle werden gar
nicht aufgezeichnet.

## Was ein Hook aufzeichnet

| Zone | Aufgezeichnet |
|---|---|
| red | Pakete installieren, entfernen und aktualisieren (auch `pacman -Syu` und `omarchy update`), `omarchy`-Befehle, die das System ändern, `systemctl enable`, `disable`, `start`, `stop`, `mask`, `unmask` |
| yellow | Schreibzugriffe in beobachtete Pfade: `cp`, `mv`, `tee`, `sed -i`, `rm`, Umleitungen sowie die Werkzeuge Edit und Write von Claude Code |
| green | jeder andere ändernde Befehl (`npm install`, `git push`, Dateien anderswo), nur solange ein Case aktiv ist |

Ein Hook zeichnet die Befehlszeile und den Pfad auf. Er zeichnet nie die
Ausgabe eines Befehls oder den Inhalt einer Datei auf. Vor dem Schreiben
entfernt die Engine Passwörter und Tokens, die sie erkennt (siehe
[Konfiguration](06-configuration.md#schwärzung)). Lesende Befehle
zeichnen nichts auf.

Befehle, die in `xargs`, `find -exec` oder einem Interpreter
(`python -c`, `node -e`) stecken, liest der Hook nicht. Die Collectors
sehen ihre Wirkung trotzdem bei der nächsten Erfassung, aber ohne den
Namen des Agenten und ohne Case. Sag deinem Agenten, Änderungen als
einfache Befehle auszuführen.

## Einen Agenten aus dem Panel starten

*Start agent* auf der Karte eines aktiven Case (oder zweimal `a` im Tab
Work) führt aus:

```sh
seldon agent start C-2026-003
```

Die Engine macht den Case zum aktiven Case und startet einen Agenten im
Ordner des Logbuchs. Der erste Prompt des Agenten nennt den Case und das
Logbuch und sagt dem Agenten, `seldon hook session-start` und
`seldon plan show C-2026-003` auszuführen; er enthält keinen Text aus
deinem Logbuch. Der Prompt ist ein Kommandozeilenargument: Solange der
Agent läuft, ist er in der Prozessliste (`ps`) sichtbar, und ein
Sitzungsjournal, das Programmstarts protokolliert, behält ihn. Sobald der
erste Befehl des Agenten aufgezeichnet ist, zeigt die Karte seinen
Namen.

Standardmäßig startet die Engine Omarchys Standard-Agenten über
`omarchy agent prompt` in einem eigenen Terminalfenster. Welcher Agent
das ist, hängt von deiner Omarchy-Einrichtung ab. Ist es Claude Code,
zeichnen die Hooks aus dem Abschnitt oben seine Befehle auf, weil er im
Ordner des Logbuchs startet.

Du kannst in `~/.config/seldon/config.toml` einen anderen Launcher
eintragen, zum Beispiel Claude Code in einem Terminalfenster (hier
Alacritty):

```toml
[agent]
launcher = ["alacritty", "-e", "claude", "{prompt}"]

[agent.launchers]
omarchy = ["omarchy", "agent", "prompt", "{prompt}"]
```

Die Engine ersetzt `{prompt}` durch den Prompt, als ein einziges
Argument. Sie startet den Launcher nie über eine Shell. Vor `{prompt}`
lehnt sie Shells, Interpreter und andere Programme ab, die ihre Argumente
bekanntermaßen als Code ausführen. Die Prüfung geht nach dem
Programmnamen: eine Heuristik, keine Sandbox. `--launcher NAME` wählt
einen aus `[agent.launchers]`.
Scheitert der Launcher, steht sein Fehler in
`~/.local/state/seldon/agent-launch.log`.
[Konfiguration](06-configuration.md#agent-launcher) nennt die Regeln.

`agent start` lehnt einen Case ab, der nicht aktiv ist. Starte ihn
vorher.

## Andere Agenten

Codex, ein Skript oder ein eigener Agent meldet sich bei Seldon mit drei
Befehlen. Zu Beginn einer Sitzung, für den Kontext:

```sh
seldon hook session-start
```

Vor jedem Befehl den Befehl als JSON auf der Standardeingabe:

```sh
echo '{"command": "systemctl --user enable syncthing", "actor": "agent:codex", "cwd": "/home/you/Seldon"}' | seldon hook generic
```

Am Ende der Sitzung:

```sh
seldon hook session-stop --actor agent:codex
```

Das JSON kann außerdem `"startedAt"` (einen Zeitstempel) und `"case"`
(eine Case-ID statt des aktiven Case) enthalten. Der Actor ist immer
`agent:` und ein Name. `seldon hook generic` gibt nichts aus und endet
immer mit 0, bricht den Agenten also nie.

## Das Omarchy-Agent-Kit

Hast du vor Seldon das Omarchy-Agent-Kit benutzt, kann der Assistent
dessen Guard und Skills in den Ordner `.claude/` des Logbuchs kopieren:
Wähl den Harness `omarchy-agent` oder führe
`seldon init --harness omarchy-agent` aus. Die Engine sucht das Kit in
`~/.local/share/seldon/harness/omarchy-agent/`. Sie überschreibt nie eine
vorhandene Datei. Um das alte Logbuch des Kits in Seldon zu holen, siehe
[Import aus omarchy-agent](09-import-from-omarchy-agent.md).

## Prüfen, was der Agent getan hat

`seldon plan show C-2026-003` gibt die Case-Datei aus. Ihre Zeile
`events:` enthält nur Ereignis-IDs. Die Ereignisse selbst liest du in
der Ledger-Ansicht des Monats; jede Zeile nennt Uhrzeit, Quelle, Case
und bei einem Agenten seinen Namen:

```sh
seldon open ledger --editor
```

Im Panel:

- Changelog, Filter `agent`: die Befehle, die Agenten ausgeführt haben,
  mit ihrem Case.
- Today: die Journal-Notizen des Agenten und die Zeile „session ended“.
- Work: die Karte des Case mit ihren Schritten.

Jeder Schritt der Engine ist ein git-Commit im Logbuch.
`git -C ~/Seldon log -p` zeigt, was sich in jeder Datei geändert hat,
auch im *Plan* und im *Log* des Case. Danach führst du `seldon drift`
aus. Alles, was der Agent geändert hat, ohne dass die Hooks es gesehen
haben, erscheint dort.

## Agenten auf dem Plan halten

- Gib dem Agenten einen Case, bevor er etwas ändert. Ohne Case werden
  seine Änderungen zu Drift.
- Lass Agenten `seldon init`, `seldon hook install`,
  `seldon import … --apply` oder `seldon agent start` nur ausführen, wenn
  du genau das verlangst. Die `AGENTS.md` des Logbuchs sagt dasselbe.
- Lass den Agenten prüfen und schließ den Case selbst ab.
- Lies das *Result* des Case und seine Spur, bevor du *Done* drückst.

---

Zurück: [Alltag](03-daily-use.md) · [Übersicht](README.md) · Weiter: [Befehlsreferenz](05-cli-reference.md)
