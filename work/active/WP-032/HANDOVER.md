```
WP-032 HANDOVER
Done: seldon rebuild [--json] writes outputs/REBUILD.md (seven sections, English headings, prose de/en), golden test, docs/TESTING.md notes
Not done: nothing in scope; follow-ups under "Decisions needed"
Verified by: just check (exit 0), cargo test --test rebuild (7 tests after review round 1), manual run on a fixture copy in a scratch home with SELDON_TEST_GUARD
Learned: memory/rust-notes.md + memory/pitfalls.md, section "WP-032"
Decisions needed: 6 (below); none blocks the merge
Touched outside WP scope: engine/src/lib.rs and engine/src/commands/mod.rs (one `pub mod rebuild;` each, needed to compile)
```

Branch `wp/032-rebuild`, worktree `wt/WP-032`. Main has not moved since the
branch point (checked before the handover). No PR, no push.

## What was done

- `engine/src/rebuild/mod.rs`: `collect(built, dossier, since, title)`
  builds the sections from `index::derive`'s `Built` (`folded` events with
  their winning resolution, `index.drift`, `summary.openDrift`) and the
  dossier fences. It does not derive cases, drift or resolutions a second
  time. `Built` has no fences, so `read_dossier` reads `system/*.md` with
  the public `index::load::{fences, fence_table, fence_kv}`. Also:
  `origin()` (repo or AUR from `meta.command` via `pkgcmd`) and `merge()`
  (the `rebuild` fence).
- `engine/src/rebuild/render.rs`: the Markdown. Headings are English
  (`## 1. Base` … `## 7. Open questions`, `### Without a case`,
  `### System units`, `### Deliberately not reproduced`). Prose is in the
  logbook language (en/de word tables, like `index/views.rs`). The text
  depends on no clock: "as of" is the newest ledger event.
- `engine/src/commands/rebuild.rs`: takes the lock, derives, renders and
  merges. The file is written atomically (`sys::write_atomic`) only when
  its bytes change. A change is autocommitted as `seldon: rebuild`. No
  ledger write and no index rebuild. Exit 3 without a logbook, exit 4
  when the lock is held (shared `ctx.lock()`).
- `main.rs`: one variant and one arm.
- `engine/tests/rebuild.rs` + `engine/tests/golden/REBUILD.md`;
  `docs/TESTING.md`: a table row and the manual run.

### Section rules (what the code does)

1. **Base:** `meta.to` and the day of the newest omarchy `update` event,
   else `omarchy.summary` `version`/`lastUpdate`.
2. **Packages:** pacman `install` with `explicit: true`, minus a later
   `remove` of the same subject, minus dismissed ones. Grouped by the
   folded `case` (cases by id, then "Without a case"). Each line is
   `omarchy pkg add <pkg>` when the logged command is `pacman -S`,
   `omarchy pkg aur add <pkg>` when it is `pacman -U <file>` (how yay
   installs what it built), else "repository unknown" with both commands.
   Each line also shows: the version, `linked` / `explained: <detail>` /
   **open**, the agent, the event id. At the end there is one combined
   line per origin, open packages left out. The intro gives the logbook's
   start day and the dossier's explicit count, so the reader knows
   pre-logbook packages are not listed.
3. **Deviations:** the `deviations.table` rows first, in table order.
   Reason and case come from the table, filled from the path's latest
   config event, whose id, resolution and open state are attached. Then
   config paths the table lacks. Left out: `~/.config/systemd/`, dismissed
   paths, a file added and removed again. A removed pre-existing file
   reads "remove this file".
4. **Plugins:** the non-first-party rows of `plugins.list`, plus ids the
   ledger added. The ledger's `plugin-remove`, dismissal and enabled state
   win over the dossier. A clone (clonedFrom) is `omarchy plugin clone
   <from>` with the note "(becomes `<username>.<id>`, enabled by the
   clone)". The script names the copy `${USER}.<id without omarchy.>` and
   enables it itself (Omarchy 4.0.4). Only a clone that the dossier lists
   as disabled gets "then `omarchy plugin disable <username>.<id>`". Any
   other row is `omarchy plugin add <url>` (`meta.url`, which no producer
   writes yet) or `omarchy plugin add <url>` with "source URL not
   recorded", followed by `omarchy plugin enable <id>` or "stays
   disabled". One extra line lists the first-party plugins disabled here,
   except a listed clone's source (the clone switches that one off).
5. **Theme:** the newest `theme-set` that was not dismissed →
   `omarchy theme set <name>`; else the dossier's theme.
