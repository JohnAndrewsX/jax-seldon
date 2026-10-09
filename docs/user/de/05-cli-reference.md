# Befehlsreferenz

<!-- source: en/05-cli-reference.md @ 0a73a182 -->

Diese Seite listet jeden Befehl von `seldon` mit jeder Option, nach
Aufgaben gruppiert. Die Hilfeblöcke sind die eigene `--help`-Ausgabe der
Engine, auf Englisch; eine Prüfung hält sie gleich mit der Engine. Der
Text drumherum ergänzt, was die Hilfe nicht sagt.

## So liest du diese Seite

- `<ID>` ist eine Case-ID (`C-2026-004`), `<EVENT>` eine Ereignis-ID (die
  lange ID, die `seldon drift` ausgibt), `<DIR>` und `<FILE>` sind Pfade.
- Freitext (eine Notiz, ein Titel, ein Grund) steht hinter `--`, als ein
  einziges Argument: `seldon log -- "Text"`.
- Jeder Block lässt die globalen Optionen weg, die unten stehen. Jeder
  Befehl versteht sie.
- `seldon help <command>` und `seldon <command> --help` geben im Terminal
  denselben Text aus.

## Globale Optionen

<!-- help: seldon -->
```text
Flight recorder and planning desk for your Omarchy system

Usage: seldon [OPTIONS] [COMMAND]

Commands:
  contract-version  Print the engine/plugin contract version
  init              Create a logbook (wizard; --non-interactive takes defaults)
  doctor            Check engine, config, logbook, collector state, agent skill, omarchy, snapper and git
  capture           Run collectors and append new events to the ledger
  log               Write a note: a ledger event and a journal entry
  event             Record an event by hand (hooks, scripts)
  plan              Plan and track cases: new, start, verify, done, drop, list, show
  decide            Create a decision record (ADR) and open it in the editor; accept a proposed one
  preview           Before the logbook exists: the last days' pacman transactions and the files edited under ~/.config, read-only, nothing written
  open              Print the path of a logbook file; --editor opens it
  index             Rebuild index.json and the ledger/*.md views; --check validates
  status            Regenerate STATUS.md, the ledger views and index.json; print a summary
  hook              Agent hooks: record commands, print session context, install into or uninstall from a harness
  drift             List open drift; link, explain, dismiss or show a drift event; propose, apply or discard a triage proposal
  agent             Start an agent on an active case, or ask one about the open changes, a change or a case
  rebuild           Write outputs/REBUILD.md: the steps to rebuild this machine
  watch             Rebuild index.json when the logbook changes (feature "watch")
  dossier           Refresh the generated fences of system/*.md from read-only queries
  import            Import an earlier logbook (dry run unless --apply) or your Markdown task files as cases
  inbox             File a text into the logbook's inbox (an agent's crash analysis, a finding)
  rules             The agent rules in the logbook's AGENTS.md: update
  config            Edit config.toml: watch one more path (the desk's Watch on a recently edited file)
  completions       Print a shell completion script for bash, zsh or fish
  mangen            Print the man page seldon(1), generated from this help
  help              Print this message or the help of the given subcommand(s)

Options:
  -V, --version        Print the engine version
      --json           Machine-readable output
      --logbook <DIR>  Logbook directory (overrides config.toml and SELDON_LOGBOOK)
      --quiet          No human output on success
      --no-commit      Do not commit logbook changes to git
      --config <FILE>  Config file (overrides SELDON_CONFIG and ~/.config/seldon/config.toml)
  -h, --help           Print help
```
<!-- /help -->

- `--json` gibt ein JSON-Objekt statt Text aus. Agenten und Skripte
  nutzen das. Ein Fehler mit `--json` ist
  `{"error": {"code": 1, "message": "…"}}`.
- `--logbook <DIR>` wählt das Logbuch für einen Befehl. Ohne die Option
  nimmt die Engine `SELDON_LOGBOOK`, dann `logbook` aus `config.toml`,
  dann `~/Seldon`.
- `--no-commit` schreibt die Dateien, lässt aber den git-Commit weg. Der
  nächste Befehl, der committet, nimmt die Änderungen mit.

## Exit-Codes

| Code | Bedeutung |
|---|---|
| 0 | ok |
| 1 | Benutzerfehler: ein falsches Argument, ein unbekannter Case oder ein unbekanntes Ereignis, ein Schritt, den der Case nicht gehen kann |
| 2 | Fehler der Engine: etwas ist schiefgegangen, das nicht hätte schiefgehen dürfen |
| 3 | das Logbuch ist nicht angelegt; führe `seldon init` aus |
| 4 | ein anderes `seldon` hält die Sperre; kurz warten und noch einmal versuchen |

`--help` endet mit 0. Eine unbekannte Option ist ein Benutzerfehler (1).

## Umgebungsvariablen

| Variable | Wirkung |
|---|---|
| `SELDON_LOGBOOK` | der Ordner des Logbuchs, wenn `--logbook` fehlt |
| `SELDON_CONFIG` | die Konfigurationsdatei statt `~/.config/seldon/config.toml`, wenn `--config` fehlt |
| `VISUAL`, `EDITOR` | der Editor, den `seldon open --editor` und `seldon decide` im Terminal starten |
| `SELDON_ACTOR` | wen `log`, `event`, `plan` und `drift` aufzeichnen, wenn `--actor` fehlt, und `hook generic`, wenn sein JSON kein `"actor"` hat. `seldon agent start` setzt sie für den Agenten (`agent:` und der Name des Launchers). Ein Wert, den der Befehl nicht annimmt, ist ein Bedienfehler (1); leer zählt als nicht gesetzt |
| `SELDON_ATTENDED` | `1` für einen Agenten, den `seldon agent start` gestartet hat: Die Sitzung hat der Benutzer begonnen. Gesetzt für die Regeln des Agenten; die Engine liest sie nie |
| `SELDON_CASE` | die Case-ID für einen Agenten, den `seldon agent start` gestartet hat. Die Hooks zeichnen eine solche Sitzung auf, wo immer sie arbeitet, solange dieser Case aktiv oder in Prüfung ist; jeder andere Wert gilt als nicht gesetzt. Befehle landen weiter auf dem aktiven Case. Seldon setzt sie; setz sie nie selbst |
| `CLAUDE_CONFIG_DIR` | der Einstellungsordner von Claude Code: `hook install claude-code` und `doctor` nehmen dessen `settings.json` statt `~/.claude/settings.json` |
| `SELDON_NOW` | eine feste Uhrzeit (RFC 3339), für Vorführungen und Tests |
| `SELDON_TEST_GUARD` | ein Ordner; die Engine verweigert den Start (Exit 2), wenn ihr Home-, Konfigurations- oder Zustandsordner außerhalb davon liegt. Nimm sie, wenn du Seldon in einem Wegwerf-Home ausprobierst |
| `SELDON_OMARCHY_AGENT_KIT` | wo `init --harness omarchy-agent` das Kit findet, statt `~/.local/share/seldon/harness/omarchy-agent/` |

## Einrichten und prüfen

### seldon init

