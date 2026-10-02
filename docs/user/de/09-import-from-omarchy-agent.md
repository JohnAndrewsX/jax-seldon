# Import aus omarchy-agent

<!-- source: en/09-import-from-omarchy-agent.md @ c47054b -->

Diese Seite ist für dich, wenn du vor Seldon ein Logbuch mit dem
omarchy-agent-Kit geführt hast: einen Obsidian-Vault mit `pipeline/`,
`journal/` und `knowledge/`. Seldon kann dessen Cases, Journal, Wissen
und Abweichungen in dein Seldon-Logbuch übernehmen. Hast du das Kit nie
benutzt, überspring diese Seite.

## Was mit deinem Vault passiert

Nichts. Der Import liest den Vault nur. Er schreibt in dein
Seldon-Logbuch, und das erst, nachdem du einen Bericht gelesen und
`--apply` gesagt hast.

## Schritt 1: Probelauf

Leg zuerst dein Seldon-Logbuch an ([Erste Schritte](01-getting-started.md)).
Dann richte den Import auf den Vault:

```sh
seldon import omarchy-agent ~/pfad/zum/vault
```

Der Probelauf ist die Vorgabe. Er schreibt eine Datei,
`outputs/IMPORT-omarchy-agent.md`, und committet sie als
`seldon: import omarchy-agent (dry run)`. Sonst ändert sich nichts im
Logbuch.

## Schritt 2: Den Bericht lesen

Öffne `~/Seldon/outputs/IMPORT-omarchy-agent.md` in deinem Editor. Er
listet:

- jeden Case mit alter und neuer ID und Status;
- IDs, die in deinem Logbuch schon vergeben waren, und die neue ID, die
  jeder dieser Cases bekommt;
- wie viele Links und IDs der Import umschreibt, pro Datei;
- Annahmen, die er getroffen hat (zum Beispiel ein abgeschlossener Case
  ohne Abschlussdatum);
- die Journal-Tage, Memory-Dateien und Abweichungszeilen, die er
  schreiben wird;
- jede Datei, die er nicht importiert, mit dem Grund;
- Fehler, die den Import aufhalten;
- Zeilen, in denen die Schwärzung etwas entfernt hat (Datei und Zeile,
  nie der Text).

Behebe Fehler im Vault oder nimm die betroffene Datei aus dem Vault
heraus, und wiederhole den Probelauf, bis der Bericht keine Fehler mehr
meldet.

## Schritt 3: Anwenden

```sh
seldon import omarchy-agent ~/pfad/zum/vault --apply
```

Der Import schreibt alles in einem Zug und macht einen Commit,
`seldon: import omarchy-agent`. Er verweigert sich (Exit 1), solange der
Bericht Fehler enthält.

Ein zweiter Lauf ist sicher. Ein zweites `--apply` meldet „Nothing
changed“ und schreibt nichts. Die Markierung
`.seldon/imports/omarchy-agent.json` hält fest, dass der Import erledigt
ist.

## Wohin was kommt

| Im Vault | In deinem Logbuch |
|---|---|
| Cases in `pipeline/cases/` und `archive/cases/` | Case-Dateien in `work/`, getaggt mit `omarchy-agent` |
| Status `new`, `planned`, `in-progress`, `verification` | `queued` (nie aktiv; das Log sagt, was er war) |
| Status `done` | `completed` |
| Status `dropped` | `dropped` |
| Abschnitt *Auftrag* | *Intent* |
| Abschnitt *Plan* | *Plan* |
| Abschnitt *Ergebnis* | *Result* |
| jeder andere Abschnitt | unter `## History`, eine Überschriftenebene tiefer |
| `journal/YYYY-MM.md` | die passenden Tagesdateien, unter `## Imported from omarchy-agent` |
| `knowledge/<topic>/*.md` | je ein Abschnitt in `memory/<topic>.md` |
| Lektionen aus `knowledge/` | Abschnitte in `memory/lessons.md` |
| offene Einträge in `system/deviations.md` mit einem Pfad | Zeilen in der Abweichungstabelle des Dossiers |
| Inbox, Dashboard, Vorlagen, `STRUCTURE.md`, der Rest von `system/`, `.obsidian/` | nicht importiert; im Bericht gelistet |

Jeder importierte Case bekommt außerdem eine Notiz im Ledger, datiert
auf seinen Anlagetag. Zone, Risiko, Priorität und Daten kommen
unverändert mit.

## Schon vergebene IDs

Ein Case behält seine ID, wenn dein Logbuch sie noch nicht hat. Ist die
ID vergeben, bekommt der Case die nächste freie ID dieses Jahres. Er
wird mit `omarchy-agent/<alte id>` getaggt, und eine Zeile unter seinem
Titel nennt die alte ID. Jeder Verweis auf die alte ID im importierten
Text (`[[C-2026-001]]` und ein bloßes `C-2026-001`) wird zur neuen ID.

## Datenschutz

Jede importierte Zeile durchläuft Seldons Schwärzungsregeln,
einschließlich deiner eigenen Muster. Pfade, die mit deinem Home-Ordner
beginnen, werden zu `~`. Der Bericht nennt die geschwärzten Zeilen mit
Datei und Zeilennummer, nie mit ihrem Inhalt.

## Wenn etwas schiefgeht

Scheitert `--apply` mittendrin, sagt die Engine, dass nichts committet
wurde, und wie du das halbe Schreiben rückgängig machst:

```sh
cd ~/Seldon
git checkout -- . && git clean -fd
```

Das entfernt jede nicht committete Änderung im Logbuch, auch deine
eigenen Änderungen seit dem letzten `seldon`-Befehl. Kopier sie vorher
woandershin. Dann behebe die Ursache und führ `--apply` noch einmal aus.

---

Zurück: [Wiederaufbau, Dossier und Update-Folgen](08-rebuild-dossier-update-impact.md) · [Übersicht](README.md) · Weiter: [Fehlersuche](10-troubleshooting.md)
