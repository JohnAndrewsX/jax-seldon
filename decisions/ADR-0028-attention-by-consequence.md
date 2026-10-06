# ADR-0028 — Attention is earned by consequence: routine changes are history, not drift

**Status:** accepted (operator decision 2026-10-06: v2 confirmed; a package installed by hand without a case is quiet attention; existing default watch lists gain the new paths, §4d). Ships in 0.1.4 (§8).
**Date:** 2026-10-06

> Extends [ADR-0027](ADR-0027-act-then-account.md) (act, then
> account). Amends ADR-0008 (what "Crisis" means), ADR-0013 §3 (routine
> test, `alwaysRed` in a full upgrade, the `update` row), ADR-0014 §2
> ("`crisis` derives from the zone"). ADR-0012 §6 (drift eligibility),
> ADR-0019, ADR-0020, ADR-0021, ADR-0023 §2 (risk scale) stand.

## Context

On the test host the operator switched the Omarchy theme from Omarchy's own
menu. Seldon recorded `theme-set` (source `theme`, zone yellow) and, because
no case covered it, made it open drift: the bar counted it, the panel asked
for *link / explain / dismiss*. Operator (translated): "Why must the user
explain why a theme was changed? … The user should notice as little of
Seldon as possible: Seldon is there, does the work and is never in the way.
Omakase style! Simple, not intrusive, no extra work — unless the user wants
it."

Today's model (SPEC-ENGINE §5, ADR-0008/0012/0013/0014): every event of a
drift-eligible source without a case is drift; drift is a to-do with three
actions; `crisis` is "zone is red". On a normal desktop: a theme switch is
a to-do; a plugin toggle is a to-do *twice* (the `plugin-enable` event and
the `config-change` of `~/.config/omarchy/shell.json`, which the shell
rewrites on every toggle and layout change and which the config collector
watches); a plain `pacman -Syu` is one yellow to-do; every `omarchy update`
is a **crisis** — its `update` event is red by table, and so is its
`-Syu` group whenever it upgrades `omarchy`, a kernel, `systemd` or
`glibc`, because `is_routine` (`engine/src/index/drift.rs:139`) rejects
every `alwaysRed` subject and the group turns red (`build.rs:476`); if
AUR packages are installed, `omarchy-update-aur-pkgs` runs `yay -Sua`,
whose `pacman -U <cache>` lines are explicit upgrades and another red
group; every hand edit of a Hyprland file is a to-do; a migration that
runs `omarchy-refresh-applications` copies every shipped `.desktop` file
and makes one to-do per file. The two auto-explain rules (§5 rules 7 and
8) cover only the engine's own writes and Seldon's own updates.

The product promise (PROJECT.md) is a complete record and the ability to
answer "why is this here?" months later. That question is asked about
packages, third-party plugins and overrides. It is never asked about a
theme switch, a toggle, a routine upgrade or Omarchy's own updater: the
dossier carries the current state and REBUILD.md needs state, not the
history of switches. A *to-do* for such an event adds a human step without
adding to the record — what ADR-0027 §1 calls wrong.

Facts checked (read-only, dev host, Omarchy 4 tree at `$OMARCHY_PATH`):

- Theme state lives under `~/.local/state/omarchy/current/` (outside
  `watchPaths`); a theme switch yields one `theme-set` and no `config-*`
  event. `omarchy-theme-set` strips Lua and terminal configs only from
  themes that have a `.git` directory (`theme_came_from_a_repo`); a theme
  directory the user wrote may carry `hyprland.lua`.
- Hyprland config in Omarchy 4 is **Lua**: `config/hypr/hyprland.lua`
  requires `hypr.monitors|input|bindings|looknfeel|autostart` (all `.lua`;
  there is no `autostart.conf`) and `default/hypr/toggles.lua` loads every
  `.lua` under `~/.local/state/omarchy/toggles/hypr/`. Every one of these
  files is code that runs at login; `autostart.lua` is not special.
- `omarchy-update` runs, in order: pkg-prune, `omarchy-snapshot create`,
  `omarchy-update-dev` (only when `OMARCHY_PATH` is a git checkout),
  keyring, `pacman -Syu --noconfirm --overwrite '/usr/share/omarchy/*'`
  (`is_plain_full_upgrade` is true for that line; tested at
  `collectors/pacman.rs:572`), `omarchy-migrate`, `omarchy-hook
  post-update`, `yay -Sua --noconfirm …` (AUR), mise, orphan review
  (interactive only; exits without a prompt when stdin is not a tty).
  `omarchy refresh pacman` runs `pacman -Syyuu`, which can downgrade.
