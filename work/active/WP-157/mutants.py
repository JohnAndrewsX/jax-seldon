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
    # private-key-base64
    ("b64: trigger never", REDACT, plain('"private-key-base64" => &["LS0tLS1CRUdJTi", "LS0tQkVHSU4", "LS0tLUJFR0lO"],', '"private-key-base64" => &["never"],')),
    ("b64: one alignment only", REDACT, plain("(?:LS0tLS1CRUdJTi|LS0tQkVHSU4|LS0tLUJFR0lO)[", "(?:LS0tLS1CRUdJTi)[")),
    ("b64: not from the token's start", REDACT, plain('r"[A-Za-z0-9+/]*(?:LS0tLS1CRUdJTi', 'r"(?:LS0tLS1CRUdJTi')),
    ("b64: any short token", REDACT, plain("|LS0tLUJFR0lO)[A-Za-z0-9+/=]{40,}", "|LS0tLUJFR0lO)[A-Za-z0-9+/=]{0,}")),
    ("b64: no wrapped lines", REDACT, plain(r"(?:\r?\n[ \t]*[A-Za-z0-9+/]+={0,2}(?mR:$)|", "(?:")),
    ("b64: no indented lines", REDACT, plain(r"(?:\r?\n[ \t]*[A-Za-z0-9+/]", r"(?:\r?\n[A-Za-z0-9+/]")),
    ("b64: no literal \\n", REDACT, plain(r"(?mR:$)|\\n[A-Za-z0-9+/]+={0,2})*", "(?mR:$))*")),
    ("b64: a line taken up to any character", REDACT, plain("={0,2}(?mR:$)|", "={0,2}|")),
    # json-secret: Python dicts
    ("json: no single-quoted key", REDACT, plain("""|'{SECRET_KEY}')""", ")")),
    ("json: no single-quoted value", REDACT, plain(r"""|'(?:[^'\\\n]|\\.)*')|""", ")|")),
    ("json: no string prefix", REDACT, plain("(?P<v>(?:[rbuf]{{1,2}})?(?:", "(?P<v>(?:")),
    ("json: no single-quote triggers", REDACT, plain("""            "password'",\n            "passwd'",\n            "passphrase'",\n            "secret'",\n            "token'",\n            "api_key'",\n            "api-key'",\n            "apikey'",\n""", "")),
    ("json: '' is a value", REDACT, plain(""".or_else(|| value.strip_prefix('\\'').and_then(|v| v.strip_suffix('\\'')))""", "")),
    ("json: a prefix is a value", REDACT, plain("    let value = value.trim_start_matches(['r', 'R', 'b', 'B', 'u', 'U', 'f', 'F']);\n", "")),
    # mount-password
    ("mount: any option list", REDACT, plain("check: Some(mount_password),", "check: Some(|_| true),")),
    ("mount: no pwd=", REDACT, plain('[&b"pass="[..], b"pwd="]', '[&b"pass="[..]]')),
    ("mount: case-sensitive", REDACT, plain("option[..name.len()].eq_ignore_ascii_case(name)", "&option[..name.len()] == *name")),
    ("mount: quotes kept", REDACT, plain("let option = option.trim_start_matches(['\\'', '\"']).as_bytes();", "let option = option.as_bytes();")),
    ("mount: no --options", REDACT, plain(r'&format!(r"\s(?:-o{GAP}*|--options(?:=|{GAP}+))"),', r'&format!(r"\s(?:-o{GAP}*)"),')),
    ("mount: scans on only for --options", REDACT, plain('"mount-password" => &["-o"],', '"mount-password" => &["--options"],')),
    ("mount: no pwd= trigger", REDACT, plain('"mount-password" => &["mount+pass=", "mount+pwd="],', '"mount-password" => &["mount+pass="],')),
    # nmcli-secret: fields to show
    ("nmcli: a field is a property to set", REDACT, plain('unless: &["field"],', "unless: &[],")),
    ("nmcli: only -g names a field", REDACT, plain(r"(?P<field>-{{1,2}}[fg][a-z-]*", r"(?P<field>-{{1,2}}[g][a-z-]*")),
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
    ("args: substitution after key", ARGS, plain('"token", "key", "cred"', '"token", "cred"')),
    ("args: substitution after =", ARGS, plain("matches!(c, '=' | '\\\\' | '\\'' | '\"')", "matches!(c, '\\\\' | '\\'' | '\"')")),
    ("args: substitution after a continued line", ARGS, plain("matches!(c, '=' | '\\\\' | '\\'' | '\"')", "matches!(c, '=' | '\\'' | '\"')")),
    ("args: substitution after a quoted option", ARGS, plain("matches!(c, '=' | '\\\\' | '\\'' | '\"')", "matches!(c, '=' | '\\\\')")),
    # fed: a pipe into the command only
    ("fed: || pipes", PKG, plain("lexer.piped |= c == '|' && self.at(i + 1) != Some('|');", "lexer.piped |= c == '|';")),
    ("fed: no pipe", PKG, plain("lexer.piped |= c == '|' && self.at(i + 1) != Some('|');", "")),
    ("fed: here-string feeds nothing", PKG, plain("                        lexer.pending = Some(Pending::Read);\n                        lexer.current.fed = true;\n", "                        lexer.pending = Some(Pending::Read);\n")),
    ("fed: substitution feeds nothing", PKG, plain("                        // `<(…)`: read for its heredocs, not kept\n                        lexer.current.fed = true;\n", "                        // `<(…)`: read for its heredocs, not kept\n")),
    ("fed: not past (", PKG, plain(
        "        let mut segment = std::mem::take(&mut self.current);\n",
        "        let mut segment = std::mem::take(&mut self.current);\n        let piped = std::mem::take(&mut self.piped);\n",
        "            segment.fed |= std::mem::take(&mut self.piped);\n",
        "            segment.fed |= piped;\n")),
    ("fed: sh -c not fed by its caller", PKG, plain("fed: inner.fed || segment.fed,", "fed: inner.fed,")),
    ("fed: every command", ARGS, plain("    segment.fed || writes\n", "    segment.fed || writes || !segment.words.is_empty()\n")),
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
