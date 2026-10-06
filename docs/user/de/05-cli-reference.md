# Befehlsreferenz

<!-- source: en/05-cli-reference.md @ 79bfb0e -->

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
  doctor            Check engine, config, logbook, collector state, omarchy, snapper and git
  capture           Run collectors and append new events to the ledger
  log               Write a note: a ledger event and a journal entry
  event             Record an event by hand (hooks, scripts)
  plan              Plan and track cases: new, start, verify, done, drop, list, show
  decide            Create a decision record (ADR) and open it in the editor
  open              Print the path of a logbook file; --editor opens it
  index             Rebuild index.json and the ledger/*.md views; --check validates
  status            Regenerate STATUS.md, the ledger views and index.json; print a summary
  hook              Agent hooks: record commands, print session context, install into or uninstall from a harness
  drift             List open drift; link, explain, dismiss or show a drift event
  agent             Start an agent on an active case
  rebuild           Write outputs/REBUILD.md: the steps to rebuild this machine
  watch             Rebuild index.json when the logbook changes (feature "watch")
  dossier           Refresh the generated fences of system/*.md from read-only queries
  import            Import an earlier logbook (dry run unless --apply)
  rules             The agent rules in the logbook's AGENTS.md: update
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
      --harness <NAME>       Agent harness to set up (repeatable) [possible values: claude-code, omarchy-agent]
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
Zustandsdateien, Omarchy, Snapper und git. Es liest nur. Jede Zeile sagt
`ok`, `degraded` oder `error`, und eine fehlerhafte Prüfung nennt den
Befehl, der sie behebt. Exit 0, wenn nichts ein Fehler ist, 1, wenn eine
Prüfung ein Fehler ist (auch, wenn `config.toml` nicht gelesen oder
geparst werden kann), 3, wenn das Logbuch nicht angelegt ist.

<!-- help: seldon doctor -->
```text
Check engine, config, logbook, collector state, omarchy, snapper and git

Usage: seldon doctor [OPTIONS]

Options:
      --path <DIR>     Logbook to check (same as the global --logbook)
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
  new     Create a case in work/queued/
  start   Start a case: queued → active; it becomes the active case
  verify  Hand an active case to verification: active → verification
  done    Complete a verified case: verification → completed
  drop    Drop a case that is queued, active or in verification
  list    List cases, optionally by status or area
  show    Print one case file with its path
  help    Print this message or the help of the given subcommand(s)

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
Hand an active case to verification: active → verification

Usage: seldon plan verify [OPTIONS] <ID>

Arguments:
  <ID>  The case id, e.g. C-2026-004

Options:
      --reason <TEXT>  Why, in one line; goes into the Log line and the event detail
      --actor <ACTOR>  Who takes the step: human or agent:NAME (default: $SELDON_ACTOR, else human)
```
<!-- /help -->

### seldon plan done

Schließt einen Case in Prüfung ab. Es schreibt eine Journal-Zeile und
leert den aktiven Case, wenn er diesen Case nannte.

<!-- help: seldon plan done -->
```text
Complete a verified case: verification → completed

Usage: seldon plan done [OPTIONS] <ID>

Arguments:
  <ID>  The case id, e.g. C-2026-004

Options:
      --reason <TEXT>  Why, in one line; goes into the Log line and the event detail
      --actor <ACTOR>  Who takes the step: human or agent:NAME (default: $SELDON_ACTOR, else human)
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
schreibt kein Ereignis ins Ledger.

<!-- help: seldon decide -->
```text
Create a decision record (ADR) and open it in the editor

Usage: seldon decide [OPTIONS] <TITLE>

Arguments:
  <TITLE>  The decision title, as one argument

Options:
      --case <ID>      The case this decision belongs to
      --no-edit        Do not open the editor
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

## Drift

### seldon drift

Listet offene Drift, Krisen zuerst: Zone, Zeit, Quelle und Art, Betreff
und Ereignis-ID. Es liest nur. Die Summen zählen jeden offenen Eintrag,
auch wenn die Liste gekürzt ist.

<!-- help: seldon drift -->
```text
List open drift; link, explain, dismiss or show a drift event

Usage: seldon drift [OPTIONS]
       seldon drift <COMMAND>

Commands:
  link     Link a drift event, and the open members of its group, to a case
  explain  Explain a drift event with a new retroactive, completed case
  dismiss  Dismiss a drift event with a reason
  show     Show a drift event and every open member of its group
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

## Agenten und Hooks

### seldon agent

<!-- help: seldon agent -->
```text
Start an agent on an active case

Usage: seldon agent [OPTIONS] <COMMAND>

Commands:
  start  Launch an agent on an active case, with the case as the active case and a prompt that names the case and the logbook
  help   Print this message or the help of the given subcommand(s)

Options:
```
<!-- /help -->

### seldon agent start

Macht den Case zum aktiven Case und startet einen Agenten im Ordner des
Logbuchs. Der erste Prompt nennt den Case und das Logbuch und sagt dem
Agenten, `seldon hook session-start` und `seldon plan show <ID>`
auszuführen; er enthält keinen Text aus dem Logbuch. Der Case muss
aktiv sein. Der Launcher kommt aus `config.toml`; siehe
[Konfiguration](06-configuration.md#agent-launcher). Der Agent läuft mit
`SELDON_ACTOR=agent:<Name des Launchers>` und `SELDON_ATTENDED=1`
([Umgebungsvariablen](#umgebungsvariablen)); ein Launcher-Name ohne
ASCII-Buchstaben oder -Ziffer wird abgelehnt.

<!-- help: seldon agent start -->
```text
Launch an agent on an active case, with the case as the active case and a prompt that names the case and the logbook

Usage: seldon agent start [OPTIONS] <ID>

Arguments:
  <ID>  The case (must be active)

Options:
      --launcher <NAME>  A launcher from `[agent.launchers]` in config.toml; `omarchy` is the built-in one (default: `[agent] launcher`)
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
`<!-- seldon:begin rules v2 -->` und `<!-- seldon:end -->`, auf die
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
`seldon doctor` nennt diesen Befehl, wenn die Regeln nicht aktuell sind;
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
  install        Merge Seldon's hooks into an agent harness's settings
  uninstall      Remove Seldon's hooks from an agent harness's settings, keeping the rest
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
standardmäßig in `.claude/settings.json` des Logbuchs. Vorhandene Hooks
bleiben. Ein zweiter Aufruf ändert nichts.

<!-- help: seldon hook install -->
```text
Merge Seldon's hooks into an agent harness's settings

Usage: seldon hook install [OPTIONS] <HARNESS>

Arguments:
  <HARNESS>  The harness [possible values: claude-code]

Options:
      --settings <FILE>  Settings file (default: <logbook>/.claude/settings.json)
```
<!-- /help -->

### seldon hook uninstall

Nimmt Seldons drei Hooks wieder aus den Einstellungen von Claude Code und
lässt alles andere stehen, auch einen Hook, den du neben einen von Seldons
gesetzt hast. Eine Datei, die nur Seldons Hooks enthielt, wird gelöscht.
Ein zweiter Aufruf ändert nichts. Das nächste `seldon capture` meldet die
Änderung nicht als Drift.

<!-- help: seldon hook uninstall -->
```text
Remove Seldon's hooks from an agent harness's settings, keeping the rest

Usage: seldon hook uninstall [OPTIONS] <HARNESS>

Arguments:
  <HARNESS>  The harness [possible values: claude-code]

Options:
      --settings <FILE>  Settings file (default: <logbook>/.claude/settings.json)
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
Import an earlier logbook (dry run unless --apply)

Usage: seldon import [OPTIONS] <COMMAND>

Commands:
  omarchy-agent  Import the omarchy-agent kit's Obsidian vault, which is only read (dry run unless --apply)
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