- 33 of 106 migrations touch `~/.config` or call `omarchy-refresh-config`
  (writes `<file>.bak.<epoch>`) or `omarchy-refresh-applications` (copies
  `$OMARCHY_PATH/applications/*.desktop`). One (`1786539345.sh`) writes a
  symlink under `~/.config/systemd/user/graphical-session.target.wants/`
  to `/usr/lib/systemd/user/omarchy-crash-watch.service`.
- `~/.config/omarchy/shell.json` is written by the shell process (its
  README, "Persisted state"): bar layout, per-entry settings, idle times,
  the enabled third-party plugin list. It is under `~/.config/omarchy`
  and not in `DEFAULT_SKIP_PATHS` (`config.rs:410`), so it is watched.
- Not watched today (`DEFAULT_WATCH_PATHS`, `config.rs:296`:
  `~/.config/hypr`, `~/.config/omarchy`, `~/.config/waybar`, `~/.bashrc`,
  `~/.zshrc`, `~/.local/share/applications`): `~/.config/systemd/user/`,
  `~/.config/autostart/` (Omarchy ships three `.desktop` files there),
  `~/.config/environment.d/`, `~/.config/uwsm/`, `~/.profile`,
  `~/.bash_profile`.
- Omarchy ships inert `*.sample` hooks under `~/.config/omarchy/hooks/`;
  `omarchy-hook` skips `*.sample` (line 25). `omarchy-version` prints `dev
  (<hash>)` or `dev` when `OMARCHY_PATH` is not `/usr/share/omarchy`; the
  collector's `valid()` (`omarchy.rs:110`) rejects the form with a space,
  accepts a bare `dev`.
- The plugins collector reads only the manifest `version` or the git HEAD
  (`plugin-update` fires on a change of either); the config collector
  excludes `~/.config/omarchy/plugins/`. An in-place edit of a non-git
  third-party plugin's QML produces no event.
- The session-start context (`commands/hook/context.rs`) lists no drift
  today; every line it emits is quoted (`quote()`, test
  `every_line_is_quoted`). `drift explain|dismiss|link` take `--actor`
  (`SELDON_ACTOR`). The AUR package ships the engine only
  (`packaging/expected-files.txt`); the plugin is installed through
  Omarchy's plugin system, so engine and plugin versions can skew.
- `plugin/components/DriftSheet.qml:313` reads "RESOLVE A RED-ZONE
  CHANGE" when `crisis`, `:370` shows `<zone> · crisis`; `Model.js:352`
  and `:682` build "(N in the red zone)" and the strip text from
  `summary.crisis`. These are labels; behaviour keys off `crisis` only.

Assumptions, named: (A1) provenance of a theme switch, a toggle or a
package command (menu, terminal, agent) is not observable, so the rule
rests on consequence; (A2) `is_plain_full_upgrade` holds for `-Syyuu`
(`-y`/`-u` repeated are flags, `targets` empty) — WP-A adds the test; (A3)
Omarchy's monitor-scaling menu is said to `sed -i` `monitors.lua`; not
found under `bin/` on this host — WP-D measures which Omarchy UI actions
write under `~/.config/hypr`; (A4) ADR number 0028 is free (WP-095 also
wants its own ADR; the orchestrator assigns numbers).

## Decision

### 1. Principle: attention is earned by consequence, not by provenance

Every change is **recorded**; that is the promise and it does not change.
Whether a recorded change also **needs attention** is decided by what a
*wrong* one would cost, through three tests applied in order:

