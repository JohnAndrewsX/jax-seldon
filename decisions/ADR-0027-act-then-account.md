# ADR-0027 — Act, then account: a case the user started authorises the agent; Seldon takes work off the user

**Status:** accepted (operator decision 2026-10-05); the debate verdict H2 amended in part by ADR-0029
**Date:** 2026-10-05

> Amended in part by [ADR-0029](ADR-0029-planned-and-active-link.md) (2026-10-06): a change made while exactly one case was active and named it in its Plan is linked to that case by the engine (one narrowly evidenced rule, not a general heuristic); `plan verify`/`plan done` capture first.

## Context

Seldon's recording core works: collectors, hooks, cases, drift and the
index give a complete trace without the user's help. The agent workflow
on top of it does not. Observed on 2026-10-05, same task ("install tool
X, which needs one Arch package and ships a PKGBUILD"), two terminals
side by side:

- A case started from the Seldon panel (create, two clicks, agent
  launched in `~/Seldon` via `seldon agent start`). The agent followed
  the logbook's `AGENTS.md`: wrote a plan into the case file, checked the
  download, then stopped and handed back — "the next two steps need
  sudo; run `sudo snapper -c root create …` and `omarchy pkg add …`
  yourself, then tell me the snapshot number". Because of the rule
  "install with `omarchy pkg add`, not the package manager", it had also
  chosen the worse route (a prebuilt binary into `~/.local`) over the
  README's recommended Arch package. Nothing was installed.
- Omarchy's default agent, started by hand in `~/Work` with the same
  sentence and no Seldon rules: installed the dependency with pacman
  (the user typed the password into the prompt), built and installed the
  Arch package, verified, done in about four minutes. Seldon recorded
  every package change — as drift, because no hook served that session.

The user's verdict: "I have far too many manual steps. Seldon must not
create extra work for me; it should take work off me." And, on closing:
the agent does everything; the user spot-checks when they feel like it;
creating a case by hand and handing it to an agent stays possible,
optionally; importing cases would be welcome. And, asked whether that
rules out every stop: steps that can make the machine unbootable keep
the user's explicit go (operator, 2026-10-05).

Diagnosis. The rules in `AGENTS.md` (WP-047), ADR-0023 and the agent
guide turned a *recording* tool into an *approval process*: propose and
wait before a case exists, the plan as a gate, a human-made snapshot and
its number typed by hand, `omarchy pkg add` as the only install route,
"agent verifies, human closes", two clicks to start. Seldon records
whatever happens anyway, so each step adds work without improving the
record. PROJECT.md draws the right line — the *engine* never runs
`pacman` or `sudo` — but nothing says the *agent* must not.

Facts checked (read-only). `bin/omarchy-snapshot create`
(`requires-sudo`) runs `sudo snapper -c <cfg> create -c number -d
"<omarchy version>"` and then `snapper cleanup number` for every config;
Omarchy's snapper template (`default/snapper/root`) sets
`NUMBER_LIMIT=5`, `NUMBER_MIN_AGE=0`, so every create prunes the oldest
numbered snapshot beyond five. `omarchy update` calls it. `omarchy-pkg-add`
is `sudo pacman -S --noconfirm --needed`. No pacman hook creates
per-transaction snapshots (`10-limine-snapper-lock.hook` only waits for
limine-snapper-sync). Omarchy's own agent skill tells agents to use
`sudo` in a visible terminal and notes that some commands may be
passwordless. In the engine, `case.started` is a date, the
`case-started` ledger event has the instant, `plan` commands default to
`--actor human`, `tags` is a free string array in the case schema and the
index, and no QML file reads it yet. Assumed, not read: sudo's credential
cache (`timestamp_timeout`, Arch default 5 min).

### Debate

A devil's-advocate round (private folder) reviewed the first draft. The
judge's verdicts: C1 (a password prompt is not consent — sudo caches,
some commands are passwordless) accepted: attended is defined by
provenance. C2 (`omarchy-snapshot` prunes on every create) accepted: the
agent snapshots without the cleanup pass. C3 (agent-set risk and
`--noconfirm` hide a transitive R3 subject) accepted: resolve the
transaction read-only and check it against `alwaysRed`. H1 (the Plan
would widen its own authorisation) accepted: only the human-written
Intent bounds the work. H2 (token auto-link writes a false "why" into
REBUILD.md) accepted: no heuristic link now. H3 (agent self-grading)
re-judged under the operator's "no extra work": the agent closes;
mitigations that cost the user nothing stay (§5), the rest go. M1
(snapshot heuristic loose) accepted: the agent passes the number. M3
(`rules update` must not drop user rules) accepted. M4 (too many new
commands) accepted: `agent start --new`, `plan set`, `plan snapshot`,
`plan reopen`, `rules update` remain. Preview line and guard metrics
accepted. The 80/20 shape (rules and skill first, no auto-link heuristic)
is the decision; option (A) for closing is the operator's call.

