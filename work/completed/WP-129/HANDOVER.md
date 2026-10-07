# WP-129 — Handover

Branch `wp/129-privileged-commands`, base `next` c1ee5d27, to be merged
into `next`. Engine and docs only; no schema, fixture or plugin change.

## What was done

- **`pkgcmd.rs`.** `PRIVILEGE_WRAPPERS` (`sudo`, `doas`, `pkexec`,
  `run0`); `Unwrapped::privilege` names the first of them that
  `unwrap_command` read past (none for a probe, which runs nothing);
  `Segment::privileged_by` carries the wrapper of a `sh -c '…'`/`eval`
  into the commands opened from it (`pkexec sh -c 'lpadmin …'`), and
  `Segment::privilege()` gives the wrapper a command runs under.
- **`commands/hook.rs`.** `mutations(line)` returns the line's class
  record as before plus at most one **privileged record**: the first
  command that runs under a privilege wrapper a program that is no probe,
  when no class records it by itself. `classify` keeps its meaning (the
  class record only). `Mutation::wrapper`; the record is `agent/command`,
  subject the program's last path component, zone red, recorded with or
  without a case (linked to the active one), `detail` and `meta.command`
  the redacted, clipped line, `meta.wrapper` the wrapper. `bash_records`
  writes both records of one tool call in one append (same `toolUseId`,
  so a replay writes nothing).
- **`case_notes.rs`.** A command with `meta.wrapper` is never a snapshot
  command (`sudo snapper -c root delete 7` must not own a snapshot).
- **ADR-0039** (proposed), DECISIONS.md row, SPEC-ENGINE §8, guide 04
  en/de, CHANGELOG.

## Decisions (what the WP left open)

1. **Kind stays `command`; the class is `meta.wrapper`.** The WP said
   "for example `privileged-command`". A new kind is an enum change in
   `event.schema.json` (contract), and every reader of hook records
   (attribution, case notes, dossier, views) keys on `kind: command`.
   `meta` admits extra string keys, so `meta.wrapper` needs no schema
   change; only privileged records carry it. For the live acceptance,
   "one `privileged-command` event" means one `agent/command` event with
   `meta.wrapper`.
2. **Zone red.** The command acts as root on the system (ADR-0014 §2);
   it also makes `plan snapshot`'s "snapshot after the case's first red
   change" warning see it.
3. **Recorded without a case.** Red, so ADR-0019 does not drop it; the
   WP's goal is "every command … in the record".
4. **Probes.** The wrappers' own (already in `pkgcmd`), plus `true`,
   `false`, `:`, `id`, `whoami`, `test`, `[` (`sudo -n true`, `pkexec
   whoami`). Reads as root (`sudo cat /etc/cups/printers.conf`) are
   recorded: a use of granted privilege.
5. **"Already recorded by its own class"** = a class record that needs no
   case, or a snapshot command (which stays green-with-a-case as WP-101
   needs it). An ADR-0019 green record that needs a case (`sudo tee
   /etc/x`, `sudo npm i -g x`) gives way to the privileged record for that
   command, so it is one event, never two.
6. **One privileged record per line**, for the first privileged command;
   beside the line's class record when another command has one
   (`pkexec pacman -S cups && pkexec lpadmin …` → `pacman` + `lpadmin`).
   Two events with the same `meta.command` are harmless to attribution
   (`max_by_key` over equal causes).
7. **Wrapper reported:** the outermost (`sudo sh -c 'pkexec x'` →
   `sudo`; `env sudo x` → `sudo`).
8. **`detail` and `meta.command` both** hold the line: the index's drift
   row and Changelog show `detail`; attribution and the dossier read
   `meta.command`. Costs up to 4 KB more per privileged line in the
   ledger.
9. **ADR number 0039:** 0036–0038 are taken on other branches.

## Stopped at: contract question (for the orchestrator / operator)

