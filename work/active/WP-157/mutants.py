#!/usr/bin/env python3
"""WP-157 manual mutants: each one undoes one rule of the WP in the engine;
the unit tests, `--test redaction` or `--test hooks privileged` must fail
for every one.

Usage: mutants.py COPY [NAME...]. COPY is a copy of the worktree the
mutants are applied to, one at a time, so the worktree itself is never
changed; its target dir is `COPY/engine/target`. Names given after it run
only the mutants whose name contains one of them; `--check` only applies
each mutant and runs nothing. On SIGTERM or SIGINT the source of the
running mutant is put back and its cargo process group is killed.
"""
import os
import signal
import subprocess
import sys

REDACT = "engine/src/redact.rs"
ARGS = "engine/src/commands/hook/secret_args.rs"
PKG = "engine/src/pkgcmd.rs"


def plain(*pairs):
    """Replace each `a` (found exactly once) with its `b`."""
    def apply(src):
        for a, b in zip(pairs[::2], pairs[1::2]):
            assert src.count(a) == 1, (a, src.count(a))
            src = src.replace(a, b)
        return src
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
    # json-secret: Python dicts
    ("json: no single-quoted key", REDACT, plain("""|'{SECRET_KEY}')""", ")")),
    ("json: no single-quoted value", REDACT, plain(r"""|'(?:[^'\\\n]|\\.)*')|""", ")|")),
    ("json: no string prefix", REDACT, plain("(?P<v>(?:[rbuf]{{1,2}})?(?:", "(?P<v>(?:")),
    ("json: no single-quote triggers", REDACT, plain("""            "password'",\n            "passwd'",\n            "passphrase'",\n            "secret'",\n            "token'",\n            "api_key'",\n            "api-key'",\n            "apikey'",\n""", "")),
    ("json: '' is a value", REDACT, plain("""        .or_else(|| value.strip_prefix('\\'').and_then(|v| v.strip_suffix('\\'')))\n        .unwrap_or(value);\n    !inner.is_empty()""", """        .unwrap_or(value);\n    !inner.is_empty()""")),
    ("json: a prefix is a value", REDACT, plain("    let value = value.trim_start_matches(['r', 'R', 'b', 'B', 'u', 'U', 'f', 'F']);\n", "")),
    # nmcli-secret: fields to show
    ("nmcli: a field is a property to set", REDACT, plain('unless: &["field"],', "unless: &[],")),
    ("nmcli: only -g names a field", REDACT, plain(r"(?P<field>-{{1,2}}[fg][a-z-]*", r"(?P<field>-{{1,2}}[g][a-z-]*")),
    # private-key-base64
    ("b64: trigger never", REDACT, plain('"private-key-base64" => &["LS0tLS1", "CRUdJTi", "LS0tQ", "kVHSU4", "LS0tLU", "JFR0lO"],', '"private-key-base64" => &["never"],')),
    ("b64: whole markers as triggers (round 2)", REDACT, plain('"private-key-base64" => &["LS0tLS1", "CRUdJTi", "LS0tQ", "kVHSU4", "LS0tLU", "JFR0lO"],', '"private-key-base64" => &["LS0tLS1CRUdJTi", "LS0tQkVHSU4", "LS0tLUJFR0lO"],')),
    ("b64: one alignment only", REDACT, plain('const PEM_BASE64_MARKERS: [&str; 3] = ["LS0tLS1CRUdJTi", "LS0tQkVHSU4", "LS0tLUJFR0lO"];', 'const PEM_BASE64_MARKERS: [&str; 3] = ["LS0tLS1CRUdJTi", "LS0tLS1CRUdJTi", "LS0tLS1CRUdJTi"];')),
    ("b64: not from the token's start", REDACT, plain(r"[ \t]*)*[A-Za-z0-9+/]*(?:{})", r"[ \t]*)*(?:{})")),
    ("b64: any short token", REDACT, plain(r"(?:{})(?:[A-Za-z0-9+/=]{{40,}}|", r"(?:{})(?:[A-Za-z0-9+/=]{{0,}}|")),
    ("b64: no wrapped lines", REDACT, plain(r"(?:\r?\n[ \t]*[A-Za-z0-9+/]+={{0,2}}(?mR:$)|\\n", r"(?:\\n")),
    ("b64: no indented lines", REDACT, plain(r"(?:\r?\n[ \t]*[A-Za-z0-9+/]+={{0,2}}(?mR:$)|", r"(?:\r?\n[A-Za-z0-9+/]+={{0,2}}(?mR:$)|")),
    ("b64: no literal \\n", REDACT, plain(r"(?mR:$)|\\n[A-Za-z0-9+/]+={{0,2}})*", r"(?mR:$))*")),
    ("b64: a line taken up to any character", REDACT, plain(r"[A-Za-z0-9+/]+={{0,2}}(?mR:$)|\\n", r"[A-Za-z0-9+/]+={{0,2}}|\\n")),
    ("b64: no short first line (round 2)", REDACT, plain(r"|[A-Za-z0-9+/=]*\r?\n[ \t]*[A-Za-z0-9+/]{{40,}}={{0,2}}(?mR:$))(?:", r")(?:")),
    ("b64: a marker never cut by a wrap (round 2)", REDACT, plain(r'chars.join(r"(?:\r?\n[ \t]*)?")', 'chars.join("")')),
    ("b64: no lines before the marker's (round 2)", REDACT, plain(r'r"(?:(?m:^)[A-Za-z0-9+/]{{40,}}\r?\n[ \t]*)*[A-Za-z0-9+/]*', r'r"[A-Za-z0-9+/]*')),
    ("b64: runs before email, not second (round 2)", REDACT, plain(
        '    "private-key-base64",\n', '',
        '    "email",\n];', '    "private-key-base64",\n    "email",\n];',
        '        rule("private-key-base64", &pem_base64(), WHOLE),\n', '',
        '        Rule {\n            unless: &["url", "unit"],', '        rule("private-key-base64", &pem_base64(), WHOLE),\n        Rule {\n            unless: &["url", "unit"],')),
    # mount-password
    ("mount: any option list", REDACT, plain("check: Some(mount_password),", "check: Some(|_| true),")),
    ("mount: no pwd=", REDACT, plain('named(&[&b"pass="[..], b"pwd="])', 'named(&[&b"pass="[..]])')),
    ("mount: case-sensitive", REDACT, plain("option[..name.len()].eq_ignore_ascii_case(name)", "&option[..name.len()] == *name")),
    ("mount: quotes kept", REDACT, plain("let option = option.trim_start_matches(['\\'', '\"']).as_bytes();", "let option = option.as_bytes();")),
    ("mount: no --options", REDACT, plain(r'&format!(r"\s(?:-[A-Za-z]*o{GAP}*|--options(?:=|{GAP}+))"),', r'&format!(r"\s(?:-[A-Za-z]*o{GAP}*)"),')),
    ("mount: no cluster (round 2)", REDACT, plain(r'&format!(r"\s(?:-[A-Za-z]*o{GAP}*|', r'&format!(r"\s(?:-o{GAP}*|')),
    ("mount: scans on only for --options", REDACT, plain('"mount-password" => &["-"],', '"mount-password" => &["--options"],')),
    ("mount: no pwd= trigger", REDACT, plain('            "mount+pwd=",\n', '')),
    ("mount: no user= triggers (round 2)", REDACT, plain('            "mount+user=",\n            "mount+username=",\n', '')),
    ("mount: a user without % (round 2)", REDACT, plain("(named(&[&b\"user=\"[..], b\"username=\"]) && option.contains(&b'%'))", "named(&[&b\"user=\"[..], b\"username=\"])")),
    ("mount: no user%password (round 2)", REDACT, plain("        named(&[&b\"pass=\"[..], b\"pwd=\"])\n            || (named(&[&b\"user=\"[..], b\"username=\"]) && option.contains(&b'%'))\n", "        named(&[&b\"pass=\"[..], b\"pwd=\"])\n")),
    # process substitution: the word before it
    ("args: substitution after =", ARGS, plain("c.is_whitespace() || matches!(c, '=' | ':' | '\\\\' | '\\'' | '\"')", "c.is_whitespace() || matches!(c, ':' | '\\\\' | '\\'' | '\"')")),
    ("args: substitution after : (round 2)", ARGS, plain("c.is_whitespace() || matches!(c, '=' | ':' | '\\\\' | '\\'' | '\"')", "c.is_whitespace() || matches!(c, '=' | '\\\\' | '\\'' | '\"')")),
    ("args: substitution after a continued line", ARGS, plain(
        "c.is_whitespace() || matches!(c, '\\\\' | '\\'' | '\"')", "c.is_whitespace() || matches!(c, '\\'' | '\"')",
        "c.is_whitespace() || matches!(c, '=' | ':' | '\\\\' | '\\'' | '\"')", "c.is_whitespace() || matches!(c, '=' | ':' | '\\'' | '\"')")),
    ("args: substitution after a quoted option", ARGS, plain(
        "c.is_whitespace() || matches!(c, '\\\\' | '\\'' | '\"')", "c.is_whitespace() || matches!(c, '\\\\')",
        "c.is_whitespace() || matches!(c, '=' | ':' | '\\\\' | '\\'' | '\"')", "c.is_whitespace() || matches!(c, '=' | ':' | '\\\\')")),
    ("args: substitution case-sensitive (round 2)", ARGS, plain("        let word = word.to_ascii_lowercase();\n", "")),
    ("args: no pass phrase source (round 2)", ARGS, plain("names(word) || (gap.ends_with(':') && names(word_before(rest).0))", "names(word) || rest.is_empty()")),
    ("args: substitution after key", ARGS, plain('"token", "key", "cred"', '"token", "no-key", "cred"')),
    ("args: substitution after pw (round 2)", ARGS, plain('["pass", "pw", "secret"', '["pass", "zz", "secret"')),
    ("args: substitution after secret (round 2)", ARGS, plain('"pw", "secret", "token"', '"pw", "zzzz", "token"')),
    ("args: substitution after cred (round 2)", ARGS, plain('"key", "cred", "auth"]', '"key", "zzzz", "auth"]')),
    ("args: substitution after auth (round 2)", ARGS, plain('"cred", "auth"]', '"cred", "zzzz"]')),
    # fed: a command after a pipe, here-string or process substitution
    ("fed: || pipes", PKG, plain("lexer.fed |= c == '|' && self.at(i + 1) != Some('|');", "lexer.fed |= c == '|';")),
    ("fed: no pipe", PKG, plain("                    lexer.fed |= c == '|' && self.at(i + 1) != Some('|');\n", "")),
    ("fed: here-string feeds nothing", PKG, plain("                        lexer.pending = Some(Pending::Read);\n                        lexer.current.fed = true;\n                        lexer.fed = true;\n", "                        lexer.pending = Some(Pending::Read);\n")),
    ("fed: here-string feeds no later command (round 2)", PKG, plain("                        lexer.pending = Some(Pending::Read);\n                        lexer.current.fed = true;\n                        lexer.fed = true;\n", "                        lexer.pending = Some(Pending::Read);\n                        lexer.current.fed = true;\n")),
    ("fed: substitution feeds nothing", PKG, plain("                        // later one\n                        lexer.current.fed = true;\n                        lexer.fed = true;\n", "                        // later one\n")),
    ("fed: substitution feeds no later command (round 2)", PKG, plain("                        // later one\n                        lexer.current.fed = true;\n                        lexer.fed = true;\n", "                        // later one\n                        lexer.current.fed = true;\n")),
    ("fed: taken by the next command (round 2, F1)", PKG, plain("            segment.fed |= self.fed;\n", "            segment.fed |= std::mem::take(&mut self.fed);\n")),
    ("fed: sh -c not fed by its caller", PKG, plain("fed: inner.fed || segment.fed,", "fed: inner.fed,")),
    ("fed: every command", ARGS, plain("    segment.fed || writes\n", "    segment.fed || writes || !segment.words.is_empty()\n")),
    # ssh-keygen
    ("args: ssh-keygen -N", ARGS, entry("ssh-keygen", "short_secret", '"P"')),
    ("args: ssh-keygen -P", ARGS, entry("ssh-keygen", "short_secret", '"N"')),
    ("args: an empty pass phrase counts", ARGS, plain("if secret && value.is_some_and(|v| !v.is_empty()) {", "if secret && value.is_some() {")),
    ("args: ssh-keygen values read as options", ARGS, entry("ssh-keygen", "short_value", '""')),
    # mkpasswd
    ("args: mkpasswd operand", ARGS, entry("mkpasswd", "positional", "false")),
    ("args: mkpasswd long values", ARGS, entry("mkpasswd", "long_value", "&[]")),
    ("args: mkpasswd short values", ARGS, entry("mkpasswd", "short_value", '""')),
    ("args: mkpasswd -s", ARGS, entry("mkpasswd", "short", '""')),
    ("args: mkpasswd --stdin", ARGS, entry("mkpasswd", "long", "&[]")),
    ("args: mkpasswd not fed", ARGS, entry("mkpasswd", "feed", "Feed::Options")),
    ("args: no operand after --", ARGS, plain("return self.positional && words.next().is_some();", "return false;")),
    # process substitution as a secret
    ("args: no substitution check", ARGS, plain("    secret_substitution(&line.text)\n        || ", "    ")),
    # xargs
    ("xargs: no wrapper", PKG, plain('        name: "xargs",', '        name: "xargz",')),
    ("xargs: optional values take the next word", PKG, plain('short_attached: "eil",', 'short_attached: "",')),
    ("xargs: long values", PKG, plain('            "--arg-file",\n            "--delimiter",\n            "--max-args",\n            "--max-procs",\n            "--max-chars",\n            "--process-slot-var",\n', "")),
    ("xargs: alone runs nothing", PKG, plain("        operands: 0,\n        alone: true,", "        operands: 0,\n        alone: false,")),
]