Legt das Logbuch, die Konfigurationsdatei, die erste Erfassung und das
Dossier an. Ohne Optionen stellt es Fragen; jede Option überspringt ihre
Frage. Es lehnt einen Ordner ab, der nicht leer ist, und überschreibt nie
eine Datei. `--non-interactive` nimmt die Optionen, dann eine vorhandene
Konfiguration, dann die Vorgaben. `--since` nimmt ein Datum
(`2026-09-29`, Mitternacht Ortszeit) oder eine Zeit nach RFC 3339;
`--baseline` braucht `--since`; `--no-capture` geht nicht zusammen mit
`--since`. Siehe
[Erste Schritte](01-getting-started.md#schritt-2-dein-logbuch-anlegen).
`--harness skills` installiert den Seldon-Agentenskill, wie es
[`seldon hook install skills`](#seldon-hook-install) tut.
`--remove-theme-hook` ist die einzige Option, die kein Logbuch anlegt:
Sie entfernt den Theme-Hook, den `--theme-hook` installiert hat, und geht
mit keiner anderen Option zusammen. Siehe
[Aktualisieren und entfernen](11-update-and-uninstall.md#entfernen).

<!-- help: seldon init -->
```text
Create a logbook (wizard; --non-interactive takes defaults)

Usage: seldon init [OPTIONS]

Options:
      --path <DIR>           Logbook directory (default ~/Seldon)
      --non-interactive      Ask nothing; take flags, then the existing config, then the defaults: ~/Seldon, language from the locale, all collectors, git on, first capture from now on, no backfill, no theme hook
      --language <LANGUAGE>  Language of the logbook prose [possible values: en, de]
      --obsidian             Add Obsidian settings (.obsidian/)
      --harness <NAME>       Agent harness to set up (repeatable) [possible values: claude-code, omarchy-agent, skills]
      --since <TS>           Backfill: the first capture also records changes since TS, a date (YYYY-MM-DD, local midnight) or an RFC 3339 time; each one opens as drift
      --baseline             Mark the backfilled drift as the pre-Seldon baseline (dismissed)
      --no-capture           Do not run the first capture
      --theme-hook           Install Omarchy's theme-set hook (`omarchy hook install theme-set`)
      --remove-theme-hook    Remove the theme-set hook that --theme-hook installed, and nothing else; needs no logbook
      --git                  Make the logbook a git repository with a first commit (default unless the existing config says otherwise)
      --no-git               Do not use git

Examples:
  seldon init
  seldon init --non-interactive --since 2026-09-01 --baseline
  seldon init --remove-theme-hook
```
<!-- /help -->

### seldon doctor

Prüft die Engine, die Konfiguration, das Logbuch (Cases, Ledger,
generierte Abschnitte), den letzten Capture der Collectors und ihre
Zustandsdateien, den Seldon-Agentenskill, Omarchy, Snapper und git. Es
liest nur. Die Zeile `skills` sagt, wo der Skill installiert ist, wo er
fehlt (optional), veraltet ist, von Hand geändert wurde oder wo ein
anderer Skill namens `seldon` liegt. Die Zeile `hooks` sagt, wo Seldons
Claude-Code-Hooks liegen: nutzerweit (`ok`), nur in `.claude/settings.json`
des Logbuchs (`degraded`: Sitzungen, die Seldon in `~/Work` startet,
werden nicht aufgezeichnet; die Lösung installiert sie nutzerweit),
beides (`ok`, mit optionalem Aufräumen) oder nirgends (`ok`, außer du
hast den Claude-Code-Harness gewählt). Die Zeile `layout` nennt jeden
Ordner und jede Datei, in die Seldon schreibt und die ein symbolischer
Link ist (oder eine Datei, wo ein Ordner hingehört, oder umgekehrt):
`error`, wenn Befehle, die dort schreiben, das verweigern, `degraded` für
eine verlinkte Ansicht, die `status` überspringt. Jede Zeile sagt
`ok`, `degraded` oder `error`, und eine fehlerhafte Prüfung nennt den
Befehl, der sie behebt. Exit 0, wenn nichts ein Fehler ist, 1, wenn eine
Prüfung ein Fehler ist (auch, wenn `config.toml` nicht gelesen oder
geparst werden kann), 3, wenn das Logbuch nicht angelegt ist.
`--only rules` prüft nur die Agentenregeln in `AGENTS.md` und startet
kein anderes Programm; das Panel fragt das, wenn es sich öffnet.

<!-- help: seldon doctor -->
```text
Check engine, config, logbook, collector state, agent skill, omarchy, snapper and git

Usage: seldon doctor [OPTIONS]

Options:
      --path <DIR>
          Logbook to check (same as the global --logbook)

      --json
          Machine-readable output

      --only <CHECK>
          Run one check only; `rules`: the logbook's agent rules, without starting omarchy, snapper or git (what the panel asks)

          Possible values:
          - rules: The rules block of the logbook's `AGENTS.md`

      --logbook <DIR>
          Logbook directory (overrides config.toml and SELDON_LOGBOOK)

      --quiet
          No human output on success

      --no-commit
          Do not commit logbook changes to git

      --config <FILE>
          Config file (overrides SELDON_CONFIG and ~/.config/seldon/config.toml)

  -h, --help
          Print help (see a summary with '-h')

Examples:
  seldon doctor
  seldon doctor --only rules --json
```
<!-- /help -->

### seldon contract-version

Gibt die Version des Index-Formats aus, das die Engine schreibt und das
Plugin liest. Das Plugin vergleicht sie mit seiner eigenen.

<!-- help: seldon contract-version -->
```text
Print the engine/plugin contract version

Usage: seldon contract-version [OPTIONS]

Options:
```
<!-- /help -->

### seldon config

Deine `config.toml`.

<!-- help: seldon config -->
```text
Edit config.toml: watch one more path (the desk's Watch on a recently edited file)

Usage: seldon config [OPTIONS] <COMMAND>

Commands:
  watch  Add a path under your home directory to watchPaths; the rest of config.toml stays as it is
  help   Print this message or the help of the given subcommand(s)

Options:
```
<!-- /help -->

### seldon config watch

Nimmt einen Pfad unter deinem Home-Verzeichnis in `watchPaths` der
`config.toml` auf: das *Watch* des Desks bei einer kürzlich bearbeiteten
Datei (System › Recently edited). Nur die Liste `watchPaths` ändert sich;
deine Kommentare und die Reihenfolge der Datei bleiben. Die nächste
Erfassung nimmt die Dateien unter dem Pfad, wie sie sind, ohne Ereignis
auf; eine spätere Änderung ist eine Konfigurationsänderung wie jede
andere. Ein Pfad, der schon beobachtet wird, ändert nichts (Exit 0).
Abgewiesen, mit Exit 1 und ohne Schreiben: ein Pfad außerhalb deines
Home-Verzeichnisses, einer in oder um Seldons eigene Dateien (das
Logbuch, `~/.local/state/seldon`, `~/.config/seldon`), einer, den es
nicht gibt, einer unter `[redaction] skipPaths`, einer, der über einen
Link aus deinem Home-Verzeichnis, in Seldons eigene Dateien oder unter
`skipPaths` führt, und eine `config.toml`,
deren Liste sich nicht erweitern lässt, ohne die Datei neu zu schreiben
(die Meldung nennt die Zeile, die du von Hand ergänzt).

<!-- help: seldon config watch -->
```text
Add a path under your home directory to watchPaths; the rest of config.toml stays as it is

Usage: seldon config watch [OPTIONS] <PATH>

Arguments:
  <PATH>  The file or folder (`~/…`; a relative path lies under the home directory)

Options:

Examples:
  seldon config watch ~/.config/alacritty/alacritty.toml
  seldon config watch --json -- ~/.config/starship.toml
```
<!-- /help -->

## Aufzeichnen

### seldon capture

Lässt die Collectors laufen, hängt neue Ereignisse ans Ledger an und
baut dann den Index neu. Ohne Optionen läuft jeder Collector, den
`config.toml` einschaltet; `--source` startet genau die genannten, auch
ausgeschaltete. Ein Collector, der seine Quelle nicht lesen kann, meldet
`degraded` mit einer Abhilfe, und die Erfassung endet trotzdem mit 0.
`--since` wirkt nur auf einen Collector, der noch nie gelaufen ist.

<!-- help: seldon capture -->
```text
Run collectors and append new events to the ledger

Usage: seldon capture [OPTIONS]

Options:
      --source <NAMES>  Collectors to run, comma-separated (default: every enabled one)
      --all             Run every enabled collector (the default)
      --since <TS>      Baseline for collectors without a cursor, an RFC 3339 time (default: the logbook's creation)

Examples:
  seldon capture
  seldon capture --source pacman,config
  seldon capture --since 2026-09-01T00:00:00+02:00
```
<!-- /help -->

### seldon log

Schreibt eine Notiz: ein Ereignis `manual` im Ledger und einen Eintrag
unter dem heutigen Datum im Journal. `--case` legt sie unter einem Case
ab; `--tag` fügt `#tag` zur Journal-Zeile hinzu.

<!-- help: seldon log -->
```text
Write a note: a ledger event and a journal entry

Usage: seldon log [OPTIONS] <TEXT>

Arguments:
  <TEXT>  The note, as one argument; after `--` when it starts with `-`

Options:
      --case <ID>      The case the note belongs to
      --actor <ACTOR>  Who writes the note: human or agent:NAME (default: $SELDON_ACTOR, else human)
      --tag <TAG>      Tag the note (repeatable): `#tag` in the journal, `meta.tags` in the ledger

Examples:
  seldon log -- "Switched the terminal font to Iosevka"
  seldon log --case C-2026-004 --tag fonts -- "Tried two fonts, kept the first"
```
<!-- /help -->

### seldon inbox

<!-- help: seldon inbox -->
```text
File a text into the logbook's inbox (an agent's crash analysis, a finding)

Usage: seldon inbox [OPTIONS] <COMMAND>

Commands:
  add   File a text into the logbook's inbox/, redacted; the same text again changes nothing
  help  Print this message or the help of the given subcommand(s)

Options:
```
<!-- /help -->

### seldon inbox add

Legt einen Text im `inbox/` des Logbuchs ab, zum Beispiel die
Absturzanalyse eines Agenten (wann, sagt der Agentenskill). Der Text wird
wie eine Notiz geschwärzt, Pfade im Home-Verzeichnis werden zu `~`, und er
landet in `inbox/<datum>-<titel>.md`, für sich committet. Derselbe Text
noch einmal ändert nichts; ein anderer Text unter einem Titel, dessen
Datei es schon gibt, bekommt `-2`. `--file -` liest den Text von stdin.

<!-- help: seldon inbox add -->
```text
File a text into the logbook's inbox/, redacted; the same text again changes nothing

Usage: seldon inbox add [OPTIONS] --title <TITLE> --file <FILE>

Options:
      --title <TITLE>  The title: one line, the file's `# heading` and name
      --file <FILE>    The text: a Markdown file, or `-` for stdin (at most 1 MiB, UTF-8)
      --tag <TAG>      Tag the text (repeatable): `tags` in the file's frontmatter
      --actor <ACTOR>  Who files it: human or agent:NAME (default: $SELDON_ACTOR, else human)

Examples:
  seldon inbox add --title "Crash: waybar (SIGSEGV)" --tag crash --file report.md
  printf '%s\n' "Zed ignores the theme" | seldon inbox add --title "Zed theme" --file -
```
<!-- /help -->

### seldon event

Zeichnet ein Ereignis von Hand auf, für eine Änderung, die kein Collector
und kein Hook sieht, etwa eine Einstellung in einer grafischen Oberfläche.
Hooks und Skripte nutzen es auch. Quelle und Art müssen dem Ledger
bekannt sein; die Fehlermeldung listet sie auf.

<!-- help: seldon event -->
```text
Record an event by hand (hooks, scripts)

Usage: seldon event [OPTIONS] --subject <SUBJECT> <SOURCE> <KIND>

Arguments:
  <SOURCE>  Event source (pacman, snapper, omarchy, plugins, theme, config, agent, manual)
  <KIND>    Event kind (install, theme-set, config-change, command, note, …)

Options:
      --subject <SUBJECT>  What it is about: package, ~-relative path, theme, plugin id, …
      --detail <TEXT>      Human-readable detail
      --case <ID>          Attribute the event to this case
      --actor <ACTOR>      Who did it: system (like a collector), human or agent:NAME; hooks and scripts name the one they act for (default: the agent command that caused a config, theme or plugins change, else $SELDON_ACTOR, else system)
      --meta <KEY=VALUE>   Extra key=value (repeatable), e.g. `--meta enabled=true`; `enabled` takes true or false
```
<!-- /help -->

### seldon status

Baut `STATUS.md`, die Monatsansichten des Ledgers und den Index neu und
gibt dann eine Zusammenfassung aus. Es committet nur, wenn sich eine
Datei des Logbuchs geändert hat.

<!-- help: seldon status -->
```text
Regenerate STATUS.md, the ledger views and index.json; print a summary

Usage: seldon status [OPTIONS]

Options:
```
<!-- /help -->

### seldon index

Baut den Index und die Ansichten des Ledgers neu, ohne `STATUS.md` und
ohne Commit. `--check` weigert sich, einen Index zu schreiben, der nicht
zum Format passt (Exit 2).

Der Graph des Desks (Abschnitt 8) entsteht aus diesem Index. Er zeigt
deshalb die neuesten 500 Ereignisse und 50 abgeschlossenen Cases, nicht
das ganze Logbuch. Einen Befehl, der das ganze Logbuch als Graph
zeichnet, gibt es noch nicht.

<!-- help: seldon index -->
```text
Rebuild index.json and the ledger/*.md views; --check validates

Usage: seldon index [OPTIONS]

Options:
      --check          Validate the index against schema/index.schema.json before writing it
```
<!-- /help -->

### seldon watch

Beobachtet das Logbuch und baut den Index zwei Sekunden nach der letzten
Änderung neu, sodass das Plugin Änderungen aus deinem Editor oder aus
Obsidian sofort zeigt. Es baut nur den Index: keine Erfassung, kein
`STATUS.md`, kein Commit. Beenden mit Strg-C. Das Release-Binary kann
es; ein Build ohne das Feature `watch` endet mit 1.
[Aktualisieren und entfernen](11-update-and-uninstall.md#der-optionale-watcher)
zeigt, wie du es als Benutzerdienst laufen lässt.

<!-- help: seldon watch -->
```text
Rebuild index.json when the logbook changes (feature "watch")

Usage: seldon watch [OPTIONS]

Options:
      --interval <SECS>  Quiet time before the index is rebuilt, in seconds (at least 2) [default: 2]
```
<!-- /help -->

## Cases und Entscheidungen

### seldon plan

Die Befehle für Cases. Jeder Schritt ist ein Ereignis im Ledger, eine
Zeile im *Log* des Case und ein Commit; die Engine verschiebt die Datei
zwischen `work/queued/`, `work/active/` und `work/completed/`.

<!-- help: seldon plan -->
```text
Plan and track cases: new, start, verify, done, drop, list, show

Usage: seldon plan [OPTIONS] <COMMAND>

Commands:
  new       Create a case in work/queued/
  start     Start a case: queued → active; it becomes the active case
  verify    Hand an active case to verification: active → verification (runs a capture first)
  done      Complete a verified case: verification → completed (runs a capture first)
  drop      Drop a case that is queued, active or in verification
  set       Change an open case's zone, risk or area, e.g. raise it to R3 before a step that can break boot
  snapshot  Record the snapper snapshot taken before the case's first red change as its rollback (checked, never refused)
  reopen    Reopen a completed case: a new active case "Reopen: <title>" with the same Intent
  list      List cases, optionally by status or area
  show      Print one case file with its path
  help      Print this message or the help of the given subcommand(s)

Options:
```
<!-- /help -->

### seldon plan new

Legt einen Case in `work/queued/` nach der Vorlage an. Vorgaben: Zone
yellow, Risiko R1, Priorität normal. Ein neuer Bereich bekommt seinen
Ordner unter `areas/`.

<!-- help: seldon plan new -->
```text
Create a case in work/queued/

Usage: seldon plan new [OPTIONS] <TITLE>

Arguments:
  <TITLE>  The case title, as one argument (after `--` when it starts with `-`)

Options:
      --zone <ZONE>          green, yellow or red [default: yellow]
      --risk <RISK>          R0 to R3 [default: R1]
      --area <AREA>          Area slug; created under areas/ on first use
      --priority <PRIORITY>  high, normal or low [default: normal]
      --actor <ACTOR>        Who creates the case: human or agent:NAME (default: $SELDON_ACTOR, else human)
```
<!-- /help -->

### seldon plan start

Macht einen geplanten Case aktiv und zum aktiven Case. Für einen roten
Case nimmst du vorher einen Snapshot und gibst seine Nummer mit
`--snapshot` an.

<!-- help: seldon plan start -->
```text
Start a case: queued → active; it becomes the active case

Usage: seldon plan start [OPTIONS] <ID>

Arguments:
  <ID>  The case id, e.g. C-2026-004

Options:
      --reason <TEXT>      Why, in one line; goes into the Log line and the event detail
      --actor <ACTOR>      Who takes the step: human or agent:NAME (default: $SELDON_ACTOR, else human)
      --snapshot <NUMBER>  Snapper snapshot number taken before the work, e.g. 42 (an R2 or R3 case started without one gets a warning, ADR-0023)
```
<!-- /help -->

### seldon plan verify

Schiebt einen aktiven Case in die Prüfung: Die Arbeit ist getan und
wartet auf deine Kontrolle.

<!-- help: seldon plan verify -->
```text
Hand an active case to verification: active → verification (runs a capture first)

Usage: seldon plan verify [OPTIONS] <ID>

Arguments:
  <ID>  The case id, e.g. C-2026-004

Options:
      --reason <TEXT>  Why, in one line; goes into the Log line and the event detail
      --actor <ACTOR>  Who takes the step: human or agent:NAME (default: $SELDON_ACTOR, else human)
      --no-capture     Do not run `seldon capture` before the step
```
<!-- /help -->

### seldon plan done

Schließt einen Case in Prüfung ab. Es schreibt eine Journal-Zeile und
leert den aktiven Case, wenn er diesen Case nannte. Der Abschluss eines
Agenten (`--actor agent:…` oder ohne die Option `SELDON_ACTOR`) wird
abgelehnt, solange *Result* des Case leer ist oder sein *Plan* keinen
Text unter `Verification:` hat; die Meldung sagt, was fehlt. Ein Case,
den ein Agent abgeschlossen hat, bekommt den Tag `closed-by-agent`. In
der Sitzung eines Agenten (`SELDON_ACTOR=agent:…`) wird `--actor human`
abgelehnt: Der Abschluss eines Agenten wird nie als der einer Person
aufgezeichnet.

<!-- help: seldon plan done -->
```text
Complete a verified case: verification → completed (runs a capture first)

Usage: seldon plan done [OPTIONS] <ID>

Arguments:
  <ID>  The case id, e.g. C-2026-004

Options:
      --reason <TEXT>  Why, in one line; goes into the Log line and the event detail
      --actor <ACTOR>  Who takes the step: human or agent:NAME (default: $SELDON_ACTOR, else human)
      --no-capture     Do not run `seldon capture` before the step
```
<!-- /help -->

### seldon plan drop

Gibt einen geplanten, aktiven oder geprüften Case auf. Sag mit
`--reason`, warum.

<!-- help: seldon plan drop -->
```text
Drop a case that is queued, active or in verification

Usage: seldon plan drop [OPTIONS] <ID>

Arguments:
  <ID>  The case id, e.g. C-2026-004

Options:
      --reason <TEXT>  Why, in one line; goes into the Log line and the event detail
      --actor <ACTOR>  Who takes the step: human or agent:NAME (default: $SELDON_ACTOR, else human)
```
<!-- /help -->

### seldon plan set

Ändert Zone, Risiko oder Bereich eines offenen Case, zum Beispiel
`seldon plan set C-2026-004 --risk R3` vor einem Schritt, der den Boot
brechen kann. Es schreibt eine *Log*-Zeile; ein Wert, den der Case schon
hat, ändert nichts. Ein abgeschlossener oder aufgegebener Case wird
abgelehnt.

<!-- help: seldon plan set -->
```text
Change an open case's zone, risk or area, e.g. raise it to R3 before a step that can break boot

Usage: seldon plan set [OPTIONS] <--zone <ZONE>|--risk <RISK>|--area <AREA>> <ID>

Arguments:
  <ID>  The case id, e.g. C-2026-004

Options:
      --zone <ZONE>    green, yellow or red
      --risk <RISK>    R0 to R3
      --area <AREA>    Area slug; created under areas/ on first use
      --actor <ACTOR>  Who changes it: human or agent:NAME (default: $SELDON_ACTOR, else human)
```
<!-- /help -->

### seldon plan snapshot

Hält den snapper-Snapshot, der vor der ersten roten Änderung des Case
entstand, als dessen Rollback fest (`snapshotBefore`), zum Beispiel
`seldon plan snapshot C-2026-004 42`. Es prüft, dass der Snapshot
existiert, nicht älter als der Start des Case und nicht neuer als seine
erste rote Änderung ist, und warnt, wenn nicht; es lehnt nie ab. Ein
Case behält seine erste Nummer: eine andere wird abgelehnt, dieselbe
noch einmal ändert nichts.

<!-- help: seldon plan snapshot -->
```text
Record the snapper snapshot taken before the case's first red change as its rollback (checked, never refused)

Usage: seldon plan snapshot [OPTIONS] <ID> <NUMBER>

Arguments:
  <ID>      The case id, e.g. C-2026-004
  <NUMBER>  The snapper snapshot number, e.g. 42 (`snapper create -p` prints it)

Options:
      --actor <ACTOR>  Who records it: human or agent:NAME (default: $SELDON_ACTOR, else human)
```
<!-- /help -->

### seldon plan reopen

Öffnet einen abgeschlossenen Case wieder: einen neuen aktiven Case
„Reopen: <Titel>“ mit derselben Zone, demselben Risiko, Bereich und
*Intent*, mit dem Tag `reopens:<ID>`. Der abgeschlossene Case bleibt
abgeschlossen und bekommt eine *Log*-Zeile. Jeder Aufruf legt einen
neuen Case an. Der neue Case wird nur dann der aktive Case, wenn kein
offener Case es ist; ein Agent, der an einem anderen Case arbeitet,
zeichnet weiter auf diesem auf.

<!-- help: seldon plan reopen -->
```text
Reopen a completed case: a new active case "Reopen: <title>" with the same Intent

Usage: seldon plan reopen [OPTIONS] <ID>

Arguments:
  <ID>  The completed case, e.g. C-2026-004

Options:
      --actor <ACTOR>  Who reopens it: human or agent:NAME (default: $SELDON_ACTOR, else human)
```
<!-- /help -->

### seldon plan list

Listet Cases: ID, Status, Zone, Risiko, Bereich und Titel. `--status`
nimmt `queued`, `active`, `verification`, `completed` oder `dropped`.
Eine Case-Datei, die sich nicht laden lässt, nennt eine `warning:`-Zeile
nach der Liste (`warnings` in `--json`); die übrigen Cases werden
trotzdem gelistet.

<!-- help: seldon plan list -->
```text
List cases, optionally by status or area

Usage: seldon plan list [OPTIONS]

Options:
      --status <STATUS>  Only cases with this status (queued, active, verification, completed, dropped)
      --area <AREA>      Only cases in this area
```
<!-- /help -->

### seldon plan show

Gibt den Pfad und die Datei eines Case aus, mit seinen Ereignissen.

<!-- help: seldon plan show -->
```text
Print one case file with its path

Usage: seldon plan show [OPTIONS] <ID>

Arguments:
  <ID>  The case id, e.g. C-2026-004

Options:
```
<!-- /help -->

### seldon decide

Legt `decisions/ADR-NNNN-<slug>.md` mit dem Status *proposed* an und
öffnet die Datei in deinem Editor; `--no-edit` lässt den Editor weg. Es
schreibt kein Ereignis ins Ledger. Eine Entscheidung mit dem Titel
„accept“ steht hinter `--` (`seldon decide -- accept`): ein bloßes
`accept` ist der Unterbefehl unten.

<!-- help: seldon decide -->
```text
Create a decision record (ADR) and open it in the editor; accept a proposed one

Usage: seldon decide [OPTIONS] <TITLE>
       seldon decide <COMMAND>

Commands:
  accept  Accept a proposed decision: status accepted, today's date, a ledger note. The user's act; an agent is refused
  help    Print this message or the help of the given subcommand(s)

Arguments:
  <TITLE>  The decision title, as one argument (a title `accept` goes after `--`)

Options:
      --case <ID>      The case this decision belongs to
      --no-edit        Do not open the editor
```
<!-- /help -->

### seldon decide accept

Nimmt eine vorgeschlagene Entscheidung an: ihr Frontmatter bekommt
`status: accepted` und das heutige Datum, das Ledger eine
`seldon`-Notiz (Subjekt die Entscheidungs-ID), `DECISIONS.md` und der
Index ziehen nach, und das Logbuch committet. Eine schon angenommene
Entscheidung bleibt, wie sie ist (Exit 0, `already` in `--json`); eine
ersetzte (*superseded*) wird abgelehnt. Annehmen ist deine Sache: ein
Agent (`--actor agent:…` oder `SELDON_ACTOR`) wird abgelehnt, ebenso
`--actor human` in der Sitzung eines Agenten (Exit 1). *Accept* im
Desk führt den Befehl aus.

<!-- help: seldon decide accept -->
```text
Accept a proposed decision: status accepted, today's date, a ledger note. The user's act; an agent is refused

Usage: seldon decide accept [OPTIONS] <ID>

Arguments:
  <ID>  The decision, ADR-NNNN

Options:
      --actor <ACTOR>  Who accepts it: human (the default); an agent is refused
```
<!-- /help -->

### seldon open

Gibt den Pfad einer Datei des Logbuchs aus. `case` ist der aktive Case,
`journal` das heutige Journal, `ledger` die Ledger-Ansicht des Monats,
`status` die `STATUS.md`, `logbook` der Ordner; eine Case- oder
Entscheidungs-ID nennt diese Datei. `--editor` öffnet sie: im Terminal
mit `$VISUAL` oder `$EDITOR`, aus dem Plugin mit Omarchys Editor-Starter.

<!-- help: seldon open -->
```text
Print the path of a logbook file; --editor opens it

Usage: seldon open [OPTIONS] <WHAT>

Arguments:
  <WHAT>  case (the active one), journal (today), ledger (this month), status, logbook, or a case or decision id

Options:
      --editor         Open it in the editor
```
<!-- /help -->

### seldon preview

Bevor du Seldon einrichtest: was diese Maschine von den letzten Tagen
selbst noch weiß — die Transaktionen von pacman und die unter `~/.config`
bearbeiteten Dateien, nur nach Änderungszeit. Kein Wer, kein Warum, und
weg, wenn die Logs rotieren. Braucht kein Logbuch und schreibt nichts.
`--days` schaut 1 bis 7 Tage zurück (Standard 7). Der Schreibtisch zeigt
es auf Heute, bis das Logbuch existiert.

<!-- help: seldon preview -->
```text
Before the logbook exists: the last days' pacman transactions and the files edited under ~/.config, read-only, nothing written

Usage: seldon preview [OPTIONS]

Options:
      --days <N>       Days to look back, 1 to 7 [default: 7]

Examples:
  seldon preview
  seldon preview --days 2 --json
```
<!-- /help -->

## Drift

### seldon drift

Listet offene Drift, Krisen zuerst: Zone, Zeit, Quelle und Art, Betreff
und Ereignis-ID. Es liest nur. Die Summen zählen jeden offenen Eintrag,
auch wenn die Liste gekürzt ist.

<!-- help: seldon drift -->
```text
List open drift; link, explain, dismiss or show a drift event; propose, apply or discard a triage proposal

Usage: seldon drift [OPTIONS]
       seldon drift <COMMAND>

Commands:
  link     Link a drift event, and the open members of its group, to a case
  explain  Explain a drift event with a new retroactive, completed case
  dismiss  Dismiss a drift event with a reason
  show     Show a drift event and every open member of its group
  propose  Store an agent's triage proposal (JSON on stdin) for the user to apply; every item needs evidence the engine can resolve
  apply    Apply a stored triage proposal as the user: link and explain its items; a crisis only when named by --item
  discard  Remove a stored triage proposal; the logbook is not touched
  help     Print this message or the help of the given subcommand(s)

Options:
      --crisis-only    Only crises (changes that can affect boot, login or the shell)
      --all            Also routine events (history, not drift), every item, uncapped
```
<!-- /help -->

### seldon drift link

Verknüpft das Ereignis und jedes offene Ereignis seiner Gruppe mit einem
Case. Der Case darf offen oder geschlossen sein. Danach sind die
Ereignisse keine Drift mehr.

<!-- help: seldon drift link -->
```text
Link a drift event, and the open members of its group, to a case

Usage: seldon drift link [OPTIONS] <EVENT> <CASE>

Arguments:
  <EVENT>  The drift event id, as `seldon drift` prints it
  <CASE>   The case id, e.g. C-2026-004 (a completed or dropped case too)

Options:
      --only           Resolve the named event only, not the rest of its group
      --actor <ACTOR>  Who resolves it: human or agent:NAME (default: $SELDON_ACTOR, else human)
```
<!-- /help -->

### seldon drift explain

Legt einen abgeschlossenen Case mit deinem Text als Titel an und
verknüpft das Ereignis und seine offene Gruppe damit. Der Case bekommt
die Zone des Drift-Eintrags, wenn du kein `--zone` angibst.

<!-- help: seldon drift explain -->
```text
Explain a drift event with a new retroactive, completed case

Usage: seldon drift explain [OPTIONS] <EVENT> <INTENT>

Arguments:
  <EVENT>   The drift event id, as `seldon drift` prints it
  <INTENT>  Why it happened, as one argument after `--`; the new case's title

Options:
      --only           Resolve the named event only, not the rest of its group
      --zone <ZONE>    Zone of the new case (default: the drift item's zone)
      --risk <RISK>    Risk of the new case [default: R1]
      --area <AREA>    Area slug of the new case; created under areas/ on first use
      --actor <ACTOR>  Who resolves it: human or agent:NAME (default: $SELDON_ACTOR, else human)

Example:
  seldon drift explain <EVENT> --area hardware -- "Driver for the new GPU"
```
<!-- /help -->

### seldon drift dismiss

Markiert das Ereignis und seine offene Gruppe als nicht case-würdig, mit
deinem Grund.

<!-- help: seldon drift dismiss -->
```text
Dismiss a drift event with a reason

Usage: seldon drift dismiss [OPTIONS] <EVENT> <REASON>

Arguments:
  <EVENT>   The drift event id, as `seldon drift` prints it
  <REASON>  Why it can be ignored, as one argument after `--`

Options:
      --only           Resolve the named event only, not the rest of its group
      --actor <ACTOR>  Who resolves it: human or agent:NAME (default: $SELDON_ACTOR, else human)

Example:
  seldon drift dismiss <EVENT> -- "Tried a theme, reverted it"
```
<!-- /help -->

### seldon drift show

Zeigt ein Drift-Ereignis mit jedem offenen Mitglied seiner Gruppe. Nimm
es, bevor du eine große Gruppe auflöst.

<!-- help: seldon drift show -->
```text
Show a drift event and every open member of its group

Usage: seldon drift show [OPTIONS] <EVENT>

Arguments:
  <EVENT>  The drift event id, as `seldon drift` prints it

Options:
```
<!-- /help -->

Alle drei auflösenden Befehle schreiben in einem Schritt eine Auflösung
pro offenem Mitglied der Gruppe. `--only` löst nur das genannte Ereignis
auf. Derselbe Befehl noch einmal schreibt nichts und endet mit 0.

### seldon drift propose

Der Befehl des Agenten (ADR-0036): legt einen Triage-Vorschlag ab, den
du anwendest, aus JSON auf stdin. Jeder Eintrag nennt eine offene
Änderung, eine Verknüpfung mit einem Case oder eine Erklärung (Titel und
Absicht) und Belege, die die Engine selbst nachschlägt: eine
Journal-Uhrzeit, eine Ereignis-Id, eine Snapshot-Nummer, einen Case oder
einen Case, dessen *Plan* die Änderung nennt. Ein Eintrag, der nicht
standhält, lehnt den ganzen Vorschlag ab und wird genannt. Ein neuer
Vorschlag ersetzt den früheren. Eine Person wird abgelehnt: Verknüpfe
oder erkläre direkt.

<!-- help: seldon drift propose -->
```text
Store an agent's triage proposal (JSON on stdin) for the user to apply; every item needs evidence the engine can resolve

Usage: seldon drift propose [OPTIONS]

Options:
      --file <FILE>    Read the proposal from FILE instead of stdin
      --actor <ACTOR>  Who proposes: agent:NAME (default: $SELDON_ACTOR)

Input (stdin):
  {"items": [{"eventId": "<EVENT>", "action": "link", "caseId": "<CASE>",
              "evidence": [{"kind": "plan", "ref": "<CASE>"}]}]}
  action explain takes "title" and "intent" instead of "caseId"; evidence kinds:
  journal (YYYY-MM-DD HH:MM), event (<EVENT>), snapshot (<N>), case (<CASE>),
  plan (<CASE>: a line of its Plan that names the change)
```
<!-- /help -->

### seldon drift apply

Wendet einen Vorschlag als du (`human`) an: Jeder Eintrag wird noch
einmal gegen das Logbuch geprüft und dann wie mit `drift link` und
`drift explain` verknüpft oder erklärt, mit dem Auflösungsdetail
`proposed by agent:<name> — <Belege>`. Bereits aufgelöste Einträge
werden übersprungen; eine Krise wird nur angewendet, wenn du sie mit
`--item` nennst. Noch einmal ausgeführt ändert er nichts. Ein Agent wird
abgelehnt.

<!-- help: seldon drift apply -->
```text
Apply a stored triage proposal as the user: link and explain its items; a crisis only when named by --item

Usage: seldon drift apply [OPTIONS] <PROPOSAL>

Arguments:
  <PROPOSAL>  The proposal id, as `seldon drift propose` and index.json's triage name it

Options:
      --item <EVENT>   Apply only this item (repeatable); the only way to apply a crisis
      --actor <ACTOR>  Who applies it: human (default: $SELDON_ACTOR, else human); an agent is refused
```
<!-- /help -->

### seldon drift discard

Entfernt einen Vorschlag, ohne ihn anzuwenden. Das Logbuch bleibt
unberührt.

<!-- help: seldon drift discard -->
```text
Remove a stored triage proposal; the logbook is not touched

Usage: seldon drift discard [OPTIONS] <PROPOSAL>

Arguments:
  <PROPOSAL>  The proposal id

Options:
      --actor <ACTOR>  Who discards it: human (default: $SELDON_ACTOR, else human); an agent is refused
```
<!-- /help -->

## Agenten und Hooks

### seldon agent

<!-- help: seldon agent -->
```text
Start an agent on an active case, or ask one about the open changes, a change or a case

Usage: seldon agent [OPTIONS] <COMMAND>

Commands:
  start     Launch an agent on an active case, with the case as the active case and a prompt that names the case and the logbook; with --new, create and start the case from one sentence first
  ask       Ask an agent about the open changes, one change or one case: the prompt holds ids only and names the skill's guide; nothing in the logbook changes (ADR-0036)
  focus     Bring the window of the agent `agent start` launched on a case to the front (Hyprland)
  sessions  List the agents `agent start` launched whose window is open, one per case (Hyprland)
  help      Print this message or the help of the given subcommand(s)

Options:
```
<!-- /help -->

### seldon agent start

Macht den Case zum aktiven Case und startet einen Agenten dort, wo
`omarchy agent prompt` es täte: im aktuellen Ordner, oder in `~/Work`
(dein Home, wenn es keins gibt), wenn der aktuelle Ordner dein Home oder
`/` ist; `[agent] workdir = "logbook"` startet ihn im Ordner des
Logbuchs. Der erste Prompt nennt den Case und das Logbuch, verweist auf
den Skill `seldon` und sagt dem Agenten, `seldon hook session-start` und
`seldon plan show <ID>` auszuführen; er enthält keinen Text aus dem
Logbuch. Der Case muss aktiv sein. Mit `--new -- "<was zu tun ist>"` legt es zuerst aus diesem
Satz einen Case an und startet ihn (Titel: sein erster Satz, höchstens
72 Zeichen; *Intent*: der ganze Text; `--zone`, `--risk`, `--area` wie
bei `plan new`). Ohne Standard-Agenten in Omarchy und mit dem
eingebauten Launcher wird nichts angelegt; die Meldung nennt
`omarchy default agent <name>`. Der Launcher kommt aus `config.toml`; siehe
[Konfiguration](06-configuration.md#agent-launcher). Der Agent läuft mit
`SELDON_ACTOR=agent:<Name des Launchers>`, `SELDON_ATTENDED=1` und
`SELDON_CASE=<ID>` ([Umgebungsvariablen](#umgebungsvariablen)); ein
Launcher-Name ohne
ASCII-Buchstaben oder -Ziffer wird abgelehnt.

<!-- help: seldon agent start -->
```text
Launch an agent on an active case, with the case as the active case and a prompt that names the case and the logbook; with --new, create and start the case from one sentence first

Usage: seldon agent start [OPTIONS] [ID] [-- <INTENT>]

Arguments:
  [ID]      The case (must be active)
  [INTENT]  With --new: what the agent should do, as one argument after `--`

Options:
      --new              Create and start a case from the text after `--` (title: its first sentence; Intent: the whole text), then launch the agent on it
      --zone <ZONE>      With --new: green, yellow or red [default: yellow]
      --risk <RISK>      With --new: R0 to R3 [default: R1]
      --area <AREA>      With --new: area slug; created under areas/ on first use
      --launcher <NAME>  A launcher from `[agent.launchers]` in config.toml; `omarchy` is the built-in one (default: `[agent] launcher`)
      --again            Start another agent although one already works on the case

Examples:
  seldon agent start C-2026-004
  seldon agent start --new -- "Install zed as a second editor"
```
<!-- /help -->

### seldon agent ask

Startet einen Agenten wie `agent start`, damit er etwas ansieht und dir
antwortet: `triage` sortiert die offenen Änderungen zu einem Vorschlag
(`drift propose`), `drift <EVENT>` sieht eine offene Änderung an,
`case <ID>` einen Case. Der Prompt nennt das Logbuch, die Id und die
Anleitung des Skills (`triage.md`, `drift.md`, `case.md`); er enthält
keinen Text aus dem Logbuch. Der Agent bekommt keinen Case zum
Bearbeiten (kein `SELDON_CASE`), und der aktive Case bleibt, wie er ist.
Nichts startet ohne Omarchy-Standard-Agent (mit dem eingebauten
Launcher), ohne installierten Skill `seldon` oder wenn `triage` nichts
Offenes findet; die Meldung nennt die Lösung.

<!-- help: seldon agent ask -->
```text
Ask an agent about the open changes, one change or one case: the prompt holds ids only and names the skill's guide; nothing in the logbook changes (ADR-0036)

Usage: seldon agent ask [OPTIONS] <COMMAND>

Commands:
  triage  Sort the open changes: the agent stores a proposal with evidence (`seldon drift propose`) for the user to apply
  drift   One open change (attention or crisis)
  case    One case, any status
  help    Print this message or the help of the given subcommand(s)

Options:
      --launcher <NAME>  A launcher from `[agent.launchers]` in config.toml; `omarchy` is the built-in one (default: `[agent] launcher`)

Examples:
  seldon agent ask triage
  seldon agent ask drift 01M3VNJ9JGZ9169T01XCW16FT0
  seldon agent ask case C-2026-004
```
<!-- /help -->

<!-- help: seldon agent ask triage -->
```text
Sort the open changes: the agent stores a proposal with evidence (`seldon drift propose`) for the user to apply

Usage: seldon agent ask triage [OPTIONS]

Options:
      --launcher <NAME>  A launcher from `[agent.launchers]` in config.toml; `omarchy` is the built-in one (default: `[agent] launcher`)
```
<!-- /help -->

<!-- help: seldon agent ask drift -->
```text
One open change (attention or crisis)

Usage: seldon agent ask drift [OPTIONS] <EVENT>

Arguments:
  <EVENT>  The drift event id, as `seldon drift` prints it

Options:
      --launcher <NAME>  A launcher from `[agent.launchers]` in config.toml; `omarchy` is the built-in one (default: `[agent] launcher`)
```
<!-- /help -->

<!-- help: seldon agent ask case -->
```text
One case, any status

Usage: seldon agent ask case [OPTIONS] <ID>

Arguments:
  <ID>  The case id

Options:
      --launcher <NAME>  A launcher from `[agent.launchers]` in config.toml; `omarchy` is the built-in one (default: `[agent] launcher`)
```
<!-- /help -->

**Ein Agent pro Case.** `agent start <ID>` lehnt ab, solange das Fenster
eines Agenten, den es auf dem Case gestartet hat, offen ist (oder 10
Sekunden nach einem Start, während das Fenster aufgeht), und nennt
`seldon agent focus <ID>`; `--again` startet trotzdem einen weiteren.
`agent focus` holt dieses Fenster nach vorn; `agent sessions` listet die
offenen, eines pro Case. Die Engine schaut auf die Agent-Fenster, die
Hyprland listet, und erkennt den Case an der Umgebung ihrer Prozesse
(`SELDON_CASE`, `SELDON_LOGBOOK`); wer das Fenster schließt, gibt den Case
frei. Ohne Hyprland wird nichts verfolgt und nichts abgelehnt.

<!-- help: seldon agent focus -->
```text
Bring the window of the agent `agent start` launched on a case to the front (Hyprland)

Usage: seldon agent focus [OPTIONS] <ID>

Arguments:
  <ID>  The case

Options:

Example:
  seldon agent focus C-2026-004
```
<!-- /help -->

<!-- help: seldon agent sessions -->
```text
List the agents `agent start` launched whose window is open, one per case (Hyprland)

Usage: seldon agent sessions [OPTIONS]

Options:
```
<!-- /help -->

### seldon rules

Die Agentenregeln in der `AGENTS.md` des Logbuchs.

<!-- help: seldon rules -->
```text
The agent rules in the logbook's AGENTS.md: update

Usage: seldon rules [OPTIONS] <COMMAND>

Commands:
  update  Bring the rules block of AGENTS.md up to this release; text outside it is kept
  help    Print this message or the help of the given subcommand(s)

Options:
```
<!-- /help -->

### seldon rules update

Bringt Seldons Block in `AGENTS.md`, zwischen den Zeilen
`<!-- seldon:begin rules v3 -->` und `<!-- seldon:end -->`, auf die
Regeln dieser Version und behält den Rest der Datei; einen Block, den
du bearbeitet hast, archiviert er zuerst. Eine Datei aus einer früheren
Version hat keinen Block: Ist sie nicht genau die Datei, die jene
Version geschrieben hat, wird sie nach `archive/AGENTS-<datum>.md`
archiviert, und die neuen Regeln werden geschrieben, darunter nur die
Zeilen, die du ergänzt hast, unter `## Your rules (kept)`. `--replace`
archiviert die ganze alte Datei und schreibt nur die neuen Regeln. Der Befehl gibt die Änderung als Diff aus
und committet sie als `seldon: rules update`; ein zweiter Aufruf ändert
nichts. Einen beschädigten Block oder einen aus einer neueren
Seldon-Version weist er ab (Exit 1) und lässt die Datei, wie sie ist.
Ein Block, den niemand bearbeitet hat, braucht keinen Befehl: Jede
Erfassung bringt ihn auf den neuen Stand. `seldon doctor` nennt diesen
Befehl, wenn die Regeln nicht aktuell sind;
siehe
[Mit Agenten arbeiten](04-working-with-agents.md#die-regeln-eines-älteren-logbuchs-erneuern).

<!-- help: seldon rules update -->
```text
Bring the rules block of AGENTS.md up to this release; text outside it is kept

Usage: seldon rules update [OPTIONS]

Options:
      --replace        Archive the whole file to archive/AGENTS-<date>.md and write the template

Examples:
  seldon rules update
  seldon rules update --replace
```
<!-- /help -->

### seldon hook

Die Befehle, die Agenten und ihre Harnesses aufrufen. Hooks geben nichts
aus und enden immer mit 0, sie brechen also nie einen Agenten. `install`
und `uninstall` sind die Ausnahmen: Du rufst sie selbst auf, und sie
berichten wie jeder andere Befehl.

<!-- help: seldon hook -->
```text
Agent hooks: record commands, print session context, install into or uninstall from a harness

Usage: seldon hook [OPTIONS] <COMMAND>

Commands:
  install        Merge Seldon's hooks into an agent harness's settings; `skills`: put the Seldon agent skill into every agent skill folder that exists
  uninstall      Remove Seldon's hooks from an agent harness's settings, keeping the rest; `skills`: remove the Seldon agent skill, keeping files changed by hand
  claude-code    Record a Claude Code tool call (hook payload on stdin; silent, exit 0)
  generic        Record any agent's command ({"command","actor"?,"cwd","startedAt"?,"case"?} on stdin; without "actor", $SELDON_ACTOR)
  session-start  Print the context block an agent session starts with
  session-stop   End a session: journal stub, capture, commit (silent, exit 0)
  help           Print this message or the help of the given subcommand(s)

Options:
```
<!-- /help -->

### seldon hook install

Fügt Seldons drei Hooks in die Einstellungen von Claude Code ein,
standardmäßig in die nutzerweite `~/.claude/settings.json`
(`$CLAUDE_CONFIG_DIR/settings.json`, wenn das gesetzt ist); ein Logbuch
braucht es nicht. Vorhandene Hooks und Einstellungen bleiben. Ein zweiter
Aufruf ändert nichts. Die letzte Zeile sagt, welche Sitzungen Seldon
aufzeichnet: die im Logbuch-Ordner und die, die `seldon agent start`
gestartet hat; jede andere nur mit `[hooks] scope = "all"`, was die Zeile
zur Warnung macht.

`seldon hook install skills` legt den Seldon-Agentenskill in jeden
Skill-Ordner eines Agenten, den es gibt: `~/.agents/skills`,
`~/.claude/skills`, `~/.codex/skills`, `~/.pi/agent/skills`,
`~/.hermes/skills` und `~/.hermes/profiles/*/skills`, die Ordner, in die
Omarchy seine eigenen Skills verlinkt. Es legt keinen davon an. Der Skill
kommt nach `<Ordner>/seldon/`; ein `seldon` dort, das Seldon nicht
geschrieben hat, oder eine Datei darin, die du geändert hast, bleibt, wie
es ist, und der Bericht nennt es. Ein älterer Skill wird aktualisiert
(das tut eine Erfassung von selbst, wo du nichts geändert hast).
`--replace` ersetzt auch einen Skill, den du geändert hast: Deine
geänderten Dateien werden zuerst nach `archive/skill-<datum>/` im
Logbuch kopiert.
Ein Ordner, in den es nicht schreiben kann, scheitert allein: Die anderen
werden trotzdem installiert, der Bericht nennt den gescheiterten, und der
Befehl endet mit 1. Ein zweiter Aufruf ändert nichts. Siehe
[Mit Agenten arbeiten](04-working-with-agents.md#der-agentenskill).

<!-- help: seldon hook install -->
```text
Merge Seldon's hooks into an agent harness's settings; `skills`: put the Seldon agent skill into every agent skill folder that exists

Usage: seldon hook install [OPTIONS] <HARNESS>

Arguments:
  <HARNESS>  The harness [possible values: claude-code, skills]

Options:
      --settings <FILE>  Settings file (default: the user-wide $CLAUDE_CONFIG_DIR/settings.json, else ~/.claude/settings.json; claude-code only)
      --replace          skills only: where you changed the skill, archive your copy to the logbook's archive/ and install it as shipped
```
<!-- /help -->

### seldon hook uninstall

Nimmt Seldons drei Hooks wieder aus den Einstellungen von Claude Code
(dieselbe Standarddatei wie `install`) und lässt alles andere stehen, auch einen Hook, den du neben einen von Seldons
gesetzt hast. Eine Datei, die nur Seldons Hooks enthielt, wird gelöscht.
Ein zweiter Aufruf ändert nichts. Das nächste `seldon capture` meldet die
Änderung nicht als Drift.

`seldon hook uninstall skills` entfernt die Dateien des Skills aus jedem
Skill-Ordner. Eine Datei, die du geändert hast, bleibt, ebenso eine, die
du hinzugefügt hast; einen Ordner `seldon`, den Seldon nicht geschrieben
hat, rührt es nicht an.

<!-- help: seldon hook uninstall -->
```text
Remove Seldon's hooks from an agent harness's settings, keeping the rest; `skills`: remove the Seldon agent skill, keeping files changed by hand

Usage: seldon hook uninstall [OPTIONS] <HARNESS>

Arguments:
  <HARNESS>  The harness [possible values: claude-code, skills]

Options:
      --settings <FILE>  Settings file (default: the user-wide $CLAUDE_CONFIG_DIR/settings.json, else ~/.claude/settings.json; claude-code only)
```
<!-- /help -->

### seldon hook claude-code

Der Hook `PreToolUse` von Claude Code ruft das mit dem Werkzeugaufruf
als JSON auf der Standardeingabe auf. Du führst es nicht selbst aus.

<!-- help: seldon hook claude-code -->
```text
Record a Claude Code tool call (hook payload on stdin; silent, exit 0)

Usage: seldon hook claude-code [OPTIONS]

Options:
```
<!-- /help -->

### seldon hook generic

Dasselbe für jeden anderen Agenten:
`{"command": "…", "actor": "agent:<name>", "cwd": "…"}` auf der
Standardeingabe. Siehe
[Mit Agenten arbeiten](04-working-with-agents.md#andere-agenten).

<!-- help: seldon hook generic -->
```text
Record any agent's command ({"command","actor"?,"cwd","startedAt"?,"case"?} on stdin; without "actor", $SELDON_ACTOR)

Usage: seldon hook generic [OPTIONS]

Options:
      --case <ID>      The case the command belongs to (default: `.seldon/active-case`)
```
<!-- /help -->

### seldon hook session-start

Gibt den Kontext aus, mit dem eine Agenten-Sitzung beginnt: Status, den
aktiven Case mit seinem Plan, die letzten Journal-Zeilen und die
Überschriften der Lektionen.

<!-- help: seldon hook session-start -->
```text
Print the context block an agent session starts with

Usage: seldon hook session-start [OPTIONS]

Options:
```
<!-- /help -->

### seldon hook session-stop

Beendet eine Sitzung: eine Journal-Zeile mit der Zahl der Ereignisse, die
die Sitzung aufgezeichnet hat, eine Erfassung, `STATUS.md`, der Index und
ein Commit.

<!-- help: seldon hook session-stop -->
```text
End a session: journal stub, capture, commit (silent, exit 0)

Usage: seldon hook session-stop [OPTIONS]

Options:
      --actor <ACTOR>  Who ends the session: human or agent:NAME [default: agent:claude-code]
```
<!-- /help -->

## Ausgaben und Import

### seldon rebuild

Schreibt `outputs/REBUILD.md`, die Schritte, die eine frische
Omarchy-Installation zu dieser Maschine machen. Siehe
[Wiederaufbau, Dossier und Update-Folgen](08-rebuild-dossier-update-impact.md).

<!-- help: seldon rebuild -->
```text
Write outputs/REBUILD.md: the steps to rebuild this machine

Usage: seldon rebuild [OPTIONS]

Options:
```
<!-- /help -->

### seldon dossier

Frischt die erzeugten Teile von `system/*.md` aus lesenden Abfragen auf.
Text außerhalb der erzeugten Teile ändert es nie. Es schreibt kein
Ereignis ins Ledger.

<!-- help: seldon dossier -->
```text
Refresh the generated fences of system/*.md from read-only queries

Usage: seldon dossier [OPTIONS]

Options:
      --section <SECTION>  Sections to write, comma-separated or repeated (default: all) [possible values: packages, services, omarchy, hardware, plugins, deviations, all]
```
<!-- /help -->

### seldon import

<!-- help: seldon import -->
```text
Import an earlier logbook (dry run unless --apply) or your Markdown task files as cases

Usage: seldon import [OPTIONS] <COMMAND>

Commands:
  omarchy-agent  Import the omarchy-agent kit's Obsidian vault, which is only read (dry run unless --apply)
  task           Import your Markdown task files as cases: one queued case per open `- [ ]` item (applies unless --dry-run; the files are only read)
  help           Print this message or the help of the given subcommand(s)

Options:
```
<!-- /help -->

### seldon import omarchy-agent

Liest einen Vault des omarchy-agent-Kits und schreibt einen Bericht. Mit
`--apply` importiert es Cases, Journal, Memory und Abweichungen in einem
Commit. Siehe [Import aus omarchy-agent](09-import-from-omarchy-agent.md).

<!-- help: seldon import omarchy-agent -->
```text
Import the omarchy-agent kit's Obsidian vault, which is only read (dry run unless --apply)

Usage: seldon import omarchy-agent [OPTIONS] <VAULT>

Arguments:
  <VAULT>  The vault directory (the one with pipeline/, journal/, knowledge/)

Options:
      --dry-run        Only write the report outputs/IMPORT-omarchy-agent.md (the default)
      --apply          Import: cases, journal, memory and deviation rows, in one commit
```
<!-- /help -->

### seldon import task

Macht aus deinen eigenen Markdown-Aufgabendateien Cases: ein Case in
queued pro offenem `- [ ]`-Punkt, oder ein Case für eine Datei ohne
Checkboxen. Importiert sofort, außer mit `--dry-run`; die Dateien werden
nur gelesen; ein zweiter Lauf überspringt, was schon importiert ist.
Siehe [Aufgabendateien](09-import-from-omarchy-agent.md#aufgabendateien).

<!-- help: seldon import task -->
```text
Import your Markdown task files as cases: one queued case per open `- [ ]` item (applies unless --dry-run; the files are only read)

Usage: seldon import task [OPTIONS] <FILE>...

Arguments:
  <FILE>...  Markdown task files under your home: one case per open `- [ ]` item; a file without checklist items is one case

Options:
      --area <AREA>    Area slug of the new cases; created under areas/ on first use
      --zone <ZONE>    green, yellow or red [default: yellow]
      --risk <RISK>    R0 to R3 [default: R1]
      --include-done   Also import `- [x]` items, as completed cases
      --dry-run        List what would be created; write nothing
      --actor <ACTOR>  Who imports: human or agent:NAME (default: $SELDON_ACTOR, else human)
```
<!-- /help -->

## Shell-Vervollständigung und Manpage

Das Paket und der Installer legen beides für dich ab. Diese Befehle geben
sie aus, für eine selbst gebaute Engine oder einen anderen Ort.

### seldon completions

Gibt das Vervollständigungs-Skript für bash, zsh oder fish aus. Sobald es
dort liegt, wo deine Shell sucht, ergänzt Tab Befehle, Optionen und ihre
Werte.

<!-- help: seldon completions -->
```text
Print a shell completion script for bash, zsh or fish

Usage: seldon completions [OPTIONS] <SHELL>

Arguments:
  <SHELL>  The shell [possible values: bash, zsh, fish]

Options:

Examples:
  seldon completions bash > ~/.local/share/bash-completion/completions/seldon
  seldon completions zsh > ~/.local/share/zsh/site-functions/_seldon
  seldon completions fish > ~/.config/fish/completions/seldon.fish
```
<!-- /help -->

### seldon mangen

Gibt die Manpage seldon(1) aus: jeder Befehl mit seinen Optionen, die
Exit-Codes, die Umgebungsvariablen und die Dateien.

<!-- help: seldon mangen -->
```text
Print the man page seldon(1), generated from this help

Usage: seldon mangen [OPTIONS]

Options:

Example:
  seldon mangen > seldon.1 && man -l seldon.1
```
<!-- /help -->

---

Zurück: [Mit Agenten arbeiten](04-working-with-agents.md) · [Übersicht](README.md) · Weiter: [Konfiguration](06-configuration.md)
