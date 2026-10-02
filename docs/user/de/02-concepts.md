# Konzepte

<!-- source: en/02-concepts.md @ 2f41188 -->

Diese Seite erklärt die Ideen hinter Seldon: das Logbuch, Cases, Zonen,
Risiko, Drift, die Baseline, Krisen, Entscheidungen und Memory. Lies sie
einmal. Die anderen Seiten verwenden diese Wörter, ohne sie noch einmal
zu erklären.

## Die Idee

Deine Maschine ändert sich jeden Tag. Du installierst Pakete, Omarchy
aktualisiert sich, ein Agent ändert eine Konfigurationsdatei. Ein paar
Wochen später weiß niemand mehr, warum etwas da ist. Das Systemlog hat
die Fakten, aber nicht die Gründe. Ein handgeschriebenes Änderungsprotokoll
hat die Gründe, bis du aufhörst, es zu schreiben.

Seldon hält beides fest und vergleicht es:

- Es zeichnet auf, was sich geändert hat. Collectors lesen das Paketlog,
  Snapper, die Omarchy-Version, die Shell-Plugins, das Theme und deine
  Konfigurationsdateien. Agenten-Hooks zeichnen die Befehle auf, die ein
  Agent ausführt.
- Es hält fest, was du ändern willst. Du schreibst Cases: eine
  Markdown-Datei pro geplanter Änderung.
- Es zeigt den Unterschied. Eine Änderung, die kein Case abdeckt, ist
  Drift. Du entscheidest, was sie war.

Seldon ist ein Rekorder. Es ändert nie dein System, startet nie einen
Paketmanager oder `sudo`, hält nie einen Befehl an und schickt nie etwas
übers Netz. Der Name stammt von Hari Seldon aus Asimovs *Foundation*: Der
Plan sagt voraus, und eine Krise ist die Stelle, an der die Wirklichkeit
den Plan verlässt.

## Die drei Teile

| Teil | Was es ist | Was es tut |
|---|---|---|
| Logbuch | ein Ordner, standardmäßig `~/Seldon` | hält die Aufzeichnung als Markdown und JSON-Zeilen; ein git-Repository |
| Engine | das Programm `seldon` | schreibt als einziges ins Logbuch; du und deine Agenten rufen es auf |
| Plugin | `jax.seldon` in der Omarchy-Bar | zeigt das Logbuch; jede Aktion ruft die Engine auf |

Das Plugin liest eine einzige Datei, den Index
(`~/.local/state/seldon/index.json`). Die Engine baut ihn nach jedem
Befehl neu. Du änderst ihn nie; wenn du ihn löschst, geht nichts
verloren, denn `seldon status` schreibt ihn wieder.

## Das Logbuch

Das Logbuch ist ein gewöhnlicher Ordner mit Markdown-Dateien. Du kannst
es in jedem Editor lesen, in Obsidian oder auf GitHub. Es enthält zwei
Arten von Aufzeichnung:

- Das Ledger (`ledger/YYYY-MM.jsonl`) schreibt die Maschine. Jede Zeile
  ist ein Ereignis: ein Paket installiert, ein Theme gewechselt, eine
  Konfigurationsdatei geändert, eine Notiz geschrieben, ein Case
  gestartet. Das Ledger wird nur ergänzt. Nichts darin wird je geändert
  oder gelöscht; eine Korrektur ist ein neues Ereignis.
- Das Journal (`journal/YYYY/YYYY-MM-DD.md`) schreiben Menschen und
  Agenten. Es hält das Warum fest: was du versucht und was du gelernt
  hast.

Seldon verbindet beides über Case-IDs: Ein Journal-Eintrag und die
Ereignisse, um die es darin geht, tragen denselben Case.

Jedes Ereignis hat eine Quelle. Es gibt neun:

| Quelle | Was sie aufzeichnet |
|---|---|
| `pacman` | installierte, entfernte, aktualisierte und zurückgestufte Pakete |
| `snapper` | angelegte und gelöschte Snapshots |
| `omarchy` | Wechsel der Omarchy-Version |
| `plugins` | Shell-Plugins: hinzugefügt, entfernt, aktiviert, deaktiviert, aktualisiert |
| `theme` | Theme-Wechsel |
| `config` | Dateien unter den beobachteten Pfaden: hinzugefügt, geändert, entfernt |
| `agent` | Befehle, die ein Agent ausgeführt hat, aufgezeichnet von einem Hook |
| `manual` | Notizen (`seldon log`) und Ereignisse, die du von Hand einträgst |
| `seldon` | die Schritte der Engine selbst: Case angelegt, gestartet, abgeschlossen, Drift aufgelöst |

[Das Logbuch](07-the-logbook.md) beschreibt jeden Ordner.

## Cases

