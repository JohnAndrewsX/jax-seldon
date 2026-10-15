# ROADMAP.md — From 0.4 to 1.0

One page. Proposed with [ADR-0056](../decisions/ADR-0056-seldon-is-an-omarchy-app.md),
[ADR-0057](../decisions/ADR-0057-ui-strings-through-catalogs.md) and
[ADR-0058](../decisions/ADR-0058-seldon-open-is-the-app.md) (E96
delegation; the operator accepts). The numbers follow
[VERSIONING.md](VERSIONING.md): before 1.0 the minor is the breaking slot.
Work packages and phases are in [PLAN.md](PLAN.md) (phases 5–7).

| Release | Content | Contract |
|---|---|---|
| **0.4.1** | Edge compatibility, patch grade: Qt 6.12 qmllint ids; the Omarchy channel family (`omarchy`/`omarchy-dev`, `omarchy-settings`/`omarchy-settings-dev`, `linux-omarchy*`) always red, a channel switch as one change; the Omarchy assumptions ledger re-anchored; Omarchy-owned new files classified (`omarchy/language`, `uwsm/env.d/50-omarchy-language`, `omarchy/defaults/*`, `toggles/crash-ignore/*`, `crash-capture-off`); `.gemini/config/skills` among the skill folders; pacman fixtures with `--`; docs for Omarchy's "Failed! (code)" | 2, unchanged |
| **0.5.0** — Seldon is an Omarchy app (breaking minor) | The `seldon` package and the app `seldon-desk`; `seldon open` = the app, `edit`, `skill`, `changes --around`; four views (Changes, Plan, Record, Radiant) at parity for the daily acts ([APP-PARITY.md](APP-PARITY.md)); the desk and the `jax.seldon.panel` shim removed; the pill with `en` and `de`; `.desktop`, icon, `.install`; the installer ships both binaries; README in the omacom shape; `?`, `Q`, Space | 2 kept; the `edit` row; plugin 0.5.0 needs engine 0.5.0; a 0.4 plugin keeps working on engine 0.5 (the `open` alias) |
| **0.6.0** | Contract 3: crash metadata, the `changes --around` fields in the index, reports as records (its own ADR; `contractReadableFrom` per ADR-0051); the other 48 languages; `post-update.d` and `post-boot.d` capture hooks through `omarchy hook install`; the package recipe for Omarchy's package repository ready for submission; the `open <target>` alias and the `jax-seldon` symlink removed; the graph's layouts (WP-189) | 3 |
| **1.0.0** | After the package is published (Omarchy's package repository or the AUR) and one Omarchy stable cycle passes without a contract change; the exit-code policy final; the logbook spec frozen | 3 |

**Why not a bigger jump.** 1.0 promises "breaking = major" at the moment
the surface is rewritten; 0.5.0 says honestly that it breaks.

**What gates 0.5.0** is parity, not a date: every desk-view scenario has
an app acceptance or a recorded drop, and the daily acts (resolve a
change; start, verify, complete a case; a note; accept a decision; the
Prime Radiant; the graph) pass in the app. The desk stays in every 0.4.x.

**What moves, what closes** (ADR-0056 §11): WP-188's shim removal moves
into 0.5.0; WP-185 and WP-186 are re-scoped to "record and answer"
(diagnosis and reports stay Omarchy's); WP-179, WP-181, WP-182 and WP-187
close as overtaken by the app; WP-163 and WP-189 move to the *Radiant*
view.

**The operator's acts** (never an agent's): accepting the three ADRs and
the AGENTS.md rows ([AGENTS-PROPOSAL.md](AGENTS-PROPOSAL.md)); PROJECT.md's
goal and definition of done; merging the branch `app` into `next`; exit
codes staying 0–4 (ADR-0058 §5; default yes); the languages, `en` and
`de` in 0.5.0 and the other 48 agent-made in 0.6.0 (ADR-0057 §7; default
yes); the AUR key, the submission to Omarchy's package repository, any
repository rename; installing a release on the dev host; whether the
channel-switch classification is rehearsed on the test host's rc channel
or on a separate edge profile (a second test host profile or a VM).
