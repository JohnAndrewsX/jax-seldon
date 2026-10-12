# WP-157 — Handover

Branch `wp/157-redaction-paths` from `next` (3ba46b8f); merges into `next`.
Security class: reviewer chain Opus, then Fable. Every secret in the tests
is a made-up value (`fake…`, `hunter2`, base64 of a made-up key body).

## What was done

SPEC-ENGINE §7 rules (`engine/src/redact.rs`, `redact::BUILTIN` now 31):

1. **`private-key-base64`** (new, second in the order): a PEM block in
   base64 (`tls.key: LS0tLS1CRUdJTi…`). The WP's pattern, widened: one
   token of base64 characters that holds `-----BEGIN` encoded at any of the
   three alignments (`LS0tLS1CRUdJTi`, `LS0tQkVHSU4`, `LS0tLUJFR0lO`, so a
   file with a line before the key, such as openssl's `Bag Attributes`,
   counts too), and at least 40 more characters, masked whole from the
   token's first character; plus each following line that holds base64
   characters only after its indentation (`base64` wraps at 76, a YAML
   block scalar indents) and each part after a literal `\n` inside a shell
   string. Second in the order, so `aws-access-key` or `key-assignment`
   never cut the token first. Triggers: the three literals as written.
2. **`json-secret`**: a Python dict's single quotes on the key, the value
   or both, and a Python string prefix before the value (`{'x-api-key':
   'k'}`, `{'password': "pw"}`, `{'token': f'…'}`); `''` and `f''` are no
   value. `{'Authorization': 'Bearer …'}` was already masked by
   `authorization-header` (WP-140); it has a row now.
3. **`mount-password`** (new, a §7 rule rather than hook-local): the
   whole option list after `mount -o`/`--options` (also `mount.cifs`,
   `mount.smb3`) when one of its options is `pass=` or `pwd=` (any case,
   quoted or not), within one command and every time; `password=` stays
   `secret-assignment`'s, so the two rules are disjoint.
4. **`nmcli-secret`**: a property after `-f`/`-g`/`--fields`/`--get-values`
   is a field to show, not a property to set: `nmcli -g wifi-sec.psk con
   show` no longer masks `con` (group `field` with `Rule::unless`).

Hook (`engine/src/commands/hook/secret_args.rs`, `engine/src/pkgcmd.rs`):

5. **`ssh-keygen -N`/`-P`** with a value that is not empty (`-N ''` sets
   no pass phrase and keeps the line whole); OpenSSH's value options are
   skipped (`-C '-N x'` is a comment).
6. **`mkpasswd`**: an operand is the password (`-m`, `-S`, `-R`, `-P` and
   their long forms take values, also as getopt prefixes; every word after
   `--` is an operand); `-s`/`--stdin`; or fed by the line.
7. **A process substitution as a secret, any program**: `<(` after an
   option or keyword whose name holds `pass`, `pw`, `secret`, `token`,
   `key`, `cred` or `auth` (`restic --password-file <(echo PW)`,
   `--passphrase-file=<(…)`, wg's `private-key <(…)`, also after a `\`
   line end or a quoted option). A plain scan of the text, no regex.
8. **xargs** is a wrapper of the shared parser: its value options
   (`-a -d -E -I -L -n -P -s` and the long forms), `-i`/`-l`/`-e` only
   with their value in the cluster, and `xargs` without a command (it runs
   `echo`) is the command itself. `… | sudo xargs htpasswd -b …` is
   `htpasswd ‹redacted›`; a privileged record's subject is the program
   xargs runs. The classifier and attribution read it too (`… | sudo xargs
   pacman -S` is a pacman command now).
9. **Fed only by a pipe into the command**: `Segment::fed` (new): the
   command after `|`/`|&` (not `||`, also past a `(`), one with a
   here-string or a process substitution among its words, and the commands
   of a fed `sh -c`/`eval`. `sudo passwd -S alice | grep alice` and `sudo
   cryptsetup status vault | grep cipher` stay whole. A file written by
   the line still feeds every command (B1 of WP-140).

