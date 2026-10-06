# Aktualisieren und entfernen

<!-- source: en/11-update-and-uninstall.md @ 5d68748 -->

Diese Seite zeigt, wie du die Engine und das Plugin aktualisierst, wie
du den optionalen Watcher betreibst und wie du Seldon ganz oder in Teilen
entfernst. Nichts davon berührt dein Logbuch, außer du löschst es selbst.

## Die Engine aktualisieren

Führ den Installer noch einmal aus, in einer der beiden Formen aus
[Erste Schritte](01-getting-started.md#schritt-1-die-engine-installieren).
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
eine Zeile, dass nur die Prüfsumme geprüft wurde.

Er installiert auch die Manpage (`man seldon`) und die Tab-Vervollständigung
für bash, zsh und fish, jeweils für eine Shell, die auf deinem Rechner
installiert ist. Für zsh gibt er eine `fpath=(…)`-Zeile aus, die du in
`~/.zshrc` vor `compinit` einträgst.

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

Das Update holt die neuen Plugin-Dateien und bittet die Shell, ihre
Plugins neu zu laden. Die Shell führt aber den Plugin-Code, den sie
zuerst geladen hat, weiter aus, bis sie neu startet. Starte sie mit
`omarchy-restart-shell` neu: Leiste, Panels und Overlays sind nach
wenigen Sekunden wieder da, deine Programme laufen weiter. Bei
gesperrtem Bildschirm verweigert der Befehl den Neustart.

Merkt das Panel, dass das installierte Plugin nicht das laufende ist,
zeigt es über den anderen Bannern „Restart the shell to finish the
update“ mit einem Knopf *Restart shell*, der `omarchy-restart-shell`
ausführt. Eine Plugin-Version, die diesen Hinweis noch nicht kennt,
zeigt nichts. Starte die Shell deshalb nach jedem Plugin-Update neu.

Plugin und Engine einigen sich über die Version des Index-Formats. Ist
eines zu alt, meldet das Panel „Index format mismatch“ und nennt das, was
du aktualisieren musst. Nach einem Update sollte `seldon doctor` nur `ok`
zeigen.

## Der optionale Watcher

Ohne den Watcher sieht das Plugin deine Änderungen beim nächsten Befehl
der Engine oder bei der nächsten Erfassung. Der Watcher baut den Index
zwei Sekunden nach jeder Änderung im Logbuch neu, sodass eine Änderung in
deinem Editor oder in Obsidian sofort im Panel erscheint. Er erfasst
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