Ein Case ist eine geplante Änderung in einer Markdown-Datei unter
`work/`. Er hat eine ID wie `C-2026-004`, einen Titel, eine Zone, ein
Risiko, einen Bereich und vier Abschnitte:

- *Intent*: warum, und was danach anders ist.
- *Plan*: Ziel, Schritte, betroffene Pfade, Rollback, Prüfung.
- *Log*: datierte Zeilen, nur ergänzt. Die Engine fügt für jeden Schritt
  eine hinzu.
- *Result*: was herausgekommen ist.

Du schreibst *Intent*, *Plan* und *Result*. Den Rest pflegt die Engine:
den Status, die Liste der verknüpften Ereignisse, die Agenten, die daran
gearbeitet haben.

Ein Case durchläuft diese Status. Die Engine verschiebt die Datei
zwischen den Ordnern; du verschiebst sie nie selbst.

| Status | Ordner | Befehl, der ihn dorthin bringt |
|---|---|---|
| queued (geplant) | `work/queued/` | `seldon plan new` |
| active (aktiv) | `work/active/` | `seldon plan start` |
| verification (in Prüfung) | `work/active/` | `seldon plan verify` |
| completed (abgeschlossen) | `work/completed/` | `seldon plan done` |
| dropped (aufgegeben) | `work/completed/` | `seldon plan drop` (aus queued, active oder verification) |

Die Reihenfolge steht fest: queued, active, verification, completed. Ein
abgeschlossener oder aufgegebener Case bleibt geschlossen. Jeder Schritt
ist ein Ereignis im Ledger und ein git-Commit.

Der zuletzt gestartete Case ist der *aktive Case*. Seine ID steht in
`.seldon/active-case`. Agenten-Hooks versehen jeden Befehl, den sie
aufzeichnen, mit ihm, und so sammelt der Case seine *Spur*: die
geordnete Liste dessen, was passiert ist, solange er offen war.

## Zonen

Jedes Ereignis und jeder Case hat eine Zone. Die Zone sagt, wie tief die
Änderung ins System greift.

| Zone | Was hineinfällt |
|---|---|
| red (rot) | Pakete, Omarchy selbst, systemd-Units (auch Unit-Dateien unter `~/.config/systemd/`), `/etc`, der Bootloader |
| yellow (gelb) | Konfigurationsdateien unter den beobachteten Pfaden, Themes, Shell-Plugins |
| green (grün) | alles, was die Collectors nicht verfolgen: Projektdateien, Paketmanager von Programmiersprachen, `git` |

Snapshots, Notizen und die Schritte der Engine selbst haben keine Zone.
Grüne Änderungen zeichnen nur Agenten-Hooks auf, und nur, solange ein
Case aktiv ist.

## Risiko

Jeder Case hat außerdem ein Risiko, von `R0` bis `R3`. Das Risiko ist
deine Einschätzung, was es braucht, die Änderung rückgängig zu machen.

| Risiko | Bedeutung |
|---|---|
| R0 | in Sekunden umkehrbar, nichts hängt daran; du machst es von Hand rückgängig, kein Snapshot |
| R1 | in Minuten von Hand umkehrbar, mit einem bekannten Befehl; der Plan nennt den Rollback-Schritt |
| R2 | der Rollback braucht den Plan und einen Snapshot oder ein Backup; mit `--snapshot` starten, vor dem Abschluss prüfen |
| R3 | kann Boot, Login oder die Shell kaputtmachen; Snapshot Pflicht, dein ausdrückliches Okay für jeden Schritt, nie unbeaufsichtigt |

Seldon speichert das Risiko und zeigt es an. Es setzt es nicht durch. Ein
neuer Case beginnt mit gelb und `R1`, wenn du nichts anderes angibst.

## Drift

Drift ist ein Ereignis, das das System ändert, keinen Case hat und noch
nicht aufgelöst ist. Nur Änderungen können Drift sein: Pakete, Omarchy,
Plugins, Theme und Konfiguration. Snapshots, Notizen und Case-Schritte
sind es nie.

Die meisten Änderungen landen von selbst bei einem Case:

- Der Befehl eines Agenten trägt den aktiven Case, weil der Hook ihn ins
  Ereignis schreibt.
- Ein Paket, das als Abhängigkeit eines Pakets aus einem Case
  mitgekommen ist, gehört zu diesem Case.

Alles andere ist Drift. Dazu gehören auch deine eigenen Änderungen
außerhalb des Terminals: Ein Collector sieht, dass sich das Theme
geändert hat, aber nicht, dass du es für einen Case gemeint hast. Nennt
ein offener Case das geänderte Paket oder den Pfad in seinem *Plan*,
schlägt Seldon diesen Case vor, und das Panel wählt ihn vor.

Du löst Drift auf eine von drei Arten auf:

