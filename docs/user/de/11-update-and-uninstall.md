# Aktualisieren und entfernen

<!-- source: en/11-update-and-uninstall.md @ c28426a -->

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

Er ersetzt `seldon`, wenn das Release neuer ist. Bei derselben Version
ändert er nichts. Er prüft die Engine gegen die `SHA256SUMS` des Release
und bricht bei einer Abweichung ab. Hat das neueste Release noch kein
`install.sh` (v0.1.0), nimm das Skript aus `main` wie in Erste Schritte.

| Option | Wirkung |
|---|---|
| `--version vX.Y.Z` | dieses Release statt des neuesten |
| `--prefix DIR` | nach `DIR/bin` installieren statt nach `~/.local/bin` |
| `--unit` | auch die Benutzer-Unit des Watchers installieren, siehe [Der optionale Watcher](#der-optionale-watcher) |
| `--force` | ein `seldon` ersetzen, das das Skript nicht installiert hat, etwa ein selbst gebautes |
| `--uninstall` | entfernen, was das Skript installiert hat |

Beim Einzeiler gibst du Optionen nach `bash -s --` an, zum Beispiel
`… | bash -s -- --version v0.1.1`.

Das AUR-Paket `jax-seldon` kommt bald. Sobald es existiert, aktualisierst
du es mit deinem AUR-Helfer (`yay -S jax-seldon`). Installiere nur aus
einer Quelle: Beide legen ein `seldon` in deinen `PATH`.

## Das Plugin aktualisieren

```sh
omarchy plugin update jax.seldon
```

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
entfernt alles außer deinem Logbuch.

1. Das Plugin:

   ```sh
   omarchy plugin remove jax.seldon
   ```

2. Den Watcher, wenn du ihn aktiviert hast:

   ```sh
   systemctl --user disable --now seldon-watch
   ```

3. Die Engine. Der Installer entfernt genau die Dateien, die er
   installiert hat; eine Datei, die du seitdem geändert hast, bleibt, und
   er sagt das. Gib dasselbe `--prefix` an, falls du eins benutzt hast:

   ```sh
   curl -fsSL https://github.com/JohnAndrewsX/jax-seldon/releases/latest/download/install.sh | bash -s -- --uninstall
   ```

4. Den Theme-Hook, wenn du ihn im Assistenten gewählt hast. Ohne die
   Engine tut er nichts, aber es ist sauberer, ihn zu entfernen:

   ```sh
   rm ~/.config/omarchy/hooks/theme-set.d/seldon-theme-set.sh
   ```

5. Die Hooks von Claude Code liegen in `.claude/settings.json` des
   Logbuchs. Sie gehen mit dem Logbuch. Hast du sie in
   `~/.claude/settings.json` installiert, entferne aus dieser Datei die
   drei Einträge, die `seldon hook` aufrufen.

6. Konfiguration und Zustand der Engine:

   ```sh
   rm -r ~/.config/seldon ~/.local/state/seldon
   ```

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
der Engine und das Logbuch (Schritte 4 und 6 oben, und das Logbuch),
dann führ `seldon init` noch einmal aus. `init` lehnt einen Ordner ab,
der nicht leer ist, also überschreibt ein neues Logbuch nie ein altes.

---

Zurück: [Fehlersuche](10-troubleshooting.md) · [Übersicht](README.md) · Weiter: [FAQ](12-faq.md)