6. **User units:** unit files from config events under
   `~/.config/systemd/` (the latest event per path, not dismissed, not a
   file added and removed again). A drop-in `<unit>.d/<file>` belongs to
   `<unit>` and reads "(drop-in for `<unit>`)". Each line says "restore
   the file", then `systemctl --user enable --now <unit>` when
   `services.enabled` lists the unit as user-scope, else `daemon-reload`.
   A pre-existing unit file that was removed reads "remove this file, then
   `systemctl --user daemon-reload`", like section 3. Then `services.enabled` rows **that have a case** and
   no file event; system scope goes under `### System units` with
   `sudo systemctl enable --now`.
7. **Open questions:** `index.drift` as the index orders it (crises first
   when capped; a "… N more: `seldon drift`" line beyond the cap), each
   with its event id and the three decision commands in the intro. Then
   "Deliberately not reproduced": dismissed changes of state (install,
   remove, plugin-*, theme-set, config-*). A pacman transaction appears
   once, led by its first explicit install. Its count is every dismissed
   member of the transaction, dependencies included. Dismissed version moves (upgrade,
   downgrade, update, plugin-update) are left out.

`--json` → `{path (absolute), sections: {packages, deviations, plugins,
units, open}, files: ["outputs/REBUILD.md"] | [], git, warnings}`.
`open` = `summary.openDrift` (open drift items; dismissed ones are answered
and not counted). `units` counts user and system units listed. On the
fixture: `{packages: 4, deviations: 5, plugins: 3, units: 2, open: 4}`.

## How it was verified

- `just check` → `check: ok` (fmt, clippy `-D warnings`, all engine tests,
  schema-validate, plugin-validate, qmllint, plugin-test on the dev host).
- `cargo test --test rebuild` (7 tests since review round 1, all in `common::Env` with
  `SELDON_TEST_GUARD`):
  - golden on a fixture copy, also checking the seven headings in order
    and German prose;
  - every package line's last code span is the id of an explicit
    `install` event of the package the command names (asserted against the
    ledger);
  - a second run at a later clock gives `files: []`, the same bytes and no
    commit;
  - user text above and below the fence survives a change, and the fence
    appears once;
  - `seldon: rebuild` is committed exactly once per change (git repo made
    in the test);
  - `drift dismiss` / `drift explain` move items into "Deliberately not
    reproduced" and out of the open questions;
  - appended ledger lines: `-U` → aur, no command → repository unknown, a
    later `remove` drops btop (English logbook);
  - an empty logbook says "none"; exit 3 without a logbook.
- Unit tests in `rebuild/mod.rs`: the repo/AUR rule and the fence merge.
- Manual run (docs/TESTING.md block) in a scratch home with all XDG
  variables and `SELDON_TEST_GUARD`: first run writes, second
  `files: []`, `diff` against the golden is empty. Nothing touched the
  real XDG dirs or `fixtures/`.

## Reviewer walkthrough

Read `engine/tests/golden/REBUILD.md` (the fixture's document, German
prose) as someone who has just installed a fresh Omarchy:

1. **Base:** "Omarchy 4.0.7-1 · aktualisiert am 2026-10-01". Install
   Omarchy, then run `omarchy update` until 4.0.7-1 or newer runs.
2. **Packages:** run `omarchy pkg add btop tailscale zed` (the combined
   line). The per-case lines say why: zed for C-2026-004, tailscale
   linked to C-2026-008, btop explained ("Kleines Monitoring-Tool …").
   `ollama` is marked **offen**: skip it until step 7 is decided.
3. **Deviations:** restore five files from your dotfiles or backup:
   input.conf, shell.json, .bashrc, monitors.conf (C-2026-002),
   bindings.conf (C-2026-004). Each line has its reason.
4. **Plugins:** `omarchy plugin clone omarchy.clock`. It becomes
   `<username>.clock` and the clone enables it. Re-add weather-plus and
   tyme from their git URLs (not recorded, so the document says so);
   enable weather-plus, leave tyme disabled. Then disable the five listed
   first-party plugins if the fresh install has them on. `omarchy.clock`
   is not in that list, because the clone already switched it off.
5. **Theme:** `omarchy theme set tokyo-night` (still open drift; proposed
   for C-2026-005).
6. **Units:** restore `~/.config/systemd/user/ollama.service` and
   `systemctl --user enable --now ollama.service` (open, from agent:codex),
   and `sudo systemctl enable --now tailscaled.service` (C-2026-008).
7. **Open questions:** four undecided items, each with its id; decide
   with `seldon drift link|explain|dismiss`. Do not bring back `kanagawa`
   ("Nur ausprobiert.").

The reviewer judges whether this reads well. Two things are deliberately
explicit: the document never shows file contents (Seldon has only
hashes), and pre-logbook packages are only counted (327), not listed.

## Decisions needed (none blocks the merge)

