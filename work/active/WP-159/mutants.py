#!/usr/bin/env python3
"""WP-159 manual mutants: each one undoes one piece of the WP in the
engine (the invisible set, the shared helper, the redaction of the visible
copy and the putting back of invisible characters, the places that drop
them); the tests named for it must fail. Run from a copy of the tree: the
script edits the checkout it lives in and puts each file back. Names given
as arguments run only the mutants whose name contains one of them;
`--check` only applies each mutant."""
import os
import subprocess
import sys
from pathlib import Path

# the checkout this script lives in: work/active/WP-159/mutants.py
WT = str(Path(__file__).resolve().parents[3])
# a target dir of its own: a mutated build must never reach another run
TARGET = f"{WT}/engine/target/mutants-wp159"
REDACT = "engine/src/redact.rs"
IMPORT = "engine/src/import/mod.rs"


def plain(a, b, count=1):
    def apply(src):
        assert src.count(a) == count, (a, src.count(a))
        return src.replace(a, b)
    return apply


def nth(a, b, n):
    """Replace only the `n`-th (0-based) occurrence of `a`."""
    def apply(src):
        at = -1
        for _ in range(n + 1):
            at = src.index(a, at + 1)
        return src[:at] + b + src[at + len(a):]
    return apply


cargo = ["cargo", "test", "--manifest-path", "engine/Cargo.toml", "--no-fail-fast"]
LIB = cargo + ["--lib", "--", "--test-threads=4"]
REDACTION = cargo + ["--test", "redaction", "--", "--test-threads=4"]
LOG = cargo + ["--test", "log", "--", "--test-threads=4"]
IMPORT_TASK = cargo + ["--test", "import_task", "--", "--test-threads=4"]
PLAN = cargo + ["--test", "plan", "--", "closing", "split", "--test-threads=4"]
HOOKS = cargo + ["--test", "hooks", "--", "privileged", "--test-threads=4"]
INDEX = cargo + ["--test", "index", "--", "--test-threads=4"]
REFERENCE = cargo + ["--test", "index", "--", "the_reference_drops", "--test-threads=4"]
CORE = [LIB, LOG, IMPORT_TASK]