The WP's **"Classification under ADR-0028: attention"** is not
implemented. Making the event attention means making it drift-eligible,
and `index.schema.json` restricts `drift[].source` to `pacman`,
`omarchy`, `plugins`, `theme`, `config` ("only system-changing collectors
produce drift"). An `agent` drift item fails `index --check`. Contract
2's pre-tag window (CONTRACT.md, ADR-0035 §6) allows new optional fields
only, not a wider enum. So this is a contract change beyond the WP.

ADR-0039 §3 lays out (a) widen contract 2 before the 0.2.0 tag
(recommended: the plugin already treats `source` as a label and has an
`agent` glyph; engine work is one small WP: eligibility in `build.rs`
×3, `reconcile.rs` ×2, `class.rs` rule `privileged-command`,
`validate-fixtures.py` port, one fixture item) or (b) defer to contract
3. Until decided the engine does (b): the record is in the ledger, the
case and the Changelog (`index.events`), never in `drift`. The test
`privileged::recorded_without_a_case` pins that and flips with (a).

## Security question (for the orchestrator / operator)

This WP writes command lines into the ledger that were never recorded
before. SPEC-ENGINE §7's rules mask URL userinfo (`ipp://user:pw@…`,
`smb://…` — tested), `--password`, `password=`, tokens, … but **not a
secret given as a plain positional argument** (read from `redact.rs`: no
rule names `nmcli`, `psk`, `htpasswd`, `usermod` or `chpasswd`), e.g.
`sudo nmcli dev wifi connect X password hunter2`,
`nmcli con modify X wifi-sec.psk hunter2`, `htpasswd -b f user pw`,
`usermod -p <hash>`. Before this WP such lines were not recorded (no
class); now they are, in clear text, unless the user has a `[redaction]
patterns` entry. I did not add a rule: it is a §7 design change, the
redaction module has its own perf budget (WP-084/108), and WP-128 is
changing `redact.rs` in parallel. Options: a follow-up WP with `nmcli`
(`password`, `*.psk`, `*.password`, `*-password` property values) and
the `htpasswd -b`/`chpasswd`-style cases; or hold the merge until it
lands. Your call.

## Scope note for the live acceptance

The printer session of 2026-10-07 ran from `~/Work`. With the default
`[hooks] scope = "logbook"` (ADR-0030 §1) a Claude Code session there is
**not served** unless `seldon agent start` launched it (`SELDON_CASE`) or
the user set `scope = "all"`. The live check must run the agent inside
the logbook or via `seldon agent start`, else nothing is recorded — by
design, not by this WP. Expected result: one `agent/command` event,
subject `lpadmin`, `meta.wrapper: pkexec`, zone red, on the case when
one is active.

## How it was verified

- Unit (`cargo test --lib`): `pkgcmd::tests::the_privilege_wrapper_is_named`
  (wrappers, paths, chains, `sh -c`, `eval`, `&&`, variables, outer
  redirection, probes), `hook::tests::privileged_commands` (13 lines →
  one privileged record each), `…_are_recorded_once` (package/systemctl/
  omarchy/watched write → class only; snapshot stays green-with-case; two
  classes in a line; one privileged per line; green elsewhere kept;
  17 probes/unwrapped lines → nothing).
- Integration (`tests/hooks.rs` `privileged::`, 7 tests): the WP's list —
  `pkexec lpadmin -p … -v ipp://… -E` one event on the case, red, wrapper,
  detail, toolUseId; replayed PreToolUse and its PostToolUse write
  nothing; `sudo nmcli con up x` without a case one event, in
  `index.events`, not in `drift`; `pkexec pacman -S x` only the package
  event; two classes in one line; probes none; `bash -c` and after `&&`
  and `sudo sh -c` recorded; `ipp://scan:hunter2@…` redacted in
  `detail`, `meta.command` and the ledger file; `hook generic` with
  `doas`.
- `tests/case_notes.rs`: `a_privileged_snapper_command_is_no_snapshot_command`.
- Mutants (separate target `target/mutants-wp129`, runner script outside
  the repo): 8/8 killed — the case-notes filter, the snapshot exemption,
  the probe list, the needs-case supersede, the `sh -c` inheritance, the
  first-wrapper rule, `meta.wrapper`, `detail`.
