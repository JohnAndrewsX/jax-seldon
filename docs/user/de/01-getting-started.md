# Erste Schritte

<!-- source: en/01-getting-started.md @ a5ae8924 -->

Diese Seite führt dich in etwa fünfzehn Minuten zu einem Rechner, den
Seldon aufzeichnet. Du fügst das Seldon-Plugin zur Bar hinzu und klickst
dich durch eine Einrichtungskarte: Engine installieren, Logbuch anlegen
und, wenn du willst, Snapshots. Dabei wird nichts gefragt. Danach
zeichnest du eine ungeplante Änderung auf und planst eine Änderung als
Case. Jeder Schritt zeigt, was du klickst oder tippst und was du sehen
solltest. Richtest du lieber im Terminal ein, macht
[Der Weg über das Terminal](#der-weg-über-das-terminal) am Ende der Seite
dasselbe mit Befehlen.

## Was du brauchst

- Omarchy 4 mit der Omarchy-Shell.
- `git` (bringt Omarchy mit).
- Ein Terminal für die Schritte 3 bis 6. Die Einrichtungskarte öffnet
  ihre eigenen Terminals. Nichts auf dieser Seite braucht `sudo`, außer
  der optionalen Snapshot-Freigabe, die einmal nach deinem Passwort
  fragt.

Seldon liest dein System nur. Es installiert nie ein Paket und hält nie
einen Befehl an. Es schreibt drei Dinge: dein Logbuch (standardmäßig
`~/Seldon`), seine Konfigurationsdatei (`~/.config/seldon/config.toml`)
und seinen Zustand (`~/.local/state/seldon/`). Außerhalb dieser Ordner
schreibt es nur, wo du zustimmst: Omarchys Theme-Hook und die
Hook-Einstellungen von Claude Code. Die Einrichtungskarte richtet keins
von beiden ein.

## Schritt 1: Das Plugin hinzufügen

```sh
omarchy plugin add https://github.com/JohnAndrewsX/jax-seldon-plugin.git --enable
```

Rechts in der Bar erscheint eine kleine Pill mit dem Seldon-Zeichen. Klick
sie an. Der Seldon-Desk öffnet sich bei Today, und seine Übersicht zeigt
die Einrichtungskarte:

```text
Set up Seldon · 3 of 3 steps to go
  1  Install the engine          Install · Copy
  2  Create the logbook
  3  Read snapshots (optional)
```

Die Kopfzeile zeigt dieselbe Zeile. Ein Klick darauf bringt dich von
überall im Desk zur Karte zurück.

Keine Pill? Prüf, ob `omarchy plugin list` das Plugin `jax.seldon` als
aktiviert zeigt (sonst `omarchy plugin enable jax.seldon`), und starte
dann die Shell mit `omarchy-restart-shell` neu.

Seldon zeichnet die Installation des Plugins und das Bar-Layout, das
Omarchy in `~/.config/omarchy/shell.json` speichert, als Routine auf:
Beides ist keine Drift.

## Schritt 2: Seldon auf der Karte einrichten

Jeder Schritt hat einen Knopf. Er öffnet ein schwebendes Terminal, das
sagt, was es tut, den Befehl zeigt und ausführt und dann sagt, was sich
geändert hat. Schließ das Terminal, wenn es „Done!“ zeigt. Die Karte geht
von selbst weiter: Nachdem ihr Terminal aufgegangen ist, schaut sie alle
paar Sekunden nach.

1. **Install the engine.** *Install* lädt die Engine, ein einziges
   Programm namens `seldon`, aus dem GitHub-Release des Projekts nach
   `~/.local/bin`. Das Installationsskript prüft sie gegen die Prüfsummen
   des Release. Es läuft als dein Benutzer und fragt nach keinem
   Passwort. Das Terminal endet mit „The engine is installed. The Seldon
   panel finds it by itself.“ Die Karte sagt nun „2 of 3 steps to go“.
   Das AUR-Paket kommt bald; bis dahin ist das der Weg.

2. **Create the logbook.** Bevor du klickst, zeigt die Liste links, was
   dieser Rechner schon ohne Seldon weiß: die Paket-Transaktionen und die
   unter `~/.config` bearbeiteten Dateien der letzten sieben Tage, ohne
   Wer und ohne Warum. *Create logbook* führt `seldon init --defaults`
   aus. Es fragt nichts: Das Logbuch kommt nach `~/Seldon`, in der
   Sprache deiner Locale, mit git, und die erste Erfassung schaut 90 Tage
   zurück. Diese Vorgeschichte wird als „before Seldon“ aufgezeichnet:
   Sie bleibt in der Zeitleiste und verlangt nichts von dir. Ist Obsidian
   installiert, bekommt das Logbuch die Einstellungen von Obsidian, damit
   du es als Vault öffnen kannst. Liegen in dem Ordner schon andere
   Dateien, sagt der Schritt das und bietet stattdessen *Choose a folder*
   an: `seldon init` fragt, wohin das Logbuch soll. Das Terminal endet so
   (deine Zahlen weichen ab):

   ```text
   Logbook     ~/Seldon (Deutsch, git repository)
   Config      ~/.config/seldon/config.toml; list noisy or secret files in its [redaction] skipPaths
   Recording   snapshots, packages, Omarchy updates, plugins, themes, config files
   Agents      none; add one with seldon hook install claude-code (or skills)
   History     Looked back 90 days: 1692 changes recorded as history before Seldon
   Snapshots   not readable yet; optional, Seldon works without them

   Seldon is recording. Nothing else to do.

   Optional, snapshots in the timeline: read access to the snapshot list
   and info files, nothing else. Asks for your password once:
     sudo setfacl -m u:$USER:rx /.snapshots
   ```

3. **Read snapshots (optional).** Omarchy legt seine Snapshots in
   `/.snapshots` ab, das dein Benutzer anfangs nicht lesen darf. *Grant*
   führt `sudo setfacl -m u:$USER:rx /.snapshots` aus und fragt einmal
   nach deinem Passwort. Es erlaubt deinem Benutzer, die Snapshot-Liste
   und die Info-Dateien zu lesen, damit Seldon die Snapshots in die
   Zeitleiste bringt; Snapshots anlegen, ändern oder löschen kann er
   damit nicht. Dateien in einem Snapshot behalten ihre eigenen Rechte.
   Seldon führt das nie für dich aus. *Not now* legt den Schritt für
   immer weg: Die Karte fragt nicht wieder, und Settings › Capture (Taste
   `,`) bietet ihn wieder an, wenn du es dir anders überlegst. Auf einem
   Rechner ohne snapper hat die Karte zwei Schritte.

Ist nichts mehr offen, macht die Karte Platz für „Seldon is recording.
Nothing to do.“ Routine bleibt still im Changelog; nur eine Änderung
ohne Case, die Boot, Anmeldung oder die Shell treffen kann, färbt die
Bar.

## Schritt 3: Eine ungeplante Änderung aufzeichnen

Ändere etwas, ohne Seldon vorher Bescheid zu geben. Ein Alias in deiner
`~/.bashrc` eignet sich gut: Omarchys `~/.bashrc` hat einen Platz für
eigene Aliase, und du machst ihn in einer Sekunde rückgängig. (Ein
Theme-Wechsel taugt dafür nicht: Seldon zählt ihn als Routine, und
Routine ist Geschichte, keine Drift.)

1. Füg den Alias hinzu:

   ```sh
   echo "alias gs='git status'" >> ~/.bashrc
   ```

2. Lass Seldon nach Änderungen suchen:

   ```sh
   seldon capture
   ```

   ```text
   Captured 1 new event(s).
     snapper     0  degraded  snapper: No permissions. This user can neither list the snapshots nor read the snapshot directory; `seldon doctor` prints the read grant.
              fix: sudo setfacl -m u:$USER:rx /.snapshots
     pacman      0
     omarchy     0
     plugins     0
     theme       0
     config      1
   ```

   Das Plugin erfasst außerdem von selbst alle 15 Minuten. Hier startest
   du die Erfassung von Hand, damit du nicht warten musst. Bis du die
   Leseberechtigung aus Schritt 2 erteilst, sagt die Zeile `snapper`, warum sie
   keine Snapshots gelesen hat, und wiederholt die Abhilfe; danach sagt
   sie, wie viele sie gelesen hat.

3. Frag Seldon, was unerklärt ist:

   ```sh
   seldon drift
   ```

   ```text
   yellow     2026-10-09 19:23  config/config-change  ~/.bashrc  01M4GV208K21QZFBQH5FRYWS5B
   1 open drift item(s), 0 crisis
   ```

   Die Änderung ist **Drift**: eine Änderung, die kein Case abdeckt.
   `yellow` ist ihre Zone: eine Konfigurationsänderung, kein Paket
   ([Konzepte](02-concepts.md#zonen) erklärt Zonen). Die letzte Spalte ist
   die Ereignis-ID (deine sieht anders aus).

   Sie ist keine Krise, also bleibt die Pill in der Bar, wie sie war: Die
   Zahl hinter dem Zeichen zählt standardmäßig nur Krisen. Der Tab Today
   des Panels (Taste `1`) zeigt die Änderung unter *without a case*, der
   Tooltip der Pill ebenso. Sollen alle Änderungen ohne Case auch in der
   Bar zählen, setz die Plugin-Einstellung `driftInBar` auf `all`
   ([Konfiguration](06-configuration.md#einstellungen-des-plugins)).

4. Erkläre sie. Die Änderung ist schon passiert, also hält Seldon deinen
   Grund als nachträglichen Case fest: ein neuer Case, gleich als
   abgeschlossen angelegt, mit deinem Text als Titel:

   ```sh
   seldon drift explain <EVENT> -- "Ein kurzes git status"
   ```

   Ersetze `<EVENT>` durch die ID aus deiner Ausgabe von `seldon drift`.

   ```text
   Explained 1 event(s) with the new completed case C-2026-001
   Case: work/completed/C-2026-001-ein-kurzes-git-status.md
   ```

Deine Case-IDs tragen das aktuelle Jahr. `seldon drift` meldet jetzt
`No open drift.`

## Schritt 4: Eine Änderung als Case planen

Jetzt der geplante Weg. Leg einen Case an, der den Alias wieder
entfernt, und starte ihn:

```sh
seldon plan new --area shell -- "Den gs-Alias wieder entfernen"
seldon plan start C-2026-002
```

```text
Created C-2026-002 "Den gs-Alias wieder entfernen" in work/queued/C-2026-002-den-gs-alias-wieder-entfernen.md
C-2026-002 queued → active (now work/active/C-2026-002-den-gs-alias-wieder-entfernen.md)
```

`--area shell` legt den Case im Bereich `shell` ab, einem von sechs
Themen, die ein neues Logbuch hat (`areas/shell/`). Nimm die ID, die
`plan new` ausgegeben hat. Der Case ist jetzt aktiv. Schreib eine Notiz
ins heutige Journal:

```sh
seldon log --case C-2026-002 -- "Entferne den gs-Alias wieder"
```

```text
Noted in journal/2026/2026-10-09.md (C-2026-002): Entferne den gs-Alias wieder
```

Lösch die Zeile `alias gs='git status'` aus `~/.bashrc` (im Editor oder mit
`sed -i "/^alias gs=/d" ~/.bashrc`). Dann erfasse noch einmal und sieh
dir die Drift an:

```sh
seldon capture
seldon drift
```

```text
yellow     2026-10-09 19:23  config/config-change  ~/.bashrc  01M4GV2AS6F90AM6RFPAMVNJDS
1 open drift item(s), 0 crisis
```

Die Änderung erscheint wieder als Drift. Seldon sieht, dass sich die
Datei geändert hat, aber ein Collector kann nicht wissen, dass du es für
diesen Case getan hast. Nur Agenten, die über Hooks arbeiten, landen von
selbst beim aktiven Case. Der Case lohnt sich trotzdem: Er enthält deine
Notiz und, sobald verknüpft, die Änderung selbst als seine Spur. Deine
eigene Änderung verknüpfst du mit einem Befehl:

```sh
seldon drift link <EVENT> C-2026-002
```

```text
Linked 1 event(s) to C-2026-002
```

Nenn die Datei beim nächsten Mal im *Plan* des Case, bevor du sie
änderst, zum Beispiel `- Affected paths: ~/.bashrc` in der Case-Datei
(`seldon open case` gibt ihren Pfad aus). Solange der Case aktiv ist,
verknüpft die Erfassung die Änderung dann von selbst und meldet
`note: 1 event(s) linked to the one case that planned them while it was open`.

Schließ den Case ab. `verify` sagt, dass die Arbeit getan ist; `done`
sagt, dass du sie geprüft hast:

```sh
seldon plan verify C-2026-002
seldon plan done C-2026-002
```

```text
C-2026-002 active → verification
C-2026-002 verification → completed (now work/completed/C-2026-002-den-gs-alias-wieder-entfernen.md)
Journal: journal/2026/2026-10-09.md
```

## Schritt 5: Das Ergebnis ansehen

```sh
seldon status
```

```text
Status of <machine> (2026-10-09)
  cases   0 active · 0 in verification · 0 queued
  drift   0 open · 0 crisis
  events  11 today · 11 in 7 days
  snapper degraded: snapper: No permissions. This user can neither list the snapshots nor read the snapshot directory; `seldon doctor` prints the read grant.
Wrote ledger/2026-10.md, STATUS.md
Index: ~/.local/state/seldon/index.json
```

Die Zeile `snapper degraded` steht dort, bis du die Leseberechtigung aus
Schritt 2 erteilst. Deine Zahlen weichen ab.

Öffne das Panel des Plugins und drück `2` für den Changelog. Du siehst
beide Änderungen an `~/.bashrc`, die Installation des Plugins, die Notiz
und die Schritte der Cases. Drück `3` für Work: Beide Cases stehen in der
Spalte Completed.

Das Logbuch ist reines Markdown, und jeder Schritt war ein git-Commit:

```sh
git -C ~/Seldon log --oneline
```

```text
bfcee65 seldon: status
8e9f3d2 seldon: C-2026-002 completed — Den gs-Alias wieder entfernen
09dbaaf seldon: C-2026-002 verification
ba7531c seldon: drift linked: 1 event(s), C-2026-002
7c6c2a2 seldon: note C-2026-002
49b142e seldon: C-2026-002 active
27a1b9a seldon: C-2026-002 created
5c50e46 seldon: drift explained: 1 event(s), C-2026-001
2e00127 seldon: dossier
a6904e1 seldon: init logbook
```

## Schritt 6: Die Wiederaufbau-Anleitung schreiben

```sh
seldon rebuild
```

```text
Wrote outputs/REBUILD.md: 0 package(s), 2 deviation(s), 1 plugin(s), 0 unit(s), 0 open
```

Deine Zahlen weichen ab. Öffne `~/Seldon/outputs/REBUILD.md`. Dort steht,
was eine frische Omarchy-Installation braucht, um wieder diese Maschine
zu werden: deine eigenen Pakete, geänderte Dateien, Plugins und das
Theme. Die Anleitung wächst mit jedem Case, den du
aufzeichnest.
[Wiederaufbau, Dossier und Update-Folgen](08-rebuild-dossier-update-impact.md)
erklärt jeden Abschnitt.

## Der Weg über das Terminal

Die Einrichtungskarte führt diese Befehle für dich aus. Nutze sie, wenn
dir das Terminal lieber ist, oder auf einem Rechner ohne das Plugin.

### Die Engine installieren

Die Engine ist ein einziges Programm, `seldon`. Das AUR-Paket kommt bald.
Bis dahin installierst du sie aus dem GitHub-Release des Projekts. Das
Installationsskript prüft die Engine gegen die Prüfsummen des Release
und installiert `~/.local/bin/seldon`. Ist die GitHub-CLI (`gh`)
installiert und angemeldet, prüft das Skript außerdem, dass der
Release-Workflow des Projekts den Download gebaut hat; sonst meldet es in
einer Zeile, dass nur die Prüfsumme geprüft wurde. Mit
`--require-verified` bricht es ab, statt ohne diese Prüfung zu
installieren; `--skip-provenance` lässt `gh` aus, wenn `gh` selbst
scheitert, etwa hinter einem Proxy.

Lade das Skript und die Prüfsummendatei aus dem neuesten Release
herunter, lies das Skript, prüfe es und führe es aus:

```sh
cd "$(mktemp -d)"
curl -fsSLO https://github.com/JohnAndrewsX/jax-seldon/releases/latest/download/install.sh
curl -fsSLO https://github.com/JohnAndrewsX/jax-seldon/releases/latest/download/SHA256SUMS
less install.sh
sha256sum -c --ignore-missing SHA256SUMS && bash install.sh
```

Das Skript sagt zuerst, was es wohin installiert, und endet mit den
Schritten, die noch fehlen: `seldon init` und das Plugin, außer du hast
sie schon.

Prüfe, ob deine Shell die Engine findet:

```sh
seldon --version
```

```text
seldon 0.1.4
```

Meldet deine Shell `command not found`, liegt `~/.local/bin` noch nicht
in deinem `PATH`. Omarchys Standard-Einrichtung für bash fügt es beim
Start einer Shell hinzu, also öffne ein neues Terminal und versuche es
noch einmal. Nutzt du eine andere Shell oder eine eigene Startdatei,
trag dort `export PATH="$HOME/.local/bin:$PATH"` ein.
[Aktualisieren und entfernen](11-update-and-uninstall.md) listet alle
Optionen der Installation.


### Dein Logbuch anlegen

```sh
seldon init
```

Es stellt eine Frage, auf Englisch: wohin das Logbuch soll (Enter nimmt
`~/Seldon`). Den Rest nimmt es wie die Karte: die Sprache deiner Locale,
git, die Einstellungen von Obsidian, wenn Obsidian installiert ist, und
eine erste Erfassung, die 90 Tage zurückschaut und diese Vorgeschichte
als „before Seldon“ aufzeichnet. Es endet mit derselben Zusammenfassung
wie das Terminal der Karte.

`seldon init --defaults` fragt gar nichts; das führt die Karte aus.
`seldon init --ask` ist der vollständige Assistent, wenn du alles selbst
wählen willst. Jede Frage hat einen Standardwert; Enter übernimmt ihn.
In einer Liste setzt oder entfernt die Leertaste ein Häkchen, Enter
bestätigt.

| Frage | Was du beim ersten Mal antwortest |
|---|---|
| Where should the logbook live? | `~/Seldon` |
| Language of the logbook prose | die Sprache, in der du Notizen schreibst (Deutsch oder Englisch) |
| Add Obsidian settings (.obsidian/)? | den Standardwert (ja, wenn Obsidian installiert ist) |
| Collectors | alle sechs behalten |
| Watched config paths | die Vorgaben behalten |
| More paths | leer lassen |
| Agent setup | „Claude Code hooks“ mit der Leertaste anhaken, wenn du Claude Code nutzt; sonst nichts |
| Record theme switches instantly? | nein (die nächste Erfassung zeichnet sie ohnehin auf) |
| Keep the logbook in git, with a first commit? | ja |
| Backfill since | Enter nimmt das Datum 90 Tage zurück; `none` zeichnet ab jetzt auf |
| Dismiss them as "before Seldon"? | ja (kommt nur nach einer Nacherfassung, die etwas gefunden hat) |

Eine Nacherfassung (Backfill) zeichnet auch ältere Änderungen auf: das
Paket-Log und die Snapshots. Die meisten davon sind Routine und landen
in der Historie. Der Rest erscheint als Drift, Änderungen ohne Case; sie
als „before Seldon“ zu verwerfen behält die Ereignisse und verlangt
nichts mehr von dir. [Konzepte](02-concepts.md#baseline) erklärt es.

`seldon init --non-interactive` fragt ebenfalls nichts, für Skripte. Es
nimmt `~/Seldon`, die Sprache deiner Locale, alle Collectors und git und
schaut ebenfalls 90 Tage zurück; es erkennt nichts (`--obsidian` fügt
die Einstellungen von Obsidian hinzu). `--since` setzt einen anderen
Beginn, `--no-capture` zeichnet noch nichts auf.

### Die Einrichtung prüfen

```sh
seldon doctor
```

```text
seldon doctor · ~/Seldon
  ok        engine   seldon 0.1.4, contract 2
  ok        config   ~/.config/seldon/config.toml
  ok        logbook  ~/Seldon · machine <machine> · de · 0 cases, 0 decisions, 0 journal days
  ok        cases    every case id has one file
  ok        ledger   0 months, every line an event
  ok        fences   STATUS.md and DECISIONS.md: every generated fence has its end marker
  ok        rules    current (v5)
  ok        rollbacks no case has a rollback snapshot
  ok        workpieces no workpiece folders
  degraded  collectors last capture failed: snapper: snapper: No permissions. This user can neither list the snapshots nor read the snapshot directory; `seldon doctor` prints the read grant.
                     fix: sudo setfacl -m u:$USER:rx /.snapshots
  ok        layout   no linked folders or files where Seldon writes
  ok        state    ~/.local/state/seldon: cursors.json, manifest.json readable
  ok        skills   no agent skill folder (~/.agents/skills, ~/.claude/skills, ~/.codex/skills, ~/.pi/agent/skills, ~/.hermes/skills); nothing to install
  ok        hooks    none: no Claude Code harness is configured
  ok        omarchy  Omarchy 4.0.4-1
  degraded  snapper  No permissions. Snapshots are not recorded until you grant your user read access to the snapshot directory once (ADR-0026). The fix grants your user read access to the snapshot directory listing and the snapshot info files (files inside a snapshot keep their own permissions), nothing else: no snapshot creation, change or deletion.
                     fix: sudo setfacl -m u:$USER:rx /.snapshots
  ok        pacman   no db.lck: pacman is not running
  ok        git      git version 2.55.0; logbook is a repository; autocommit on
  ok        watch    watchPaths: 13 path(s), every default included
  ok        drift    attention normal · routine: sysupgrade, upgrade, keyring, omarchy-update, plugin-toggle, seldon-self, theme, omarchy-default, system-link, routine-paths, theme-assets, theme-repo, toggle-flag · routinePaths 2 · routinePackages 2 · alwaysRedPaths 9 · alwaysRed 20; all defaults; Omarchy's copies count as evidence (/usr/share/omarchy)
doctor: ok
```

Alle Zeilen sollten `ok` zeigen, nur `snapper` und `collectors` sagen
`degraded`, bis du die Freigabe ausführst: Die erste Erfassung konnte die
Snapshots nicht lesen.
Wenn du Snapshots auf der Zeitleiste sehen willst, führe die Abhilfe aus,
die `doctor` ausgibt: dieselbe Freigabe wie Schritt 3 der Karte.

## Wie es weitergeht

- [Konzepte](02-concepts.md): was Cases, Zonen, Risiko und Drift bedeuten.
- [Alltag](03-daily-use.md): die Pill, das Panel und der Prime Radiant.
- [Mit Agenten arbeiten](04-working-with-agents.md): Claude Code oder
  einen anderen Agenten einen Case bearbeiten lassen, während Seldon
  aufzeichnet, was er tut.

---

[Übersicht](README.md) · Weiter: [Konzepte](02-concepts.md)
