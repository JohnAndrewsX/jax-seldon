# WP-140 — Handover

Branch `wp/140-redaction-more` from `next` (ac5bc9f9: WP-128 CRLF
redaction, WP-129 privileged hook records); merges into `next`.

## What was done

1. **PEM private keys** — new built-in rule `private-key`, first in
   `redact::BUILTIN` (now 29 rules), `engine/src/redact.rs`.
   `-----BEGIN <words> PRIVATE KEY-----` (none, RSA, EC, DSA, OPENSSH,
   ENCRYPTED, …; PGP's `PRIVATE KEY BLOCK`) up to the next
   `-----END … PRIVATE KEY-----`: the BEGIN and END lines stay, all
   between them becomes one `‹redacted›`, over any lines, LF or CRLF, also
   with literal `\n` inside a shell string. Cut by a clip: no END → masked
   to the end of the text; a lone END (the BEGIN cut off) → the run of
   base64 characters, blanks and line ends before it. Trigger
   `private key`. Every path through `Redactor`: `redact`,
   `redact_keeping_lines`, `matching_rules`; the index texts; and the
   **vault import** (`import::Scrubber::text`), which redacted line by
   line, so a key's body lines passed: it now redacts the whole text
   keeping lines (as `import task` does), and counts each changed line
   under every rule with a match that touches it
   (`Redactor::matching_rules_by_line`, new).
2. **The invisible set** — `import::is_direction_or_format` gains U+00AD,
   U+061C, U+180E, U+2061–U+2064, U+206A–U+206F, U+FFF9–U+FFFB,
   U+E0000–U+E007F. It serves the index texts (`shown_text`), `source`
   and the task import's path check. The plugin commit-subject cleaner
   (`collectors/plugins.rs`) had its own copy of the old set; it now calls
   the shared function. `scripts/validate-fixtures.py` (the reference
   derive and the source check) has the same set.
3. **Quoted header values** — `authorization-header` and `secret-header`
   take `"…"` (with `\"`), `\"…\"` and `'…'`, closed on their line;
   `Authorization` also as a quoted key (`"Authorization": "…"`,
   `'authorization':'…'`, `\"Authorization\": \"…\"`). `json-secret`
   takes `"x-api-key"` (`api[_-]?key`).
4. **Marker-only matches** — `Rule::masks_only_markers`: a built-in match
   whose masked part (the match without the groups its replacement keeps)
   holds a marker and else only markers and white space is left as it is.
   `matching_rules` follows, so no rule counts on such text.
5. **Plain-argument secrets** — hook-local, WP-129's `-S` style
   (`engine/src/commands/hook/secret_args.rs`, called from
   `bash_records`): the line's records become `<program> ‹redacted›` for
   `chpasswd`, `chgpasswd`, `htpasswd -b`/`-i`, `smbpasswd -s`/`-w`,
   `passwd -s`/`--stdin` (prefixes getopt takes), `useradd`/`usermod`/
   `groupadd`/`groupmod -p` (in a cluster too; value options skip their
   value), and `cryptsetup` on a line holding `|`, `<<<` or `<(`. nmcli:
   a §7 rule `nmcli-secret` (option rule, every time in one command): the
   value after the keyword `password` or a property whose last part names
   a secret (`….psk`, `….password`, `….password-raw`, `….secrets`,
   `….wep-key0-3`, `….private-key`, `….preshared-key`, `….pin`, with an
   optional `+`/`-`).
6. **Docs** — SPEC-ENGINE §3 (vault import), §6 (the set), the commit
   subjects, §7 (the rules, idempotence, not masked / masked too much),
   §8 (the hook list); amendment notes at the end of ADR-0038 (the set) and
   ADR-0039 (the hook list); CONTRACT.md rule 9 (the set); guide 04 and 06
   en/de (de source lines set, docs-check without warnings); CHANGELOG;
   TESTING.md rows (a new one for `engine/tests/redaction.rs`).

## Decisions (what the WP left open)

1. **nmcli is a §7 rule, not hook-local.** Its secrets are named by their
   property, so the value can be masked and the line keeps its other
   words (`sudo nmcli con mod Home wifi-sec.psk ‹redacted› ipv4.dns …`),
   and a note (`seldon log`) gets the mask too. The bare keywords `psk`
   and `pin` were dropped after the bench showed them as false matches;
   nmcli has them only as properties.
2. **The PEM rule keeps the BEGIN and END lines** (the label says what was
   there, as an option name does); the clipped forms over-mask (rest of
   the text; the base64 words before a lone END, glued prose included).
3. **Quoted header values need white space before them, or a quoted
   name.** A quote right after the colon of a bare name is read as the
   end of a shell word: `curl -H 'Authorization:' -H 'X: y'` (curl's way
   to drop a header) and `grep 'authorization:' f 'x'` stay as they are.
   So `X-Api-Key:"x"` (compact, bare name) is not masked; documented. A
   quote its line does not close starts no value; documented.
4. **The quoted-key form only for `Authorization`.** A quoted
   `"X-Auth-Token"` is `json-secret`'s already, and the disjointness test
   (the import counts a line once per rule) refuses a second rule for it;
   `"x-api-key"` went to `json-secret` instead.
5. **An empty header value is no value** (behaviour change from WP-128):
   the bare value may not start with white space or a marker. Before,
   `Authorization: ` masked the blank (`Authorization:‹redacted›`); now
   nothing is masked. The two WP-128 `CONTINUED` rows for empty values
   moved to `an_empty_header_value_is_no_value` (LF and CRLF, unchanged
   text, no rule). Needed for item 3: after `"Authorization": "x", "Accept":
   …` is masked, a second pass must not take `‹redacted›, ` as a value.
6. **"Only markers" allows white space between markers**, needs at least
   one marker, and holds for built-in rules only (a user pattern is
   applied as written, SPEC §7).
7. **Import counts.** `matching_rules` on already-redacted text now finds
   nothing where the masked part is only markers (every `TABLE` and
   `CONTINUED` row, redacted, matches no rule: tested). The task import
   counts `redactedLines` as changed lines and is not affected; its
   scrubber pass over redacted text no longer merges glued markers, so a
   task whose text holds a built-in marker beside a user pattern's could
   hash differently once (imported again as "changed"); needs a user
   pattern next to a masked value, so accepted.
8. **The hook list holds more than the WP named**: `chgpasswd`, `passwd
   -s`/`--stdin`, `groupadd`/`groupmod -p` (same shapes, same table).
   `cryptsetup` counts on a line with `|`, `<<<` or `<(` (`cryptsetup
   close x || true` is over-masked); a heredoc's body is cut from the line
   before it is recorded, so it needs no mask.
9. **`sudo sh -c 'echo u:pw | chpasswd'`** is recorded as `echo
   ‹redacted›`: the privileged record names the first command of the
   script (WP-129 decision 6); the secret is masked either way.

## Not done / open

- `Cookie: "a=b"` (a quoted cookie value) stays unmasked, as on the base;
  not in the WP.
- `openssl passwd PW`, `wpa_passphrase SSID PW` and similar positional
  secrets are not in the hook list (not named by the WP or WP-129).
- 0.2.0 is tagged only with this WP merged (E17): the operator's call.
- No live check on the test host: redaction and the hook are covered end
  to end in temp homes.

## Guard

The guard hook blocked one read-only `grep` whose command text named
sudo's stdin option (looking for the guide's German paragraph). Not
reworded: the files were read with the Read tool, and every privileged
command line lives in test files written with Write/Edit; the guide edits
ran from a script file.