- `git diff next | grep /home/`: only the existing `/home/user` fixture
  paths.
- **The hook budget bench changed** (`claude_code::hook_budget`): its
  third line pipes into `sudo -E bash`, which is now a privileged record
  beside the `git commit`, so each call writes two events; the bench
  expects that (and that the last one is `bash` with `meta.wrapper:
  sudo`), and the 900-line case keeps 180 instead of 150 lines below the
  rebuild threshold for them.

## Not done / open

- Attention class (contract, above).
- Positional-secret redaction (security, above).
- Live acceptance on the test host: the orchestrator's, with the
  operator (second network printer).
- Commands run by `xargs`, `find -exec`, an interpreter, or inside
  `$(…)` stay unread (SPEC-ENGINE §8 limits), privileged or not.

## Check

- `flock /tmp/seldon-check.lock just check` on ce401bd4: `check: ok`
  (exit 0; log `target/check-wp129-r2.log`). r1 on 708c30b7 was ok too.
- `flock /tmp/seldon-check.lock just check-perf` on ce401bd4: exit 0.
  Hook medians (bench profile, tmpfs, dev host under other agents' load):
  10 000 lines — not recorded 0.75 ms, recorded 1.58 ms, curl with a
  marker 3.02 ms, `sudo -E bash` line (two events) 2.53 ms; 900 lines
  with the rebuild — 0.72 ms, 3.31 ms, 4.45 ms, 4.37 ms (was 4.25 ms in
  r1 before the bench change, one event fewer); unrelated session
  0.70/0.91/0.90 ms (budget 1 ms). r1 failed only on the bench's event
  count (above), every timing within budget.

## Merge of next

`next` moved to afc7a586 (WP-124a, WP-125, WP-135, WP-136..140 work
files) after the branch point. Merged it into the branch (dc02b665):
conflicts only in CHANGELOG.md (both entries kept), DECISIONS.md (ADR-0036
row, then ADR-0039) and the guide 04 de source line (set to the merge
commit in 9aa6cbe0; the English page holds both changes, docs-check ok).
No engine conflict.

- `flock /tmp/seldon-check.lock just check` on 9aa6cbe0: `check: ok`
  (exit 0; log `target/check-wp129-r3.log`).
- `check-perf` was not re-run after the merge: next's engine changes
  (triage) do not touch the hook path; the r2 numbers above are on
  ce401bd4.

## Round 2

Brief: the orchestrator's round-2 brief from the stage-1 review (SEND
BACK for B1, plus N2–N4 and decisions 1–5).

### Fixed

- **B1 — a password piped into `sudo -S`.** `unwrap_command` reports
  `Unwrapped::password_on_stdin`: sudo's `-S`, alone or in a cluster
  (`-Su root`, `-kS`), `--stdin` and the abbreviations getopt takes
  (`--st`, `--std`, `--stdi`; `--s` is ambiguous in sudo and is not
  taken). It is read also when the wrapper only probes (`echo PW | sudo
  -S -v && sudo pacman -S x`, `sudo -vS`): a probe now reads the rest of
  its options before it returns, so a later `-S` is seen. `-uS` stays the
  user `S`. The hook records every record of a line in which any command
  has it (`sh -c` opened) as `<program> ‹redacted›`, the skipPaths form.
  Hook-local; `redact.rs` untouched.
  - **Other wrappers:** `doas` (OpenBSD and opendoas: `-a -C -L -n -s -u`)
    asks on the terminal and has no stdin option; `pkexec` and `run0`
    authenticate through the polkit agent and have none. Only sudo's
    `-S` is in the rule. `sudo -A` (askpass) runs a program and puts no
    password on the line.
  - Tests: `pkgcmd::a_password_on_stdin_is_seen` (10 positive, 9
    negative forms); `hooks::privileged::a_password_on_the_wrappers_stdin_is_never_recorded`
    (six lines, also `<<<` and a probe beside a package command): no
    `hunter2` in the ledger, `detail`, `meta.command` or `index.json`.
