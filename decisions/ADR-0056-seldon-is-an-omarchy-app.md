# ADR-0056 — Seldon is an Omarchy app: the engine stays, a Qt Quick window replaces the desk, the plugin becomes the pill

**Status:** proposed (E96 delegation; operator accepts)
**Date:** 2026-10-14

> Writes down the E96 architecture decision (the E92 step-back audit:
> edge survey, reference apps, conformance audit, and the advisor's
> decision, option A). Supersedes in part ADR-0001, ADR-0002, ADR-0004,
> ADR-0016, ADR-0022, ADR-0024, ADR-0034 and ADR-0045; the rows are in
> §11. Companion ADRs: [ADR-0057](ADR-0057-ui-strings-through-catalogs.md)
> (UI strings through catalogs) and
> [ADR-0058](ADR-0058-seldon-open-is-the-app.md) (`seldon open` is the
> app; `edit`; `skill`; `changes --around`). No contract change:
> `contractVersion` stays 2; the one `--json` row that moves is
> ADR-0058's. Takes effect with 0.5.0; nothing here changes 0.4.x.

## Context

1. **Omarchy 4.5 ships an `omarchy-app` skill** (`$OMARCHY_PATH/default/
   agents/skills/omarchy-app/`): "one window, one job, driven from the
   keyboard, following the desktop theme, and installed like any other
   package", built as C++ and Qt Quick with qmake6 and make. Hype,
   Monologue, Omacut and Signed are built that way; Hype and Signed add a
   windowless CLI with `--json` and `<name> skill install`.
2. **Measured against it, Seldon 0.4.0 is a shell plugin plus a CLI, not
   an app.** The conformance audit (80 rules: 37 pass, 14 partial, 25
   fail, 4 not applicable) finds the plugin side near-exemplary (Plugin
   UI Safety, theme tokens, careful writes, the four states, offscreen
   tests). Almost every failure sits in four clusters: app shape (no
   window, no launcher entry, no icon, no `.desktop`), i18n, keyboard
   conventions (`?`, `Q`, Space) and packaging (curl into bash while the
   AUR package never shipped).
