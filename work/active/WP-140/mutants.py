#!/usr/bin/env python3
"""WP-140 manual mutants: each one undoes one rule of the WP in the engine;
the unit tests, `--test redaction` or `--test hooks privileged` must fail
for every one. Names given as arguments run only the mutants whose name
contains one of them."""
import os
import subprocess
import sys
from pathlib import Path

# the checkout this script lives in: work/active/WP-140/mutants.py
WT = str(Path(__file__).resolve().parents[3])
# a target dir of its own: a mutated build must never reach another run
TARGET = f"{WT}/engine/target/mutants-wp140"
REDACT = "engine/src/redact.rs"
IMPORT = "engine/src/import/mod.rs"
ARGS = "engine/src/commands/hook/secret_args.rs"
HOOK = "engine/src/commands/hook.rs"


def plain(a, b, count=1):
    def apply(src):
        assert src.count(a) == count, (a, src.count(a))
        return src.replace(a, b)
    return apply


def entry(name, field, value):
    """Set `field` of the SECRET_ARGS entry `name` to `value`."""
    def apply(src):
        i = src.index(f'name: "{name}",')
        j = src.index("},", i)
        block = src[i:j]
        k = block.index(f"{field}: ")
        end = block.index(",\n", k)
        return src[:i] + block[:k] + f"{field}: {value}" + block[end:] + src[j:]
    return apply


MUTANTS = [
    # 1. private-key
    ("pem: no END, no mask to the end", REDACT, plain(r"-----END {PEM_LABEL}-----|\z)|", r"-----END {PEM_LABEL}-----)|")),
    ("pem: no lone END", REDACT, plain(r"|[A-Za-z0-9+/=\\ \t\r\n]+(?P<tail>-----END {PEM_LABEL}-----)", "")),
    ("pem: body on one line only", REDACT, plain(r"-----)(?s:.*?)(?P<end>", r"-----)(?:.*?)(?P<end>")),
    ("pem: no PGP block", REDACT, plain(r'PRIVATE KEY(?: BLOCK)?";', r'PRIVATE KEY";')),
    ("pem: label words only RSA", REDACT, plain(r'const PEM_LABEL: &str = r"(?:[A-Z0-9]+ )*', r'const PEM_LABEL: &str = r"(?:RSA )?')),
    ("pem: trigger never", REDACT, plain('"private-key" => &["private key"],', '"private-key" => &["private  key"],')),
    # 2. the invisible set
    ("set: no U+00AD", IMPORT, plain("        '\\u{00AD}'\n            | '\\u{061C}'", "        '\\u{061C}'")),
    ("set: no U+061C", IMPORT, plain("            | '\\u{061C}'\n", "")),
    ("set: no U+180E", IMPORT, plain("            | '\\u{180E}'\n", "")),
    ("set: no U+2061-U+2064", IMPORT, plain("'\\u{2060}'..='\\u{2064}'", "'\\u{2060}'")),
    ("set: no U+206A-U+206F", IMPORT, plain("'\\u{2066}'..='\\u{206F}'", "'\\u{2066}'..='\\u{2069}'")),
    ("set: no U+FFF9-U+FFFB", IMPORT, plain("            | '\\u{FFF9}'..='\\u{FFFB}'\n", "")),
    ("set: no tags", IMPORT, plain("            | '\\u{E0000}'..='\\u{E007F}'\n", "")),
    # 3. quoted header values
    ("header: no \"…\"", REDACT, plain(r'''    r#"(?:"(?:[^"\\\r\n]|\\[^\r\n])*"|\\"''', r'''    r#"(?:\\"''')),
    ("header: no \\\"…\\\"", REDACT, plain(r'''|\\"(?:[^"\\\r\n]|\\[^"\r\n])*\\"|'[^'\r\n]*')"#;''', r'''|'[^'\r\n]*')"#;''')),
    ("header: no '…'", REDACT, plain(r'''|'[^'\r\n]*')"#;''', r''')"#;''')),
    ("header: no quoted name", REDACT, plain('&header("authorization", true)', '&header("authorization", false)')),
    ("header: quoted value right after the colon", REDACT, plain(r'r"(?i)({name}(?:{quoted_name}:\s+))', r'r"(?i)({name}(?:{quoted_name}:\s*))')),
    ("header: bare value may start at a marker", REDACT, plain(r'''const HEADER_BARE: &str = r#"[^'"\s‹]''', r'''const HEADER_BARE: &str = r#"[^'"\s]''')),
    ("json-secret: no api-key", REDACT, plain("|token|api[_-]?key)", "|token|api_?key)")),
    # 4. marker-only matches
    ("markers: masked again", REDACT, plain("            && !self.masks_only_markers(found, text, markers)\n", "")),
    ("markers: white space between counts", REDACT, plain('                .all(char::is_whitespace)', '                .all(|_| false)')),
    # the vault import
    ("vault: lines one by one", IMPORT, plain("let whole = self.redactor.redact_keeping_lines(text);", "let whole: String = text.split_inclusive('\\n').map(|l| self.redactor.redact(l)).collect();")),
    ("vault: a match counts on its first line only", REDACT, plain("..=line_of(end.max(start + 1) - 1)]", "..=line_of(start)]")),
    # 5. hook-local secrets
    ("args: wiring", HOOK, plain("        || secret_args::secret_on_the_line(&line)\n", "")),
    ("args: chpasswd", ARGS, entry("chpasswd", "feed", "Feed::Options")),
    ("args: chgpasswd", ARGS, entry("chgpasswd", "feed", "Feed::Options")),
    ("args: htpasswd -b", ARGS, entry("htpasswd", "short", '"i"')),
    ("args: htpasswd -i", ARGS, entry("htpasswd", "short", '"b"')),
    ("args: smbpasswd -s", ARGS, entry("smbpasswd", "short", '"w"')),
    ("args: smbpasswd -w", ARGS, entry("smbpasswd", "short", '"s"')),
    ("args: passwd -s", ARGS, entry("passwd", "short", '""')),
    ("args: passwd --stdin", ARGS, entry("passwd", "long", "&[]")),
    ("args: useradd -p", ARGS, entry("useradd", "short", '""')),
    ("args: usermod -p", ARGS, entry("usermod", "short", '""')),
    ("args: groupadd -p", ARGS, entry("groupadd", "short", '""')),
    ("args: groupmod -p", ARGS, entry("groupmod", "short", '""')),
    ("args: cryptsetup", ARGS, entry("cryptsetup", "feed", "Feed::Options")),
    ("args: no process substitution", ARGS, plain('["|", "<<<", "<("]', '["|", "<<<"]')),
    ("args: a value word is read as options", ARGS, plain("                        words.next();\n", "")),
    ("args: long options exact", ARGS, plain("self.long.iter().any(|l| l[2..].starts_with(name))", "self.long.iter().any(|l| l[2..] == *name)")),
    # nmcli-secret
    ("nmcli: no keyword password", REDACT, plain(r"\s[+-]?(?:password|(?:[a-z0-9-]+\.)+", r"\s[+-]?(?:(?:[a-z0-9-]+\.)+")),
    ("nmcli: no psk", REDACT, plain("password-raw|psk|secrets", "password-raw|secrets")),
    ("nmcli: no secrets", REDACT, plain("|secrets|wep-key", "|wep-key")),
    ("nmcli: no wep keys", REDACT, plain("|wep-key[0-3]|", "|")),
    ("nmcli: no pin", REDACT, plain("|preshared-key|pin))", "|preshared-key))")),
    ("nmcli: no + before a property", REDACT, plain(r"\s[+-]?(?:password|", r"\s(?:password|")),
    ("nmcli: scans on only for psk", REDACT, plain('"nmcli-secret" => &["pass", "psk", "secret", "key", "pin"],', '"nmcli-secret" => &["psk"],')),
]

