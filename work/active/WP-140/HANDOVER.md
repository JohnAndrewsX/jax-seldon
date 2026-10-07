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
