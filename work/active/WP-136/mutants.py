#!/usr/bin/env python3
"""WP-136 manual mutants (rounds 1 and 2): each change must make the plugins tests fail
(`--lib` collectors::plugins::tests and `--test collectors_user`)."""
import os, subprocess, sys
from pathlib import Path
# the checkout this script lives in: work/active/WP-136/mutants.py
WT = str(Path(__file__).resolve().parents[3])
F = "engine/src/collectors/plugins.rs"
# a target dir of its own: a mutated build must never reach another run
TARGET = f"{WT}/engine/target/mutants"
MUTANTS = [
    ("hooks not disabled", '\n    "core.hooksPath=/dev/null",\n', '\n    "core.hooksPath=",\n'),
    ("transport allowed", '\n    "protocol.allow=never",\n', '\n    "protocol.allow=always",\n'),
    ("system config read", '\n    ("GIT_CONFIG_NOSYSTEM", "1"),\n', '\n    ("GIT_CONFIG_NOSYSTEM", "0"),\n'),
    ("global config read", '\n    ("GIT_CONFIG_GLOBAL", "/dev/null"),\n', '\n    ("GIT_CONFIG_GLOBAL", "/etc/gitconfig"),\n'),
    ("repository variables kept", "            cmd.env_remove(var);\n", "            let _ = var;\n"),
    ("no ceiling", 'cmd.env("GIT_CEILING_DIRECTORIES", parent);', 'let _ = parent;'),
    ("heads not checked before git", "if !is_hash(old) || !is_hash(new) || old == new {", "if old == new {"),
    ("pull read as rollback", '(0, came) => ("pull", format!("{old}..{new}"), came),', '(0, came) => ("rollback", format!("{old}..{new}"), came),'),
    ("rollback lists the wrong side", '(left, 0) => ("rollback", format!("{new}..{old}"), left),', '(left, 0) => ("rollback", format!("{old}..{new}"), left),'),
    ("ellipsis for one commit", "Some(first) if self.count > 1 =>", "Some(first) if self.count > 0 =>"),
    # `.take(COMMITS_MAX)` backs up git's --max-count: neither alone is killed
    ("more than 20 subjects", 'let max = format!("--max-count={COMMITS_MAX}");', 'let max = format!("--max-count={}", COMMITS_MAX + 5);'),
    ("format characters kept", ".filter(|c| !invisible(*c))", ".filter(|_| true)"),
    ("control characters kept", "c.is_control() || matches!(c, '\\u{2028}' | '\\u{2029}')", "matches!(c, '\\u{2028}' | '\\u{2029}')"),
    ("not redacted", "let redacted = redactor.redact(clean);", "let redacted = clean.to_string();"),
    ("not clipped", "if redacted.chars().count() <= COMMIT_SUBJECT_MAX {", "if true {"),
    ("last head forgotten", "                    .or_else(|| last?.head.clone())\n", "\n"),
    ("first-party clones tracked", "(!p.first_party && git_dir == GitDir::Contained)", "(git_dir == GitDir::Contained)"),
    ("third-party clones not tracked", "(!p.first_party && git_dir == GitDir::Contained)", "(false && git_dir == GitDir::Contained)"),
    ("clone not named on add", "if seen_of(id).repo.is_some() && n.head.is_some() {", "if false {"),
    ("ref names not checked", "if !plain || name.contains(\"..\") || name.ends_with(\".lock\") {", "if false {"),
    ("dot ref names allowed", "&& !part.starts_with('.')", ""),
    ("packed before loose", "        return full(loose.trim_end_matches('\\n'));\n", "        let _ = loose;\n"),
    # round 2: (name, [(old, new), ...], file); every pair must match once
    ("R2-B1 grafts read", [('\n    ("GIT_GRAFT_FILE", "/dev/null"),\n', '\n    ("GIT_UNUSED", "0"),\n')], F),
    ("R2-B1 output not capped", [("sys::run_command_capped(self.command(dir, args), self.timeout, GIT_OUTPUT_MAX);",
                                  "(sys::run_command(self.command(dir, args), self.timeout), false);")], F),
    ("R2-N1 lazy fetch guards off", [('\n    "--no-lazy-fetch",\n', '\n    "--no-pager",\n'),
                                     ('\n    ("GIT_ALLOW_PROTOCOL", "none"),\n', '\n    ("GIT_UNUSED", "0"),\n'),
                                     ('\n    ("GIT_NO_LAZY_FETCH", "1"),\n', '\n    ("GIT_UNUSED", "0"),\n')], F),
    ("R2-N1 only GIT_ALLOW_PROTOCOL off", [('\n    ("GIT_ALLOW_PROTOCOL", "none"),\n', '\n    ("GIT_UNUSED", "0"),\n')], F),
    ("R2-N1 only the lazy-fetch pair off", [('\n    "--no-lazy-fetch",\n', '\n    "--no-pager",\n'),
                                            ('\n    ("GIT_NO_LAZY_FETCH", "1"),\n', '\n    ("GIT_UNUSED", "0"),\n')], F),
    ("R2-N2 a linked or file .git read", [("    if !meta.is_dir() {\n", "    if false {\n")], F),
    ("R2-N2 alternates and commondir read", [('if exists("objects/info/alternates") || exists("commondir") {', "if false {")], F),
    ("R2-N2 include sections read", [("Ok(Some(text)) if !includes(&text) => {}", "Ok(Some(_)) => {}")], F),
    ("R2-N2 no reason in the detail", [("if seen_of(id).outside {", "if false {")], F),
    ("R2-N3 process group (R1)", [("run_with(cmd, timeout, Group::Own, cap)", "run_with(cmd, timeout, Group::Engine, cap)")], "engine/src/sys.rs"),
    ("R2-N4 timeout 60 s (R2)", [("const GIT_TIMEOUT: Duration = Duration::from_secs(2);", "const GIT_TIMEOUT: Duration = Duration::from_secs(60);")], F),
    ("R2-N4 isolates kept (R7)", [("| '\\u{2060}' | '\\u{2066}'..='\\u{2069}' |", "| '\\u{2060}' |")], F),
    ("R2-N4 separators kept", [("c.is_control() || matches!(c, '\\u{2028}' | '\\u{2029}')", "c.is_control()")], F),
    ("R2-N4 log encoding of the clone", [('\n    "i18n.logOutputEncoding=UTF-8",\n', '\n    "core.unused=0",\n')], F),
]
env = dict(os.environ, CARGO_TARGET_DIR=TARGET)
results = []
# optional arguments: run only the mutants whose name starts with one of them
only = sys.argv[1:]
for m in MUTANTS:
    if only and not any(m[0].startswith(o) for o in only):
        continue
    if isinstance(m[1], list):
        name, pairs, rel = m
    else:
        name, pairs, rel = m[0], [(m[1], m[2])], F
    path = os.path.join(WT, rel)
    orig = open(path).read()
    bad = [a for a, _ in pairs if orig.count(a) != 1]
    if bad:
        results.append((name, f"PATTERN COUNT {[orig.count(a) for a in bad]}")); continue
    mutated = orig
    for a, b in pairs:
        mutated = mutated.replace(a, b)
    try:
        open(path, "w").write(mutated)
        r = subprocess.run(["cargo", "test", "--manifest-path", "engine/Cargo.toml",
                            "--no-fail-fast", "--lib", "--test", "collectors_user", "plugin"],
                           cwd=WT, env=env, capture_output=True, text=True)
    finally:
        open(path, "w").write(orig)
    out = r.stdout + r.stderr
    failed = [l.split()[1] for l in out.splitlines() if l.startswith("test ") and l.endswith("FAILED")]
    results.append((name, ("killed by " + ", ".join(failed)) if r.returncode != 0 and failed else
                    ("SURVIVED" if r.returncode == 0 else "build error:\n" + r.stderr[-800:])))
for n, r in results:
    print(f"{n}: {r}")