# `--check`: only apply each mutant, run nothing
check = "--check" in sys.argv
only = [a for a in sys.argv[1:] if a != "--check"]
env = dict(os.environ, CARGO_TARGET_DIR=TARGET)
cargo = ["cargo", "test", "--manifest-path", "engine/Cargo.toml", "--no-fail-fast"]
runs = [
    cargo + ["--lib", "--", "--test-threads=4"],
    cargo + ["--test", "redaction", "--", "--test-threads=4"],
    cargo + ["--test", "hooks", "--", "privileged", "--test-threads=4"],
]
results = []
for name, file, mutate in MUTANTS:
    if only and not any(o in name for o in only):
        continue
    path = os.path.join(WT, file)
    orig = open(path).read()
    try:
        mutated = mutate(orig)
    except (AssertionError, ValueError) as e:
        results.append((name, f"NOT APPLIED {e}"))
        continue
    if mutated == orig:
        results.append((name, "NOT APPLIED (no change)"))
        continue
    if check:
        results.append((name, "killed (not run)"))
        continue
    try:
        open(path, "w").write(mutated)
        failed = []
        for cmd in runs:
            r = subprocess.run(cmd, cwd=WT, env=env, capture_output=True, text=True)
            if "error[E" in r.stderr or "error: could not compile" in r.stderr:
                failed.append("does not compile")
                break
            if r.returncode != 0:
                failed.append(cmd[6] if cmd[5] == "--test" else "lib")
    finally:
        open(path, "w").write(orig)
    results.append((name, f"killed ({', '.join(failed)})" if failed else "SURVIVED"))
    print(f"{name}: {results[-1][1]}", flush=True)

print()
killed = sum(r.startswith("killed") for _, r in results)
print(f"{killed}/{len(results)} killed")
for name, r in results:
    if not r.startswith("killed"):
        print(f"  {name}: {r}")