MUTANTS = [
    # 1. the set: each range WP-159 added
    ("set: no U+034F", REDACT, plain("            | '\\u{034F}'\n", ""), [LIB, REFERENCE]),
    ("set: no Hangul choseong/jungseong fillers", REDACT, plain("            | '\\u{115F}'..='\\u{1160}'\n", ""), [LIB, REFERENCE]),
    ("set: no Khmer inherent vowels", REDACT, plain("            | '\\u{17B4}'..='\\u{17B5}'\n", ""), [LIB, REFERENCE]),
    ("set: Mongolian selectors only U+180E", REDACT, plain("'\\u{180B}'..='\\u{180F}'", "'\\u{180E}'..='\\u{180E}'"), [LIB, REFERENCE]),
    ("set: no U+180F", REDACT, plain("'\\u{180B}'..='\\u{180F}'", "'\\u{180B}'..='\\u{180E}'"), [LIB, REFERENCE]),
    ("set: no U+2065", REDACT, plain("            | '\\u{2060}'..='\\u{206F}'\n", "            | '\\u{2060}'..='\\u{2064}'\n            | '\\u{2066}'..='\\u{206F}'\n"), [LIB, REFERENCE]),
    ("set: no U+3164", REDACT, plain("            | '\\u{3164}'\n", ""), [LIB, REFERENCE]),
    ("set: no FE00-FE0F", REDACT, plain("            | '\\u{FE00}'..='\\u{FE0F}'\n", ""), [LIB, REFERENCE]),
    ("set: only FE0F", REDACT, plain("'\\u{FE00}'..='\\u{FE0F}'", "'\\u{FE0F}'..='\\u{FE0F}'"), [LIB, REFERENCE]),
    ("set: no U+FFA0", REDACT, plain("            | '\\u{FFA0}'\n", ""), [LIB, REFERENCE]),
    ("set: no E0100-E01EF", REDACT, plain("            | '\\u{E0100}'..='\\u{E01EF}'\n", ""), [LIB, REFERENCE]),
    ("set: E0100-E01EE", REDACT, plain("'\\u{E0100}'..='\\u{E01EF}'", "'\\u{E0100}'..='\\u{E01EE}'"), [LIB, REFERENCE]),
    # 2. the helper
    ("helper: never drops", REDACT, plain("    if text.is_ascii() || !text.chars().any(is_invisible) {", "    if true {"), CORE),
    ("helper: drops nothing when the text is not ASCII", REDACT, plain("    if text.is_ascii() || !text.chars().any(is_invisible) {", "    if !text.is_ascii() || !text.chars().any(is_invisible) {"), CORE),
    # 3. the redaction reads the visible copy
    ("redact: reads the text as given", REDACT, plain("        let Cow::Owned(visible) = without_invisible(text) else {", "        let Cow::Owned(visible) = Cow::<str>::Borrowed(text) else {"), CORE + [REDACTION]),
    ("matching_rules: reads the text as given", REDACT, nth("        let visible = without_invisible(text);\n", "        let visible = Cow::<str>::Borrowed(text);\n", 0), [LIB]),
    ("matching_rules_by_line: reads the text as given", REDACT, nth("        let visible = without_invisible(text);\n", "        let visible = Cow::<str>::Borrowed(text);\n", 1), [LIB, cargo + ["--test", "import", "--", "--test-threads=4"]]),
    # 4. the origin map
    ("map: a replacement adds no entries", REDACT, plain("                    map.resize(out.len(), NO_ORIGIN);\n", ""), CORE),
    ("map: a copy adds no entries", REDACT, plain("                map.extend_from_slice(&origin[from..to]);\n", ""), CORE),
    ("map: not handed back", REDACT, plain("            *origin = map;\n", ""), CORE),
    ("map: not carried through the rules", REDACT, plain("out = rule.replace_with(&out, keep_lines, origin.as_deref_mut());", "out = rule.replace_with(&out, keep_lines, None);"), CORE),
    # 5. putting the runs back
    ("restore: every run back, at a match's edge too", REDACT, plain("        if next == Some(from)\n", "        if from != NO_ORIGIN\n"), CORE),
    ("restore: no run back inside the text", REDACT, plain("        if next == Some(from)\n", "        if next == Some(from) && false\n"), CORE),
    ("restore: the start counts as a replacement", REDACT, plain("    let mut next = Some(0);\n", "    let mut next = None;\n"), CORE),
    ("restore: no run back at the end", REDACT, plain("    if next == Some(visible)\n", "    if next == Some(visible) && false\n"), CORE),
    ("restore: off by one after a character", REDACT, plain("        next = (last != NO_ORIGIN).then(|| last + 1);\n", "        next = (last != NO_ORIGIN).then_some(last);\n"), CORE),
    ("restore: visible offsets count characters", REDACT, plain("            visible += c.len_utf8();\n", "            visible += 1;\n"), CORE),
    # 6. the places that drop the set
    ("path: invisible characters allowed", IMPORT, plain("    c.is_control() || is_invisible(c) || matches!", "    c.is_control() || matches!"), [LIB, IMPORT_TASK]),
    ("hook: keeps invisible characters", "engine/src/commands/hook.rs", plain("    let command = crate::redact::without_invisible(command);\n", "    let command = std::borrow::Cow::<str>::Borrowed(command);\n"), [HOOKS]),
    ("closing summary: keeps invisible characters", "engine/src/commands/plan.rs", plain("    let tail: String = crate::redact::without_invisible(&tail)\n", "    let tail: String = std::borrow::Cow::<str>::Borrowed(tail.as_str())\n"), [LIB, PLAN]),
    ("plugin subject: keeps invisible characters", "engine/src/collectors/plugins.rs", plain("    let clean: String = without_invisible(raw)\n", "    let clean: String = std::borrow::Cow::<str>::Borrowed(raw)\n"), [LIB]),
    ("index text: keeps invisible characters", "engine/src/index/build.rs", plain("    let plain: String = without_invisible(text)\n", "    let plain: String = std::borrow::Cow::<str>::Borrowed(text)\n"), [INDEX]),
    ("import task: keeps invisible characters", "engine/src/commands/import/task.rs", plain("    let text = crate::redact::without_invisible(&text).into_owned();\n", "    let text = text.clone();\n"), [IMPORT_TASK]),
]

check = "--check" in sys.argv
only = [a for a in sys.argv[1:] if a != "--check"]
env = dict(os.environ, CARGO_TARGET_DIR=TARGET)
results = []
for name, file, mutate, runs in MUTANTS:
    if only and not any(o in name for o in only):
        continue
    path = os.path.join(WT, file)
    orig = open(path).read()
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
