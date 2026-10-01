# Host notes — verified on the dev host, 2026-10-01

Kickoff checklist step 3 (ORCHESTRATION.md §9). Machine names, IPs and
private paths live in `memory/local.md` (git-ignored); this file stays
generic so it can be committed.

## Versions

| Thing | Dev host | Test host |
|---|---|---|
| Omarchy | 4.0.4-1 (`omarchy-version`) | 4.0.4-1 |
| Kernel | 7.2.5-4-omarchy | 7.2.5-4-omarchy |
| quickshell | 0.3.1 | 0.3.1 |
| qt6-declarative (qmllint) | 6.11.2 at `/usr/lib/qt6/bin/qmllint` (not on PATH) | 6.11.2 at `/usr/bin/qmllint` |
| Rust | rustup 1.29.1, rustc/cargo 1.98.1, clippy, rustfmt, target `x86_64-unknown-linux-musl` (static-pie build verified) | pacman `rust` 1.98.1, **no rustup, no musl target** |
| just | 1.58.0 via mise | not installed |
| herdr | 0.9.1, claude integration installed | 0.8.2 |
| snapper | 0.13.1, config `root` | — |
| Third-party plugins installed | none | 28 (incl. `palccod.omalog`, `io.github.fabiopauli.omaharness`, `io.github.salemsayed.omaherd`) — reference corpus for WP-010 |

Tooling convention on the dev host: CLI tools come from **mise**
(`~/.config/mise/config.toml`: claude, codex, gh, node, just). Rust came
from `omarchy install dev-env rust` (= rustup, user-space, no sudo).

## Where the kit's assumptions differ from the host

1. **Omarchy lives in `/usr/share/omarchy`**, not `~/.local/share/omarchy`.
   `$OMARCHY_PATH` defaults to it. It is a **package install, not a git
   checkout** (no `.git`). Consequences:
   - SPEC-ENGINE §4 "git HEAD of the omarchy repo" → use `omarchy-version`
     (prints `4.0.4-1`) or `pacman -Q omarchy`; the `omarchy` upgrade line in
     `pacman.log` is the update event.
   - WP-033 update-impact "git log/tags of the local repo" has no source on a
     package install → needs another source (decision; see STATUS.md).
   - `/usr/share/omarchy/version` says `4.0.0.alpha` — stale, do not use.
2. **`omarchy --version` does not exist.** Use `omarchy-version`.
3. `omarchy plugin list --json` has no `version` field (fields: id, name,
   kinds, enabled, active, canDisable, firstParty, clonedFrom).
   `omarchy plugin catalog` emits every manifest as JSON (with version) —
   use that for `plugin-update` detection.
4. **Current theme** is in `~/.local/state/omarchy/current/theme.name`
   (slug, e.g. `osaka-jade`). `omarchy theme current` prints a prettified
   name (`Osaka Jade`); the collector should read the file. Hook dir
   `~/.config/omarchy/hooks/theme-set.d/` exists; install with
   `omarchy hook install theme-set <file>`. Other hook dirs: battery-low.d,
   font-set.d, post-boot.d, post-update.d (candidate for the omarchy update
   event), pre-refresh-pacman.d.
5. **snapper needs privileges.** As the user, `snapper --jsonout list`
   prints `No permissions.` and `sudo -n` needs a password. Options:
   `ALLOW_USERS`/`ALLOW_GROUPS` in `/etc/snapper/configs/root` (red zone,
   the operator decides) or run the collector degraded (`ok: false`, message with
   the fix). `omarchy snapshot create|restore` wraps snapper with sudo.
6. `/var/log/pacman.log` is world-readable. Install-time lines use
   `pacman -b /mnt//var/lib/pacman/ -r /mnt -Sy ...` (archinstall) — add to
   the parser fixtures. Timestamps are `+0000` on this host.
7. **`omarchy plugin add <url>` is a plain `git clone`** of the URL; no
   branch or subdirectory syntax. The manifest must be at the **repo root**.
   This repo's manifest is in `plugin/` → the installable plugin needs its
   own git root (separate repo, or a published branch made with
   `git subtree split --prefix=plugin`). Decision open, see STATUS.md.
8. Confirmed to exist as the specs assume: `omarchy plugin validate <dir>`,
   `omarchy plugin add|update|remove|enable|disable|clone|catalog`,
   `omarchy pkg add|aur add|drop`, `omarchy agent prompt [--inline] <text>`,
   `omarchy update`, `omarchy-shell shell <ipc>`, `omarchy-restart-shell`,
   `omarchy dev ui preview [section]` (qs.Ui gallery), `omarchy hook install`.
9. `~/.config/omarchy/shell.json` exists and is customised (authoritative,
   no deep-merge with defaults). `~/.config/omarchy/plugins/` is empty.
10. No `~/Seldon` and no `~/Omarchy-Agent` on the dev host. The WP-043 source
    is on a backup drive; not needed before Phase 4.
11. git: user.name/email set, `init.defaultBranch=main`, `pull.rebase=true`,
    `push.autosetupremote=true`, rerere on. gh is logged in (ssh protocol).

## Changes made to the dev host during preparation (2026-10-01)

Recorded here because Seldon does not exist yet to record them.

- `omarchy install dev-env rust` → rustup into `~/.rustup`, `~/.cargo`;
  the installer appended `. "$HOME/.cargo/env"` to `~/.bashrc`,
  `~/.profile`, `~/.bash_profile`.
- `rustup target add x86_64-unknown-linux-musl`.
- `mise use -g just@latest` → `just = "latest"` in `~/.config/mise/config.toml`.
- Repository created at `~/Work/johnandrewsx/jax-seldon`, pushed to
  `github.com/JohnAndrewsX/jax-seldon` (private).
- Nothing under `/etc`, no pacman, no sudo, no systemd changes.
