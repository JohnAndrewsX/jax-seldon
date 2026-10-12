# Alltag

<!-- source: en/03-daily-use.md @ e0df9cca -->

Diese Seite behandelt die Teile von Seldon, die du jeden Tag siehst: die
Pill in der Bar und den Desk, den sie öffnet, mit seinen neun
Abschnitten, seinen Tasten und seiner Breite. Alles hier geht mit der
Maus und mit der Tastatur. Die Beschriftungen der Oberfläche sind in
dieser Version englisch; diese Seite nennt sie so, wie du sie siehst.

## Ein Tag mit Seldon

- Morgens ein Blick auf die Pill. Das Zeichen, dann `2 · 1`, heißt: zwei
  aktive Cases und eine Krise. Ohne Krise fällt die zweite Zahl weg.
- Vor einer Änderung legst du einen Case an (`+` auf dem Desk) und
  startest ihn, oder du schreibst einen Satz in *New case* unter Today
  und lässt es einen Agenten tun.
- Während der Arbeit schreibst du eine Notiz, wenn du etwas lernst (`n`
  auf dem Desk).
- Nimmt die Pill die Fehlerfarbe deines Themes an, öffnest du den Desk.
  Today listet die Krise unter NEEDS YOU. Nur darauf sollst du schauen.
  Andere Änderungen ohne Case warten leise im Changelog; lös sie auf,
  wann du willst, oder lass sie von einem Agenten sortieren.
- Am Ende eines Case gibst du ihn in Work in die Prüfung und schließt
  ihn ab.
- Einmal pro Woche öffnest du den Prime Radiant und schaust dir das Bild
  an.

## Die Pill

Die Pill sitzt rechts in der Bar.

