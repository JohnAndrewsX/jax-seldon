==> system/deviations.md <==
# Abweichungen vom Omarchy-Standard

Eine Zeile pro Abweichung. Seldon trägt Pfad, Datum und Case ein; den Grund schreibe ich.

<!-- seldon:begin deviations.table -->
| path | reason | date | case |
|---|---|---|---|
| ~/.config/hypr/input.conf | Tastatur de, Caps als Escape | 2026-09-01 | — |
| ~/.config/omarchy/shell.json | Bar: Wetter rechts, Uhr mit Sekunden | 2026-09-01 | — |
| ~/.bashrc | mise-Aktivierung | 2026-09-01 | — |
| ~/.config/hypr/monitors.conf | Dual-WQHD, Skalierung 1.25 | 2026-09-12 | [[C-2026-002]] |
| ~/.config/hypr/bindings.conf | SUPER+E öffnet Zed | 2026-10-01 | [[C-2026-004]] |
<!-- seldon:end -->
==> system/hardware.md <==
# Hardware

Eigene Notizen: Netzteil getauscht 2026-08.

<!-- seldon:begin hardware.summary -->
- cpu: Intel(R) Core(TM) i7-14700K
- memory: 63 GiB
- machine: MS-7D91
- rootfs: btrfs
<!-- seldon:end -->
==> system/omarchy.md <==
# Omarchy

Paketinstallation (`/usr/share/omarchy`), kein Git-Checkout.

<!-- seldon:begin omarchy.summary -->
- version: 4.0.7-1
- theme: tokyo-night
- lastUpdate: 2026-10-01T09:21:00+02:00
<!-- seldon:end -->
==> system/packages.md <==
# Pakete

<!-- seldon:begin packages.summary -->
- explicit: 15
- total: 23
- aur: 3
<!-- seldon:end -->

## Verlauf

<!-- seldon:begin packages.history -->
| date | explicit | total |
|---|---|---|
| 2026-09-01 | 323 | 2004 |
| 2026-09-03 | 324 | 2005 |
| 2026-10-01 | 15 | 23 |
<!-- seldon:end -->

## Explizite Pakete

<!-- seldon:begin packages.explicit -->
- base · repo · pre-logbook
- base-devel · repo · pre-logbook
- brave-bin · aur · pre-logbook
- btop · repo · since 2026-09-03
- firefox · repo · pre-logbook
- git · repo · pre-logbook
- hyprland · repo · pre-logbook
- linux · repo · pre-logbook
- linux-firmware · repo · pre-logbook
- neovim · repo · pre-logbook
- ollama · repo · since 2026-10-01
- omarchy · repo · pre-logbook
- tailscale · repo · since 2026-10-01 [[C-2026-008]]
- yay · aur · pre-logbook
- zed · repo · since 2026-10-01 [[C-2026-004]]
<!-- seldon:end -->
==> system/plugins.md <==
# Plugins

<!-- seldon:begin plugins.list -->
| id | enabled | firstParty | clonedFrom |
|---|---|---|---|
| io.github.example.tyme | no | no | — |
| io.github.example.weather-plus | yes | no | — |
| omarchy.active-window | no | yes | — |
| omarchy.agents | yes | yes | — |
| omarchy.audio | yes | yes | — |
| omarchy.background | yes | yes | — |
| omarchy.bar | yes | yes | — |
| omarchy.battery | yes | yes | — |
| omarchy.bluetooth | yes | yes | — |
| omarchy.clipboard | yes | yes | — |
| omarchy.clock | no | yes | — |
| omarchy.dev-gallery | yes | yes | — |
| omarchy.disk-speedtest | yes | yes | — |
| omarchy.dropbox | no | yes | — |
| omarchy.emojis | yes | yes | — |
| omarchy.idle | yes | yes | — |
| omarchy.image-picker | yes | yes | — |
| omarchy.indicators | yes | yes | — |
| omarchy.keyboard-layout | yes | yes | — |
| omarchy.lock | yes | yes | — |
| omarchy.media | no | yes | — |
| omarchy.menu | yes | yes | — |
| omarchy.microphone | no | yes | — |
| omarchy.monitor | yes | yes | — |
| omarchy.network | yes | yes | — |
| omarchy.nightlight | yes | yes | — |
| omarchy.notifications | yes | yes | — |
| omarchy.osd | yes | yes | — |
| omarchy.polkit | yes | yes | — |
| omarchy.power | yes | yes | — |
| omarchy.reminders | yes | yes | — |
| omarchy.spacer | no | yes | — |
| omarchy.speedtest | yes | yes | — |
| omarchy.system-update | yes | yes | — |
| omarchy.tailscale | yes | yes | — |
| omarchy.tray | yes | yes | — |
| omarchy.weather | yes | yes | — |
| omarchy.wifiqr | yes | yes | — |
| omarchy.workspaces | yes | yes | — |
| user.clock | yes | no | omarchy.clock |
<!-- seldon:end -->
==> system/services.md <==
# Dienste

<!-- seldon:begin services.enabled -->
| unit | scope | case |
|---|---|---|
| NetworkManager.service | system | — |
| bluetooth.service | system | — |
| fstrim.timer | system | — |
| getty@.service | system | — |
| systemd-timesyncd.service | system | — |
| tailscaled.service | system | [[C-2026-008]] |
| elephant.service | user | — |
| ollama.service | user | — |
| pipewire.socket | user | — |
<!-- seldon:end -->
