# Aktualisieren und entfernen

<!-- source: en/11-update-and-uninstall.md @ ce4e1c26 -->

Diese Seite zeigt, wie du die Engine und das Plugin aktualisierst, wie
du den optionalen Watcher betreibst und wie du Seldon ganz oder in Teilen
entfernst. Nichts davon berührt dein Logbuch, außer du löschst es selbst.

## Die Engine aktualisieren

Führ den Installer noch einmal aus, in einer der beiden Formen aus
[Erste Schritte](01-getting-started.md#die-engine-installieren).
Der Einzeiler:

```sh
curl -fsSL https://github.com/JohnAndrewsX/jax-seldon/releases/latest/download/install.sh | bash
```

Er ersetzt `seldon`, wenn die Version des Release von der installierten
abweicht; mit `--version` kann das auch ein älteres Release sein. Bei
derselben Version ändert er nichts. Er prüft die Engine gegen die `SHA256SUMS` des Release
und bricht bei einer Abweichung ab. Ist die GitHub-CLI (`gh`) installiert
und angemeldet, prüft er außerdem die Build-Herkunft (`gh attestation
verify`): Der Download muss vom Release-Workflow des Projekts für das Tag
dieses Release stammen, sonst wird nichts installiert. Ohne `gh` meldet
eine Zeile, dass nur die Prüfsumme geprüft wurde. Seine nächsten
Schritte lassen weg, was du schon hast: kein `seldon init`, wenn dein
Logbuch eingerichtet ist, keine Plugin-Zeile, wenn das Plugin installiert
ist. Bleibt nichts zu tun, sagt er „Your logbook is already set up;
nothing else to do.“ Ist zsh installiert, steht stattdessen bei jedem
Lauf die `fpath=(…)`-Zeile von unten da: Sie ist für die
zsh-Vervollständigung und braucht nichts mehr, sobald sie in deiner
`~/.zshrc` steht. Aktualisiere auch das Plugin
([Das Plugin aktualisieren](#das-plugin-aktualisieren)): Es merkt nicht
von selbst, dass es ein neueres gibt.

Er installiert auch die Manpage (`man seldon`) und die Tab-Vervollständigung
für bash, zsh und fish, jeweils für eine Shell, die auf deinem Rechner
installiert ist. Für zsh gibt er eine `fpath=(…)`-Zeile aus, die du in
`~/.zshrc` vor `compinit` einträgst, bei jedem Lauf (er prüft nicht, ob
sie schon dort steht).

| Option | Wirkung |
|---|---|
| `--version vX.Y.Z` | dieses Release statt des neuesten |
| `--prefix DIR` | nach `DIR/bin` installieren, Manpage und Vervollständigung nach `DIR/share`, statt nach `~/.local` |
| `--unit` | auch die Benutzer-Unit des Watchers installieren, siehe [Der optionale Watcher](#der-optionale-watcher) |
| `--force` | ein `seldon`, eine Vervollständigung oder eine Manpage ersetzen, die das Skript nicht installiert hat, etwa ein selbst gebautes `seldon` |
| `--require-verified` | nur installieren, wenn `gh` die Build-Herkunft bestätigt hat; abbrechen, wenn `gh` fehlt oder nicht angemeldet ist, und bei Releases bis v0.1.1, die vor den Attestierungen erschienen sind |
| `--skip-provenance` | `gh` nicht fragen, nur die Prüfsumme prüfen (eine Zeile sagt das); für ein `gh`, das von sich aus scheitert, etwa hinter einem Proxy. Nicht zusammen mit `--require-verified` |
| `--uninstall` | entfernen, was das Skript installiert hat |

Beim Einzeiler gibst du Optionen nach `bash -s --` an, zum Beispiel
`… | bash -s -- --version v0.1.1`.

Das AUR-Paket `jax-seldon` kommt bald. Sobald es existiert, aktualisierst
du es mit deinem AUR-Helfer (`yay -S jax-seldon`). Das Paket installiert
die Manpage und die Vervollständigung für alle drei Shells. Installiere
nur aus einer Quelle: Beide legen ein `seldon` in deinen `PATH`.

Von 0.1.3 auf 0.1.4: Die erste Erfassung nach dem Update kopiert Seldons
Claude-Code-Hooks aus `.claude/settings.json` des Logbuchs nach
`~/.claude/settings.json`, einmal, und lässt dort alles andere stehen
(nicht bei `[agent] workdir = "logbook"`; siehe
[Mit Agenten arbeiten](04-working-with-agents.md#hooks-eines-älteren-logbuchs)).

## Das Plugin aktualisieren

```sh
omarchy plugin update jax.seldon
omarchy-restart-shell
```

Omarchy zeigt jede geänderte Zeile des Plugins und fragt
`Update jax.seldon?`; antworte mit ja. Die Liste ist lang, wenn viele
Dateien geändert sind; das ist Omarchys normale Prüfung, kein Fehler.

Bietet das Panel von Plugin 0.1.0 `omarchy pkg aur add jax-seldon` an?
Dieses Paket gibt es noch nicht, der Befehl scheitert also. Aktualisiere
zuerst das Plugin, mit den beiden Befehlen oben; das neue Plugin
installiert die Engine stattdessen aus dem GitHub-Release.

Das Update holt die neuen Plugin-Dateien und bittet die Shell, ihre
Plugins neu zu laden. Die Shell führt aber den Plugin-Code, den sie
zuerst geladen hat, weiter aus, bis sie neu startet. Starte sie mit
`omarchy-restart-shell` neu: Die Leiste und die anderen Flächen der
Shell sind nach wenigen Sekunden wieder da, deine Programme laufen
weiter. Bei
gesperrtem Bildschirm verweigert der Befehl den Neustart.

Merkt das Plugin, dass das installierte Plugin nicht das laufende ist,
zeigt der Desk über den anderen Hinweisen „Restart the shell to finish
the update“ mit einem Knopf *Restart shell*, der `omarchy-restart-shell`
ausführt. Eine Plugin-Version, die diesen Hinweis noch nicht kennt (vor
0.1.4), zeigt nichts. Starte die Shell deshalb nach jedem Plugin-Update
neu.

Plugin und Engine einigen sich über die Version des Index-Formats. Ist
eines zu alt, meldet der Desk „Index format mismatch“ und nennt das, was
du aktualisieren musst. Ist die Engine neuer und sagt ihr Index, dass
dieses Plugin ihn noch lesen kann, arbeitet das Plugin weiter und bittet
nur in einem leisen Hinweis um sein eigenes Update („The engine is newer
than the plugin“). Nach einem Update sollte `seldon doctor` nur `ok`
zeigen.

## Was sich gegenüber 0.4.0 geändert hat

0.4.1 behält das Index-Format von 0.2.0 bis 0.4.0 (Version 2).
Aktualisiere Engine und Plugin deshalb in beliebiger Reihenfolge, mit
den Befehlen oben, und starte nach dem Update des Plugins die Shell neu.
Ein Plugin 0.4.1 arbeitet mit einer Engine ab 0.2.0, und ein älteres
Plugin liest den Index einer Engine 0.4.1. Was sich für dich ändert:

- Seldon läuft auf Omarchys Edge-Kanal (Qt 6.12) wie auf Stable. Auf
  Edge verdeckt Qt 6.12 einem Plugin 0.4.0 die Farben des Omarchy-Themes;
  Desk, Pill und Sheets von 0.4.1 übernehmen sie wieder.
- Die eigenen Pakete von Edge zählen wie ihre Stable-Namen:
  `omarchy-dev`, `omarchy-settings-dev` und Omarchys andere Kernel wie
  `linux-omarchy-bore` bekommen genau das Urteil von `omarchy`,
  `omarchy-settings` und `linux-omarchy`
  ([Konfiguration](06-configuration.md#drift)).
- Ein Wechsel von Omarchys Kanal, von Stable zu Edge oder zurück, ist
  eine Änderung, *Omarchy changed channel*, statt einer Entfernung,
  einer Installation und einer niedrigeren Versionsnummer. Ohne Case ist
  er mit der vorgegebenen Krisenliste eine Krise, wie eine Installation
  von `omarchy` beim Namen; plane den Wechsel in einem Case, dann bleibt
  er still. *Why loud?* erklärt ihn.
- Die Einstellungsdateien, die Omarchy selbst schreibt, sind Routine,
  wenn sie genau das enthalten, was Omarchy schreibt: die Sprache
  (`omarchy language`, Edge), der Standard-Agent und das
  Diktier-Backend. Bisher war die Sitzungsdatei der Sprache unter
  `~/.config/uwsm/` eine Krise. Stummgeschaltete Absturzmeldungen waren
  schon Routine und bleiben es.
- Ein Terminal-Schritt von Seldon, der fehlschlägt, endet mit Status 1,
  sodass Omarchy Edge sein rotes „Failed (exit code 1)!“ statt „Done!“
  zeigt. Ein älteres Omarchy zeigt in beiden Fällen „Done!“; die Zeile
  darüber sagt, was passiert ist.
- `seldon hook install skills` installiert den Agentenskill auch in den
  Ordner von Antigravity, `~/.gemini/config/skills`, wenn es ihn gibt
  ([Mit Agenten arbeiten](04-working-with-agents.md#der-agentenskill)).

## Was sich gegenüber 0.3.x geändert hat

0.4.0 behält das Index-Format von 0.2.0 und 0.3.0 (Version 2).
Aktualisiere Engine und Plugin deshalb in beliebiger Reihenfolge, mit
den Befehlen oben, und starte nach dem Update des Plugins die Shell neu.
Ein Plugin 0.3.0 oder 0.2.0 liest den Index einer Engine 0.4.0, und ein
Plugin 0.4.0 arbeitet mit einer Engine 0.2.0 oder 0.3.0. Was sich für
dich ändert:

- Deine Agenten warten weniger. In einer Sitzung, die weder im Logbuch
  liegt noch von Seldon gestartet wurde, kehren die Hooks für den Start
  und das Ende einer Sitzung und für andere Agenten (`hook
  session-start`, `hook session-stop`, `hook generic`) jetzt innerhalb
  einer Millisekunde zurück, wie `hook claude-code` schon bisher.
- *By agent*, *Import tasks…* und *New case* in Work haben einen Weg per
  Taste: ↑ auf der ersten Zeile von Work (oder in einer leeren Liste)
  erreicht *By agent*, ←/→ wählen einen Knopf, Enter bereitet vor, ein
  zweites Enter führt aus; ↓ geht zurück zur Liste, ↑ weiter zu den
  Hinweisen. Keiner davon startet einen Agenten. Die Tabelle steht unter
  [Alltag](03-daily-use.md#tasten).
- Kein Zustand ist nur an der Farbe zu erkennen: Neben einem farbigen
  Streifen in einer Liste des Desk steht sein Wort (`! crisis`,
  `attention`, `active`, `proposed`, `failing`). Eine leere Liste sagt,
  was hineingehört, und bietet einen Schritt an: *Clear search* (oder
  Esc) bei einer Suche ohne Treffer, *New case* in einem leeren Work,
  *New decision* in leeren Decisions.
- *Agent sorts N open changes* im Changelog heißt jetzt *Ask agent to
  sort N changes*. Nur die Beschriftung ist neu: Zweimal `a` startet den
  Agenten wie bisher, Enter tut dort weiter nichts.
- Das Detail einer pacman-Transaktion sagt wieder, wenn es nur einen
  Teil der Transaktion zeigt, auch bei einer alten offenen Änderung
  eines einzelnen gelisteten Pakets.
- Auf einem schmalen Desk bricht die Zeile unter dem Seldon-Zeichen
  neben den Zahlen um, statt unter ihnen durchzulaufen.
- Mehr Geheimnisse werden geschwärzt, bevor sie ins Logbuch kommen:
  Passwörter in Pythons dreifachen Anführungszeichen, Schlüssel wie
  `"secret_key"` oder `"aws_secret_access_key"`, eine Passwortdatei als
  `/dev/fd/3`, die die Zeile aus `<(…)` füllt, und ein WLAN-Schlüssel,
  der mit `nmcli … -f wifi-sec.psk` gesetzt wird
  ([Konfiguration](06-configuration.md#schwärzung)).
- Ein Hook liest die Befehle in `$(…)`, in Backticks und in `<(…)` als
  Befehle der Zeile, deshalb wird `x=$(sudo mkpasswd …)` aufgezeichnet,
  sein Passwort geschwärzt. Eine Zeile mit sehr tief verschachteltem
  `$(…)` bringt den Hook nicht mehr zum Absturz: Ab der 17. Ebene liest
  der Hook die Zeile nicht weiter; ein Befehl vor dieser Stelle wird
  geschwärzt aufgezeichnet, ein Befehl darin oder danach wird nicht
  aufgezeichnet
  ([Mit Agenten arbeiten](04-working-with-agents.md#was-ein-hook-aufzeichnet)).
- `omarchy-shell jax.seldon.panel …` leitet noch an den Desk weiter;
  ein späteres Release entfernt es.

## Was sich gegenüber 0.2.x geändert hat

0.3.0 behält das Index-Format von 0.2.0 (Version 2). Aktualisiere Engine
und Plugin deshalb in beliebiger Reihenfolge, mit den Befehlen oben, und
starte nach dem Update des Plugins die Shell neu. Ein Plugin 0.2.0 liest
den Index einer Engine 0.3.0, und ein Plugin 0.3.0 arbeitet mit einer
Engine 0.2.0. Was sich für dich ändert:

- Die Knöpfe des Desk haben einen Weg per Taste, außer *By agent* und
  *Import tasks…* in Work, die noch einen Klick brauchen. ↑ oben in einer
  Liste erreicht die Knöpfe der Hinweise, ↑ auf der ersten Zeile von
  Today die Einrichtungskarte, ↑ auf der ersten Zeile des Changelog den
  Vorschlag des Agenten oder *Ask agent to sort N changes* (zweimal `a`),
  Enter unter Settings › Appearance die Breite und die Seitenleiste. ←/→
  wählen einen Knopf oder einen Wert, Enter bereitet vor, ein zweites
  Enter führt aus. In einem gezeigten Vorschlag geht ↓ durch seine
  Krisen, und zweimal Enter wendet die Krise unter dem Cursor allein an.
  Die Tabelle steht unter [Alltag](03-daily-use.md#tasten).
- Ein sehr großes Logbuch sperrt den Desk nicht mehr aus. Das Plugin
  liest einen Index unter 1 MB; die Engine lässt jetzt die ältesten
  Zeilen weg, bis er passt (Vergangenes zuerst, Krisen zuletzt), zählt
  weiter jeden wartenden und aktiven Case, jede offene Änderung und
  Krise, und `seldon status` nennt, was sie weggelassen hat. Das Logbuch und jeder Befehl behalten jede Zeile. Die Köpfe in
  Work zeigen dann beide Zahlen, „QUEUED · 50 / 450“
  ([Alltag](03-daily-use.md#work-3)). Ein Plugin 0.2.0 zählt die Gruppen
  in Work bei einem solchen Index nach den gelisteten Zeilen, bis du es
  aktualisierst.
- Jede offene Änderung, die der Index listet, hat ihre Zeile im
  Changelog und unter *Needs you*, egal wie alt die Änderung ist.
- Der Agenten-Skill ist ein kurzer Kern, und seine Einzelheiten stehen in
  Themendateien, die der Kern dort verlinkt, wo eine Aufgabe sie braucht
  ([Mit Agenten arbeiten](04-working-with-agents.md#der-agentenskill)).
  Die nächste Erfassung aktualisiert einen Skill, den du nicht geändert
  hast; ein geänderter bleibt, und `seldon doctor` sagt `outdated`.
- Mehr Geheimnisse werden geschwärzt, bevor sie ins Logbuch kommen:
  private Schlüssel in base64, Passwörter in Einstellungen im Stil von
  Python und in Mount-Optionen und Passphrasen auf der Befehlszeile
  ([Konfiguration](06-configuration.md#schwärzung)).
- `omarchy-shell jax.seldon.panel …` leitet noch an den Desk weiter;
  ein späteres Release entfernt es.

## Was sich gegenüber 0.1.x geändert hat

0.2.0 ändert die Oberfläche des Plugins und das Index-Format.
Aktualisiere deshalb Engine und Plugin zusammen und starte danach die
Shell neu:

```sh
curl -fsSL https://github.com/JohnAndrewsX/jax-seldon/releases/latest/download/install.sh | bash
omarchy plugin update jax.seldon
omarchy-restart-shell
seldon doctor
```

Zwischen den ersten beiden Befehlen zeigt das Plugin 0.1.x womöglich
„Index format mismatch“: Die neue Engine schreibt schon das neue Format.
Nach `omarchy plugin update` zeigt ein Plugin 0.1.4 einmal „Restart the
shell to finish the update“ mit *Restart shell*; ein älteres zeigt
nichts. Starte die Shell in beiden Fällen neu. Was sich für dich ändert:

- Der Desk ersetzt Panel und Overlay. Das Popup-Panel der Leiste mit
  seinen sechs Tabs und das bildschirmfüllende Overlay des Prime Radiant
  gibt es nicht mehr. Der Desk hat einen Header mit dem Status-Chip und
  der KPI-Leiste, darunter die Hinweise, eine Seitenleiste mit neun
  Zielen sowie eine Liste und ein Detail. Today, Changelog, Work,
  Decisions, System und Memory sind die Abschnitte 1 bis 6, der Prime
  Radiant ist Abschnitt 7, der neue Graph Abschnitt 8, und Settings ist
  `,`. Ein Linksklick auf die Pill öffnet den Desk, ein Mittelklick
  öffnet ihn beim Prime Radiant.
- `1` bis `8` wählen jetzt einen Abschnitt. Die Zeiträume, die im
  Overlay von 0.1 auf `1` bis `4` lagen, sind in Abschnitt 7 `←`/`→`
  oder `h`/`l`. Tab und Shift+Tab tun nichts, denn der Desk ist kein
  Popup der Leiste. Die ganze Tabelle der Tasten steht in
  [Alltag](03-daily-use.md#tasten).
- `omarchy-shell shell toggle jax.seldon` öffnet und schließt den Desk;
  leg deine Taste darauf. Das Ziel des Panels von 0.1,
  `omarchy-shell jax.seldon.panel …`, funktioniert weiter und leitet an
  den Desk weiter; ein späteres Release entfernt es, und das Changelog
  kündigt das vorher an.
- Das Index-Format hat die Version 2. Ein Plugin 0.1.x zeigt gegen eine
  Engine 0.2.0 „Index format mismatch“, ein Plugin 0.2.0 ebenso gegen
  eine Engine 0.1.x; beide nennen die Seite, die du aktualisieren musst.
  Zurück zu einer Engine 0.1.x geht nicht: Sie überspringt die neuen
  Arten von Ledger-Zeilen als unlesbar.
- Deine Einstellungen des Plugins behalten ihre Werte. Die zwei neuen
  beginnen mit ihren Vorgaben: `deskWidth` 100 (Prozent der
  Bildschirmbreite) und `deskSidebar` `open`. Du änderst sie auf dem Desk
  unter Settings › Appearance oder in Omarchys Einstellungen der Leiste
  ([Konfiguration](06-configuration.md#einstellungen-des-plugins)).
- Verlinkte Ordner und Dateien im Logbuch verweigern das Schreiben. Ein
  Ordner oder eine Datei des Logbuchs, in die die Engine schreibt, muss
  echt sein, kein symbolischer Link. Ist es ein Link, bricht jeder
  Befehl, der dort schreiben würde, mit Exit 1 ab, nennt ihn und schreibt
  nichts; vor 0.2.0 ging das Schreiben durch den Link hindurch. Die Zeile
  `layout` von `seldon doctor` nennt jeden solchen Link. Ersetze jeden
  Link durch den Ordner oder die Datei, auf die er zeigt. Der
  Logbuch-Ordner selbst darf weiter ein Link sein.

## Der optionale Watcher

Ohne den Watcher sieht das Plugin deine Änderungen beim nächsten Befehl
der Engine oder bei der nächsten Erfassung. Der Watcher baut den Index
zwei Sekunden nach jeder Änderung im Logbuch neu, sodass eine Änderung in
deinem Editor oder in Obsidian sofort auf dem Desk erscheint. Er erfasst
nicht und committet nicht. Er ist standardmäßig aus; du entscheidest, ob
du ihn willst.

Probier ihn zuerst im Terminal aus. Beenden mit Strg-C:

```sh
seldon watch
```

Um ihn als Benutzerdienst laufen zu lassen, installierst du seine Unit
mit dem Installer und aktivierst sie:

```sh
curl -fsSL https://github.com/JohnAndrewsX/jax-seldon/releases/latest/download/install.sh | bash -s -- --unit
systemctl --user enable --now seldon-watch
```

Der Installer legt die Unit nur nach `~/.config/systemd/user/`; er
aktiviert sie nie. Sein Log verfolgst du mit
`journalctl --user -u seldon-watch -f`.

Um ihn nicht mehr zu nutzen:

```sh
systemctl --user disable --now seldon-watch
```

## Entfernen

Entferne die Teile, die du nicht mehr willst. Die Reihenfolge unten
entfernt alles außer deinem Logbuch. Die Schritte 3 und 4 brauchen die
Engine, also führ sie vor Schritt 5 aus.

1. Das Plugin:

   ```sh
   omarchy plugin remove jax.seldon
   ```

2. Den Watcher, wenn du ihn aktiviert hast:

   ```sh
   systemctl --user disable --now seldon-watch
   ```

3. Den Theme-Hook, wenn du ihn im Assistenten gewählt hast. Das
   entfernt den Hook und sonst nichts und braucht kein Logbuch:

   ```sh
   seldon init --remove-theme-hook
   ```

4. Die Hooks von Claude Code. Sie liegen in deiner
   `~/.claude/settings.json`, und ohne diesen Schritt ruft Claude Code
   weiter ein `seldon` auf, das es nicht mehr gibt. Der Befehl entfernt
   nur die Hooks von Seldon; deine eigenen Einstellungen und Hooks
   bleiben, und eine Datei, die sonst nichts enthielt, wird gelöscht:

   ```sh
   seldon hook uninstall claude-code
   ```

   Ein Logbuch von vor 0.1.4 kann sie auch enthalten, in seiner eigenen
   `.claude/settings.json`. Behältst du das Logbuch, nimm sie dort mit
   demselben Befehl und `--settings` heraus:

   ```sh
   seldon hook uninstall claude-code --settings ~/Seldon/.claude/settings.json
   ```

   Jeder Befehl gibt aus, was er entfernt hat, oder sagt, dass nichts
   installiert war. Das nächste `seldon capture` hält die Entfernungen
   fest, ohne Drift zu öffnen. Löschst du das Logbuch ebenfalls, gehen
   seine eigenen Hooks mit ihm, und nur der erste Befehl ist nötig.

5. Die Engine. Der Installer entfernt genau die Dateien, die er
   installiert hat, Manpage und Vervollständigung eingeschlossen; eine
   Datei, die du seitdem geändert hast, bleibt, und er sagt das. Gib
   dasselbe `--prefix` an, falls du eins benutzt hast:

   ```sh
   curl -fsSL https://github.com/JohnAndrewsX/jax-seldon/releases/latest/download/install.sh | bash -s -- --uninstall
   ```

6. Konfiguration und Zustand der Engine:

   ```sh
   rm -r ~/.config/seldon ~/.local/state/seldon
   ```

   Wenn du Seldon vielleicht wieder installierst und das Logbuch
   behältst, sichere den Zustandsordner vorher (siehe
   [Den Zustandsordner sichern und wiederherstellen](07-the-logbook.md#den-zustandsordner-sichern-und-wiederherstellen))
   und stelle ihn vor dem ersten Capture wieder her. Sonst fängt dieses
   Capture neu an und hält einen Zustands-Reset im Ledger fest. Nach der
   Neuinstallation sagt dir `seldon doctor` das, solange noch Zeit ist:
   Bis der Zustand zurück ist, meldet seine `state`-Zeile, dass das
   nächste Capture einen Zustands-Reset festhalten wird (siehe
   [doctor sagt, das nächste Capture hält einen Zustands-Reset fest](10-troubleshooting.md#doctor-sagt-das-nächste-capture-hält-einen-zustands-reset-fest)).

Was bleibt, ist dein Logbuch, `~/Seldon`, wenn du keinen anderen Ort
gewählt hast. Es ist reines Markdown in einem git-Repository und bleibt
ohne Seldon lesbar. Lösch es nur, wenn du sicher bist, dass du seine
Geschichte nicht mehr brauchst:

```sh
rm -r ~/Seldon
```

## Neu anfangen

Um Seldon auf derselben Maschine von vorn auszuprobieren, behältst du
Engine und Plugin. Entferne den Theme-Hook, Konfiguration und Zustand
der Engine und das Logbuch (Schritte 3 und 6 oben, und das Logbuch),
dann führ `seldon init` noch einmal aus. `init` lehnt einen Ordner ab,
der nicht leer ist, also überschreibt ein neues Logbuch nie ein altes.

---

Zurück: [Fehlersuche](10-troubleshooting.md) · [Übersicht](README.md) · Weiter: [FAQ](12-faq.md)