Docs: SPEC-ENGINE §7 (the rules, not masked, masked too much) and §8 (the
wrapper list, the programs, what feeds a command); an amendment note at the
end of ADR-0039 (as WP-140 did); CHANGELOG [Unreleased]; guide 06 en/de
(de source line set to e69d7954); TESTING.md rows; module docs of
`redact.rs`, `hook.rs` (xargs no longer "not read yet") and `pkgcmd.rs`.

## Decisions (what the WP left open)

1. **mount is a §7 rule, not hook-local** — the WP lists it under the
   hook's programs; for the same reason as WP-140's nmcli decision: its
   secret is named (`pass=`), so the option list can be masked and the
   line keeps its device and mount point, and a note gets the mask too.
2. **The base64 rule takes all three alignments and the lines after the
   token** — the WP's single pattern misses a file whose key does not
   start at byte 0 and leaves the key body of a wrapped or YAML-indented
   block on the following lines. Masked too much, by design: a base64
   certificate or public key, and a word of base64 characters alone on the
   next line (`done`). Not masked: a first line with fewer than 40
   characters after `-----BEGIN`, base64 lines after a line that holds
   other text.
3. **`ssh-keygen -N ''` stays whole** — an empty pass phrase is no secret
   and the commonest form an agent writes.
4. **xargs: what xargs reads becomes arguments**, so a program run by a
   fed xargs counts as fed: `printf 'a\nb' | sudo xargs -n1 passwd -S` is
   masked (too much, documented); `echo PW | xargs mkpasswd` must be.
5. **Process substitution: any command inside** — `--password-file <(pass
   show x)` is masked too; the hook cannot tell `echo PW` from a password
   manager, and over-masking a recorded line is the safe side.

## Linear time

- **Regexes:** the `regex` crate guarantees linear time in the length of
  the text (finite automata, no backtracking; the crate rejects
  look-around and back-references). The new patterns use only classes,
  bounded and unbounded repetition, alternation and the `(?mR:$)`
  assertion, which the lazy DFA supports. Measured, bench profile:
  `engine/tests/redaction.rs` rows (in `just check-perf`) and a scaling
  probe from 64 to 512 KB (log `gates/scale-dev-157.log`, probe not
  committed): cost per KB flat for every new rule — base64 run 10.0–10.7
  µs/KB, one wrapped base64 key 11.8–12.7, base64 keys 13.5–19.9, python
  dicts 50.7–55.9, mount option lists 44.9–47.0, mount passwords 49–85
  (64 KB noisy), nmcli fields and secrets 45–81 (64 KB noisy); 512 KB at
  most 28.6 ms.
- **Hook code** (`secret_substitution`, the `fed` flag, xargs options):
  one pass; the backward look from each `<(` stops at the `(` of the one
  before it, so each character is read a bounded number of times. Measured
  end to end: `privileged::a_long_privileged_line_is_read_in_linear_time`
  (in `just check-perf`): 16 KB 19.4 ms, 64 KB 21.2 ms, 128 KB 26.5 ms
  (budget 60 ms), process start and the record on a disk TMPDIR included;
  the increment is ~0.07 ms per KB.

## How it was verified

- **`SELDON_FULL_CHECK=1 flock /tmp/seldon-check.lock just check` on
  9e87c95d: exit 0, `check: ok`** (log `gates/check-dev-157-r2.log`; private
  `CARGO_TARGET_DIR`, `TMPDIR`, `JUST_TEMPDIR` under the private gates
  folder, private `HOME` and a 0700 `XDG_RUNTIME_DIR`): 112 test binaries, 2901 passed, 0 failed, 22 ignored (the timing tests); install 229/0, bar-view 196/0, deploy-test-host 326/0, qmllint ok (52 files), docs-check ok (no warning; the de page matches en), plugin-test ok with the Quickshell harnesses. Where:
  the engine tests are *fixture* tests; the plugin harnesses ran
  *headless* (offscreen Quickshell). An earlier full run on 65e31e96
  (code complete, before the timing rows and two test rows) was also
  `check: ok`: 112 test binaries, 2901 passed, 0 failed (log
  `gates/check-dev-157.log`).
