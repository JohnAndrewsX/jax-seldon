#!/usr/bin/env python3
"""WP-136 manual mutants: each change must make the plugins tests fail
(`--lib` collectors::plugins::tests and `--test collectors_user`)."""
import os, subprocess
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
    ("control characters kept", ".map(|c| if c.is_control() { ' ' } else { c })", ".map(|c| c)"),
    ("not redacted", "let redacted = redactor.redact(clean);", "let redacted = clean.to_string();"),
    ("not clipped", "if redacted.chars().count() <= COMMIT_SUBJECT_MAX {", "if true {"),
    ("last head forgotten", "                    .or_else(|| last?.head.clone())\n", "\n"),
    ("first-party clones tracked", "(!p.first_party && is_clone(&dir))", "is_clone(&dir)"),
    ("third-party clones not tracked", "(!p.first_party && is_clone(&dir))", "(false && is_clone(&dir))"),
    ("clone not named on add", "if seen_of(id).repo.is_some() && n.head.is_some() {", "if false {"),
    ("ref names not checked", "if !plain || name.contains(\"..\") || name.ends_with(\".lock\") {", "if false {"),
    ("dot ref names allowed", "&& !part.starts_with('.')", ""),
    ("packed before loose", "        return full(loose.trim_end_matches('\\n'));\n", "        let _ = loose;\n"),
]
env = dict(os.environ, CARGO_TARGET_DIR=TARGET)
results = []
for name, a, b in MUTANTS:
    path = os.path.join(WT, F)
    orig = open(path).read()
    if orig.count(a) != 1:
        results.append((name, f"PATTERN COUNT {orig.count(a)}")); continue
    try:
        open(path, "w").write(orig.replace(a, b))
        r = subprocess.run(["cargo", "test", "--manifest-path", "engine/Cargo.toml",
                            "--lib", "--test", "collectors_user", "plugin"],
                           cwd=WT, env=env, capture_output=True, text=True)
    finally:
        open(path, "w").write(orig)
    out = r.stdout + r.stderr
    failed = [l.split()[1] for l in out.splitlines() if l.startswith("test ") and l.endswith("FAILED")]
    results.append((name, ("killed by " + ", ".join(failed)) if r.returncode != 0 and failed else
                    ("SURVIVED" if r.returncode == 0 else "build error:\n" + r.stderr[-800:])))
for n, r in results:
    print(f"{n}: {r}")
