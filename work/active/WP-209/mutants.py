#!/usr/bin/env python3
"""WP-209 manual mutants: each one undoes one rule of the WP in the engine;
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
HOOK = "engine/src/commands/hook.rs"

# the json-secret value alternatives
DQ3 = r'''"""(?:"?"?(?:[^"\\]|\\(?s:.)))*"""|'''
SQ3 = r"""'''(?:'?'?(?:[^'\\]|\\(?s:.)))*'''|"""
ESC3 = r'''\\"\\"\\"(?s:.)*?\\"\\"\\"|'''


def plain(*pairs):
    """Replace each `a` (found exactly once) with its `b`."""
    def apply(src):
        for a, b in zip(pairs[::2], pairs[1::2]):
            assert src.count(a) == 1, (a, src.count(a))
            src = src.replace(a, b)
        return src
    return apply


def wrapper(name, field, value):
    """Set `field` of the WRAPPERS entry `name` to `value`."""
    def apply(src):
        i = src.index(f'name: "{name}",')
        j = src.index("},", i)
        block = src[i:j]
        k = block.index(f"{field}: ")
        end = block.index(",\n", k)
        return src[:i] + block[:k] + f"{field}: {value}" + block[end:] + src[j:]
    return apply


MUTANTS = [
    # json-secret: triple quotes
    ("json: no triple double quotes", REDACT, plain(DQ3, "")),
    ("json: no triple single quotes", REDACT, plain(SQ3, "")),
    ("json: no escaped triple quotes", REDACT, plain(ESC3, "")),
    ("json: triple double quotes on one line", REDACT, plain(r'''"""(?:"?"?(?:[^"\\]|''', r'''"""(?:"?"?(?:[^"\\\n]|''')),
    ("json: an empty triple is a value", REDACT, plain('["\\\\\\"\\\\\\"\\\\\\"", "\\\\\\"", "\\"\\"\\"", "\'\'\'", "\\"", "\'"]', '["\\\\\\"\\\\\\"\\\\\\"", "\\\\\\"", "\\"", "\'"]')),
    ("json: an empty escaped triple is a value", REDACT, plain('["\\\\\\"\\\\\\"\\\\\\"", "\\\\\\"", ', '["\\\\\\"", ')),
    # json-secret: a key word with `key` after it
    ("json: no key after the key word", REDACT, plain("|api[_-]?key)(?:[_-]?(?:access[_-]?)?key)?\";", "|api[_-]?key)\";")),
    ("json: no access key", REDACT, plain("(?:[_-]?(?:access[_-]?)?key)?", "(?:[_-]?key)?")),
    ("json: any word after the key word", REDACT, plain("(?:[_-]?(?:access[_-]?)?key)?", "(?:[_-]?[a-z]+)?")),
    ("json: no key triggers", REDACT, plain(
        '            // a key word, then `key` and the quote (`secret_key`, WP-209)\n'
        '            "password>key\\"",\n            "password>key\\\\\\"",\n            "password>key\'",\n'
        '            "passwd>key\\"",\n            "passwd>key\\\\\\"",\n            "passwd>key\'",\n'
        '            "passphrase>key\\"",\n            "passphrase>key\\\\\\"",\n            "passphrase>key\'",\n'
        '            "secret>key\\"",\n            "secret>key\\\\\\"",\n            "secret>key\'",\n'
        '            "token>key\\"",\n            "token>key\\\\\\"",\n            "token>key\'",\n', "")),
    # nmcli: a field option before the object word only
    ("nmcli: a field anywhere", REDACT, plain("            keep: Some(nmcli_field),\n", '            unless: &["field"],\n')),
    ("nmcli: never a field", REDACT, plain("            keep: Some(nmcli_field),\n", "            keep: None,\n")),
    ("nmcli: no option values", REDACT, plain(
        "        value_next = !name.is_empty()\n            && !name.contains('=')\n            && WITH_VALUE.iter().any(|o| o.starts_with(name));\n",
        "        value_next = false && !name.is_empty();\n")),
    ("nmcli: a field after the command's first option", REDACT, plain(
        "    let (Some(cmd), Some(field)) = (caps.name(\"cmd\"), caps.name(\"field\")) else {\n        return false;\n    };",
        "    let (Some(cmd), Some(field)) = (caps.name(\"cmd\"), caps.name(\"field\")) else {\n        return caps.name(\"field\").is_some();\n    };")),
    # stdbuf and setsid
    ("wrappers: no stdbuf", PKG, plain('        name: "stdbuf",', '        name: "stdbuz",')),
    ("wrappers: no setsid", PKG, plain('        name: "setsid",', '        name: "setsiz",')),
    ("stdbuf: short values", PKG, wrapper("stdbuf", "short_value", '""')),
    ("stdbuf: long values", PKG, wrapper("stdbuf", "long_value", "&[]")),
    ("setsid: no probes", PKG, wrapper("setsid", "probe_short", '""')),
    # substitutions opened
    ("subst: not opened", PKG, plain(
        "            segments: std::mem::take(&mut outer.segments),\n", "",
        "        outer.segments = inner.segments;\n", "")),
    ("subst: <( ) not opened", PKG, plain(
        "let end = self.read(i + 1, Stop::Paren, &mut inner);",
        "let end = self.read(i + 1, Stop::Paren, &mut Lexer::default());")),
    ("subst: not fed by the line", PKG, plain("            fed: outer.fed,\n", "")),
    ("subst: feeds nothing after it", PKG, plain("        outer.fed |= inner.fed;\n", "")),
    ("subst: numbered 0", PKG, plain("            segment.substitution = self.substitution;\n", "")),
    ("subst: a cd moves the line", PKG, plain("                (shell, previous, stack) = saved;\n", "                let _ = saved;\n")),
    ("subst: no depth limit", PKG, plain(
        "        let deep = self.depth >= SUBSTITUTION_DEPTH_MAX;", "        let deep = false;",
        "i = if self.depth >= SUBSTITUTION_DEPTH_MAX {", "i = if false {")),
    ("subst: too deep recorded whole", HOOK, plain("    let skipped = line.too_deep\n        || password_on_stdin(&line)", "    let skipped = password_on_stdin(&line)")),
    # a descriptor the line feeds (N2)
    ("fd: no <( ) descriptor", ARGS, plain("                fds.insert(descriptor(before));\n", "                let _ = before;\n")),
    ("fd: no here-string descriptor", ARGS, plain("        fds.insert(descriptor(&text[..at]));\n", "        let _ = at;\n")),
    ("fd: any descriptor", ARGS, plain("!fd.is_empty() && fds.contains(fd) && ", "!fd.is_empty() && ")),
    ("fd: no secret name", ARGS, plain("fds.contains(fd) && names_a_secret(&text[..at])", "fds.contains(fd)")),
    ("fd: no /dev/stdin", ARGS, plain('            let fd = if file == "/dev/stdin" {\n                "0"', '            let fd = if file == "/dev/stdin" {\n                ""')),
    ("fd: no /proc/self/fd", ARGS, plain('["/dev/fd/", "/proc/self/fd/", "/dev/stdin"]', '["/dev/fd/", "/dev/stdin"]')),
    ("fd: no stdin without digits", ARGS, plain('        "" => "0",\n', '        "" => "",\n')),
    ("fd: a descriptor's first digit", ARGS, plain("                &digits[..end]\n", "                &digits[..end.min(1)]\n")),
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