- **`just check-perf` on ceaacd22: not green, as on the base.** Run right
  after the same commands on next 3ba46b8f (extracted with `git archive`,
  own target dir), same disk TMPDIR, host load 13–20 (other agents'
  gates). Failing on HEAD and on the base alike, none in code this WP
  touches:
  - `capture_cost_of_the_recent_config_scan`: 11.0 ms (base 14.7 ms),
    budget 10 ms;
  - `hook, recorded (tmpfs), 10 000 lines`: 13.0–15.4 ms (base 19.5),
    budget 5 ms — the state is on the disk TMPDIR, not tmpfs, so the sync
    of §1 comes on top;
  - `hook, recorded (tmpfs), 900 lines`: 30.5–32.1 ms (base 26.8–32.2),
    budget 5 ms — same cause;
  - `hook session-start, unrelated session`: 1.01–1.08 ms (base 1.29),
    budget 1 ms — load;
  - the index bench ×150: 85.3 ms on HEAD in the last run (passed), 118
    ms in an earlier run, base 115–127 ms (failed), budget 100 ms — load.
  The first three are the three non-index timings known to fail on a disk
  TMPDIR on the base. Everything else passed: `hook, not recorded` 0.85
  ms (10 000 lines), 0.92 ms (900), unrelated claude-code 0.80 ms, status
  71.8 ms, index ×10 13.5 ms, every redaction row (WP-157 rows: base64
  run 64 KB 0.64 ms, base64 near misses 0.65 ms, mount words 0.91 ms,
  nmcli fields 0.67 ms, single-quoted keys 1.67 ms, budget 2 ms; at 128 KB
  base64 keys 1.75 ms, mount option lists 5.74 ms, mount passwords 6.43
  ms, nmcli fields and secrets 6.43 ms, python dicts 11.15 ms, budget 20
  ms). Logs `gates/perf-dev-157-final.log`, `perf-dev-157-base.log`,
  `perf-dev-157-redaction.log`.
  One timing failure was this WP's and is fixed: my first `mount options`
  row (an `-o` every 37 bytes, each a match whose value check rejects it)
  was in the no-match group and took 2.67 ms at 64 KB (budget 2 ms),
  linear (0.66 ms at 16 KB). It is a many-matches workload, so it moved
  to that group (128 KB: 5.74 ms, budget 20 ms; ceaacd22), and the
  no-match group has a mount row that never matches (`mount words`, 0.91
  ms at 64 KB). The reviewer may judge that move.
- **Mutants:** `work/active/WP-157/mutants.py COPY` (applies each mutant
  to a copy of the tree with its own target dir; restores the source and
  kills the cargo process group on SIGTERM): **50/50 killed**, *fixture*.
  First run on ceaacd22: 49/50, the survivor `nmcli: only -g names a
  field` (no `-f <property> ` row); f353c4cd adds `nmcli -f wifi-sec.psk
  con show Home` to `CLEAR`, and that mutant is killed. One kill was
  "does not compile" (`substitution after key` shrank a fixed array);
  9e87c95d keeps the length, killed by the unit tests. Logs
  `gates/mutants-dev-157.log`, `mutants-dev-157-r2.log`.
- Rows: a positive and a negative test for each item (TABLE/CLEAR in
  `engine/tests/redaction.rs`, `CONTINUED` for the wrapped key and a
  continued `mount -o`, unit rows in `secret_args.rs` and
  `pkgcmd::tests::{xargs_is_a_wrapper, fed_commands}`, end to end
  `privileged::more_secrets_on_a_privileged_line`);
  `base64_key_and_mount_rules_are_disjoint_and_stable`; every TABLE row
  redacted twice is unchanged.
- shellcheck: *not run* — no shell script changed.
- *test host*, *desktop*, *CI*: *not run*.
- `git diff 3ba46b8f..HEAD`: no added home path, user or host name (the
  existing `/home/user/Seldon` fixture constant only).

## Not done / open questions

- `pwd=` as a mount option: CIFS knows `pass=` and `password=`; the WP
  names `pwd=`, so it is in, harmless outside a `mount -o`.
- mount passwords in `/etc/fstab` content or a credentials file are not a
  command line and stay as the WP-140 notes say.
- Not on `main`; merge with `next` only.