3. **The desk is an application in everything but its window**
   (ADR-0034: "reads as an application"). As a full-width layer-shell
   overlay with exclusive keyboard focus inside `omarchy-shell` it gets
   no tiling, no Alt-Tab, no launcher, no dock; every Seldon bug is a
   shell crash (E26; Omarchy's `diagnose-crash` reporting guide puts
   shell and plugin crashes in Omarchy's sphere); new plugin code needs
   a shell restart; the shell thread parses up to 1 MB of index. A large
   share of its 14k QML lines imitates window behaviour (per-monitor
   geometry, width slider, stacked layout, stepping aside, WP-156).
   Omarchy's own overlays are transient pickers.
4. **One job is not visible in the product.** The tagline is one job;
   the surface is nine sections, two modes (ADR-0045), 21 top-level
   commands. Omarchy's apps are "dead simple on purpose".
5. **The engine is the asset.** About 66k lines of Rust, 1,487 tests, the
   collectors and attribution, ADR-0028's judgement, the redaction
   corpus, the contract with forward compatibility (ADR-0051) and the
   size budget (ADR-0053). The contract is already the `omarchy.agents`
   pattern: a CLI writes records, the UI only displays them.
6. **Hook latency.** ADR-0002 chose a static Rust binary so that `seldon
   hook claude-code` answers in a few milliseconds on every agent tool
   call. Any binary that links Qt pays Qt's dynamic loading on every
   exec, also on its CLI path. Hype can afford that (nobody runs `hype
   check` two hundred times a session); Seldon's hook cannot.
7. **Options weighed by the advisor** (private audit, decision §1):
   A — Rust engine unchanged plus a separate C++/Qt Quick app from the
   templates; B — Rust core and Qt Quick in one binary (cxx-qt or CMake
   with Corrosion); C — rewrite everything in C++; D — the QML window run
   by Quickshell (Flea's way); E — polish the plugin and stop. B needs a
   crate outside AGENTS.md §7, CMake (which the skill rules out), loses
   the static build and still pays Qt on every hook. C discards the
   engine to satisfy a sentence written for GUI apps. D keeps `qs.Ui`,
   which is Omarchy-internal (14 of 17 ledger files changed in one edge
   jump) and needs symlinks into Omarchy's tree. E cannot clear the
   failures above and was rejected by the operator.
8. **Precedent for a non-C++ core in Omarchy's package world:** `gliff`
   (omacom, Rust, GTK) is in the Omarchy package repository on the fast
   ring; `omarchy-task-manager` and `omatrack` (Rust and Qt Quick) are
   there too; Hype already runs a child process of itself and reads its
   JSON lines.
9. **Name.** `seldon` is free in Arch and the AUR (checked for E96).

## Decision

1. **Option A.** Seldon becomes an Omarchy app with three artefacts in
   one package:

   | Artefact | Language | Path | Job |
   |---|---|---|---|
   | engine | Rust, static musl (unchanged) | `/usr/bin/seldon` | the only writer; collectors, commands, hooks, index; what agents drive |
   | app `seldon-desk` | C++ and Qt Quick, qmake6 + make, from the omarchy-app templates | `/usr/lib/seldon/seldon-desk` | the window; GUI only, no CLI of its own |
   | plugin `jax.seldon` | QML | `/usr/share/seldon/plugin/`, copied into `~/.config/omarchy/plugins/jax.seldon/` | the pill and the capture timer |

   The app sets `setDesktopFileName("seldon")`, so its Wayland app id,
   `.desktop` file and icon are all `seldon`. `seldon open` starts it
   (ADR-0058); the `.desktop` entry runs `seldon open`. The app reads
   `index.json` and the files it points to, watches it, and asks the
   engine for every write through `QProcess` with fixed argument lists,
   exactly as the plugin does today. AGENTS.md §3 holds for every
   surface: no Seldon UI parses Markdown, YAML or logs.
2. **One job**, in Omarchy's voice: *Seldon tells you what changed on
   your machine, who changed it, and why.* Everything else is an input to
   *why* (cases: planned reasons; journal: written reasons; decisions:
   lasting reasons; drift resolution: after-the-fact reasons), a view of
   the same record (Prime Radiant, the graph), or agent plumbing (hooks,
   skill, rules, `agent start|ask`) that belongs in the CLI.
   - *Black box* = the engine, headless.
   - *Logbook* = the files (unchanged; SPEC-LOGBOOK).
   - *Doctor*: `seldon doctor` stays Seldon's self-check. The machine
     question "what changed before it broke" is the timeline query
     `seldon changes --around <time> --json` (ADR-0058) plus crash
     metadata (E27), offered to Omarchy's `diagnose-crash` flow through
     the Seldon skill. No second crash UI, no report drafts of Seldon's
     own: diagnosis and reporting stay Omarchy's (E28/E32 re-scoped to
     "record and answer"; §10).
3. **Four views** in the app: *Changes* (the default: the timeline by day
   with its chips and the resolution acts link, explain, dismiss; absorbs
   Today), *Plan* (cases by status and their steps), *Record* (journal,
   decisions, memory and the dossier as readable documents, with the few
   human acts: note, accept a decision), *Radiant* (the charts and the
   graph). **EASY | PRO is retired** (supersedes ADR-0045): one set of
   views, progressive disclosure inside them, no mode switch.
4. **The plugin shrinks to the pill.** Counts, the crisis colour and the
   tooltip; the 15-minute capture timer (ADR-0005 stands: no daemon);
   left click runs `seldon open --json`, which starts the app detached
   and returns at once (ADR-0058 §1), so the window is never a child of
   `omarchy-shell` and a crash of one never takes the other down; right
   click captures. No panel: the
   app is one key away. The plugin ships read-only under
   `/usr/share/seldon/plugin/` and is copied into the user's plugin
   folder on the user's say-so (Meeting Recorder's split; a copy, never a
   symlink, AGENTS.md §7); the store listing stays the second route
   (ADR-0009 stands).