- **N2:** `privileged::without_a_case_only_the_privileged_command_of_the_line`
  — `sudo lpadmin -x X; echo done > /tmp/log`, no case → one `lpadmin`.
- **N3:** SPEC-ENGINE §8 "Non-mutating commands produce no event, except
  privileged commands (below)." Guide 04 en "known secret forms
  removed", de "bekannte Geheimnis-Formen entfernt".
- **N4:** ADR-0039 Consequences. Checked in the code and pinned by
  `privileged::an_always_red_subject_raises_the_r3_advisory`: `sudo
  mkinitcpio -P` in an R1 case raises the **index build's** R3 warning
  (`status --json` warnings). The case's advisory *Log line* is written
  by a capture only for the events that capture writes, so a hook record
  gets the warning, not the Log line; the ADR says so.

### Decisions applied

1. **No `agent` drift source in 0.2.0.** ADR-0039 §3 rewritten as
   option (b), decided by the orchestrator: an event in the Changelog,
   red, on the case; the change itself is drift through its collector
   (WP-131 for the printer configuration); contract 3 if the live week
   shows a need. SPEC §8, guide 04, CHANGELOG and DECISIONS.md say the
   same.
2. B1 hook-local (above).
3. **Plain-argument secrets → WP-140.** Forms seen while reading
   `redact.rs` and the commands an agent uses with a wrapper: `nmcli dev
   wifi connect X password PW`, `nmcli con modify X wifi-sec.psk PW`
   (also `802-1x.password`, `vpn.secrets`), `htpasswd -b FILE USER PW`,
   `usermod -p HASH` / `useradd -p HASH` (a hash, still a secret),
   `smbpasswd`/`chpasswd` only via stdin (covered when piped into
   `sudo -S`, not when piped into the program itself: `echo u:pw | sudo
   chpasswd` keeps `u:pw` in the line). No rule names any of them.
4. **N1 accepted noise:** ADR-0039 Consequences: a privileged read counts
   as the case's first red change; rare, each asked for with a password.
5. **PreToolUse:** nothing in the hook confirms success (a PostToolUse
   for a recorded `tool_use_id` writes nothing; `tool_response` is never
   read). So the privileged record's `detail` is `asked to run: <line>`
   (`hook::ASKED_TO_RUN`; `meta.command` stays the bare line, which
   attribution and the dossier parse); SPEC, ADR and guide say "asked to
   run", and that a cancelled prompt still leaves the record. If "the
   detail" in the brief meant only the ADR's description and not the
   event's `detail`, the prefix is one line in `record()` to drop.

### Mutants

Separate target `target/mutants-wp129`, runner outside the repo: 7/7
killed — the hook's use of the flag, the probe path dropping it, cluster
detection, the abbreviation rule (prefix, minimum length), a probe letter
ending the cluster scan, the `asked to run` prefix. Round 1's 8 stay
killed (their tests unchanged but for the `detail` prefix).

### Merge of next

`next` moved to cdb45963 (one work file, WP-140): merged without
conflicts.

### Check (round 2)

- `flock /tmp/seldon-check.lock just check` on 3df8bfe4: `check: ok`
  (exit 0; log `target/check-wp129-r2-1.log`).
