#!/usr/bin/env python3
"""WP-128 manual mutants: each one reverts one CRLF form in
engine/src/redact.rs; `--test redaction` must fail for every one."""
import os, subprocess
from pathlib import Path

# the checkout this script lives in: work/active/WP-128/mutants.py
WT = str(Path(__file__).resolve().parents[3])
F = "engine/src/redact.rs"
# a target dir of its own: a mutated build must never reach another run
TARGET = f"{WT}/engine/target/mutants"
ESC = r"\\(?:\r\n|(?s:.))"
OLD_ESC = r"\\(?s:.)"


def nth(line_start, k):
    """Revert the k-th CRLF escape on the line that starts with `line_start`."""
    def apply(src):
        i = src.index(line_start)
        j = src.index("\n", i)
        line = src[i:j]
        parts = line.split(ESC)
        assert len(parts) > k + 1, (line_start, k)
        line = ESC.join(parts[: k + 1]) + OLD_ESC + ESC.join(parts[k + 1 :])
        return src[:i] + line + src[j:]
    return apply


def plain(a, b, count=1):
    def apply(src):
        assert src.count(a) == count, (a, src.count(a))
        return src.replace(a, b)
    return apply


MUTANTS = [
    ("GAP: \\ LF only", plain(r'const GAP: &str = r"(?:\s|\\\r?\n)";', r'const GAP: &str = r"(?:\s|\\\n)";')),
    ("HTTPIE_GAP: bare LF only", plain(r'r"(?:[ \t]|\r?\n|\\\r?\n)"', r'r"(?:[ \t]|\n|\\\r?\n)"')),
    ("HTTPIE_GAP: \\ LF only", plain(r'r"(?:[ \t]|\r?\n|\\\r?\n)"', r'r"(?:[ \t]|\r?\n|\\\n)"')),
    ("httpie triggers without \\r", lambda s: "".join(l for l in s.splitlines(True) if not (l.strip().startswith('"') and '\\r+-a"' in l))),
    ("db-client-password: prefix LF only", plain(r"(?-u:\b)(?:\\\r?\n|[^\n])*?\s-p ?)", r"(?-u:\b)(?:\\\n|[^\n])*?\s-p ?)")),
    ("db-client-password: tail LF only", plain(r"\s-p ?)\S(?:\\\r?\n|[^\n])*", r"\s-p ?)\S(?:\\\n|[^\n])*")),
    ("COMMAND_REST: bare escape", nth("const COMMAND_REST", 0)),
    ("WORD: escape in \"…\"", nth("const WORD", 0)),
    ("WORD: escape in $'…'", nth("const WORD", 1)),
    ("WORD: bare escape", nth("const WORD", 2)),
    ("PASS_ARG: escape in \"pass:…\"", nth("const PASS_ARG", 0)),
    ("PASS_ARG: escape in $'pass:…'", nth("const PASS_ARG", 1)),
    ("PASS_ARG: escape in a later \"…\"", nth("const PASS_ARG", 2)),
    ("PASS_ARG: escape in a later $'…'", nth("const PASS_ARG", 3)),
    ("PASS_ARG: bare escape", nth("const PASS_ARG", 4)),
    ("trailing \\r not put back", plain("if matched.ends_with('\\r') && !out.ends_with('\\r') {", "if false {")),
    ("lost breaks put back as LF", plain("if matched[..j].ends_with('\\r') {", "if false {")),
]

env = dict(os.environ, CARGO_TARGET_DIR=TARGET)
path = os.path.join(WT, F)
results = []
for name, mutate in MUTANTS:
    orig = open(path).read()
    try:
        mutated = mutate(orig)
    except AssertionError as e:
        results.append((name, f"NOT APPLIED {e}"))
        continue
    if mutated == orig:
        results.append((name, "NOT APPLIED (no change)"))
        continue
    try:
        open(path, "w").write(mutated)
        r = subprocess.run(
            ["cargo", "test", "--manifest-path", "engine/Cargo.toml", "--test", "redaction", "--", "--test-threads=4"],
            cwd=WT, env=env, capture_output=True, text=True)
    finally:
        open(path, "w").write(orig)
    failed = [l.split()[1] for l in r.stdout.splitlines() if l.startswith("test ") and l.endswith("FAILED")]
    results.append((name, ("killed by " + ", ".join(failed)) if r.returncode != 0 and failed else
                    ("SURVIVED" if r.returncode == 0 else "build error:\n" + r.stderr[-800:])))
for n, r in results:
    print(f"{n}: {r}")