5. **The desk ends with 0.5.0.** The overlay entry point, the desk's
   QML and the `jax.seldon.panel` IPC shim are removed in the release
   that ships the app; WP-188's shim removal moves into 0.5.0 (E87
   adjusted). 0.5.0 is gated on parity for the daily acts, not on a date:
   resolve a change; start, verify and complete a case; write a note;
   accept a decision; the Prime Radiant; the graph. The parity list
   ([docs/APP-PARITY.md](../docs/APP-PARITY.md)) maps every desk-view
   harness scenario to an app acceptance or a recorded drop.
6. **One package, one name: `seldon`.** `provides`, `conflicts` and
   `replaces` carry `jax-seldon`; the `jax-seldon` binary symlink goes in
   0.6.0. The GitHub repository and the plugin id `jax.seldon` keep their
   names (the plugin system needs a namespace; the repository name is
   nobody's rule). Supersedes in part ADR-0001.
7. **The package is the install route.** `omarchy pkg add seldon` once
   the package is in Omarchy's package repository or the AUR (the
   submission and the AUR key are the operator's acts). Until then the
   GitHub installer ships the musl engine and the app binary built in
   CI's Arch container against Omarchy stable's mirror (ADR-0055's job;
   Qt is forward-compatible within 6.x). The plugin's curl-into-bash
   constant is removed the day a package exists. Supersedes in part
   ADR-0004, ADR-0016, ADR-0022 and ADR-0024.
8. **The app follows the templates line by line**, keeping their MIT
   attribution: `Theme` (with Hype's fuller palette — background,
   foreground, panel, border, muted, error from `colors.toml` — because
   `Tones.js` needs it; `theme.radius` everywhere; `#FFD60A` fallback),
   `Translator` (ADR-0057), the portal file picker, Qt Test offscreen
   with temp dirs, `?` overlay, `Q` quits, Space is the selected row's
   main action, the four required states with one fix each. `Model.js`
   and `Tones.js` are plain JavaScript and load into Qt Quick unchanged;
   the rewrite is the views, not the logic.
9. **Recorded exceptions to the omarchy-app skill**, each with its
   reason and precedent:

   | Skill rule | Seldon | Why | Precedent |
   |---|---|---|---|
   | "a single binary" | two binaries in one package (`seldon` engine, `seldon-desk` app) | hook latency (Context 6): the hook path must not load Qt | gliff (omacom, Rust); `omarchy-task-manager`, `omatrack` (Rust + Qt); Hype runs a child of itself |
   | "exits 0 or 1" | exit codes 0–4 stay (ADR-0058 §5) | 2–4 refine "1"; "non-zero = failure" holds for every agent; the plugin keys on 3 and 4 | — (a stated deviation; revisit at 1.0 only on the operator's word) |
   | "Ctrl+Z / Ctrl+Shift+Z undo and redo" | no undo key | the ledger is append-only; there is no true inverse (`plan reopen` makes a new case); SPEC-PLUGIN §5.3's house rule "no undo key without a true inverse" | — |
   | "static musl release build" (AGENTS.md §7, Seldon's own) | scoped to the engine | no Qt binary links statically; Qt comes from Arch | — |

   Everything else in the skill applies to the app as written. One rule
   is read, not excepted: "C++ owns the logic" speaks of the app's
   backend against its QML, so in `seldon-desk` C++ owns the window's
   state, the index watch and the engine calls, and QML owns layout and
   keys. The engine is a separate headless program the app calls through
   `QProcess`, as the skill's apps call `ffmpeg`; its logic is not the
   app's logic.
10. **Precedence of Omarchy's skills.** Shipped Omarchy skills
    (`omarchy`, `omarchy-app`, `diagnose-crash`) rank above Seldon's own
    house rules wherever Seldon records no justified exception (this
    ADR, §9). Supersedes in part the precedence list of
    `docs/skills/omarchy-ux/SKILL.md` §1 ("SPEC-PLUGIN wins").
11. **What is superseded, amended, re-scoped** (all *in part*):

    | Item | Change |
    |---|---|
    | ADR-0001 | package `jax-seldon` → `seldon`; app id `seldon`; the `jax-seldon` symlink ends in 0.6.0; repository and plugin id unchanged |
    | ADR-0002 | engine Rust (static), app C++/Qt Quick (dynamic), plugin QML; the contract couples all three |
    | ADR-0004, ADR-0016, ADR-0022, ADR-0024 | the package `seldon` is the route; the installer carries both binaries until it exists; curl into bash ends with the package |
    | ADR-0034 | the desk as a shell overlay → the app window; the pill stays; §7's shim removal lands in 0.5.0 |
    | ADR-0045 | EASY \| PRO retired |
    | ADR-0007 | its consequence "UI i18n deferred" → [ADR-0057](ADR-0057-ui-strings-through-catalogs.md) |
    | SPEC-PLUGIN §5, §8 | the desk and its keys go with 0.5.0; the binding form becomes `{ launch = "seldon" }` (today `{ panel = … }` / the command form) |
    | SPEC-ENGINE §3 `open` | [ADR-0058](ADR-0058-seldon-open-is-the-app.md) |
    | omarchy-ux skill §1 | §10 above |
    | E87 / WP-188 | the shim removal moves into 0.5.0 with the desk |
    | E28, E32 / WP-185, WP-186 | re-scoped to "record and answer" (crash metadata in the ledger; `changes --around`); diagnosis and reports stay Omarchy's |
    | WP-179, WP-181, WP-182, WP-187 | desk features and EASY \| PRO: closed as overtaken (the `?` overlay and keys return in the app) |
    | WP-163, WP-189 | the graph work moves to the app's *Radiant* view |

    Standing unchanged: ADR-0003 (Markdown record, JSONL ledger),
    ADR-0005 (no daemon), ADR-0006, ADR-0009 (plugin repository),
    ADR-0028 and its amendments, ADR-0041 (its `--editor` focus moves to
    `seldon edit`, ADR-0058), ADR-0051, ADR-0053, ADR-0055, and every
    contract ADR.

## Consequences

- **Crash isolation.** After 0.5.0 the shell holds a few hundred lines of
  pill; a bug in Seldon's window is an app crash, not a shell crash. The
  1 MB index parse leaves the shell thread.
- **A real window.** Tiling, workspaces, Alt-Tab, launcher (`Super +
  Space`), dock, no exclusive keyboard grab; the app needs no shell
  restart to update.
- **Two languages, two toolchains, three artefacts, three test stacks**
  (cargo, Qt Test offscreen, the plugin's Quickshell harnesses for the
  pill). The build host needs `qmake6`, `make`, a C++ compiler and Qt's
  base, declarative, SVG and Wayland modules (all part of Omarchy).
  `cmake` is not used.
- **The app is dynamic.** It is built against Arch's Qt; CI builds it on
  Omarchy stable's mirror (ADR-0055) so a release works on stable (E93);
  the dev host's edge Qt is a forward-compatibility check. The app keeps
  a `QT_MIN 6.9` gate as Hype does and uses no 6.12-only API.
- **The engine and the contract do not change** for this ADR. The engine
  gains the commands of ADR-0058; contract 3 (0.6.0) is a separate ADR.
- **AGENTS.md needs rows** (§2 catalogs, §3 every surface reads only the
  index, §4 `app/`, §6 the language file, §7 an app block and "static
  musl" scoped to the engine). They are listed for the operator in
  [docs/AGENTS-PROPOSAL.md](../docs/AGENTS-PROPOSAL.md); this ADR does not
  edit AGENTS.md. PROJECT.md's goal and definition of done follow on the
  operator's word.
- **The cost to today's user is bounded by the parity list.** The desk
  stays in every 0.4.x release and leaves only in the release whose app
  passes the parity list; what the app does not take over is a recorded
  drop there, visible before the tag, not discovered after it.
- **Version plan** ([docs/ROADMAP.md](../docs/ROADMAP.md)): 0.4.1 edge
  compatibility (no app); 0.5.0 the app (breaking minor, contract 2
  kept); 0.6.0 contract 3, the other 48 languages, the alias and symlink
  removals; 1.0.0 after the package is published and one Omarchy stable
  cycle passes without a contract change.
- **What would reopen this ADR:** a first-party Omarchy app shipping Rust
  and Qt in one binary *and* a hook path that does not exec the binary;
  or the operator refusing C++ (then the remaining shape is a second,
  Rust GUI binary — still two binaries, still this ADR's split).