## How it was verified

- **`flock /tmp/seldon-check.lock just check` on f2c8f52e: exit 0**
  (`check: ok`; log `engine/target/check-wp140-r1.log`): 90 test
  binaries, 2278 tests passed, 0 failed; install 209/0, bar-view 194/0,
  deploy-test-host, qmllint, docs-check and plugin-test ok. The commit
  after it adds only this handover.
- **`flock /tmp/seldon-check.lock just check-perf` on f2c8f52e: exit 0**
  (log `engine/target/perf-wp140-r1.log`), every budget on the first
  attempt. Redaction, new rows: key mentions 64 KB 0.41 ms, nmcli
  properties 64 KB 0.45 ms, quoted headers 64 KB 0.51 ms (budget 2 ms);
  128 KB with many masked values: private keys 2.94 ms, nmcli secrets
  4.97 ms, quoted header values 2.95 ms (budget 20 ms). Old rows:
  quoted line ends 64 KB 0.95 ms, two option kinds 128 KB 10.5 ms,
  password and token 128 KB 4.97 ms (budget 10 ms). Hook, 10 000 lines:
  not recorded 0.78 ms, recorded 1.56 ms, curl with a marker 3.01 ms;
  900 lines with the rebuild: 0.75 / 3.03 / 4.43 / 4.36 ms (budget
  5 ms); unrelated session 0.71–0.89 ms (budget 1 ms). Index ×10
  5.6 ms, ×150 65.6 ms; status 49.9 ms.
