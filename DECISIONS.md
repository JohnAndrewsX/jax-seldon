# DECISIONS.md — ADR index

| ID | Title | Status |
|---|---|---|
| ADR-0001 | Name, identifiers and binary name | accepted |
| ADR-0002 | Engine in Rust, plugin in QML, JSON contract between them | accepted |
| ADR-0003 | Markdown is the record; the ledger's JSONL is the machine source | accepted |
| ADR-0004 | Engine ships via AUR, plugin stays thin | accepted |
| ADR-0005 | No daemon by default; shell-timer driven capture | accepted |
| ADR-0006 | Logbook follows the Hermes project layout | accepted |
| ADR-0007 | Language policy | accepted |
| ADR-0008 | Drift is a first-class object | accepted; meaning of Crisis superseded in part by ADR-0028 |
| ADR-0009 | Monorepo for development, `jax-seldon-plugin` for the installable plugin | accepted |
| ADR-0010 | Default logbook path and the wizard's path options | accepted |
| ADR-0011 | Snapper collector degrades without privileges; the user opts in | superseded by ADR-0026 |
| ADR-0012 | Contract v1: schema clarifications and index derivation rules | accepted |
| ADR-0013 | Drift is grouped per pacman transaction; routine upgrades are yellow | accepted; §3 superseded in part by ADR-0028; §5 absent read as absent or stale (WP-160; rule in SPEC-ENGINE §4 pacman, "Stale lock") |
| ADR-0014 | Event attribution, zones, plugin version source, agent file edits | accepted; §2 superseded in part by ADR-0028 |
| ADR-0015 | Drift-group schema rules and the proposal token rule (supersedes ADR-0012 §13) | accepted |
| ADR-0016 | Engine install and update commands go through the AUR helper (supersedes ADR-0004 on the install command) | accepted |
| ADR-0017 | Hook command events carry the start time; attribution rules sharpened (clarifies ADR-0014 §1) | accepted |
| ADR-0018 | No `plugin-update` events for first-party plugins (clarifies ADR-0014 §3) | accepted |
| ADR-0019 | Green zone for agent commands no collector tracks (clarifies ADR-0014 §2) | accepted |
| ADR-0020 | The index lists at most 200 open drift items, crises first | accepted |
| ADR-0021 | The index folds `case` from any resolution that carries one (clarifies ADR-0012 §8) | accepted |
| ADR-0022 | The AUR package builds against glibc; the static musl binary is a release asset | accepted |
| ADR-0023 | An agent verifies, the human closes; risk levels R0–R3 defined, enforced by advice only | accepted; §1 superseded in part by ADR-0027 |
| ADR-0024 | While the AUR package does not exist, the plugin's one-click engine install runs the verified GitHub installer | accepted |
| ADR-0025 | The index clips long texts of events and drift items with a visible marker (extends ADR-0020); the reference clip followed in WP-077 | accepted |
| ADR-0026 | Snapper access by a read grant on the snapshot directory; doctor prints the revert of the old opt-in (supersedes ADR-0011) | accepted |
| ADR-0027 | Act, then account: a case the user started authorises the agent; the agent verifies and closes; only steps that can make the machine unbootable need the user's go | accepted; H2 amended in part by ADR-0029, §1 prompt target by ADR-0031 |
| ADR-0028 | Attention is earned by consequence: routine changes are history, not drift; crisis by the harm test | accepted; four §2 rows amended by ADR-0037, two rows added by ADR-0042 |
| ADR-0029 | A change made while exactly one case planned it is that case's; a case captures before it closes | accepted |
| ADR-0030 | The Seldon agent starts like the Omarchy agent: from the caller's folder, hooks user-wide, served by a launch marker | accepted; §1 clause (b), §3 and §5 amended in part by ADR-0032 |
| ADR-0031 | Password prompts follow Omarchy: as few as the route allows; snapshot `root` only | accepted |
| ADR-0032 | The launch marker serves a session only while its case is open; existing installs' hooks go user-wide once | accepted |
| ADR-0033 | A new logbook looks back 90 days and marks that history "before Seldon", without asking | accepted |
| ADR-0034 | The plugin is a desk: one wide panel with sidebar, list and detail; Prime Radiant and the graph live inside | accepted |
| ADR-0035 | Contract v2: the case's risk in the ledger (`case-updated`, `meta.risk`), the autocommit result, `meta.truncated`, the `state-loss` kind, `decisions[].cases`, the triage proposal | accepted |
| ADR-0036 | Agent prompts carry identifiers, never logbook text; proposals are evidence or nothing (`agent ask`, `drift propose|apply|discard`) | accepted |
| ADR-0037 | ADR-0028 amended: toggles are routine both ways, `system-link` narrowed, the `authorized_keys` files default persistence paths | accepted |
| ADR-0038 | The index carries what the desk's details show: `drift[].rule`, `cases[].intent`/`result`/`source`, `decisions[].lead` (optional, contract 2) | accepted; §2/§3 amended by ADR-0048 |
| ADR-0039 | The hook records an agent's privileged commands (`sudo`, `doas`, `pkexec`, `run0`); events, not drift, in contract 2 | accepted |
| ADR-0040 | Accepting a decision is the user's act: `seldon decide accept`, one click in the desk | accepted |
| ADR-0041 | One agent per case: a session is a window; `agent focus`, `agent sessions`, `--again`, `open --editor` focuses its window | accepted |
| ADR-0042 | ADR-0028 amended: a file pacman left is attention, a crisis beside the boot and login files (mkinitcpio, Limine, PAM) | accepted |
| ADR-0043 | A pacman transaction that did not complete says so: `meta.txStatus` `failed`/`interrupted`/`unfinished` (optional, contract 2) | accepted; its Consequence on a stale lock is resolved by WP-160 (rule in SPEC-ENGINE §4 pacman, "Stale lock") |
| ADR-0044 | The desk reads a case's whole Intent (`plan show --json` `intent {text, lines, truncated, hidden}`) and imports task files (`import task`): two plugin-command rows; an imported Intent fits the desk, nothing invisible reaches it | accepted |
| ADR-0045 | EASY \| PRO: one desk, two views; the mode is the plugin setting `deskMode`, a fresh install starts in EASY | accepted |
| ADR-0046 | Recently edited files under `~/.config` outside the watch paths in the index (`system.recentConfig`, optional, contract 2); `seldon config watch` adds one to `watchPaths` (the desk's *Watch*); amends ADR-0047 §3 (one walker: root `~/.config`, file links inside it) | proposed |
| ADR-0047 | Before the logbook exists, the desk shows what the machine remembers on its own: `seldon preview --json` (pacman transactions, files edited under `~/.config`, 7 days, read-only), one plugin-command row and `preview.schema.json` | accepted |
| ADR-0048 | ADR-0038 amended: the invisible set is every default-ignorable code point; redaction reads the text without invisible and control characters and as given | accepted |

ADR files live in `decisions/`. New ADR: copy `decisions/ADR-0000-template.md`.
