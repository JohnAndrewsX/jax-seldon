# WP-013 findings — engine ↔ plugin on a real logbook

Runs: 2026-10-01, `main` at `78aa1ff` (engine and plugin unchanged by this
WP). The test host ran Omarchy 4.0.4-1 and quickshell 0.3.1; the dev host
the same.
- Dev host: `just e2e --engine-only`.
- Test host: `SELDON_TEST_HOST=<alias> just e2e`, and once with
  `SELDON_E2E_OUT=<scratch dir>` for the excerpts below.

Excerpts are trimmed. The machine id and the logbook path are left out,
because both name the host.

## 1. Engine output vs plugin display: no mismatch found

Every surface the WP names agrees with the index the engine wrote. The
table lists what was compared; the figures are from the test host
(`SELDON_E2E_OUT` run).

| Surface | Command | Plugin shows | Index says |
|---|---|---|---|
| Status | `omarchy-shell jax.seldon.service status` | `"status":"ok","engine":"present","lastError":""` | `state.status: "ok"`, `contractVersion: 1` |
| Pill text | `omarchy-shell jax.seldon.panel pill` | `"text":"⟡ 1 · 2"` | `summary: {"activeCases":1,"openDrift":2,"crisis":1}` |
| Pill tone | same | `"tone":"urgent"` | `crisis: 1 > 0` |
| Tooltip | service `status` | `Seldon — 1 active case, 2 unexplained changes (1 in the red zone), last capture just now` | same counts |
| Crisis strip | `omarchy-shell jax.seldon.panel view` | `1 change in the red zone needs a reason` | one drift item with `crisis: true` |
| Snapper banner | same | `Snapshots not readable` | `state.collectors[snapper].ok: false` |
| Today | `jax.seldon.panel tab today`, `view` | `today.entries: 1` | `today.entries`: the `seldon log` note, text verbatim (quotes, `$(true)`) |
| Changelog | `jax.seldon.panel filter pacman`, `view` | `rows: 4`, `badges: ["brave-bin +3"]` | 4 `source: pacman` events, one group with `members: 3` |
| QuickEntry | `wtype n`, the text, `wtype -k Return`, then `view` | `result: "Saved to the journal · <id>"`, `today.entries: 2` after the FileView refresh | note in `today.entries` and in `ledger/2026-10.jsonl` |

On the dev host, the headless `Service.qml` (dev mode) against the
`--engine-only` index gave `status: ok` and pill `⟡ 1 · 22`, for
`summary {"activeCases":1,"openDrift":22,"crisis":22}`.