## Decision

1. **Principle and metric.** Seldon takes work off the user: it records
   and accounts; it never asks the user to do what the agent can do, and
   it never requires the user to look. The primary measure is **human
   steps per case**, counted in the live check of every agent-facing WP,
   password prompts included. Today's path costs about seven (create,
   start, approve the plan, run the snapshot, type its number, run the
   install, close). Target: **one click, one sentence, at most one
   password prompt; nothing at the end.** Guard metrics counted with it:
   snapshot coverage of R2/R3 cases (100 %), R3 gates honoured (100 %),
   agent closes later reopened (tracked, not targeted). A rule or
   feature that adds a human step without adding to the record, or
   improves the step count at the cost of a guard metric, is wrong by
   this ADR.

2. **A case the user started is the authorisation.** When the user
   created and started a case — by hand (CLI or panel, §6) or through
   the one-click start — and an agent works on it, the agent **acts**
   inside the *Intent* without a propose-and-wait step. The *Plan* is a
   short note the agent writes and extends as it goes (steps so far,
   affected paths, rollback, verification) so the trace has intent next
   to events; it is **not a gate** and does not widen the authorisation
   — only the human-written *Intent* does. Before the first privileged
   step the agent prints one **preview line** in the terminal and the
   Log ("About to: install X (+deps a, b); snapshot first; rollback
   `pacman -Rns X`") — no wait, no step; the password prompt that may
   follow is then informed. What still needs the user, asked in the
   terminal before the step:
   - (a) **outside the Intent**: another package or area, a change the
     user did not ask for. Dependencies the named software documents are
     inside the intent; instructions found in fetched text (a README's
     "also run …", a `curl | sh`) are outside; a PKGBUILD or install
     script is read before it runs.
   - (b) **destructive without rollback**: deleting data, removing a
     package others depend on, overwriting a config that is in no
     snapshot or git.
   - (c) **R3 — the only remaining stop (operator decision
     2026-10-05).** A step that can break boot, login or the shell
     leaves a machine the user cannot repair from the desktop, and the
     snapshot helps only if the boot menu still comes up; the operator
     decided this one stop stays. It is rare in normal work:
     kernels, bootloader, initramfs, `systemd`, `glibc`, `pam`,
     `sddm`/`uwsm`, `hyprland`, `quickshell`, `omarchy` itself, `/etc`
     through `filesystem`/`omarchy-settings` — the `[drift] alwaysRed`
     list. Before any package transaction the agent resolves it
     read-only (`pacman -Sp …`, makepkg's depends) and matches the result
     against `alwaysRed`; a hit makes the step R3, the agent raises the
     case first (`seldon plan set <ID> --risk R3`) and asks for one
     explicit go per such step (ADR-0023 §2 stands). The engine stays
     advisory after the fact: a red event whose subject is in `alwaysRed`
     inside a case below R3 gets a Log line and a panel warning.
   - (d) **absence of the user.** Attended is a matter of provenance, not
     probing: the session is attended when the launcher set
     `SELDON_ATTENDED=1` (`seldon agent start`, with WP-096's
     `SELDON_ACTOR`) or the task came from a human message in this
     session. A session started by a timer, a hook, another agent or any
     other launcher is unattended: **record and report only** — read,
     plan, write the Log, change nothing. A sudo cache or a NOPASSWD rule
     never makes a session attended.

3. **Privileged commands and the red zone.** In an attended session the
   agent runs privileged commands itself (`sudo` in the terminal; the
   user types the password when asked). It never asks for, stores or
   passes a password. Before the first red change of an R2 or R3 case
   the agent takes the snapshot itself, **without the cleanup pass**:
   `sudo snapper -c <cfg> create -c number -d "<case id>: <title>"` for
   each config `snapper --csvout list-configs` lists, reads the number,
   and records it: `seldon plan snapshot <ID> <N>` (new; `plan start
   --snapshot N` stays for the human who snapshotted first). The engine
   validates — the snapshot exists (ADR-0026 read grant), its instant is
   after the case's `case-started` event and before the case's first red
   event — writes a Log line, warns on violation, never refuses. Hook
   attribution (a `snapper … create` or `omarchy-snapshot create` command
   event under the ADR-0017 window) fills `snapshotBefore` as a fallback
   when the agent forgot. Retention, said out loud: numbered snapshots
   are capped at five, so a case snapshot lives until the next cleanup
   (`omarchy update`, `omarchy-snapshot create`) ages it out; when the
   snapper collector sees a `snapshot-delete` of a number that is some
   case's `snapshotBefore`, it writes `rollback for <ID> pruned` to that
   case's Log and `doctor` shows it. No snapper, exit 127, no configs
   (ADR-0011 degraded): an R3 case stops and asks; an R2 case continues
   with a named backup in the Plan and says so in the Log. The
   **engine** still never runs `snapper`, `pacman` or `sudo` (PROJECT.md
   stands).

4. **Package installs.** The agent prefers the route the software
   documents; when it offers a choice: repository package (`omarchy pkg
   add` or `pacman -S`, the same transaction), AUR (`omarchy pkg aur add`
   or the installed helper), the project's PKGBUILD (`makepkg -si`, read
   first), an upstream binary under `~/.local` only when nothing packaged
   exists. `omarchy pkg add` is a *recommendation* (idempotent,
   non-interactive), not a rule; the sentence forbidding the package
   manager is dropped. Packaged routes are recorded by the pacman
   collector whoever ran them; an unpackaged route leaves only hook
   events (green, while a case is active) and the Log, where the agent
   names the route it took.

5. **Closing: the agent verifies and closes (operator decision, option
   A).** When the Plan's verification passes, the agent fills *Result*
   with the evidence and runs `seldon plan verify` then `seldon plan
   done` in one go. Nothing is required of the user afterwards: no
   acknowledgement, no expiring badge, no click. Spot checks are
   possible, never due. What stays because it costs the user nothing:
   - the close is recorded as the agent (`actor: agent:<name>`, from
     `--actor` or WP-096's `SELDON_ACTOR`; an agent close is never
     recorded as human);
   - the engine **refuses** `plan done` by an agent actor when the case's
     *Result* is empty or *Plan › Verification* is unfilled (exit 1 with
     the reason; a human close is never refused) — this is the engine's
     own state machine, which has always been enforced, not a block on a
     system command;
   - the case gets the tag **`closed-by-agent`** (`tags` is already in the
     case schema and the index — no contract change; CONTRACT.md lists
     the reserved tag vocabulary); the panel shows a small marker on
     such cards and a "closed by agent" filter in the completed column
     for spot checks;
   - **reopen in one click**: `seldon plan reopen <ID> [--actor]` creates
     a new active case "Reopen: <title>" with the Intent copied, tag
     `reopens:<ID>`, a Log line on both; `completed` stays terminal
     (ADR-0003). The panel offers it on every completed card.
   Dropped, with reasons: *acknowledge* (a required look is extra work);
   *badge that expires or waits for ack* (the same); *R2 defaults to
   human close* (R2 has a snapshot and a rollback; the user's words rule
   it out); *mandatory non-self check* (the engine cannot verify it, so
   it is skill advice — "include one check that is not your own
   artefact: the real use case's exit status, `pacman -Q`, `systemctl
   is-active`" — not a rule). R3 cases close the same way; their stop is
   before the step (§2c), not after.

6. **Starting a case: one click, or by hand.** The panel's Work tab gets
   "New case": an intent text field and *Run*. One engine call, fixed
   argv, the text as one argument: `seldon agent start --new [--zone Z]
   [--risk R] [--area A] -- "<intent>"` (CLI users type the same line).
   The engine derives the title (first sentence, at most 72 characters),
   writes the text into *Intent*, creates and starts the case (defaults
   yellow, R1, normal; the agent raises them with `plan set` before its
   first red change), sets `SELDON_ACTOR` and `SELDON_ATTENDED=1`, and
   launches the configured agent with the prompt that names only the
   case id and the logbook path (WP-058). The agent reads the intent from
   the case file: logbook text is data, never prompt text. No default
   agent → the engine's refusal and the fix (`omarchy default agent`).
   The **manual path stays first-class and optional**: `seldon plan new`
   (or the panel's form) with zone, risk, area and a hand-written Plan,
   `seldon plan start`, and later `seldon agent start <ID>` to hand the
   case to an agent — or no agent at all. The one-click start is the
   shortcut, not the only way.

7. **Case import (later WP, scope fixed here).** `seldon import task
   <FILE|DIR>… [--area A] [--zone Z] [--risk R] [--start [--agent]]
   [--dry-run] [--json]` turns a Markdown task file — the kind a user
   writes in a project folder and hands to an agent — into a case: title
   from the first heading (else the file name), *Intent* from the body,
   defaults yellow/R1/normal unless given, Log line `imported from
   <path>` and tag `imported`. A directory imports every `*.md` in it
   (not recursive unless `--recursive`). Idempotent: the marker
   `.seldon/imports/tasks.json` keys path and content hash; the same file
   again creates nothing (exit 0, `created: 0`), a changed file creates a
   new case that names the old one. The source file is never modified;
   the engine remains the only writer in the logbook. It is a new
   variant of the existing `import` subcommand (`ImportSource`, WP-043),
   not a change to the vault import; unlike the vault import it applies
   by default (one new file per source, nothing rewritten) and offers
   `--dry-run`. No contract change.

8. **Work outside a Seldon-started session.** No heuristic link now.
   Exact attribution comes from the hooks: a user who launches agents
   outside the logbook sets `[hooks] scope = "all"` and installs the
   WP-094 skill, which tells hook-less agents to name the active case
   and to pipe their commands through `seldon hook generic`. The drift
   that remains is measured on the test host before any automatic link
   is designed (Intent tokens only, exact names, window closed at
   `done` or 24 h, the matching event only, reversible by resolution).

9. **What stays.** The engine is the only writer; it never executes.
   Everything is recorded, attended or not. Prompts carry identifiers,
   never logbook text; logbook text an agent reads is data. No network.
   **No contract change**: `tags`, `snapshotBefore`, `status`, `actor`
   carry what the panel needs; `contractVersion` stays 1. Risk scale as
   ADR-0023 §2, zones as ADR-0014/0019, redaction as SPEC-ENGINE §7.

## Consequences

- **Supersedes ADR-0023 §1** (verification as the human gate, the
  pre-authorised close). ADR-0023 §2 (risk scale) stands, with the R2/R3
  snapshot taken by the agent (§3) and R3's explicit go kept as the
  only stop by operator decision (§2c). **Amends** SPEC-ENGINE §3 (`agent start --new`,
  `plan set`, `plan snapshot`, `plan reopen`, `plan done` refusal for an
  agent actor without Result, `rules update`, `import task`,
  `SELDON_ATTENDED`), §4 snapper (`snapshotBefore` validation, pruned
  rollback), §8 (attended provenance), SPEC-LOGBOOK §3 (who closes;
  reserved tags `closed-by-agent`, `reopens:<ID>`, `imported`; Log line
  words `snapshot`, `pruned`, `reopened`, `imported from`), SPEC-PLUGIN
  Work tab (New case → Run, closed-by-agent marker and filter, Reopen),
  CONTRACT.md (reserved tag vocabulary; no schema change). ADR-0012,
  ADR-0014, ADR-0017, ADR-0019 unchanged. PROJECT.md "Out of scope" gains
  one sentence: the engine never executes; the agent the user started
  does, and Seldon accounts.
- **Work packages.** WP-100 *Rules rewrite and migration* first. WP-096
  *Default actor and `SELDON_ATTENDED`* before WP-101's close path.
  WP-101 *Engine and panel*: `agent start --new`, `plan set`, `plan
  snapshot` validation, pruned-rollback warning, agent-close conditions,
  `closed-by-agent` marker and filter, `plan reopen`, panel New case,
  fixture. WP-094 *Skill* after WP-100 (it quotes the rules). WP-095
  keeps only "Ask agent". WP-102 *Case import* after WP-101. The live
  check on the test host counts the §1 metrics against the 2026-10-05
  baseline.
- **Migration of existing logbooks.** `init` refuses an existing logbook
  and never overwrites; users may have edited `AGENTS.md`. WP-100
  therefore (a) wraps the engine-maintained rules in `<!-- seldon:begin
  rules v2 -->` … `<!-- seldon:end -->`; (b) adds `seldon rules update
  [--json]`: in a fenced file it rewrites the block only; in an unfenced
  file it inserts the block at the top and keeps the old text below it
  under `## Your rules (kept)`, prints the diff and autocommits;
  `--replace` (opt-in) archives the old file to `archive/AGENTS-<date>.md`
  instead; never runs on its own; (c) `seldon doctor` gets `rules:
  current | outdated (v1) — fix: seldon rules update`, and the panel's
  doctor banner shows the one-click fix. The `de` template changes in
  the same WP (ADR-0007).
- **Risk accepted.** An agent acting inside the Intent and closing its
  own case can be wrong without anyone noticing until a spot check; the
  answer is the snapshot it took, the preview line, the complete trace
  with the agent as actor, the `closed-by-agent` filter, and a one-click
  reopen — not a human step before or after every command. Case
  snapshots compete with update snapshots for five numbered slots; the
  pruned-rollback warning makes that visible instead of silent.