def main():
    args = [a for a in sys.argv[1:] if a != "--check"]
    check = "--check" in sys.argv
    if not args:
        sys.exit(__doc__)
    copy, only = os.path.abspath(args[0]), args[1:]
    env = dict(os.environ, CARGO_TARGET_DIR=os.path.join(copy, "engine/target"))
    cargo = ["cargo", "test", "--manifest-path", "engine/Cargo.toml", "--no-fail-fast"]
    runs = [
        cargo + ["--lib", "--", "--test-threads=4"],
        cargo + ["--test", "redaction", "--", "--test-threads=4"],
        cargo + ["--test", "hooks", "--", "privileged", "--test-threads=4"],
    ]
    current = {}

    def stop(signum, _frame):
        proc = current.get("proc")
        if proc and proc.poll() is None:
            os.killpg(proc.pid, signal.SIGKILL)
        if "path" in current:
            open(current["path"], "w", newline="").write(current["orig"])
        sys.exit(128 + signum)

    signal.signal(signal.SIGTERM, stop)
    signal.signal(signal.SIGINT, stop)
    results = []
    for name, file, mutate in MUTANTS:
        if only and not any(o in name for o in only):
            continue
        path = os.path.join(copy, file)
        orig = open(path, newline="").read()
        try:
            mutated = mutate(orig)
        except (AssertionError, ValueError) as e:
            results.append((name, f"NOT APPLIED {e}"))
            print(f"{name}: {results[-1][1]}", flush=True)
            continue
        if mutated == orig:
            results.append((name, "NOT APPLIED (no change)"))
            continue
        if check:
            results.append((name, "applies"))
            continue
        current.update(path=path, orig=orig)
        try:
            open(path, "w", newline="").write(mutated)
            failed = []
            for cmd in runs:
                proc = subprocess.Popen(cmd, cwd=copy, env=env, stdout=subprocess.PIPE,
                                        stderr=subprocess.PIPE, text=True, start_new_session=True)
                current["proc"] = proc
                _, err = proc.communicate()
                if "error[E" in err or "error: could not compile" in err:
                    failed.append("does not compile")
                    break
                if proc.returncode != 0:
                    failed.append(cmd[6] if cmd[5] == "--test" else "lib")
        finally:
            open(path, "w", newline="").write(orig)
            current.clear()
        results.append((name, f"killed ({', '.join(failed)})" if failed else "SURVIVED"))
        print(f"{name}: {results[-1][1]}", flush=True)
    print()
    ok = sum(r.startswith(("killed", "applies")) for _, r in results)
    print(f"{ok}/{len(results)} {'apply' if check else 'killed'}")
    for name, r in results:
        if not r.startswith(("killed", "applies")):
            print(f"  {name}: {r}")


if __name__ == "__main__":
    main()
