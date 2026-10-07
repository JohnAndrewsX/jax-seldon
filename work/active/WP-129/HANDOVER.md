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
