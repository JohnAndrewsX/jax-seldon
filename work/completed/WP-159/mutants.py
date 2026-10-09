#!/usr/bin/env python3
"""WP-159 manual mutants: each one undoes one piece of the WP in the
engine (the invisible set, the shared helper, the two readings of the
redaction (round 2), the putting back of the characters the first one
leaves out, and the places that redact, then drop or mark them); the
tests named for it must fail. Run from a copy of the tree: the
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


def both(first, second):
    """Apply two mutations as one."""
    return lambda src: second(first(src))


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
    # 3. the first reading: the copy without invisible and control characters
    ("redact: reads only the text as given", REDACT, plain("        let Cow::Owned(copy) = reading_copy(text) else {", "        let Cow::Owned(copy) = Cow::<str>::Borrowed(text) else {"), CORE + [REDACTION]),
    ("copy: keeps control characters", REDACT, plain("    is_invisible(c) || (c.is_control() && !c.is_whitespace())", "    is_invisible(c)"), [LIB, LOG]),
    ("copy: drops white-space controls too", REDACT, plain("    is_invisible(c) || (c.is_control() && !c.is_whitespace())", "    is_invisible(c) || c.is_control()"), [LIB]),
    ("copy: ASCII text keeps its controls", REDACT, plain("        text.bytes().any(|b| hides_from_rules(char::from(b)))", "        false"), [LIB, LOG]),
    ("matching_rules: only the text as given", REDACT, nth("        let copy = reading_copy(text);\n", "        let copy = Cow::<str>::Borrowed(text);\n", 0), [LIB]),
    ("matching_rules_by_line: only the text as given", REDACT, nth("        let copy = reading_copy(text);\n", "        let copy = Cow::<str>::Borrowed(text);\n", 1), [LIB, cargo + ["--test", "import", "--", "--test-threads=4"]]),
    # 4. the second reading: the text as given (round 2, B1)
    ("redact: no second reading", REDACT, plain("        self.passes(&restored, keep_lines, None)\n", "        restored\n"), CORE + [REDACTION]),
    ("matching_rules: no union", REDACT, nth("        if let Cow::Owned(_) = copy {\n", "        if false {\n", 0), [LIB]),
    ("matching_rules_by_line: no union", REDACT, nth("        if let Cow::Owned(_) = copy {\n", "        if false {\n", 1), [LIB]),
    # 5. the origin map
    ("map: a replacement adds no entries", REDACT, plain("                    map.resize(out.len(), NO_ORIGIN);\n", ""), CORE),
    ("map: a replacement's entries point at the start", REDACT, plain("                    map.resize(out.len(), NO_ORIGIN);\n", "                    map.resize(out.len(), 0);\n"), [LIB]),
    ("map: a copy adds no entries", REDACT, plain("                map.extend_from_slice(&origin[from..to]);\n", ""), CORE),
    ("map: not handed back", REDACT, plain("            *origin = map;\n", ""), CORE),
    ("map: not carried through the rules", REDACT, plain("out = rule.replace_with(&out, keep_lines, origin.as_deref_mut());", "out = rule.replace_with(&out, keep_lines, None);"), CORE),
    # 6. putting the runs back
    ("restore: every run back, at a match's edge too", REDACT, plain("        if next == Some(from)\n", "        if from != NO_ORIGIN\n"), CORE),
    ("restore: no run back inside the text", REDACT, plain("        if next == Some(from)\n", "        if next == Some(from) && false\n"), CORE),
    ("restore: the start counts as a replacement", REDACT, plain("    let mut next = Some(0);\n", "    let mut next = None;\n"), CORE),
    ("restore: no run back at the end", REDACT, plain("    if next == Some(copied)\n", "    if next == Some(copied) && false\n"), CORE),
    ("restore: off by one after a character", REDACT, plain("        next = (last != NO_ORIGIN).then(|| last + 1);\n", "        next = (last != NO_ORIGIN).then_some(last);\n"), CORE),
    ("restore: copy offsets count characters", REDACT, plain("            copied += c.len_utf8();\n", "            copied += 1;\n"), CORE),
    ("restore: the walk skips the run it looks for", REDACT, plain("        while runs.next_if(|&(v, _)| v < at).is_some() {}\n", "        while runs.next_if(|&(v, _)| v <= at).is_some() {}\n"), CORE),
    # 7. redact, then drop (round 2, B1b) and mark (B2)
    ("helper: drops nothing after the redaction", REDACT, plain("        without_invisible(&self.redact(text)).into_owned()\n", "        self.redact(text)\n"), [LIB, HOOKS]),
    ("path: invisible characters allowed", IMPORT, plain("    c.is_control() || is_invisible(c) || matches!", "    c.is_control() || matches!"), [LIB, IMPORT_TASK]),
    ("hook: drops before the redaction", "engine/src/commands/hook.rs", plain(".redact_dropping_invisible(command),", ".redact_dropping_invisible(&crate::redact::without_invisible(command)),"), [HOOKS]),
    ("hook: keeps invisible characters", "engine/src/commands/hook.rs", plain("    let command = match crate::redact::without_invisible(command) {", "    let command = match std::borrow::Cow::<str>::Borrowed(command) {"), [HOOKS]),
    ("closing summary: drops before the redaction", "engine/src/commands/plan.rs", plain("        redactor.redact_dropping_invisible(&tail).trim(),", "        redactor\n            .redact_dropping_invisible(&crate::redact::without_invisible(&tail))\n            .trim(),"), [LIB, PLAN]),
    ("closing summary: bidi controls as spaces", "engine/src/commands/plan.rs", plain("            if c.is_control() || matches!(c, '\\u{2028}' | '\\u{2029}') {", "            if c.is_control() || super::is_line_breaking(c) {"), [LIB]),
    ("plugin subject: drops before the redaction", "engine/src/collectors/plugins.rs", plain("    let redacted = redactor.redact_dropping_invisible(&clean);", "    let redacted = redactor.redact_dropping_invisible(&crate::redact::without_invisible(&clean));"), [LIB]),
    ("index text: drops before the redaction", "engine/src/index/build.rs", plain("    redactor.redact_dropping_invisible(&spaced(text))", "    redactor.redact_dropping_invisible(&crate::redact::without_invisible(&spaced(text)))"), [LIB, INDEX]),
    ("plan show: marks before the redaction", "engine/src/index/build.rs", both(
        plain("    let redacted = redactor.redact(&spaced(text));", "    let redacted = spaced(text);"),
        plain("    (marked, hidden)\n", "    (redactor.redact(&marked), hidden)\n")), [LIB, IMPORT_TASK]),
    ("plan show: counts only what the redaction left", "engine/src/index/build.rs", plain("    let hidden = text.chars().filter(|c| is_invisible(*c)).count();", "    let hidden = redactor.redact(text).chars().filter(|c| is_invisible(*c)).count();"), [LIB, IMPORT_TASK]),
    ("import task: keeps invisible characters", "engine/src/commands/import/task.rs", plain("    let text = scrubber.text_dropping_invisible(&shown, &text);", "    let text = scrubber.text(&shown, &text);"), [IMPORT_TASK]),
    ("import task: drops after the home paths", IMPORT, plain("            out.push_str(&self.home_paths(&content));\n", "            out.push_str(&if drop_invisible {\n                without_invisible(&self.home_paths(done.trim_end_matches('\\r'))).into_owned()\n            } else {\n                self.home_paths(&content)\n            });\n"), [IMPORT_TASK]),
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