| Aktion | Befehl | Nimm sie, wenn |
|---|---|---|
| verknüpfen (link) | `seldon drift link <EVENT> <CASE>` | die Änderung zu einem Case gehört (offen oder geschlossen) |
| erklären (explain) | `seldon drift explain <EVENT> -- "<warum>"` | sie einen Grund hatte, aber keinen Case; Seldon legt einen abgeschlossenen Case mit deinem Text als Titel an |
| verwerfen (dismiss) | `seldon drift dismiss <EVENT> -- "<grund>"` | sie keine Rolle spielt (eine Abhängigkeit, Rauschen) |

Das Ereignis bleibt in jedem Fall im Ledger. Die Auflösung ist ein neues
Ereignis, das darauf verweist. Pakete aus einer Transaktion bilden einen
Drift-Eintrag: Ein Routine-Upgrade von vierzig Paketen ist ein Eintrag,
und ein Befehl löst alle auf. Mit `--only` löst du nur dieses eine
Ereignis auf.

## Krise

Eine Krise ist Drift in der roten Zone. Sie hat dieselben drei Aktionen.
Pill und Panel zeigen Krisen zuerst, in der Fehlerfarbe deines Themes,
mit der Zeile „N changes in the red zone need a reason“.

Routine-Upgrades sind keine Krisen. Eine Transaktion, die bei einem
vollständigen System-Upgrade nur Pakete aktualisiert, ist gelbe Drift.
Sie wird rot, wenn sie ein Paket installiert oder entfernt oder ein
Paket von der Immer-rot-Liste berührt (standardmäßig `linux*`,
`systemd`, `glibc`, `hyprland`, `omarchy`, `quickshell`; siehe
[Konfiguration](06-configuration.md#drift)).

## Baseline

Ein neues Logbuch zeichnet ab dem Moment auf, in dem du es anlegst. Der
Assistent kann auch *nacherfassen*: Änderungen seit einem früheren Datum
aufzeichnen, aus dem Paketlog und von Snapper. Keine dieser älteren
Änderungen gehört zu einem Case, also öffnet jede als Drift, die meisten
als Krise.

Die Baseline räumt das auf. Nach einer Nacherfassung fragt der
Assistent, ob er alles Gefundene als Baseline vor Seldon markieren soll.
Sagst du ja, verwirft die Engine jeden offenen Eintrag mit dem Grund
„pre-Seldon baseline“. Die Ereignisse bleiben im Ledger und in den
Diagrammen; sie verlangen nur keinen Grund mehr. Ohne Nacherfassung gibt
es keine Baseline.

## Entscheidungen

Eine Entscheidung ist eine kurze Aufzeichnung einer Wahl, die die
Maschine prägt, im ADR-Format (Architecture Decision Record): Kontext,
Entscheidung, Folgen. `seldon decide -- "<titel>"` legt
`decisions/ADR-NNNN-<slug>.md` mit dem Status *proposed* an und öffnet
sie in deinem Editor. Den Status änderst du in der Datei auf *accepted*
oder *superseded*. Der Tab Decisions listet sie auf.

## Memory

Memory ist das, was Agenten über diese Maschine gelernt haben, in
`memory/`. `memory/lessons.md` hat eine `##`-Überschrift pro Lektion:
was passiert ist und was beim nächsten Mal zu tun ist. Jeder Agent liest
sie zu Beginn einer Sitzung. Andere Dateien, etwa `memory/hyprland.md`,
halten Notizen zu einem Thema. Du und deine Agenten schreibt diese
Dateien direkt; die Engine liest sie nur.

## Das Dossier

Das Dossier (`system/`) beschreibt die Maschine, wie sie jetzt ist:
Pakete, Dienste, Omarchy-Version und Theme, Hardware, Plugins und die
Dateien, die du gegenüber Omarchys Vorgaben geändert hast. Die Engine
füllt die Teile zwischen den Markierungen `<!-- seldon:begin … -->` und
`<!-- seldon:end -->`; du schreibst drumherum. Siehe
[Wiederaufbau, Dossier und Update-Folgen](08-rebuild-dossier-update-impact.md).

## Erfassung

Eine Erfassung (Capture) lässt die Collectors laufen und hängt neue
Ereignisse ans Ledger an. Das passiert:

- wenn du `seldon capture` ausführst;
- alle 15 Minuten, solange das Plugin läuft, und beim Start der Shell;
- wenn du im Panel *Capture now* drückst oder mit rechts auf die Pill
  klickst;
- am Ende einer Agenten-Sitzung, die Seldons Hooks hat.

Zweimal hintereinander ausgeführt, schreibt sie beim zweiten Mal nichts.
Jeder Collector merkt sich, wie weit er gelesen hat.

---

Zurück: [Erste Schritte](01-getting-started.md) · [Übersicht](README.md) · Weiter: [Alltag](03-daily-use.md)
