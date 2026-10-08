# WP-140 — Plan

Branch `wp/140-redaction-more` from `next` (ac5bc9f9: WP-128 CRLF
redaction, WP-129 privileged hook records); merges into `next`.

## Items

1. **`private-key`** (new built-in rule, `redact.rs`): `-----BEGIN
   … PRIVATE KEY-----` (any label words: none, `RSA`, `EC`, `DSA`,
   `OPENSSH`, `ENCRYPTED`, `ANY`; also PGP's `PRIVATE KEY BLOCK`) up to
   the matching `-----END … PRIVATE KEY-----` line, or to the end of the
   text when no END follows (a block cut by a clip). The BEGIN line stays,
   the rest becomes `‹redacted›`; a lone END line (the block's start cut
   off) masks the base64 run before it. Runs first, before every other
   rule, so no later rule cuts the body into pieces. LF and CRLF;
   `redact`, `redact_keeping_lines`, `matching_rules`. Trigger
   `private key`. The vault import (`import::Scrubber::text`) redacts
   line by line, so a block's body lines would pass: it redacts the
   whole text keeping lines first (as `import task` does since WP-102),
   then counts per line; a changed line without a rule of its own counts
   under the rules of its run of changed lines.
2. **The extended invisible set**: U+00AD, U+061C, U+180E, U+2061–U+2064,
   U+206A–U+206F, U+FFF9–U+FFFB, U+E0000–U+E007F join
   `import::is_direction_or_format` (paths, index texts). The commit
   subject cleaner in `collectors/plugins.rs` keeps a copy of the same set;
   it calls the shared function instead. `scripts/validate-fixtures.py`
   (the reference derive) gets the same set. A test per code point.
3. **Quoted header values**: `authorization-header` and `secret-header`
   take a value in `"…"` (with `\"` inside), `\"…\"` (inside a shell
   string) or `'…'`, and a header name followed by a quote
   (`"Authorization": "Bearer x"`, JSON and Python dicts).
4. **Merged markers**: a built-in match whose masked part (the match
   without the groups its replacement keeps) is only markers (and white
   space) is left as it is. `matching_rules` follows, so the import counts
   no rule on a line that only already holds markers.
5. **Plain-argument secrets** (hook-local, the `-S` style: the line's
   records become `<program> ‹redacted›`): `chpasswd`/`chgpasswd`;
   `htpasswd -b`/`-i`; `smbpasswd -s`/`-w`; `passwd -s`/`--stdin`;
   `useradd`/`usermod`/`groupadd`/`groupmod -p`; `cryptsetup` on a line
   that can feed it a key (`|`, `<<<`, `<(`); `nmcli` with a secret
   property or keyword. Plus a cheap §7 rule `nmcli-secret` (the value
   after `password`, `psk`, `….psk`, `….password`, `….secrets`, …), so
   a note gets it too.

## Order of work

Item 2 → 1 → 3 → 4 → 5 (hook) → 5 (§7 nmcli) → docs (SPEC §7/§8,
ADR-0038 amendment note, ADR-0039 amendment note, CONTRACT, CHANGELOG,
TESTING) → mutants (`work/active/WP-140/mutants.py`, own target) →
`just check`, `just check-perf` → HANDOVER.

## Tests

- `engine/tests/redaction.rs`: TABLE rows for `private-key`,
  `nmcli-secret`, quoted headers; CLEAR rows (public keys, certificates,
  `key-mgmt wpa-psk`); a PEM test (LF, CRLF, clipped, END only, keeping
  lines, second pass, the vault import); a merged-markers test (WP-128 decision 5 cases);
  the timing test gets a PEM row.
- `import/mod.rs` unit test: each code point of the set.
- `index` texts: one hidden-split token per range.
- `engine/tests/hooks.rs` (`privileged::`): each hook-local form, no
  secret in ledger, `detail`, `meta.command`, `index.json`; negatives
  (`usermod -aG`, `passwd -S`, `htpasswd -D`, `cryptsetup open` alone).
- Every privileged command line lives in test files (Write/Edit), never
  in Bash command text (guard hook).
