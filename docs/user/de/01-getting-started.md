# Erste Schritte

<!-- source: en/01-getting-started.md @ 474b2ed -->

Diese Seite führt dich in etwa fünfzehn Minuten zu einem fertigen
Logbuch. Du installierst die Engine, legst dein Logbuch an, fügst das
Bar-Plugin hinzu, zeichnest eine ungeplante Änderung auf und planst eine
Änderung als Case. Jeder Schritt zeigt den Befehl und das, was du sehen
solltest.

## Was du brauchst

- Omarchy 4 mit der Omarchy-Shell.
- `git` (bringt Omarchy mit).
- Ein Terminal. Die Befehle auf dieser Seite laufen als dein Benutzer;
  keiner braucht `sudo`.

Seldon liest dein System nur. Es installiert nie ein Paket und hält nie
einen Befehl an. Es schreibt drei Dinge: dein Logbuch (standardmäßig
`~/Seldon`), seine Konfigurationsdatei (`~/.config/seldon/config.toml`)
und seinen Zustand (`~/.local/state/seldon/`). Außerhalb dieser Ordner
schreibt es nur, wo du bei der Einrichtung zustimmst: Omarchys
Theme-Hook und die Hook-Einstellungen von Claude Code.

## Schritt 1: Die Engine installieren

Die Engine ist ein einziges Programm, `seldon`. Das AUR-Paket kommt bald.
Bis dahin installierst du sie aus dem GitHub-Release des Projekts. Das
Installationsskript prüft die Engine gegen die Prüfsummen des Release
und installiert `~/.local/bin/seldon`.

Lade das Skript und die Prüfsummendatei aus dem neuesten Release
herunter, lies das Skript, prüfe es und führe es aus:

```sh
cd "$(mktemp -d)"
curl -fsSLO https://github.com/JohnAndrewsX/jax-seldon/releases/latest/download/install.sh
curl -fsSLO https://github.com/JohnAndrewsX/jax-seldon/releases/latest/download/SHA256SUMS
less install.sh
sha256sum -c --ignore-missing SHA256SUMS && bash install.sh
```

Prüfe, ob deine Shell die Engine findet:

```sh
seldon --version
```

```text
seldon 0.1.1
```

Meldet deine Shell `command not found`, liegt `~/.local/bin` noch nicht
in deinem `PATH`. Omarchys Standard-Einrichtung für bash fügt es beim
Start einer Shell hinzu, also öffne ein neues Terminal und versuche es
noch einmal. Nutzt du eine andere Shell oder eine eigene Startdatei,
trag dort `export PATH="$HOME/.local/bin:$PATH"` ein.
[Aktualisieren und entfernen](11-update-and-uninstall.md) listet alle
Optionen der Installation.

## Schritt 2: Dein Logbuch anlegen

Starte den Assistenten einmal:

```sh
seldon init
```

Er stellt ein paar Fragen, auf Englisch. Jede hat einen sinnvollen
Standardwert; Enter übernimmt ihn. In einer Liste setzt oder entfernt
die Leertaste ein Häkchen, Enter bestätigt.

| Frage | Was du beim ersten Mal antwortest |
|---|---|
| Where should the logbook live? | `~/Seldon` |
| Language of the logbook prose | die Sprache, in der du Notizen schreibst (Deutsch oder Englisch) |
| Add Obsidian settings (.obsidian/)? | ja, wenn du Obsidian nutzt, sonst nein |
| Collectors | alle sechs behalten |
| Watched config paths | die Vorgaben behalten |
| More paths | leer lassen |
| Agent harnesses | „Claude Code hooks“ mit der Leertaste anhaken, wenn du Claude Code nutzt; sonst nichts |
| Record theme switches the moment they happen? | nein (lässt sich später nachholen) |
| Make the logbook a git repository with a first commit? | ja |
| Backfill since | leer lassen |