- **Mutants:** `work/active/WP-140/mutants.py` (own target
  `engine/target/mutants-wp140`, run from a copy of the tree so the
  worktree stayed untouched): **48/48 killed** — 6 PEM, 7 code-point
  ranges, 7 header/JSON, 2 marker-only, 2 vault import, 17 hook-local
  (each program and option, the wiring, `<(`, value skipping, long
  prefixes), 7 nmcli. The first run left one alive (`nmcli` scanning on
  only for `psk`: a `next` compiled once stays for the process); fixed
  with fresh-rule rows in `redact::tests::option_rules_scan_on_only_with_their_literals`
  (f2c8f52e), then that mutant was killed.
- `git diff ac5bc9f9..HEAD`: no added home path but the `/home/<user>`,
  `/home/user` and `/home/alice` placeholders; no user or host name.
- No schema, fixture or plugin change; the index fixture is unchanged
  (its texts hold none of the new code points).

## Round 2

Brief: the orchestrator's round-2 brief from the Opus stage-1 review
(SEND BACK for B1 and B2, plus N1–N4, more programs and code points).
`next` had not moved (ac5bc9f9), so nothing was merged.

### Fixed

- **B1 — a key file the line writes.** `secret_args::fed` (new) counts a
  file that any command of the line writes as feeding it, beside `|`,
  `<<<` and `<(`: `printf PW > k; sudo cryptsetup open … -d k` is
  `cryptsetup ‹redacted›`. A write to `/dev/null`, `/dev/stdout` or
  `/dev/stderr` feeds nothing, so `cryptsetup open … 2>/dev/null` stays
  whole (hooks row and unit rows, both ways).
- **B2 — `passwd` fed from the line.** New `Feed::OptionsOrFed`:
  `passwd` counts through `-s`/`--stdin` or when the line feeds it
  (`printf 'PW\nPW' | sudo passwd alice`, `sudo passwd alice <<< $'…'`).
  `sudo passwd -S alice` stays whole, also with `2>/dev/null`. New unit
  rows `passwd -s`/`--stdin alice < pw.txt` keep the options tested on a
  line that feeds nothing (the first mutant run showed both surviving
  without them).
- **N1:** `long: &["--password"]` for `useradd`, `usermod`, `groupadd`,
  `groupmod`: `--passw=…`, `--pass x` and `--password $(openssl passwd
  -6 PW)` are `<program> ‹redacted›`; `usermod --login bob` stays.
- **More programs:** `openssl passwd` (a new `subcommand` field; `openssl
  rand`, `openssl req … -passin env:PW` stay) and `wpa_passphrase`, both
  always.