1. **Harm test → crisis.** Could it break boot, login, the shell or
   security, or lose data, so that the desktop cannot repair it — *and*
   did nobody ask for it in a case? Then it is a crisis: a *named*
   transaction on an `alwaysRed` package (ADR-0023 R3 subjects) and a
   write to a *persistence path* (§2, code that runs at login or on
   events without being the user's ordinary configuration surface).
   Crisis is the one visible signal Seldon has; it must stay rare, or it
   is no signal.
2. **Reason test → attention.** Would "why is this here?" need an answer
   months later — does it create durable state a rebuild would have to
   reproduce *with intent*: a package installed or removed by name, a
   third-party plugin added, removed or updated, an override under a
   watched path? Then it is open drift — **quiet**: listed in the panel,
   never a demand, explained by the agent where the agent has evidence,
   never counted in the bar by default.
3. **Everything else → routine.** Reversible in one step from the
   desktop, or the normal operation of Omarchy itself (its updater, its
   theme switcher, a toggle, a plain full upgrade, a file Omarchy itself
   ships), or an upgrade of something already present. Recorded as
   history in the Changelog; **not drift**. No reason is ever asked.

Where the operator's two intents pull apart — "nothing done through
Omarchy's own UI ever asks for a reason" against "no new silent path for
persistence or unsandboxed code" — this ADR chooses as follows. A
third-party plugin installed from Omarchy's menu and a package installed
from a terminal are *attention*, not routine: the panel lists them, the
bar does not change, nothing is asked; the record keeps the question open
for the agent or a curious user. Code that runs at login without being
configuration (units, autostart entries, hooks, environment files) is a
*crisis* whoever wrote it, because the one thing a logbook must not do is
stay quiet while persistence is planted. Hyprland Lua, shell rc, `.desktop`
launchers and themes are the user's ordinary configuration surface and
stay attention: a red bar after every binding tweak would train the user
to ignore red, and the files remain visible in the panel and in every
agent session.

Provenance does not enter the tests (A1). Attribution keeps doing what it
does: an agent's event inside a case is linked (§5 rule 1), a dependency
inherits (rule 2), a planned subject is proposed (rule 3). The tests apply
*after* rules 1–3, so a routine event an open case names still gets its
proposal (§3).

The test for any future event kind: write the three questions into the
kind's row in SPEC-ENGINE §5; "needs attention" must be argued from the
cost of a wrong one, never from "we would like a reason".

### 2. Classification table (normative, total; replaces the zone-derived crisis)

Zone stays the ledger field of ADR-0014 (where the change acts). The
*class* is computed at index time from the event, the config and these
rules; it is not written to the ledger. A pacman group's class is the
**highest** class of its members; an event no row names is **attention**.
An event with any resolution keeps it, whatever its class.

| Event | Class | Rule / resolution |
|---|---|---|
| pacman, plain full upgrade (`-Syu`, `-Syyuu`, `-Su`, bare `yay`; names no package): every member of kinds `upgrade`, `reinstall`, `install`, `remove` (`:: Replace`), **including `alwaysRed` subjects** | **routine** | `sysupgrade`. Amends ADR-0013 §3: a distro-driven upgrade of the kernel is Omarchy's normal operation; the R3 guard for `alwaysRed` is ADR-0027 §2c's stop *before* an agent's step, not a crisis after the user's updater ran |
| pacman, plain full upgrade: `downgrade` or `remove` of an `alwaysRed` member | **attention** | `sysupgrade-red`: rare, distro-driven, reversible; told quietly |
| pacman `upgrade`/`reinstall` named on the command line or via `-U <cache path>` (yay/paru), not `alwaysRed` | **routine** | `upgrade`: an upgrade of a package already present creates no new durable state; its reason was recorded when it was installed |
| pacman `upgrade`/`reinstall` named, `alwaysRed` (`pacman -S linux`, `-U linux-…`) | **attention** | `upgrade-red` |
| pacman `install`/`remove`/`downgrade` named on the command line, not `alwaysRed` | **attention** | reason test; **operator decision 2026-10-06**: quiet attention, not a crisis |
| pacman `install`/`remove`/`downgrade` named, `alwaysRed`, outside a plain full upgrade | **crisis** | harm test |
| pacman dependency of an explicit member | follows its explicit member | rule 2 and ADR-0013 grouping unchanged |
| pacman explicit transaction naming only `[drift] routinePackages` (default `archlinux-keyring`, `omarchy-keyring`) | **routine** | `keyring` |
| pacman of `jax-seldon`, `plugin-*` of `jax.seldon` | auto-explained | §5 rule 8 unchanged |
| omarchy `update`, both versions package-shaped (`N…-N`), attributed (`omarchy.rs:132`) to an `omarchy`/`omarchy-dev` pacman event inside a plain full upgrade | **routine** | `omarchy-update`; stays a `release` marker on the timeline |
| omarchy `update`, otherwise (bare `dev`, downgrade, unattributed) | **attention** | nothing is broken; a dev checkout is the developer's own doing and the record says so |
| `plugin-add` / `plugin-remove` / `plugin-update`, third-party (ADR-0018) | **attention** | reason test; unsandboxed code in the shell — visible, listed to every agent session; the capture gap for in-place edits is WP-E |
| `plugin-enable` / `plugin-disable` | **routine** | `plugin-toggle` (one click back; the add stays attention until resolved) |
| `theme-set` | **routine** | `theme`; the dossier carries the current theme |
| `config-*` whose hash matches an engine-owned write, or a template compiled into the engine (the theme hook `THEME_HOOK_SCRIPT`, the watcher unit with any `ExecStart` prefix) | auto-explained | §5 rule 7, evidence extended by the built-in templates so a lost state directory or `install.sh --unit` yields no crisis |
| `config-*` with `meta.matches = "omarchy-default"`: at capture the new content equals the same relative file under `$OMARCHY_PATH/config/` (for `~/.config/<rel>`) or `$OMARCHY_PATH/applications/` and `default/alacritty/Alacritty.desktop` (for `~/.local/share/applications/<name>`) | **routine** | `omarchy-default`: Omarchy's own copy, by evidence. Covers `*.sample` hooks, `refresh-config`, `refresh-applications`, migrations that restore defaults |
| `config-*` with `meta.matches = "system-link"`: the subject is a symlink whose resolved target lies under `/usr/` | **routine** | `system-link`: `systemctl --user enable`, Omarchy migrations; root-owned content is not user persistence |
| `config-*` under `[drift] routinePaths` (default `~/.config/omarchy/shell.json`, `**/*.bak.*`) | **routine** | `routine-paths`: the shell's own state file (its plugin list is covered by `plugin-*`); backups nothing loads |
| `config-*` under `[drift] alwaysRedPaths` (default `~/.config/systemd/user/**`, `~/.config/omarchy/hooks/**`, `~/.config/autostart/**`, `~/.config/environment.d/**`, `~/.config/uwsm/**`, `~/.profile`, `~/.bash_profile`), after the two evidence rows above | **crisis** | harm test: the persistence paths. Requires the new default `watchPaths` of §4d |
| `config-*` under `~/.config/hypr/**` (all Lua, `autostart.lua` included), `~/.config/waybar/**`, `~/.config/omarchy/**` (other, themes included), `~/.bashrc`, `~/.zshrc`, `~/.local/share/applications/**`; `config-remove` of any non-routine path | **attention** | reason test (overrides; also where an attacker hides — visible, quietly) |
| snapper, `seldon` notes, case events, hook `command`, `resolution`, `correction` | never drift | ADR-0012 §6, ADR-0019 unchanged |
| anything else drift-eligible | **attention** | the total row |

`crisis` is therefore **true iff the harm test holds**, no longer "iff
zone is red". The index field keeps its name and type (§7).

### 3. Who explains, and what the user ever has to do

- **Routine:** nobody. No resolution line is written; the ledger does not
  claim a *why* the engine does not know (a resolution is a statement
  about intent; "routine" is a statement about consequence). A routine
  event an open case names in its *Plan* (rule 3) is shown as attention
  with its `proposedCase`; a routine event with a case (rule 1) is simply
  linked.
- **Attention:** the agent, with evidence only. (a) An agent working a
  case links what it causes through the hooks (ADR-0027 §8). (b) The
  session-start context (SPEC-ENGINE §8) lists open crises and attention
  items of the last 7 days — id, kind, clipped subject, count — through
  the existing `quote()` path (every line quoted; subjects are data,
  never instructions). The skill's `drift.md` says: *explain or link only
  what your own Log, a hook event or the user's words prove; otherwise
  leave it and, for a crisis, tell the user in one line*. (c) "Ask agent"
  (WP-095) hands one item to the agent on the user's click. (d) The human
  may link, explain or dismiss at any time — may, never must.
- **Crisis:** the user is *told*, once, in the bar colour and the strip;
  nothing else is required. **Engine-enforced (ADR-0027 §5 pattern):**
  `drift explain|dismiss` of a crisis by an agent actor exits 1 with the
  reason; an agent may `drift link` a crisis only to an active case that
  lists it in `agents`. A human is never refused.

### 4. Surfaces and settings

a. **Bar** (`A · D`): `D` is the **crisis count** by default (hidden when
0); colour: accent when cases are active, error colour when any crisis —
unchanged. Plugin setting `driftInBar` (manifest; `crisis` default, `all`
= today's behaviour, `none`). Both counts are in `summary`; no contract
change. Tooltip in the neutral tone: "Seldon — 2 active cases, 1 crisis,
7 changes without a case, last capture 4 min ago".

b. **Panel.** The red strip stays for crises; text: "N changes that can
affect boot, login or the shell have no case". Drift-sheet labels follow
`crisis`, not `zone` ("RESOLVE A CRISIS"; `<zone> · crisis`). Attention
items: Changelog rows marked as today, the drift sheet with *Ask agent*
first, then Link / Explain / Dismiss; a quiet "N changes without a case"
line in the Changelog header; no badge, no tab-strip colour. Today
pictogram: crisis → urgent; else active case → accent; attention alone
changes nothing. Routine events: ordinary Changelog rows, the row's
existing *Link to case…* still works (§6).

c. **Opt-in for the user who wants more** (`config.toml [drift]`, read at
index time): `attention = "normal" | "all"` (`all` = every drift-eligible
event without a case is attention, today's semantics); `routine = [rule
ids]` (default all of §2; drop `"theme"` to make theme switches attention
again); `routinePaths`, `routinePackages`, `alwaysRedPaths` (globs,
defaults in §2); `alwaysRed` unchanged. `seldon drift --all` lists routine
events too; `drift show` reports `class` and `rule`; `doctor` prints the
effective `[drift]` rule set and marks non-default values. **Decided
(operator, 2026-10-06):** a package someone installs *without a case* is
quiet attention, not a crisis. Reasons recorded: the harm test does not
hold for an ordinary package; a red bar after every `pacman -S htop`
spends the one signal Seldon has on alarm fatigue; the record is complete
either way, the item is listed to every agent session, and a user who
wants red for installs sets `attention = "all"` or widens `alwaysRed`.

d. **Watch paths.** The persistence paths of §2 need six new default
`watchPaths` (`~/.config/systemd/user`, `~/.config/autostart`,
`~/.config/environment.d`, `~/.config/uwsm`, `~/.profile`,
`~/.bash_profile`). Capture cost: a dozen small text files, SHA-256 only
(SPEC-LOGBOOK §7: hashes and `~`-paths in the ledger, never content).
`init` writes them into a new config. **Decided (operator,
2026-10-06):** an existing config whose `watchPaths` still equals the
default list of an earlier engine gains the new defaults on upgrade (the
engine adds them, says so once in the capture output, and the CHANGELOG
names them); a config whose `watchPaths` the user changed keeps its own,
and `doctor` names the missing paths with the one-line fix. Seldon never
widens a list the user wrote. Anything beyond these (plugin trees,
`~/.ssh/authorized_keys`, the toggles state directory, `/etc/pacman.conf`)
is a capture-cost and scope question for WP-E/WP-F, not this ADR's core.

### 5. Migration, append-only, idempotency

Classification is computed on every index build from ledger and config:
nothing is written to the ledger, nothing rewritten. The two capture-time
evidence marks (`meta.matches`, `meta.linkTarget` is **not** recorded —
only the boolean fact as a string key) are written into *new* events
only; old events without them classify by path. On the first build with
this rule set, open `theme-set`, toggles, routine groups, `update` rows,
`shell.json` changes and Omarchy-default copies leave `index.drift`;
crises that fail the harm test become attention. Ledger lines, zones and
resolutions stay and keep folding. Two builds from the same ledger and
config give byte-identical indexes (golden test pinned to one config).
`series.drift` is recomputed: a resolution counts as resolved only if its
`refersTo` counted as opened (so the curve cannot go negative); the
historical curve changes and the CHANGELOG says so. `attention = "all"`
restores today's picture, which is the rollback.

### 6. Costs and risks, and their limits

- *A real problem hidden as routine.* The routine set is narrow and
  evidence-based: `sysupgrade` requires a command line that names no
  package (ADR-0013 argv rule); `omarchy-default` and `system-link` are
  byte or target evidence recorded at capture; `upgrade` applies only to
  what is already installed; a theme has no code path unless the user
  wrote one (then it is attention under `themes/**`); hooks, units and
  autostart entries are crises.
- *The attacker who edits a file the user never looks at.* Persistence
  paths are crises and tunable; Lua, rc and `.desktop` files stay
  attention — visible, listed to every agent session, never aged out
  (ADR-0020 keeps crises first). Nothing here auto-resolves an event the
  engine has no evidence for. An attacker who can rewrite
  `~/.config/seldon/config.toml` to silence rules can rewrite the ledger
  too; the defence is `doctor`'s non-default marker and `drift --all`,
  not a self-auditing crisis (rejected, appendix F8).
- *Fewer reasons in the logbook.* Accepted: the reasons that feed
  REBUILD.md (installs, removals, plugins, overrides) stay attention.
- *A quiet list grows.* Accepted by design (Omakase); agents resolve what
  they caused; ADR-0020 bounds the index.
- *Semantics of "Crisis" change* (ADR-0008, CONCEPT.md): the word now
  means "can hurt and nobody asked", not "red zone". Docs follow (WP-C).

### 7. Contract

**No bump for v1.** `drift[].crisis`, `drift[].zone`, `summary.openDrift`,
`summary.crisis`, `resolution`/`resolutionDetail` carry everything the
plugin shows; attention count = `openDrift − crisis`. `drift[].zone` stays
the **ledger zone** of the lead event (pacman items are red by ADR-0014;
the computed yellow of `build.rs:476` goes, because routine groups are no
longer items): zone says where a change acts, and redefining it by the
harm test would pre-fill a false zone into the explain form and hence
into a resolution line. The schema *description* of `crisis` ("true iff
zone is red") becomes "true iff the item fails the harm test of
ADR-0028"; `contractVersion` stays 1 because no field's name, shape,
presence or producer changes and the plugin keys behaviour off `crisis`
alone. The skew case (old plugin, new engine; they ship separately) is
cosmetic: three labels say "red zone" where "crisis" is meant; WP-B fixes
them and the CHANGELOG names it. This ADR plus the fixture update in WP-A
is what AGENTS.md §3 requires for a semantic change; if the operator wants
the bump anyway, it is `drift[].class` and `summary.attention` (below)
and is done in the same PR.

**Contract v2 (v0.2.0) wish list, not decided:** `events[].routine:
"<rule id>"`, `drift[].class: "attention" | "crisis"`,
`summary.attention`.

### 8. Work packages

**Release sizing.** ADR-0028 ships in **0.1.4** with WP-A, WP-B and WP-C:
without all three the user sees either an engine that stopped counting
behind a plugin that still asks, or docs that promise what the engine does
not do. WP-D, WP-E and WP-F follow later (0.1.5 or on the operator's word);
none of them changes a row of §2 — they add evidence rules, watch paths or
sources. The six new default `watchPaths` of §4d are in WP-A because they
cost a dozen SHA-256s per capture and without them the crisis class would
cover hooks only; the orchestrator may still defer that one line item to
0.1.5 without touching this ADR — the `alwaysRedPaths` row then simply
does not fire for unwatched paths until it lands.

- **WP-A (0.1.4) — engine: classification by consequence.** Class after rules
  1–3; the total table of §2 with group = max; `[drift] attention,
  routine, routinePaths, routinePackages, alwaysRedPaths`; `crisis` by
  harm test; `drift[].zone` = ledger zone; `drift --all` / `drift show`
  (read folded events, not the capped `index.drift`); a `linkable` set
  (eligible, no case, no resolution) separate from `open_drift` so
  `drift link` of a routine event resolves it and `explain|dismiss` of a
  routine event exits 1 "routine"; agent-actor refusal on crises (§3);
  capture: `meta.matches` for `omarchy-default` and `system-link`,
  built-in template hashes as rule-7 evidence (theme hook; the unit with
  any `ExecStart` prefix); new default `watchPaths` with `init` and
  `doctor` behaviour (§4d); `series.drift` counting fix; port the rules
  to `scripts/validate-fixtures.py` (`ALWAYS_RED`, `routine()`, the
  `crisis` line at 672); fixture: one crisis on a hook path (yellow
  ledger zone, `crisis: true`), attention items with and without
  `proposedCase`, a `plugin-enable`, a downgrade, a `config-remove`, a
  routine `theme-set`, `-Syu` group and `shell.json` change as plain
  events. SPEC-ENGINE §2, §4 (capture marks), §5, §6; schema description.
  *Acceptance:* table-driven test over every row incl. `-Syyuu` (A2) and
  the Omarchy update line; two builds identical, zero ledger writes;
  migration fixture → routine rows leave `drift`, ledger byte-identical,
  old resolutions still fold, series never negative; `attention = "all"`
  reproduces the pre-ADR golden index; `just check` 0.
- **WP-B (0.1.4) — plugin: quiet surfaces.** `driftInBar`, bar count and tooltip,
  strip text, labels keyed on `crisis` (DriftSheet 313/370, Model.js 352/
  682), Changelog header line, Today pictogram, drift-sheet order with
  the *Ask agent* slot; harness tests. SPEC-PLUGIN §4, §5.
  *Acceptance:* `omarchy plugin validate`, `qmllint`, harness; no new
  engine call; reads only `summary` and `drift`.
- **WP-C (0.1.4) — agent texts and docs.** Session-start context (crises and
  attention, 7 days, quoted); rules block "drift: explain only with
  evidence" (block version bump, `seldon rules update`); WP-094
  `drift.md`; user guide 02/03/06 en/de; CONCEPT.md; CHANGELOG (series
  change, label skew). *Acceptance:* live check on the test host: theme
  switch → nothing; plugin toggle → nothing (incl. `shell.json`); `omarchy
  update` with a kernel in it → no crisis, one release marker; terminal
  `pacman -S` → quiet attention; a new file under
  `~/.config/omarchy/hooks/post-update.d/` → red strip; `systemctl --user
  enable` of a packaged unit → nothing.
- **WP-D (later) — measured follow-up (after one real update on the test host).**
  Which Omarchy UI actions write under `~/.config/hypr` (A3) and what
  `omarchy-migrate` left as attention; decide further `routinePaths` or
  evidence rules (e.g. a `.git`-cloned theme under `themes/<t>/` as
  `theme-repo`); propose the v2 fields.
- **WP-E (later) — code the collectors cannot see today (scope decision first).**
  Third-party plugin tree hashing (one `plugin-update` on a tree-hash
  change; hashes only); the toggles state directory
  `~/.local/state/omarchy/toggles/hypr/*.lua`; `~/.ssh/authorized_keys`
  as an opt-in `alwaysRedPaths` example. Each is a capture-cost and
  privacy line item the operator approves separately.
- **WP-F (later) — repositories (needs an AGENTS.md §6 amendment by the
  operator).** Read-only hash of `/etc/pacman.conf` and
  `/etc/pacman.d/*.conf`; a repo-set or `SigLevel` change is a crisis.
  Outside this ADR's core; not started without the operator's word.

## Consequences

- Seldon stops asking. A normal week on a desktop — theme switches,
  toggles, `omarchy update` with kernels and AUR packages, Omarchy's own
  config refreshes — produces zero to-dos and zero bar changes; a crisis
  is the only thing that colours the bar, and it is rare and real.
- ADR-0008's "red zone → Crisis", ADR-0013 §3's "crisis iff red" and its
  `alwaysRed`-in-`-Syu` rule, and ADR-0014 §2's derivation are superseded
  by the harm test; grouping, the argv routine test, fan-out and the
  `alwaysRed` list itself are reused unchanged; ADR-0014's zone table
  stays the ledger truth. ADR-0027 §1's metric gains a line: *to-dos per
  week for a user who plans nothing* (target 0 without a crisis).
- The ledger stays append-only and claims no intent it lacks; two small
  evidence marks are added at capture, never retroactively; the index
  stays contract v1; the config is the only switch between quiet and loud.
- Decided: a package without a case is attention (§4c). Open: the
  operator's confirmation of v2 as a whole, and WP-E/WP-F scope.

## Alternatives considered

- **Auto-explain routine events with a resolution line.** Rejected: a
  false *why* in the record (ADR-0027 H2), one line per event, not
  retroactively switchable. Kept for rules 7/8 where there is evidence.
- **Make theme/plugins/update non-drift-eligible.** Rejected: eligibility
  is a property of the source; class of the event and config; opt-in and
  proposals must still reach them.
- **Keep crisis = red zone and compute `drift[].zone` by the harm test**
  (devil's advocate F3). Rejected: zone is where a change acts and is
  copied into resolution lines; a harm-class zone would write a false
  statement into the ledger through the explain form. Labels are fixed
  instead.
- **Third-party `plugin-add` as crisis** (F4). Rejected: it is an action
  in Omarchy's own UI with a one-click undo; attention keeps it visible
  and listed to agents without spending the bar; the real gap (in-place
  edits leave no event) is a collector gap, WP-E.
- **A bounded update window making all config events routine** (v1
  WP-D). Replaced by evidence (`omarchy-default`, `system-link`): a time
  window would also whitewash what an attacker does during an update.
- **Age out ignored attention; status quo plus bulk dismiss.** Rejected
  as in v1.

## Appendix — Findings disposition (devil's advocate, v1)

- **F1 `omarchy update` still a crisis via `alwaysRed`** — accept. Verified (`drift.rs:139`, `build.rs:476`). §2 row 1: `alwaysRed` upgrades inside a plain full upgrade are routine; downgrade/remove of one is attention.
- **F2 persistence paths wrong** — accept modified. Verified: it is `hypr/autostart.lua`; `systemd/user`, `autostart`, `environment.d`, `uwsm`, `.profile`, `.bash_profile` unwatched. Added as default watch + crisis paths (§4d). `hyprland.lua`/`autostart.lua` **not** crisis: in Omarchy 4 every hypr file is Lua, singling two out is theatre, and a red bar per binding edit breaks the intent — they are attention. `authorized_keys` and the toggles dir → WP-E. `install.sh --unit` → built-in template hash as rule-7 evidence.
- **F3 `crisis ⇒ red` load-bearing** — accept the problem, reject the fix. Verified labels only (DriftSheet 313/370, Model.js 352/682). `drift[].zone` stays ledger zone; labels keyed on `crisis` in WP-B; skew named cosmetic (§7).
- **F4 plugins are code** — accept the gap, reject the class. `plugin-add` stays attention (Omarchy UI, visible, listed to agents); `plugin-enable` stays routine (the add is already the open item); tree hashing → WP-E.
- **F5 `shell.json` churn** — accept. Verified watched and shell-written. `routinePaths` default.
- **F6 themes can carry code** — accept modified. Verified (`theme_came_from_a_repo`). User-made theme files under `themes/**` are attention (the arrival of code is recorded, quietly); not crisis: the user's own theme directory is ordinary configuration; a repo theme is stripped by Omarchy itself.
- **F7 fake `update`** — accept modified. Verified: `valid()` rejects `dev (hash)`, accepts bare `dev`. `omarchy-update` routine only when attributed to a package upgrade in a full upgrade; otherwise attention, not crisis (nothing is broken).
- **F8 knobs silence after the fact** — reject. The config is the user's; an attacker there can edit the ledger; a self-auditing crisis adds a new event kind (schema) for no real defence. `doctor` marks non-defaults; `drift --all` keeps routine visible.
- **F9 agent whitewash / injection** — accept modified. Engine refuses agent `explain|dismiss` of a crisis, `link` only to its own case (§3); context already quotes every line; lists id/kind/clipped subject.
- **F10 Omarchy's own copies flood** — accept. `omarchy-default` evidence at capture; `**/*.bak.*` routine; plus `system-link` for the migration that writes unit symlinks. Monitor-scaling `sed` not found on this host (A3) → WP-D.
- **F11 `*.sample` hooks, lost `owned.json`** — accept; covered by `omarchy-default` (byte-equal to the shipped sample) and the built-in template hash.
- **F12 AUR and orphans** — accept the AUR half as the general `upgrade` rule (an upgrade of what is installed is routine, `-U` from a cache included). Orphan half rejected: `omarchy-update-orphan-pkgs` prompts only in a tty and defaults to no; a confirmed `-Rns` of named packages is a removal the user chose → attention.
- **F13 repositories** — accept as WP-F; outside this ADR's core; needs the operator's AGENTS.md §6 amendment.
- **T1 table not total** — accept: last row "anything else → attention", group = max, `-Syyuu` and `:: Replace` rows added.
- **T2 `drift link` on routine is a no-op** — accept. Verified (`reconcile.rs:90`). `linkable` set; `explain|dismiss` of routine exits 1; `--all`/`show` read folded events.
- **T3 `series.drift` negative** — accept. Verified (`build.rs:657`). Resolution counts only if its `refersTo` was counted as opened.
- **T4 reference derivation and fixture lag** — accept; WP-A ports and extends the fixture; golden pinned to one config.
- **T5 ADR-0020 unaffected** — noted; holds also with ledger zone, since the cap sorts on `crisis`.
- **T6 A3 already tested** — accept; dropped from the assumptions (`pacman.rs:572`).