1. **AUR rule.** `pacman -U <file>` → `omarchy pkg aur add`, `pacman -S` →
   `omarchy pkg add`, anything else → "repository unknown". A local
   `makepkg -i` build also logs `-U`, so it is shown as AUR. The fixture
   has no `-U` line; the rule is proven by the appended-line test and
   memory/host.md item 6 (yay's `-S … -- repo/pkg` for repo packages).
   Accept, or record it in an ADR?
2. **Open drift stays in its section**, marked **open**/**offen** with
   "see 7". The other choice was to show it only in section 7. I chose
   this so the document matches the machine. `open` in `--json` counts
   open drift items only.
3. **Section 6 goes beyond the WP text.** It also lists `services.enabled`
   rows that carry a case. That puts `tailscaled.service` (system,
   C-2026-008) under `### System units`; without it, tailscale would not
   run after a rebuild. `units` counts them. Keep it, or drop it to stay
   strictly "user units from config events"?
4. **Plugin source URL.** The plugins collector records no URL, so every
   third-party `plugin add` reads `<url>` ("source URL not recorded").
   Proposed follow-up: the collector records `meta.url` (the plugin
   directory's git remote, redacted) on `plugin-add`. `rebuild` already
   reads it, as a non-conventional `meta` key. That needs ADR-0012 §3's
   conventional-key list, or stays in `extra`.
5. **The first-party "disabled here" line** cannot know Omarchy's
   defaults. It lists every first-party plugin that is disabled and says
   "disable each after the install". Fine, or should it be dropped?
6. **Duplicated fence merge.** `rebuild::merge` repeats
   `index::views::merge_status` with another fence name (index/ is
   outside this WP). A later cleanup could make `merge_status` take the
   fence name.

### SPEC-ENGINE §3 wording for the orchestrator

Replace the `seldon rebuild` line with:

```
seldon rebuild                                 # outputs/REBUILD.md (WP-032): 1 base, 2 explicit packages by
                                               # case (`omarchy pkg add|aur add` from meta.command: pacman -S →
                                               # repo, -U → AUR, else "repository unknown"), 3 deviations, 4 plugins,
                                               # 5 theme, 6 units, 7 open drift + dismissed ("deliberately not
                                               # reproduced"); English headings, prose in the logbook language;
                                               # fence `rebuild`, text outside kept; written atomically and only on
                                               # change, autocommit `seldon: rebuild`; no ledger write, no index
                                               # rebuild. --json → {path, sections: {packages, deviations, plugins,
                                               # units, open}, files, git, warnings}; open = open drift items
```

## Review round 1 (APPROVE with six small items, all fixed in one commit)

1. **Clone line.** Renders only `omarchy plugin clone <from>` plus
   "(becomes `<username>.<id>`, enabled by the clone)" (de: "wird zu …,
   vom Klonen aktiviert"); the `enable user.clock` step is gone. I checked
   `omarchy-plugin-clone` read-only on the host: `new_id=
   "${USER…}.${source_id#omarchy.}"`, then `omarchy-plugin-enable
   "$new_id"`. One addition beyond the review: a clone the dossier lists as
   *disabled* gets "then `omarchy plugin disable <username>.<id>`",
   because otherwise the document would leave it enabled. The fixture has
   no such clone.
2. **"Disabled here" line** leaves out a listed clone's source.
   `omarchy.clock` is gone from the golden.
3. **Dismissed transaction count.** I chose to count every dismissed
   member, dependencies included. Covered by the new test
   `a_dismissed_transaction_counts_every_member_and_units_show_drop_ins_and_removals`:
   an explicit `foo` plus a dependency `libfoo` in one transaction,
   dismissed via `seldon drift dismiss` → "pacman install `foo` · 2 events
   in this transaction". The old code would have counted 1.
4. **Removed unit file** (config-remove of a pre-existing file) now shows
   in section 6: "remove this file, then `systemctl --user
   daemon-reload`". Same test.
5. **Trace assertion** is the helper `packages_trace`, which accepts
   `omarchy pkg add` and `omarchy pkg aur add`. The English test now runs it
   over its AUR line too.
6. **Drop-ins.** `rebuild::unit_of` maps `…/<unit>.d/<file>` to `<unit>`,
   with the unit test `a_drop_in_belongs_to_its_unit`. Section 6 shows
   "(drop-in for `<unit>`)", and the enable command names the unit (in the
   new test).

Golden diff: only the clone line and `omarchy.clock` leaving the disabled
list. Counts unchanged (4/5/3/2/4).

Guard note: a read-only `grep` whose *pattern* contained the plugin-clone
command text was blocked as "omarchy command that changes the system".
Nothing ran. I did not reword the command; I read the file with the Read
tool instead.