- Das Seldon-Zeichen, dann `A · D`: A ist die Zahl der aktiven Cases, D
  die Zahl der Krisen. Teile, die null sind, fallen weg: das Zeichen
  allein, `2`, `· 1`. Das Zeichen nimmt die Farbe der Pill. Änderungen
  ohne Case, die keine Krise sind, zählt sie nicht: Der Kopf des Desks
  und der Tooltip zählen sie (*attention*, *without a case*). Die
  Einstellung `driftInBar` auf `all` zählt sie auch in der Bar (siehe
  [Konfiguration](06-configuration.md#einstellungen-des-plugins)).
- Sie nimmt die Akzentfarbe deines Themes, solange Cases aktiv sind, die
  Warnfarbe des Themes bei einer Krise und wird blasser, solange etwas
  repariert werden muss.
- Der Tooltip sagt, was die Zahlen bedeuten, wie viele Änderungen keinen
  Case haben und wann die Engine zuletzt erfasst hat: „Seldon — 2 active
  cases, 1 crisis, 7 changes without a case, last capture 4 min ago“.

| Klick | Wirkung |
|---|---|
| Links | Desk öffnen oder schließen |
| Mitte | Desk beim Prime Radiant öffnen (Abschnitt 7) |
| Rechts | jetzt erfassen |

Für eine Taste, die den Desk öffnet, fügst du diese Zeile zu deinen
Hyprland-Bindings hinzu (Seldon richtet sie nie für dich ein).
`SUPER + SHIFT + L` (L wie Logbuch) ist ein Vorschlag, den Omarchy frei
lässt; jede freie Taste geht:

```text
o.bind("SUPER + SHIFT + L", "Seldon", "omarchy-shell shell toggle jax.seldon")
```

Der Desk öffnet sich auf dem Monitor, der den Fokus hat, und merkt sich
Abschnitt und Auswahl bis zum nächsten Öffnen. Esc, ein Klick neben ihn,
ein Linksklick auf die Pill oder die Taste noch einmal schließt ihn.

## Der Desk

![Der Desk bei Today mit dem Beispiel-Logbuch, Tokyo Night: der Kopf mit 2 active, 1 verification, 3 queued, 2 crises und 4 attention; die Seitenleiste mit den neun Abschnitten; NEEDS YOU mit zwei Krisen und das heutige Journal; die Übersicht mit den aktiven Cases als Kacheln](../../images/desk-tokyo-night-today.png)

*Beispieldaten.*

Der Desk ist die eine Oberfläche des Plugins. Er öffnet sich unter der
Bar, die über ihm sichtbar bleibt, und wirkt wie ein Anwendungsfenster:
Nichts dahinter wird abgedunkelt. Von oben nach unten:

- **Kopf.** Das Seldon-Zeichen, „SELDON“ und eine Unterzeile: die
  Maschine, die Omarchy-Version und „captured N ago“. Dann der
  Status-Chip, nur solange ein Hinweis steht: der Titel des ersten
  Hinweises und „+N“ für weitere; ein Klick klappt die Hinweise ein. Dann
  die KPI-Leiste: active, verification, queued, crises (in der Warnfarbe,
  solange es eine gibt) und attention. Jede Zahl führt per Klick zu ihrem
  Abschnitt: Work, oder der Changelog, gefiltert auf die Krisen oder die
  offenen Änderungen. Rechts Settings (`,`) und *Esc*.
- **Hinweise.** Unter dem Kopf, über die ganze Breite: etwas muss
  repariert werden, jeweils mit der Reparatur per Klick (siehe
  [Fehlersuche](10-troubleshooting.md#hinweise-auf-dem-desk)). Auch die
  Tasten erreichen die Knöpfe: ↑ oben in der Liste. Solange Seldon
  nicht eingerichtet ist, zeigt Today stattdessen die Setup-Karte (siehe
  [Erste Schritte](01-getting-started.md)).
- **Seitenleiste.** „SECTIONS“: die neun Ziele mit Symbol, Name und
  Zahl, unten das Suchfeld und der Knopf zum Einklappen. Today zählt die
  Ereignisse von heute, der Changelog die offenen Änderungen (in der
  Warnfarbe bei einer Krise), Work active · verification · queued,
  Decisions die vorgeschlagenen („N new“), Memory die Lektionen und
  Themen.
- **Der Abschnitt.** Eine Listenspalte und ein Detailbereich. Das Detail
  beginnt mit einer festen Aktionsleiste: die Aktionen für das gewählte
  Element, rechts seine ID und, solange eine Aktion scharf ist, der
  Hinweis. Die Leiste bleibt stehen, während das Detail scrollt. Der
  Prime Radiant (7) und der Graph (8) haben keine Listenspalte; sie
  nehmen die ganze Breite.
- **Fußzeile.** Der letzte Fehler, falls es einen gibt, und rechts der
  Tastenhinweis.

Die Auswahl ist der Cursor: ↑ und ↓ bewegen ihn, und das Detail folgt.
Er bleibt auf seinem Element, wenn ein neuer Index Zeilen darüber
einfügt.

Die Bilder auf dieser Seite sind Renderings des Beispiel-Logbuchs im
Theme Tokyo Night. Dein Desk nimmt dein Theme und zeigt deine Daten.

### Breite und Seitenleiste

![Der Desk bei 50 % Breite auf einem 1920 px breiten Bildschirm, mittig unter der Bar, links und rechts der Desktop: Today in drei Spalten, die Case-Kacheln untereinander](../../images/desk-tokyo-night-50.png)

Der Desk füllt in der Grundeinstellung die Breite des Bildschirms. Zwei
Einstellungen ändern das, beide unter Settings › Appearance auf dem Desk
und in Omarchys Bar-Einstellungen (Widget Seldon):

- `deskWidth`, „Desk width“: 50 bis 100 % des Bildschirms. Der Desk
  steht mittig und ist nie schmaler als 960 px, außer der Bildschirm ist
  es.
- `deskSidebar`, „Sidebar“: `open` zeigt die Namen und Zahlen der
  Abschnitte, `collapsed` nur ihre Symbole. Der Knopf zum Einklappen unten
  in der Seitenleiste schaltet das auch um.

Ist der Desk schmaler als 960 px (nur auf einem so schmalen Bildschirm),
zeigt die Seitenleiste nur Symbole, egal was `deskSidebar` sagt. Unter
760 px stehen Liste und Detail untereinander: Zuerst kommt die Liste,
Enter oder ein Klick zeigt das Detail, und „‹ Back to the list“ (oder
Esc) führt zurück.

## Tasten

Die Tasten des Desks gelten, solange der Desk offen ist.

| Taste | Wirkung |
|---|---|
| `1` bis `8` | ein Abschnitt: Today 1, Changelog 2, Work 3, Decisions 4, System 5, Memory 6, Prime Radiant 7, Graph 8 |
| `,` | Settings |
| Alt+↓ / Alt+↑ | das nächste / vorherige der neun Ziele, im Kreis |
| `/` | die Suche der Seitenleiste: filtert die aktuelle Liste; Enter verlässt das Feld und behält den Filter, Esc leert ihn |
| ↑ / ↓, `k` / `j` | in der Liste bewegen; ↑ oben in der Liste erreicht die Knöpfe der Hinweise |
| Enter, Leertaste | die erste Aktion des Details, die nichts startet (Work: scharf schalten, dann ausführen; offene Drift: ihr Formular; die Zeile von gestern in Today: auf- oder zuklappen); auf einem schmalen Desk das Detail zeigen. Enter startet nie einen Agenten |
| Esc | ein Formular verlassen, dann die Suche leeren, dann zurück zur Liste (schmaler Desk), dann schließen |
| `c` | jetzt erfassen |
| `n` | das Notizfeld in Today (aus jedem Abschnitt) |
| `+` | neuer Case in Work (aus jedem Abschnitt) |
| `i` | das Feld *New case* in Today, das Feld für den einen Satz in Work |
| `e` | im Editor öffnen: Today das Journal, Changelog das Ledger des Monats, Work den Case, Decisions die Entscheidung, System `STATUS.md`, Memory das Logbuch |
| `f` / `F` | Changelog: der nächste / vorherige Chip |
| ↑ / `k` in der ersten Zeile | Changelog: der Triage-Vorschlag des Agenten, dann *Agent sorts N open changes*; ↓ / `j` zurück zur Liste. Today, solange Seldon nicht eingerichtet ist: die Zeile der Setup-Karte |
| ← / →, `h` / `l` auf einem Hinweis | die Knöpfe des Hinweises; ↑ / ↓ die Hinweise, ↓ auf dem letzten oder Esc zurück zur Liste |
| Enter auf Appearance | Settings: der Schieberegler für die Breite, die Vorgaben, die Seitenleiste; ↑ / ↓ ein Element, ← / → sein Wert, Esc zurück |
| `a` | Work: den Case einem Agenten übergeben (zweimal drücken); solange ein Agent daran arbeitet, *Focus* (einmal) |
| `x` | Work: den Case aufgeben (zweimal drücken; endgültig) |
| `r` | Work: einen abgeschlossenen Case wieder öffnen (einmal) |
| `d` | Decisions: neue Entscheidung |
| ← / →, `h` / `l` | Prime Radiant: vorheriger / nächster Zeitraum |
| ← / → | Graph: der Stichtag einen Tag zurück / vor |
| `p` | Graph: das Wachstum abspielen oder anhalten |
| `0`, `-`, `=` | Graph: die Ansicht einpassen, hinaus-, hineinzoomen |

Hat ein Textfeld oder ein Formular den Fokus, geht jede Taste dorthin.
Esc gibt die Tasten an den Desk zurück und behält, was du getippt hast.
Tab und Shift-Tab tun auf dem Desk nichts: Er ist kein Popup der Bar.

### Zwei Tastendrücke für alles, was schreibt

Aktionen, die schreiben, werden beim ersten Druck scharf geschaltet und
laufen beim zweiten. Der erste Druck macht aus dem Knopf *Confirm …*, und
die Aktionsleiste nennt die Taste: „Drop C-2026-004? Press x again or
click Confirm. This is final.“ Der zweite Druck oder ein Klick auf
*Confirm …* führt sie aus. Jede andere Taste, eine andere Auswahl oder
ein neuer Index entschärft sie. Das gilt auch für die Maus: *Start*, *To
verification*, *Complete*, *Drop*, *Hand to agent*, *Accept* und
*Discard* brauchen je zwei Klicks. In den Drift-Formularen und bei einer
neuen Entscheidung schaltet Enter scharf und das zweite Enter führt aus;
ein Klick auf den Knopf des Formulars führt sofort aus.

Ein Druck genügt, wo nichts verloren geht: eine Notiz, ein Satz für *New
case* oder *Run*, der Dialog für einen neuen Case, *Reopen* (es fügt nur
einen Case hinzu), *Focus*, *Apply proposals*, *Capture now* und *Open in
editor*.

Jeder Knopf hat einen Weg über die Tasten. Die Knöpfe über der Liste (die
Hinweise, die Setup-Karte, *Agent sorts N open changes*) und die
Elemente in Settings › Appearance erreichst du mit den Pfeilen; der Knopf
unter dem Cursor hat einen Rahmen, und eine blasse Zeile daneben nennt
die Tasten. Dort schaltet Enter den Knopf scharf, und das zweite Enter
führt ihn aus, was immer er tut (auch *Copy*): „Restart shell? Press
Enter again.“ *Agent sorts* ist die Ausnahme: Enter startet nie einen
Agenten, also schaltet `a` ihn scharf und ein zweites `a` startet den
Agenten, wie bei einem Case. Jede andere Taste, ein Klick auf den Desk
oder eine andere Auswahl bricht ab. ← / → wählen einen anderen Knopf
oder Wert. Ein gehaltenes ↑ hält oben in der Liste an; ein weiterer
Druck führt zu den Hinweisen.

Eine gehaltene Taste wirkt einmal. Ihre Wiederholungen schalten nie
scharf, bestätigen nie, schreiben nie und starten nichts; eine scharfe
Aktion bleibt scharf, bis du die Taste loslässt und neu drückst. Nur die
Tasten, die bewegen, wiederholen sich: die Pfeile, die Bildtasten, `j` /
`k`, `h` / `l`, `-` / `=`.

## Die Abschnitte

### Today (1)

Die Liste zeigt das Datum mit dem Zustand des Tages, *Open in editor*
(`e`, das heutige Journal), die Kacheln Ereignisse heute und in sieben
Tagen und das Notizfeld. Notiz tippen, Enter drücken, und sie landet über
`seldon log` im heutigen Journal. Wähl unter dem Feld einen offenen Case,
um die Notiz unter ihm abzulegen. Das Feld leert sich erst, wenn die
Notiz gespeichert ist. Danach folgen **NEEDS YOU**, die Krisen, die
neueste zuerst, und **JOURNAL**, die heutigen Einträge; die von gestern
liegen hinter einer Zeile.

Ist nichts gewählt, ist das Detail die Übersicht: ein Satz („Seldon is
recording. 2 changes need you.“), deine aktiven Cases als Kacheln und
*New case*. Eine Kachel zeigt ID, Risiko, Titel und Fortschritt des
Plans („2/4 steps · claude-code“); solange eine Agenten-Sitzung an dem
Case läuft, beginnt die Zeile mit „agent working · “. Ein Klick auf eine
Kachel öffnet den Case in Work. *New case* (`i`) startet einen Case mit
einem Satz, wie *Run* in Work (siehe unten).

Wähl eine Krise, und du siehst sie so, wie der Changelog sie zeigt;
Enter öffnet ihr Formular. Löst du sie dort auf, verlässt sie NEEDS YOU.

### Changelog (2)

![Der Changelog: oben Agent sorts 6 open changes und der Vorschlag des Agenten, die Chips open, crisis, attention, routine, in case und all, die offenen Änderungen nach Tagen mit Streifen für Krise und zur Kenntnis, und rechts der gewählte Theme-Wechsel mit Ask agent, Link to C-2026-005…, Explain…, Dismiss… und Hide](../../images/desk-tokyo-night-changelog.png)

*Beispieldaten.*

Der Changelog listet, was geschehen ist, das neueste zuerst, nach Tagen
gruppiert. Die Chips oben filtern: **open · crisis · attention · routine
· in case · all**, jeder mit seiner Zahl (`f` / `F`; der Changelog öffnet
mit *open*). Offene Änderungen tragen einen Streifen, eine Krise in der
Warnfarbe deines Themes, ein Punkt zur Kenntnis in der Akzentfarbe.
Routine-Änderungen (ein Theme-Wechsel, ein Schalter, ein einfaches
Upgrade) sind gewöhnliche Zeilen: Geschichte, nichts aufzulösen. Eine
leise Zeile sagt „N changes without a case“. *Capture now* (`c`) startet
eine Erfassung und sagt, was sie gefunden hat; *Ledger* (`e`) öffnet die
Ledger-Ansicht des Monats in deinem Editor.

Wähl eine Änderung, und das Detail zeigt sie: wann, wer, was, den Befehl
und jedes Paket einer pacman-Transaktion, ihren Case, ihre Regel. Eine
Krise sagt, warum sie laut ist. Eine Transaktion, die fehlschlug oder
abgebrochen wurde, ist markiert und erklärt.

Um eine offene Änderung aufzulösen, nimm die Aktionsleiste oder drück
Enter auf ihrer Zeile. Du darfst; du musst nie. *Ask agent* fragt deinen
Agenten nach genau dieser Änderung. *Link to case…*, *Explain…* und
*Dismiss…* öffnen ein Formular im Detail:

| Aktion | Führt aus | Das Detail sagt danach |
|---|---|---|
| *Link* | `seldon drift link <EVENT> <CASE>` (offene Cases; der vorgeschlagene ist vorgewählt) | `Resolved: linked to C-…` |
| *Explain* | `seldon drift explain <EVENT> -- <warum>`, wahlweise mit Zone, Risiko und Bereich | `Resolved: explained · C-…` |
| *Dismiss* | `seldon drift dismiss <EVENT> -- <grund>` | `Resolved: dismissed: <grund>` |

`<EVENT>` ist die Ereignis-ID der Änderung, `<CASE>` eine Case-ID. Eine
Paket-Transaktion löst du als Ganzes auf (*All N*). *Only <Paket>* löst
nur die Zeile auf, die du geöffnet hast. Dein Text bleibt im Formular,
bis die Engine ihn geschrieben hat. Hat inzwischen jemand anderes die
Änderung aufgelöst, sagt das Formular „Already resolved: …“ und schreibt
nichts.

*Hide* nimmt einen Punkt zur Kenntnis für diese Shell-Sitzung aus dem
Blick und hält nichts fest; *Show* holt ihn zurück. Eine Krise hat kein
*Hide*. *Dismiss* ist der festgehaltene Weg.

#### Den Agenten die offenen Änderungen sortieren lassen

![Ein Triage-Vorschlag eines Agenten im Changelog: oben Apply proposals (2) und Discard, die zurückgehaltene Krise unter CRISES — EACH ON ITS OWN mit Apply this crisis, und unter WHAT APPLY TAKES jeder Punkt mit seinen Belegen und wer sie geschrieben hat](../../images/desk-tokyo-night-triage.png)

Solange Änderungen offen sind, bietet der Kopf des Changelogs **Agent
sorts N open changes** an. Ein Klick startet deinen Agenten mit `seldon
agent ask triage`. Der Agent schreibt nichts ins Logbuch. Er legt einen
Vorschlag ab: für jede Änderung eine Verknüpfung mit einem Case oder eine
Erklärung, jede mit Belegen, die die Engine geprüft hat. Der Vorschlag
erscheint als Zeile, „Proposal · N items proposed by …“, und sein Detail
listet jeden Punkt mit seinen Belegen. Belege, die ein Agent geschrieben
hat oder deren Autor unbekannt ist, sind mit „Read twice“ markiert. Du
öffnest ihn mit einem Klick oder mit den Tasten: Der Vorschlag ist die
Zeile über der Liste, also erreicht ↑ (oder `k`) in der ersten Zeile ihn,
und ↓ (oder `j`) führt zurück. Ein weiteres ↑ erreicht *Agent sorts N
open changes*: Zweimal `a` startet den Agenten.

- *Apply proposals* wendet die Punkte mit einem Klick an, als du. Mit
  der Tastatur schaltet Enter es scharf, ein zweites Enter wendet es an.
  Ein zweites Apply ändert nichts.
- Krisen sind nie Teil von Apply. Sie stehen unter **CRISES, EACH ON ITS
  OWN**, jede mit ihrem eigenen *Apply this crisis*. Mit der Tastatur
  geht ↓ vom Vorschlag aus durch die Krisen; Enter schaltet die unter dem
  Cursor scharf, ein zweites Enter wendet nur diese Krise an.
- *Discard* wirft den Vorschlag weg (zwei Klicks). Das Logbuch ändert
  sich nicht.
- Kommt ein neuerer Vorschlag, während du einen liest, schalten sich
  Apply und Discard ab, und *Review the new proposal* zeigt ihn.

[Mit Agenten arbeiten](04-working-with-agents.md#einen-agenten-die-offenen-änderungen-sortieren-lassen)
erklärt, was der Agent tut und was er nicht darf.

### Work (3)

![Work: das Feld für den einen Satz mit Run, 2 / 3 active, die Cases in den Gruppen Active, Verification, Queued und Completed; der gewählte Case C-2026-003, an dem ein Agent arbeitet, mit Focus, To verification, Drop, Open in editor und Ask agent, seinen Angaben, Plan, Log und verknüpften Änderungen](../../images/desk-tokyo-night-work.png)

*Beispieldaten.*

Das Feld oben startet einen Case mit einem Satz: Schreib, was erledigt
werden soll („Installiere Tool X, es bringt ein PKGBUILD mit“), und drück
Enter oder *Run*. Seldon macht aus dem Satz einen Case, startet ihn und
startet deinen Agenten darauf; der Agent macht den Rest und schließt den
Case selbst ab (siehe
[Mit Agenten arbeiten](04-working-with-agents.md#einen-agenten-aus-dem-desk-starten)).
Solange das läuft, steht auf dem Knopf *Running*, danach ist der neue
Case gewählt. Kann kein Agent starten, sagt die Zeile unter dem Feld,
warum und wie du es behebst, und dein Satz bleibt im Feld.

„2 / 3 active“ vergleicht deine aktiven Cases mit deinem Limit. Das
Limit warnt; es blockiert nie. Die Zeile steht über den Knöpfen, oder
neben ihnen, wenn der Desk so schmal ist, dass er Liste und Detail
untereinander zeigt. *By agent* zeigt unter Completed nur die
Cases, die ein Agent abgeschlossen hat, für eine Stichprobe, wenn dir
danach ist (fällig ist sie nie). *Import tasks…* macht aus deiner eigenen
Markdown-Aufgabenliste geplante Cases, zuerst als Probelauf. *New case*
(`+`) fragt nach Titel, Zone, Risiko, Priorität und einem optionalen
Bereich. Es beginnt mit gelb, R1, normal. Nimm es, wenn du einen Case
selbst planen oder später einem Agenten übergeben willst.

Darunter folgen die Cases in Gruppen: ACTIVE, VERIFICATION, QUEUED und
COMPLETED (die letzten 50; aufgegebene eingeschlossen). Eine Zeile zeigt
ID, Risiko, Bereich und die Schritte des Plans. „N proposed“ heißt,
Seldon hält offene Drift für einen Teil dieses Case. „agent working“
markiert einen Case, an dem eine Agenten-Sitzung läuft.

Bei einem sehr großen Logbuch listet der Index nur einen Teil der
wartenden oder aktiven Cases (die Engine hält ihn unter 1 MB). Der Kopf
der Gruppe zeigt dann beide Zahlen, „QUEUED · 50 / 450“, und die
WIP-Zeile zählt jeden aktiven Case. Eine Gruppe, von der der Index keinen
Case listet, behält ihren Kopf, darunter eine gedimmte Zeile „The index
lists none of them. This lists them:“ und den Befehl, zum Beispiel
`seldon plan list --status queued`. Die Suche findet nur die gelisteten
Cases.

Das Detail zeigt den Case: Status, Risiko, Zone, Bereich, Agent,
Rollback-Snapshot, den ersten Absatz von Intent und Result, den
Fortschritt des Plans, sein Log und seine verknüpften Änderungen. Die
Aktionsleiste hängt vom Status ab:

| Status | Aktionen |
|---|---|
| queued | *Start*, *Drop* |
| active | *Hand to agent* (oder *Focus*), *To verification*, *Drop* |
| verification | *Complete*, *Drop* |
| completed | *Reopen* |
| jeder Status | *Open in editor*, *Ask agent* |

*Hand to agent* (`a` zweimal) schickt einen Agenten an den Case. Solange
sein Fenster offen ist, heißt der Knopf *Focus* und holt dieses Fenster
nach vorn; Seldon startet einen Agenten pro Case. *Ask agent* fragt
deinen Agenten nach dem Case, ohne ihm den Case zur Arbeit zu übergeben.

*Reopen* (ein Klick oder `r`) lässt den abgeschlossenen Case unberührt:
Es legt einen neuen aktiven Case „Reopen: <Titel>“ mit demselben
*Intent* an, und der neue Case ist gewählt. Sein Detail nennt den Case,
den er wieder öffnet. Ist ein anderer offener Case der aktive Case,
bleibt er es, damit ein Agent, der daran arbeitet, weiter dort
aufgezeichnet wird; die Ergebniszeile sagt das.

### Decisions (4)

![Decisions: vier Entscheidungen, die neueste zuerst, ADR-0004 vorgeschlagen; die gewählte ADR-0003, angenommen, mit ihrem ersten Absatz, Datum, Datei und den zwei Cases, die sie nennt](../../images/desk-tokyo-night-decisions.png)

*Beispieldaten.*

Decisions listet deine ADRs, die neueste zuerst: Titel, ID und Status,
rechts das Datum; eine vorgeschlagene trägt einen Streifen. Das Detail
zeigt den ersten Absatz der Entscheidung und die Cases, die sie nennt;
ein Klick auf einen Case öffnet ihn in Work. *Open in editor* (`e`)
öffnet die Entscheidung. *Accept* braucht bei einer vorgeschlagenen
Entscheidung zwei Klicks: Beim ersten heißt der Knopf *Confirm accept*,
der zweite führt `seldon decide accept` aus, das den Status und das
heutige Datum setzt. *New decision* (oder `d`) fragt nach einem Titel;
zweimal Enter legt die Entscheidung als *proposed* an und öffnet sie in
deinem Editor.

### System (5)

![System: die sieben Kacheln Omarchy, Packages, Snapshots, Deviations, Collectors, Recently edited und Ignored by pacman; die gewählte Kachel Omarchy mit Version, Theme, letztem Update und Plugins](../../images/desk-tokyo-night-system.png)

*Beispieldaten.*

System zeigt die Maschine als Kacheln, jede mit einem großen Wert:
Omarchy (Version, Theme, letztes Update, Plugins), Packages, Snapshots,
Deviations, Collectors (der Zustand jedes Collectors, der Name der
Maschine, die Version der Engine), *Recently edited* und *Ignored by
pacman*. Wähl eine Kachel für ihre Einzelheiten.

*Recently edited* listet die Dateien unter `~/.config`, die in den
letzten sieben Tagen bearbeitet wurden und die kein beobachteter Pfad
abdeckt. *Watch* an einer Datei nimmt sie in deine beobachteten Pfade
auf (`seldon config watch`); ab der nächsten Erfassung hält Seldon sie
fest. *Ignored by pacman* listet die Pakete und Gruppen, die pacmans
volles Update auslässt (`IgnorePkg`, `IgnoreGroup`; `pacman -S`
aktualisiert sie trotzdem). *Open in editor* (`e`) öffnet `STATUS.md`.

### Memory (6)

![Memory: drei Überschriften von Lektionen und zwei Memory-Themen; die gewählte Lektion im Detail, mit Open in editor](../../images/desk-tokyo-night-memory.png)

*Beispieldaten.*

Memory zeigt, was deine Agenten zu Beginn einer Sitzung lesen: die
Überschriften von `memory/lessons.md` und die anderen Memory-Dateien mit
ihrer letzten Änderung. *Open in editor* (`e`) öffnet den Ordner des
Logbuchs.

### Prime Radiant (7)

![Der Prime Radiant, Abschnitt 7, bei 90 d: die Zeitraum-Auswahl, die Heatmap der Ereignisse pro Tag, die Paketreihen, die Drift-Balken, der Risiko-Donut, die Zeitleiste mit Releases, Snapshots, Cases und Krisen, und der Plan mit den zwei aktiven Cases](../../images/desk-tokyo-night-radiant.png)

*Beispieldaten.*

Der Prime Radiant ist das Bild deiner Maschine. Du öffnest ihn mit `7`
oder mit einem Mittelklick auf die Pill. Er zeigt sechs Diagramme:

| Diagramm | Zeigt |
|---|---|
| Heatmap | Ereignisse pro Tag als Kalender, Wochen als Spalten, Montag oben |
| Series | explizite und gesamte Paketzahl über die Zeit |
| DriftBars | geöffnete und aufgelöste Drift pro Woche |
| RiskDonut | Cases nach Risiko, R0 bis R3, immer über die ganze Zeit |
| Timeline | Omarchy-Releases, Snapshots und Krisen oben, Cases als Spannen darunter |
| The Plan | die aktiven Cases mit Zone, Risiko, Schritten und Agent |

Die Titelzeile jedes Diagramms trägt eine Zusammenfassung. Zeigst du auf
ein Diagramm, nennt die Titelzeile stattdessen das Element unter dem
Zeiger: einen Tag und seine Ereignisse nach Quelle, die Zahlen einer
Woche, einen Case.

Die Zeitraum-Auswahl oben bestimmt das Fenster: 30 Tage, 90 Tage, 365
Tage oder All. Es reicht bis heute; 30 d sind heute und die 29 Tage
davor. ← und → (oder `h` und `l`) gehen durch die Zeiträume. Der
Abschnitt öffnet jedes Mal mit 90 d. RiskDonut und The Plan beachten den
Zeitraum nicht. Der Prime Radiant zeigt nur; er startet nie die Engine.

### Graph (8)

![Der Graph, Abschnitt 8: die Bereiche des Beispiels als Ringe, Cases, Entscheidungen und Änderungen als Netz darum, die Krisen in der Warnfarbe, und oben Play growth mit dem Tages-Schieberegler](../../images/desk-tokyo-night-graph.png)

Der Graph zeichnet das Gedächtnis deiner Maschine als Netz: Bereiche,
Cases, Entscheidungen und Änderungen, verbunden so, wie das Logbuch sie
verbindet. Eine Änderung hängt an ihrem Case, ein Case an seinem
Bereich, eine Entscheidung an den Cases, die sie nennt. Eine
gestrichelte Linie führt von einer Änderung zu dem Case, den Seldon für
sie vorschlägt. Krisen sind rot. *Play growth* spielt das Netz vom ersten
Tag an ab; der Schieberegler wählt den Tag, an dem es abgeschnitten
wird.

| Tun | Wie |
|---|---|
| einen Knoten bewegen | ziehen |
| die Ansicht verschieben | den Hintergrund ziehen |
| zoomen | das Mausrad, `-` und `=`; `0` passt die Ansicht ein |
| die Karte eines Knotens sehen | darauf zeigen; ein Klick hält die Karte |
| einen Case öffnen | *Open case* auf seiner Karte: Work zeigt ihn |
| abspielen | *Play growth*, Leertaste, Enter oder `p`; noch einmal hält an |
| den Stichtag verschieben | der Schieberegler, oder ← und → um einen Tag |
| Esc | das Abspielen anhalten, eine gehaltene Karte loslassen, dann schließen |

Der Graph entsteht aus dem Index und enthält deshalb, was der Index
enthält: 500 Ereignisse (die Änderung hinter jedem offenen Punkt, den
der Index nennt, auch eine alte, und die neuesten der übrigen) und 50
abgeschlossene Cases. Die Fußzeile nennt die Zahlen. Über 400 Knoten werden die Änderungen eines
Tages und einer Quelle zu einem Knoten („+12“); seine Karte zählt sie
auf. Das Layout bewegt sich nur, solange der Abschnitt zu sehen ist, und
hält nach wenigen Sekunden an.

### Settings (,)

![Settings › Appearance: der Schieberegler Desk width bei 100 % mit den Vorgaben 50 %, 67 %, 75 % und Full, die Pixel-Zeile mit dem kleinen Bild des Bildschirms, und Sidebar mit Open und Collapsed](../../images/desk-tokyo-night-settings.png)

Settings hat vier Gruppen. **Appearance** ändert den Desk sofort: „Desk
width“ ist ein Schieberegler von 50 bis 100 % in Zehnerschritten, mit
den Vorgaben 50 %, 67 %, 75 % und Full und einer Zeile, die sagt, wie
viele Pixel das auf diesem Bildschirm sind. Ziehen zeigt die Breite zur
Probe; das Loslassen oder eine Vorgabe speichert sie. „Sidebar“ ist
*Open* oder *Collapsed*. Seldon speichert beides in seinem Eintrag in
Omarchys `shell.json`, und nur, wenn du einen Wert änderst. Nimmt die
Shell die Änderung nicht an, sagt die Seite das; dann änderst du es in
Omarchys Bar-Einstellungen. Mit den Tasten: Enter auf Appearance
erreicht den Schieberegler, ↑ / ↓ die Vorgaben und die Seitenleiste,
← / → ändern den Wert (die Breite zeigt sich sofort, wie beim Ziehen),
zweimal Enter speichert ihn, Esc führt zurück.

**Capture**, **Agents** und **Quiet** zeigen nur die geltenden Werte und
wo jeder eingestellt wird: das Erfassungsintervall, das Limit aktiver
Cases und die in der Bar gezählten Änderungen in Omarchys
Bar-Einstellungen; die Collectors, den Launcher und was laut sein darf
in `~/.config/seldon/config.toml`. Der Desk ändert `config.toml` nie.
Capture bietet außerdem den Snapshot-Schritt wieder an, nachdem du auf
der Setup-Karte *Not now* gewählt hast.

## Der Desk tritt zur Seite

Öffnet der Desk ein Fenster für dich, schließt er sich, damit das neue
Fenster vorne liegt und nicht dahinter. Das geschieht, nachdem die Engine
geantwortet hat: ein Agent gestartet (*Hand to agent*, *Run*, *New case*)
oder nach vorn geholt (*Focus*), dein Editor geöffnet (*Open in editor*,
eine neue Entscheidung), ein Agent gefragt (*Ask agent*, *Agent sorts N
open changes*). Eine Reparatur im Terminal aus einem Hinweis schließt ihn
sofort. Verweigert die Engine, bleibt der Desk offen und sagt, warum. Der
Desk merkt sich Abschnitt und Auswahl, beim nächsten Öffnen siehst du
also den Case, bei dem du warst.

Ein Knopf, dessen Aufruf läuft, ist beschäftigt (*Starting…*,
*Focusing…*, *Opening…*), und *Open in editor* öffnet dieselbe Datei nie
zweimal innerhalb von zwei Sekunden.

## Vom Terminal aus

Für alles auf dem Desk gibt es einen Befehl. Der Desk führt dieselben
Befehle aus, das Ergebnis ist also dasselbe:

| Auf dem Desk | Im Terminal |
|---|---|
| Notizfeld | `seldon log -- "Text"` |
| *Capture now* | `seldon capture` |
| *Run*, *New case* in Today | `seldon agent start --new -- "Was zu tun ist"` |
| *New case* (`+`) | `seldon plan new -- "Titel"` |
| *Start*, *To verification*, *Complete*, *Drop* | `seldon plan start <ID>`, `verify`, `done`, `drop` |
| *Reopen* | `seldon plan reopen <ID>` |
| *Hand to agent*, *Focus* | `seldon agent start <ID>`, `seldon agent focus <ID>` |
| *Ask agent* | `seldon agent ask drift <EVENT>`, `seldon agent ask case <ID>` |
| *Agent sorts N open changes* | `seldon agent ask triage` |
| *Apply proposals*, *Discard* | `seldon drift apply <PROPOSAL>`, `seldon drift discard <PROPOSAL>` |
| *Link*, *Explain*, *Dismiss* | `seldon drift link`, `explain`, `dismiss` |
| *New decision*, *Accept* | `seldon decide -- "Titel"`, `seldon decide accept <ID>` |
| *Watch* | `seldon config watch -- <PATH>` |
| *Update rules* | `seldon rules update` |
| *Open in editor* | `seldon open journal --editor` |

`<ID>` ist eine Case-ID wie `C-2026-001`, bei `decide accept` eine
Entscheidungs-ID wie `ADR-0001`. `<PROPOSAL>` ist die ID des Vorschlags
des Agenten. Die [Befehlsreferenz](05-cli-reference.md) listet jeden
Befehl.

---

Zurück: [Konzepte](02-concepts.md) · [Übersicht](README.md) · Weiter: [Mit Agenten arbeiten](04-working-with-agents.md)