Index excerpt (test host, after the plugin's start-up capture):

```json
"summary": {"activeCases":1,"queuedCases":0,"openDrift":2,"crisis":1,"eventsToday":3,"events7d":7},
"drift": [
  {"eventId":"01M3VRYNSNK9Q0NAAJPTKAX1HJ","source":"pacman","kind":"upgrade","subject":"brave-bin",
   "zone":"yellow","crisis":false,"txId":"tx-20260930T075143","members":3},
  {"eventId":"01M3VRYNSM2CH6NKRHB75C8QYT","source":"pacman","kind":"reinstall","subject":"archlinux-keyring",
   "zone":"red","crisis":true}
]
```

Panel view (`jax.seldon.panel view`, Changelog, filter all):

```json
{"rows":7,"badges":["brave-bin +3"],"folded":0,"snapshots":0,
 "driftTones":["mise-bin accent","flea accent","brave-bin accent","archlinux-keyring urgent"]}
```

## 2. Reading questions (consistent data, worth a decision)

1. **Drift rows vs the drift count.** The pill and the tooltip say 2
   unexplained changes, but the Changelog marks 4 rows as drift: three
   members of one `txId` group (yellow → accent) and the red keyring
   reinstall.
   - The leader row's badge reads `+3`. N counts every member including
     the leader (schema `members`; `Model.js` cites ADR-0013 §2). The other
     two members are still rows of their own.
   - ADR-0013 §2 says "members get no rows of their own". That is true
     for `index.drift`. The Changelog lists events, so they appear
     anyway.
   - A user may read `+3` as "three more".
   - The data agrees on both sides. Whether the Changelog should hide
     collapsed members, or show `+2`, is a plugin/ADR reading for the
     orchestrator.
2. **A backfilled first capture opens with a red pill.**
   - A fresh logbook records nothing before its `created` (SPEC-ENGINE
     §3). So the e2e passes `--since <now − 7 d>` to get package events.
   - With that window, every package change becomes open drift:
     - test host: 2 items, one crisis (the `archlinux-keyring`
       reinstall, a named package → red, ADR-0013 §3);
     - dev host: 22 items, all 22 crises.
   - This is the specified behaviour. It means that a user who backfills
     at onboarding starts with an urgent pill and a crisis strip.
3. **Without `--since`, the first capture shows nothing.**
   - After `seldon init` and the plugin's own `capture --all`, the
     Changelog has only the case and note events, until the system
     changes.
   - This is the specified behaviour, and it is the reason the e2e needs
     the window.

## 3. Engine output that disagrees with itself (not a display mismatch)

1. **`seldon init --json` reports a stale capture step.** Command:
   `seldon init --non-interactive --path ~/Seldon-e2e --json`.

   ```json
   "capture":{"ran":false,"reason":"no collectors in this engine version"},
   "nextSteps":["seldon doctor","seldon capture --all", …]
   ```

   The engine on `main` has six collectors (`capture --all` right after
   it wrote 1230 events on the dev host). The field dates from WP-003.
   Either `init` runs the first capture, or the reason text should say
   "run `seldon capture --all`". This is for an engine WP; the plugin
   does not read `init` output.

## 4. Test host behaviour found by the runs

1. **A shell restart right after the rsync crashed Quickshell.**
   - In the first full run, `omarchy-restart-shell` ran about 4 s after
     `rsync` had rewritten the plugin dir.
   - The shell then logged "Local plugin changed, reloading: jax.seldon"
     about 20 times, once per file. The old instance segfaulted on exit
     (`~/.cache/quickshell/crashes/<run id>/report.txt`: signal 11, top
     frame in libc; log tail: `Exiting due to IPC request`, then a
     desktop-entry rescan).
   - Quickshell's crash handler opened a crash-report window (`class
     org.quickshell`, workspace 6), which stayed open.
   - A later restart (`quickshell kill -p … --any-display`) closed it:
     the host is back to one `quickshell` process.
   - Not attributable to jax.seldon: the trace has no QML frame, and the
     host has older crash reports from August, before Seldon.
   - What the script does about it:
     - it rsyncs with `--checksum`;
     - it waits for the reloads (ping, then 5 s) before any restart;
     - the restore restarts only when the plugin code differs from what
       the shell runs;
     - every run fails if a new crash report appears.
   - After this change, three full runs passed and a fourth stopped at the
     locked session (item 2). None of them added a crash report: the count
     stayed at 3.
2. **The test host's session locked during the runs** (15:03:36,
   `omarchy lock lock-requested`, 28 s after a shell start). The journal
   names no idle or lock trigger.
   - The next `omarchy-restart-shell` saw a locked session and re-locked
     it after the restart (by design, `omarchy-restart-shell`).
   - The run that followed failed at its own restart, which is refused
     while the session is locked. Its EXIT-trap restore ran and put the
     host back.
   - The script now refuses to start on a locked session. It re-checks
     the lock before the restart and before every `wtype`, so keys never
     go to a lock screen.
   - **The test host is still locked**; only the operator can unlock it.
3. **`pgrep -x quickshell` is not the shell.** It also matched the
   crash-report window, so the first version of the log check read that
   window's log and passed vacuously. The script now takes the pid from
   `quickshell list -a -j` (config path `$OMARCHY_PATH/shell/shell.qml`),
   and requires a "Configuration Loaded" line before it trusts the log.