Bei der Nacherfassung (Backfill) heißt leer: Seldon zeichnet ab jetzt
auf. Ein Datum lässt die erste Erfassung auch ältere Änderungen
aufzeichnen. Keine davon gehört zu einem Case, also erscheint jede als
Drift. Der Assistent bietet dann an, sie als Baseline vor Seldon zu
markieren. Lass die Nacherfassung fürs Erste weg;
[Konzepte](02-concepts.md#baseline) erklärt sie.

Am Ende fasst der Assistent zusammen, etwa so (gekürzt):

```text
Logbook created at ~/Seldon (machine <machine>, language de, 31 files).
Config: ~/.config/seldon/config.toml
Git: repository initialised, first commit "seldon: init logbook"
Snapper: degraded — No permissions. Snapshots are not recorded until you allow your user once (ADR-0011). The fix adds your user to ALLOW_USERS of the root snapper config, which also lets your user create, change and delete root snapshots without a password.
First capture: 0 event(s); degraded: snapper (see seldon doctor); 0 open drift item(s), 0 crisis
Dossier: Wrote system/hardware.md, system/omarchy.md, system/packages.md, system/plugins.md, system/services.md (7 fence(s) changed)
Next steps:
  seldon doctor
```

`Snapper: degraded` ist auf Omarchy normal. Dein Benutzer darf
Snapshots anfangs eventuell nicht auflisten. Seldon funktioniert auch
ohne sie; Schritt 3 zeigt die Abhilfe.

Ganz ohne Fragen geht es mit `seldon init --non-interactive`. Das nimmt
`~/Seldon`, die Sprache deiner Locale, alle Collectors und git und
zeichnet ab jetzt auf.

## Schritt 3: Die Einrichtung prüfen

```sh
seldon doctor
```

```text
seldon doctor · ~/Seldon
  ok        engine   seldon 0.1.1, contract 1
  ok        config   ~/.config/seldon/config.toml
  ok        logbook  /home/you/Seldon · machine <machine> · de · 0 cases, 0 decisions, 0 journal days
  ok        omarchy  Omarchy 4.0.4-1
  degraded  snapper  No permissions. Snapshots are not recorded until you allow your user once (ADR-0011). The fix adds your user to ALLOW_USERS of the root snapper config, which also lets your user create, change and delete root snapshots without a password.
                     fix: sudo snapper -c root set-config ALLOW_USERS=$USER SYNC_ACL=yes
  ok        git      git version 2.55.0; logbook is a repository; autocommit on
doctor: ok
```

Alle Zeilen sollten `ok` zeigen, nur `snapper` darf `degraded` sagen.
Wenn du Snapshots auf der Zeitleiste sehen willst, führe die Abhilfe aus,
die `doctor` ausgibt. Sie ändert die Snapper-Konfiguration von root, also
entscheidest du: Sie trägt deinen Benutzer in `ALLOW_USERS` ein, und
Snapper kennt dort keine Nur-Lese-Stufe. Dein Benutzer kann danach also
auch Snapshots von root ohne Passwort anlegen, ändern und löschen. Seldon
führt sie nie für dich aus.

## Schritt 4: Das Bar-Plugin hinzufügen

Das Plugin zeigt dein Logbuch in der Omarchy-Bar. Es ist optional. Mit
ihm siehst du auf einen Blick, was Seldon aufzeichnet.

```sh
omarchy plugin add https://github.com/JohnAndrewsX/jax-seldon-plugin.git --enable
```

Rechts in der Bar erscheint eine kleine Pill mit dem Seldon-Zeichen. Ein
Klick öffnet das Panel. [Alltag](03-daily-use.md) erklärt jeden Teil davon.

Keine Pill? Prüf, ob `omarchy plugin list` das Plugin `jax.seldon` als
aktiviert zeigt (sonst `omarchy plugin enable jax.seldon`), und starte
dann die Shell mit `omarchy-restart-shell` neu. Zeigt das Panel ein
Banner statt Daten, ist sein Knopf die Abhilfe; *Check again* sucht die
Engine noch einmal. [Fehlersuche](10-troubleshooting.md#banner-im-panel)
listet jedes Banner.

## Schritt 5: Eine ungeplante Änderung aufzeichnen

Ändere etwas, ohne Seldon vorher Bescheid zu geben. Ein Theme-Wechsel
eignet sich gut: Er ist sichtbar und in einer Sekunde rückgängig gemacht.

1. Merk dir dein aktuelles Theme, damit du später zurückwechseln kannst:

   ```sh
   omarchy theme current
   ```

2. Wechsle zu einem anderen Theme, mit Omarchys Theme-Switcher oder mit
   `omarchy theme set <name>`. `omarchy theme list` zeigt die Namen.

3. Lass Seldon nach Änderungen suchen:

   ```sh
   seldon capture
   ```

   ```text
   Captured 1 new event(s).
     snapper     0
     pacman      0
     omarchy     0
     plugins     0
     theme       1
     config      0
   ```

   Das Plugin erfasst außerdem von selbst alle 15 Minuten. Hier startest
   du die Erfassung von Hand, damit du nicht warten musst.

4. Frag Seldon, was unerklärt ist:

   ```sh
   seldon drift
   ```

   ```text
   yellow  2026-10-02 19:54  theme/theme-set  gruvbox  01M3YW134EVKJ23C1GXVHDVVEH
   1 open drift item(s), 0 crisis
   ```

   Der Theme-Wechsel ist **Drift**: eine Änderung, die kein Case abdeckt.
   `yellow` ist ihre Zone: eine Konfigurations- oder Theme-Änderung, kein
   Paket ([Konzepte](02-concepts.md#zonen) erklärt Zonen). Die letzte
   Spalte ist die Ereignis-ID (deine sieht anders aus). Wenn du das Plugin
   hinzugefügt hast, zeigt die Pill in der Bar jetzt `· 1` hinter dem
   Zeichen.

5. Erkläre sie. Die Änderung ist schon passiert, also hält Seldon deinen
   Grund als nachträglichen Case fest: ein neuer Case, gleich als
   abgeschlossen angelegt, mit deinem Text als Titel:

   ```sh
   seldon drift explain <EVENT> -- "Anderes Theme ausprobiert"
   ```

   Ersetze `<EVENT>` durch die ID aus deiner Ausgabe von `seldon drift`.

   ```text
   Explained 1 event(s) with the new completed case C-2026-001
   Case: work/completed/C-2026-001-anderes-theme-ausprobiert.md
   ```

Deine Case-IDs tragen das aktuelle Jahr. `seldon drift` meldet jetzt
`No open drift.`

## Schritt 6: Eine Änderung als Case planen

Jetzt der geplante Weg. Leg einen Case für den Rückwechsel an und starte
ihn:

```sh
seldon plan new --area themes -- "Zurück zu meinem üblichen Theme"
seldon plan start C-2026-002
```

```text
Created C-2026-002 "Zurück zu meinem üblichen Theme" in work/queued/C-2026-002-zurueck-zu-meinem-ueblichen-theme.md
C-2026-002 queued → active (now work/active/C-2026-002-zurueck-zu-meinem-ueblichen-theme.md)
```

`--area themes` legt den Case im Bereich `themes` ab, einem von sechs
Themen, die ein neues Logbuch hat (`areas/themes/`). Nimm die ID, die
`plan new` ausgegeben hat. Der Case ist jetzt aktiv. Schreib eine Notiz
ins heutige Journal:

```sh
seldon log --case C-2026-002 -- "Wechsle zurück zu meinem üblichen Theme"
```

Wechsle zurück zu dem Theme, das du dir in Schritt 5 gemerkt hast. Dann
erfasse noch einmal und sieh dir die Drift an:

```sh
seldon capture
seldon drift
```

Der Wechsel erscheint wieder als Drift. Seldon sieht, dass sich das
Theme geändert hat, aber ein Collector kann nicht wissen, dass du es für
diesen Case getan hast. Nur Agenten, die über Hooks arbeiten, landen von
selbst beim aktiven Case. Der Case lohnt sich trotzdem: Er enthält deine
Notiz und, sobald verknüpft, die Änderung selbst als seine Spur. Nennt
der *Plan* des Case das Theme (zum Beispiel `tokyo-night`), schlägt
Seldon diesen Case für die Änderung vor, und der Drift-Dialog des Panels
wählt ihn vor. Deine eigene Änderung verknüpfst du mit einem Befehl:

```sh
seldon drift link <EVENT> C-2026-002
```

```text
Linked 1 event(s) to C-2026-002
```

Schließ den Case ab. `verify` sagt, dass die Arbeit getan ist; `done`
sagt, dass du sie geprüft hast:

```sh
seldon plan verify C-2026-002
seldon plan done C-2026-002
```

```text
C-2026-002 active → verification
C-2026-002 verification → completed (now work/completed/C-2026-002-zurueck-zu-meinem-ueblichen-theme.md)
Journal: journal/2026/2026-10-02.md
```

## Schritt 7: Das Ergebnis ansehen

```sh
seldon status
```

```text
Status of <machine> (2026-10-02)
  cases   0 active · 0 in verification · 0 queued
  drift   0 open · 0 crisis
  events  9 today · 9 in 7 days
Wrote ledger/2026-10.md, STATUS.md
Index: ~/.local/state/seldon/index.json
```

Öffne das Panel des Plugins und drück `2` für den Changelog. Du siehst
beide Theme-Wechsel, die Notizen und die Schritte der Cases. Drück `3`
für Work: Beide Cases stehen in der Spalte Completed.

Das Logbuch ist reines Markdown, und jeder Schritt war ein git-Commit:

```sh
git -C ~/Seldon log --oneline
```

## Schritt 8: Die Wiederaufbau-Anleitung schreiben

```sh
seldon rebuild
```

```text
Wrote outputs/REBUILD.md: 0 package(s), 0 deviation(s), 1 plugin(s), 0 unit(s), 0 open
```

Deine Zahlen weichen ab. Öffne `~/Seldon/outputs/REBUILD.md`. Dort steht,
was eine frische Omarchy-Installation braucht, um wieder diese Maschine
zu werden: deine eigenen Pakete, geänderte Dateien, Plugins und das
Theme aus Schritt 6. Die Anleitung wächst mit jedem Case, den du
aufzeichnest.
[Wiederaufbau, Dossier und Update-Folgen](08-rebuild-dossier-update-impact.md)
erklärt jeden Abschnitt.

## Wie es weitergeht

- [Konzepte](02-concepts.md): was Cases, Zonen, Risiko und Drift bedeuten.
- [Alltag](03-daily-use.md): die Pill, das Panel und der Prime Radiant.
- [Mit Agenten arbeiten](04-working-with-agents.md): Claude Code oder
  einen anderen Agenten einen Case bearbeiten lassen, während Seldon
  aufzeichnet, was er tut.

---

[Übersicht](README.md) · Weiter: [Konzepte](02-concepts.md)