- `flock /tmp/seldon-check.lock just check-perf` on 3df8bfe4: exit 0.
  Hook medians, bench profile, tmpfs, shared dev host: 10 000 lines —
  recorded 1.67 ms, curl with a marker 3.17 ms, `sudo -E bash` line
  2.55 ms; 900 lines with the rebuild — 3.25 ms, 4.40 ms, 4.73 ms (worst,
  budget 5 ms; r1's 4.37 ms, the line now also runs the stdin check);
  unrelated session 0.87–0.89 ms (budget 1 ms).
- `git log -p --no-merges next..HEAD` (this WP's own commits): no added
  `/home/` but the `/home/user` fixture placeholder. (`git diff
  c1ee5d27 HEAD` also shows three `/home/<user>`-style placeholder lines
  from other WPs' handovers that came in with `next`.)

## Round 3

From the Fable stage 2 (round 2 confirmed; one small round).

### Done

- **Reserved words.** `pkgcmd::unwrap_command` reads past `if`, `then`,
  `elif`, `else`, `do`, `while`, `until`, `!` and `{` in the same loop as
  the leading assignments (`RESERVED_BEFORE_COMMAND`), before the wrapper
  lookup. So `for p in a b; do sudo lpadmin -x $p; done` and `if ! sudo
  lpadmin -x X; then echo no; fi` are one privileged `lpadmin`/`sudo`
  record each, `for p in a; do sudo pacman -S $p; done` is red `pacman`
  (also `if ! pacman -S x; …`), and `do`, `if`, `!`, `{`, `then fi`, `do
  done` record nothing. It applies to every class, since every class reads
  `command_argv`. Test `hook::tests::reserved_words_are_read_past` (8
  privileged forms incl. `while`, `until`, `elif`, `else`, `{ …; }`);
  mutant (no skip) killed.
- **SPEC-ENGINE §8:** after "is re-parsed": "a shell reserved word before
  a command (`do`, `if`, `!`, `{`; also `then`, `elif`, `else`, `while`,
  `until`) is read past".
- **ADR-0039:** a *Limits* paragraph at the end of §1 (the text of the
  brief), and the stdin bullet of §2 ends "The check reads the same
  commands as the classification: a `sudo -S` inside `$(…)` is not seen."

### Guard

The guard hook blocked one read-only `grep` whose Bash command text named
a wrapper (finding the ADR lines to edit). Not reworded; the lines were
read with the Read tool and edited with Edit.

### Merge of next

`next` moved to 00d5f73e (WP-127 merged: ADR-0038, index details; work
files WP-140..148). Merged as b67b3ae3; one conflict, DECISIONS.md
(ADR-0038 row, then ADR-0039). No engine conflict.

### Check (round 3)

- `flock /tmp/seldon-check.lock just check` on 783d5f4d (before the
  merge): `check: ok` (log `target/check-wp129-r3-1.log`).
- The same on b67b3ae3 (after the merge): `check: ok` (log
  `target/check-wp129-r3-2.log`).
- `check-perf` not re-run: the change adds a 9-entry slice lookup for
  each word the wrapper loop of `unwrap_command` looks at; round 2's numbers (worst 4.73 ms)
  stand.

## Accepted and merged next

- **ADR-0039 accepted** by the operator on 2026-10-07 (E16), after Opus
  and Fable stage 2. The ADR's status line and DECISIONS.md row say so
  (b01cfd0c); the CHANGELOG entry no longer says "proposed".
- **Merged next b5e20c13** (WP-113, WP-124a, WP-127; `origin/next` was
  the same commit after `git fetch`) as a115c25c. One conflict:
  CHANGELOG.md, both entries kept. WP-113's `pkgcmd.rs` change (yay's and
  paru's long options with a value in `LONG_WITH_ARG`) merged without a
  conflict beside this WP's wrapper code; `hook.rs` was not touched by
  next. This WP touches no fixture, so no `--write-index` was needed.
- `flock /tmp/seldon-check.lock just check` on a115c25c: `check: ok`
  (exit 0; log `target/check-wp129-accepted-1.log`).
- `next` moved during that check to d747daa8 (work/queued WP-154..156
  only, three Markdown files); merged without conflict. `just check` was
  not re-run for it: no engine, plugin, schema, fixture or doc page
  changed (docs-check ok).

## Merge of next (2)

- Merged next 2ad42a43 (WP-128, redaction CRLF: `redact.rs`,
  `tests/redaction.rs`, `import/task.rs`, SPEC-ENGINE §7, CHANGELOG) as
  f96c70e7. No conflict; CHANGELOG.md and SPEC-ENGINE.md merged
  automatically. This WP's hook tests (`privileged::`, 10) pass on top of
  WP-128's redaction, the `ipp://…` userinfo and the `sudo -S` line
  redaction included.
- `flock /tmp/seldon-check.lock just check` on f96c70e7: `check: ok`
  (exit 0; log `target/check-wp129-merge2-1.log`).