- **N2:** R1 — two `CLEAR` rows, a PUBLIC block in a text that says
  "private key" and an `RSA PUBLIC KEY` block. R3 —
  `collectors::plugins::tests::a_subject_drops_every_format_character_before_the_redaction`
  (a `token=` split by each code point in a commit subject). R4 —
  `index.rs` `the_reference_drops_the_engines_format_characters`: the
  sets `DIRECTION_OR_FORMAT` and `BAD_PATH` (without the controls) of
  `scripts/validate-fixtures.py` equal `import::is_direction_or_format`
  over every code point (python `-I -B`, so no `__pycache__`; skipped
  without `python3`, like the reference test).
- **N3:** SPEC §7 now says exactly when two passes give the same text
  with user patterns: a gap at the end of a value (followed by white
  space, a line end or the end) merges nothing; text glued after the
  masked gap joins the value on the second pass and is masked too
  (`--password x;tail` with a pattern for `;` → `‹redacted›‹redacted›tail`
  → `‹redacted›`; more, never less); a pattern across a marker's edge is
  applied as written. Pinned in `a_match_of_markers_only_is_left_as_it_is`.
- **N4 (cheap, done):** a quoted header value takes the text glued after
  its closing quote up to white space, a quote, a backslash, `,`, `;`, a
  closing bracket or a marker (`Authorization: "Bearer "SECRET next` →
  `Authorization: ‹redacted› next`), and a Python string prefix (`f`,
  `r`, `b`, `u`, two of them: `{'Authorization': f'Bearer {t}', …}`,
  `x-api-key: rb'…'`). JSON keeps its `,` and `}`.
- **More code points:** U+0600–U+0605, U+1BCA0–U+1BCA3, U+1D173–U+1D17A
  in Rust and Python, with unit, index, commit-subject and reference-set
  tests.
- **Docs:** SPEC-ENGINE §3/§6/§7/§8, the ADR-0038 and ADR-0039 amendment
  notes, CONTRACT rule 9, CHANGELOG, guide 04 en/de (de source line
  set), TESTING.

### Accepted as documented

A BEGIN with no END masks the rest of the text, the rest of a vault file
on import included (over-masks rather than leaks; SPEC §7).

### How it was verified

- **`flock /tmp/seldon-check.lock just check` on b0c20275: exit 0**
  (`check: ok`; log `engine/target/check-wp140-r2b.log`): 90 test
  binaries, 2282 passed, 0 failed. (A first round-2 run, queued on
  022962e2, was stopped by its own PID before it got the lock, because
  b0c20275 followed; log `check-wp140-r2.log`, exit 143.)
- **`flock /tmp/seldon-check.lock just check-perf` on b0c20275: exit 0**
  (log `engine/target/perf-wp140-r2.log`), every budget on the first
  attempt: redaction 64 KB without a masked value 0.40–0.67 ms (budget
  2 ms); 128 KB with many: private keys 2.81 ms, nmcli secrets 4.86 ms,
  quoted header values 2.90 ms (20 ms), two option kinds 10.3 ms (20 ms),
  password and token 4.87 ms (10 ms). Hook, 10 000 lines: 0.79 / 1.70 /
  3.01 / 2.67 ms; 900 lines with the rebuild: 0.75 / 3.08 / 4.49 /
  4.24 ms (5 ms); unrelated session 0.71–0.88 ms (1 ms). Index ×10
  5.5 ms, ×150 65.5 ms; status 49.0 ms.
- **Mutants:** `work/active/WP-140/mutants.py`, now 65 (round 2 adds the
  written file, `/dev/null`, `passwd` fed, the four `--password` longs,
  `openssl passwd` and its subcommand, `wpa_passphrase`, the three new
  ranges in Rust and one in Python, the string prefix, the glued tail and
  its `,` stop; the runner also runs the reference-set test of `--test
  index`). Run from a copy of the tree with its own target
  `engine/target/mutants-wp140`: 63/65 killed at 022962e2; the two
  survivors (`passwd -s`, `passwd --stdin`) were killed after the unit
  rows above (b0c20275): **65/65 killed**.
- `git diff 3b5592ab..HEAD`: no added home path, user or host name.

