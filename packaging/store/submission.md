# Store submission: JAX Seldon (draft for the operator, WP-042)

The issue that lists `jax.seldon` in the Omarchy plugin store
(`omacom/omarchy-plugin-marketplace`). **Not filed.** Filing needs the
operator's separate go after the `v0.2.0` tag and the E65 rulesets; the
operator checks every attestation. Form read at store commit
`5d22a9fbee57f7b22652e09df128ddf12f14a65b` (`SUBMISSION.md:57-126`,
`.github/ISSUE_TEMPLATE/submit-plugin.yml`); re-read it before filing.

## Title

```
[Plugin]: JAX Seldon
```

## Body

Everything between the two rules, as it would be passed to
`gh issue create --body-file`.

---

### Repository URL

https://github.com/JohnAndrewsX/jax-seldon-plugin

### Category

System

### Tags

system, bar, ai

### Suggest a missing tag

_No response_

### Maintainer notes

This repository is generated from `plugin/` of [JohnAndrewsX/jax-seldon](https://github.com/JohnAndrewsX/jax-seldon) (`git subtree split` on each release tag); issues and pull requests go there.

**External dependency: the `seldon` engine** (Rust CLI, MIT, same project). The plugin reads the index the engine writes (`~/.local/state/seldon/index.json`) and runs `seldon` with fixed argument lists for every capture and every change the user makes from the panel; the engine does the writing. Without the engine the plugin shows an "engine not installed" banner. The engine is installed separately, from the GitHub release with `install.sh` (README, "Install": download, read, verify, run). `install.sh` checks the release tarball against the release's `SHA256SUMS`, refuses on a mismatch, installs `~/.local/bin/seldon` as the user and never uses root; with the GitHub CLI logged in it also verifies the tarball's build provenance (`gh attestation verify`, tag-bound to the release workflow), and `--require-verified` refuses without that check. An AUR package `jax-seldon` is planned.

**One-click install while the AUR package does not exist** (`Model.js:64`, [ADR-0024](https://github.com/JohnAndrewsX/jax-seldon/blob/main/decisions/ADR-0024-install-fix-during-aur-pause.md)): the engine-missing banner's *Install* runs the constant `curl -fsSL https://github.com/JohnAndrewsX/jax-seldon/releases/latest/download/install.sh | bash` in Omarchy's floating terminal, only on the user's click, with the command shown first. On this path `install.sh` itself is not checked against `SHA256SUMS`; it still checks the engine. The README says so, and its manual path checks `install.sh` against `SHA256SUMS` and, optionally, with `gh attestation verify` (the release workflow attests `install.sh`, `SHA256SUMS` and both tarballs). When the AUR package exists, the button runs `omarchy pkg aur add jax-seldon` instead.

**`privilege`**: the optional snapshot grant `sudo setfacl -m u:$USER:rx /.snapshots` ([ADR-0026](https://github.com/JohnAndrewsX/jax-seldon/blob/main/decisions/ADR-0026-snapper-read-grant.md)): read access to the snapshot directory listing and the snapshot info files, no snapshot creation, change or deletion. It runs only when the user presses *Grant* on the snapshot banner, in a terminal that shows the command and asks for the password; the plugin never runs it on its own. The README's removal of the AUR package (`omarchy pkg drop`, which runs `sudo pacman -Rns`) is the other password step.

**`package-manager`**: README lines that update or remove the engine's AUR package, and an advice text in the transaction view (`pacman -S`). The plugin never starts a package manager.

The plugin itself makes no network connections and writes no files; its own settings (desk width, sidebar) are stored by the shell in the plugin's entry of `~/.config/omarchy/shell.json` when the user changes them. Security contact: `SECURITY.md`.

### Submission checklist

- [ ] The repository is public and contains installation and removal instructions.
- [ ] I have documented the plugin license and any external dependencies.
- [ ] I confirm that I own or have permission to submit this plugin and its preview assets.
- [ ] The plugin does not overwrite user configuration without explicit consent.
- [ ] I understand that approval is for listing and is not a security review.

---

## For the operator

The store creates the submission only when all five boxes are checked
(`SUBMISSION.md:102`, `:140`). Check a box (`[x]`) only when the statement
is true for you:

1. *Public, install and removal*: `jax-seldon-plugin` is public; README
   "Install" and "Remove" (removal of the plugin, the engine from GitHub
   and from the AUR).
2. *Licence and dependencies*: `LICENSE` (MIT) in the root; README
   "Requirements" names the `seldon` engine and the logbook.
3. *Ownership*: yours to confirm. `preview.png` and the README images are
   renders of the plugin's own QML (`docs/TESTING.md`) and the project's
   own artwork.
4. *No overwrite without consent*: the plugin writes only its own settings
   entry in `~/.config/omarchy/shell.json`, through the shell, on a click or
   a slider release; the shell writes the whole file back in its own
   formatting (README, "Security, privacy, privileges").
5. *Not a security review*: the store's own wording.

Category and tags are proposals. `System` matches the manifest's bar
category. Tags: `system` (what it records), `bar` (the pill), `ai` (agents
read and write the logbook through the engine). The study proposed
`quickshell, system`; any one to three of the allowed tags work.

## Checked against the form

| Form (`SUBMISSION.md`, `submit-plugin.yml`) | This draft |
|---|---|
| Title `[Plugin]: plugin_name`, human-readable name (`:98`, `:111`, `:115`) | `[Plugin]: JAX Seldon`, the manifest's `name` |
| Six headings in order (`:63-83`, `:100`) | Repository URL, Category, Tags, Suggest a missing tag, Maintainer notes, Submission checklist |
| Repository root URL, no trailing slash or path (`:95`) | `https://github.com/JohnAndrewsX/jax-seldon-plugin` |
| One category, exact spelling (`:24-34`, `:53`, `:96`) | `System` |
| One to three tags from the list (`:36-51`, `:97`) | `system, bar, ai`, comma-separated (`:53`) |
| `_No response_` where nothing is added (`:77`, `:100`) | Suggest a missing tag |
| Maintainer notes: installation, permissions, dependencies (form `notes`) | engine, `install.sh`, the one-click banner, both capabilities |
| Five checklist lines verbatim (`:85-89`; form `checks`) | verbatim, unchecked for the operator |
| Agents show title and body and create the issue only after explicit approval (`:117-126`) | not filed; this file is the body to approve |

Differences between the two forms at that commit: the web form's tag
labels are capitalised (`Quickshell`, `Power management`), the CLI guide's
values are lower-case (`quickshell`, `power-management`); this draft uses
the CLI guide's values, as it would be filed with `gh issue create`.

## When filing (not WP-042)

- Search the store's issues for the repository URL and `jax.seldon` first;
  reuse an open request (one issue only).
- Rerun the baseline (`baseline.md`) on the `jax-seldon-plugin` commit to
  be filed; freeze that commit until the store has validated it, and
  record the validated full SHA.
- Answer the bot's validation and baseline comments on that issue. Later
  updates go through the store's verification form with the full 40-hex
  SHA (`SUBMISSION.md:142-146`).
