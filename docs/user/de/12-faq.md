# FAQ

<!-- source: en/12-faq.md @ 2e4bfa8 -->

Kurze Antworten auf die Fragen, die zuerst kommen. Jede Antwort verweist
auf die Seite mit den Einzelheiten.

## Ändert Seldon mein System?

Nein. Die Engine liest das Paketlog, Snapper, die Omarchy-Version, die
Plugin-Liste, das Theme und deine beobachteten Konfigurationsdateien. Sie
schreibt nur ihr Logbuch, `~/.config/seldon/config.toml` und
`~/.local/state/seldon/`. Zwei Dinge passieren nur, wenn du sie im
Assistenten wählst: der Theme-Hook im Hook-Ordner von Omarchy und die
Hooks von Claude Code in `.claude/settings.json` des Logbuchs. Sie
startet nie einen Paketmanager, `sudo` oder `systemctl` mit einem
ändernden Befehl.

## Hält Seldon einen Agenten von etwas Gefährlichem ab?

Nein. Seldon zeichnet auf; es bewacht nichts. Hooks halten nie einen
Befehl an. Für Grenzen nimm die Berechtigungen deines Agenten. Siehe
[Mit Agenten arbeiten](04-working-with-agents.md#was-seldon-tut-und-was-nicht).

## Verlässt etwas meine Maschine?

Nein. Engine und Plugin bauen keine Netzwerkverbindung auf. Der einzige
Download ist der Installer, den du selbst startest. Seldon pusht das
git-Repository des Logbuchs nirgendwohin.

## Brauche ich Obsidian?

Nein. Das Logbuch ist reines Markdown und funktioniert in jedem Editor.
Obsidian ist ein optionaler Betrachter; siehe
[Das Logbuch](07-the-logbook.md#obsidian).

## Brauche ich das Bar-Plugin?

Nein. Alles geht vom Terminal aus. Das Plugin macht Drift und Cases auf
einen Blick sichtbar und erfasst alle 15 Minuten. Ohne es führst du
`seldon capture` selbst aus oder lässt das Ende einer Agenten-Sitzung
das erledigen.

## Warum ist meine eigene Änderung Drift? Ich hatte einen Case offen.

Ein Collector sieht, dass sich etwas geändert hat, aber nicht, wer es
war oder warum. Nur der Befehl eines Agenten, aufgezeichnet von einem
Hook, trägt den aktiven Case. Deine eigene Änderung verknüpfst du mit
`seldon drift link <EVENT> <CASE>` oder mit *Link* im Drift-Dialog. Siehe
[Konzepte](02-concepts.md#drift).

## Ist jedes Paket-Upgrade eine Krise?

Nein. Ein Routine-Upgrade des ganzen Systems ist gelbe Drift, ein
Eintrag pro Transaktion, aufgelöst mit einem Befehl. Es wird rot, also
eine Krise, wenn es ein Paket installiert oder entfernt oder ein Paket
von der Immer-rot-Liste berührt (Kernel, systemd, glibc, Hyprland,
Omarchy, Quickshell).

## Darf ich die Dateien von Hand bearbeiten?

Ja, die meisten: Text in Cases, Journal, Memory, Entscheidungen,
Bereiche, `PROJECT.md`, `AGENTS.md`, deinen Text in `system/`. Nicht das
Ledger, die erzeugten Dateien, die Felder der Engine in der Frontmatter
oder `.seldon/`. Siehe [Das Logbuch](07-the-logbook.md#was-dir-gehört).

## Wie mache ich einen Fehler rückgängig?

Das Ledger wird nur ergänzt, also bleibt ein falsches Ereignis stehen,
und ein neues Ereignis korrigiert es. Eine Drift-Auflösung lässt sich
nicht per Befehl zurücknehmen. Für Dateien, die du bearbeitet hast, hat
git jede frühere Fassung: `git -C ~/Seldon log -p <file>`.

## Kann ich Seldon auf Deutsch benutzen?

Ja. Wähl im Assistenten Deutsch als Sprache des Logbuchs oder führ
`seldon init --language de` aus. Journal-Zeilen und `STATUS.md` sind dann
deutsch. Überschriften, Frontmatter-Schlüssel und Befehle bleiben in
jeder Sprache englisch. Die Beschriftungen des Plugins sind in dieser
Version englisch. Diese Anleitung gibt es auch im englischen Original:
[English](../en/README.md).

## Kann ich zwei Logbücher haben?

Eine Maschine, ein Logbuch funktioniert am besten: Der Zustand der
Engine gehört zu einem Logbuch. Für ein Test-Logbuch führst du
`seldon init --path <DIR>` aus und nimmst `--logbook <DIR>` für seine
Befehle. `init` macht den neuen Ordner zur Vorgabe, also stell `logbook`
in `config.toml` danach zurück.

## Was bedeutet der Name?

In Isaac Asimovs *Foundation* sagt Hari Seldons Plan die Zukunft voraus,
und eine Krise ist die Stelle, an der die Wirklichkeit den Plan
verlässt. Der Prime Radiant ist das Gerät, das den Plan zeigt. Seldon
macht die Stellen sichtbar, an denen deine Maschine deinen Plan
verlassen hat.

## Wo melde ich einen Fehler?

Unter <https://github.com/JohnAndrewsX/jax-seldon/issues>. Was
hineingehört, steht in [Fehlersuche](10-troubleshooting.md#einen-fehler-melden).

---

Zurück: [Aktualisieren und entfernen](11-update-and-uninstall.md) · [Übersicht](README.md) · Weiter: [Glossar](13-glossary.md)