## Round 3

Brief: the orchestrator's round 3 from the Fable stage 2 (everything held
but one input; two items).

### Merge of next

`next` had moved to d30aca6d (the merge of main: 0.1.4, WP-117/118/130;
work files WP-132..134 and WP-157, the redaction follow-ups of this
stage 2). Merged as 7df3d8e7 without a conflict; docs-check ok.

### Done

1. **HTTPie's and xh's quoted headers.** `redact::header` has a third
   alternative, `({name}:)` + `HEADER_GLUED` (the brief's pattern): a
   quote right after the colon of a bare name opens a value when no white
   space follows it; the text glued after its closing quote goes with it,
   as for `HEADER_QUOTED`. `KEEP_EITHER` keeps group 1, 2 or 3. Rows:
   `http POST … Authorization:'Bearer tokABC123' a=1`, `https …
   Authorization:"Bearer tokDEF456" -v`, `xh GET … X-Api-Key:'keyABC123xyz'
   Accept:json` masked; `curl -H 'Authorization:' -H 'X: y' …`, `grep -i
   'authorization:' f 'x'` and `grep -i "x-api-key:" f "y"` unchanged
   (`CLEAR`). Bench: a 128 KB line of glued header values, 5.29 ms
   (budget 20 ms); the quoted-header rows below. SPEC §7: the header
   sentence and the "Not masked" clause (now only a quote followed by
   white space, `Authorization:' Bearer x'`).
2. **Format characters on a hook command line.** `hook::bash_records`
   drops `import::is_direction_or_format` characters from the line before
   `parse_shell` and the record (the subject, `meta.command` and `detail`
   come from that line). Test
   `privileged::format_characters_hide_no_secret_on_a_command_line`:
   `tok<U+200B>en=hunter2abc` → `token=‹redacted›`; `'Autho<U+200B>rization:
   <U+00AD>Bearer …'` → `'Authorization: ‹redacted›'`; U+2060 inside a URL
   dropped; none of the three in the ledger; a `seldon log` note keeps its
   U+200D. SPEC §7 has one sentence on where the set is dropped (a hook's
   command line, the index texts and `source`, plugin commit subjects) and
   where it stays (notes, case and decision files, the journal, an event's
   other texts), with what that means.

A first draft of the test used `Authorization:' Bearer …'` as its second
line; that is the documented "not masked" form (a quote followed by white
space closes a shell word), so the line was changed to the quoted-word
form an agent writes.

### How it was verified

- **`flock /tmp/seldon-check.lock just check` on a129df40: exit 0**
  (`check: ok`; log `engine/target/check-wp140-r3.log`): 90 test
  binaries, 2290 passed, 0 failed.
- **`flock /tmp/seldon-check.lock just check-perf` on a129df40: exit 0**
  (log `engine/target/perf-wp140-r3.log`), every budget on the first
  attempt. Quoted headers 64 KB 0.49 ms (2 ms); glued header values
  128 KB 5.29 ms, quoted header values 3.04 ms, private keys 2.80 ms,
  nmcli secrets 4.74 ms (20 ms); two option kinds 10.1 ms (20 ms),
  password and token 4.89 ms (10 ms). Hook, 10 000 lines: 0.78 / 1.61 /
  3.03 / 2.63 ms; 900 lines: 0.80 / 3.06 / 4.49 / 4.30 ms (5 ms);
  unrelated session 0.73–0.91 ms (1 ms). Index ×10 5.5 ms, ×150
  64.9 ms; status 48.5 ms.
- **Mutants:** 68 (round 3 adds: the glued alternative dropped, a glued
  value that may start with white space, the hook keeping format
  characters; the two N4 tail mutants now target `HEADER_QUOTED`, as the
  tail stands in both constants). Run from a copy with its own target
  `engine/target/mutants-wp140`: **68/68 killed** (log
  `engine/target/mutants-wp140-r3.log`).
- This round's own diff: no added home path, user or host name.
