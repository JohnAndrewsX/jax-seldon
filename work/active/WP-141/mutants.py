#!/usr/bin/env python3
"""WP-141 mutants: each patch must make its test fail. Restores every file.

Run from anywhere: python3 work/active/WP-141/mutants.py [M1 M7 …]
The repository root comes from this file's place; the build goes to its own
CARGO_TARGET_DIR ($SELDON_MUTANTS_TARGET, else ~/.cache/seldon-target-wp141-mutants),
so a parallel `just check` is not disturbed. M12–M13 run the plugin's model
tests with node.
"""
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
P = "engine/src/collectors/pacman.rs"
C = "engine/src/index/class.rs"
JS = "plugin/Model.js"
LIB = ["--lib"]
DRIFT = ["--test", "drift"]
NODE = None  # node tests/plugin/model.test.js

# (name, file, old, new, cargo test selection or NODE, test name filter)
M = [
    # the classification
    ("M1 a left file never a crisis", C,
     "        if self.pacnew_red.matches(file) {",
     "        if false && self.pacnew_red.matches(file) {",
     LIB, "index::class"),
    ("M2 a left file always a crisis", C,
     "        if self.pacnew_red.matches(file) {",
     "        if true || self.pacnew_red.matches(file) {",
     LIB, "index::class"),
    ("M3 a left file routine", C,
     "            Verdict::new(Class::Attention, \"pacnew\")",
     "            Verdict::new(Class::Routine, \"pacnew\")",
     LIB, "index::class"),
    ("M4 no note row: falls to the package rows", C,
     "        if e.kind == Kind::Note {\n            return Some(self.pacnew(&e.subject));\n        }\n",
     "",
     LIB, "index::class"),
    ("M5 the suffix is not stripped", C,
     "            .find_map(|s| subject.strip_suffix(s))",
     "            .find_map(|s| subject.strip_suffix(s).map(|_| subject))",
     LIB, "index::class"),
    ("M6 PAM left out of the list", C,
     "    \"/etc/pam.d\",\n",
     "    \"/etc/pam.d.none\",\n",
     LIB, "index::class"),
    # txId versus meta.transaction
    ("M7 the note takes the transaction's txId", P,
     "            Event::new(l.ts, Source::Pacman, Kind::Note, &l.left)\n                .detail(format!(\"{} {verb} as {}\", l.file, l.left))\n                .meta(meta)\n",
     "            {\n                let mut e = Event::new(l.ts, Source::Pacman, Kind::Note, &l.left)\n                    .detail(format!(\"{} {verb} as {}\", l.file, l.left))\n                    .meta(meta);\n                e.tx_id = self.tx_id.clone();\n                e\n            }\n",
     DRIFT, "files_pacman_left_are_their_own_items"),
    ("M8 no meta.transaction", P,
     "                meta.extra\n                    .insert(TRANSACTION_KEY.into(), Value::String(tx.clone()));",
     "                let _ = tx;",
     DRIFT, "files_pacman_left_are_their_own_items"),
    ("M9 attribution ignores meta.transaction", P,
     "            .then(|| e.meta.extra.get(TRANSACTION_KEY)?.as_str())\n            .flatten()",
     "            .then(|| e.meta.extra.get(TRANSACTION_KEY)?.as_str())\n            .flatten()\n            .filter(|_| false)",
     DRIFT, "files_pacman_left_are_their_own_items"),
    # the line rule
    ("M10 any warning line is a left file", P,
     "        .any(|(v, suffix)| *v == verb && left.strip_suffix(suffix) == Some(file))",
     "        .any(|(v, _)| *v == verb || true)",
     LIB, "collectors::pacman"),
    ("M11 a transaction of left files only is dropped", P,
     "    txs.retain(|t| !t.lines.is_empty() || !t.left.is_empty());",
     "    txs.retain(|t| !t.lines.is_empty());",
     LIB, "collectors::pacman"),
    # the plugin's hint
    ("M12 no hint", JS,
     "PACNEW_SUFFIX.test(row.subject) ? PACNEW_HINT : \"\"",
     "PACNEW_SUFFIX.test(row.subject) ? \"\" : \"\"",
     NODE, ""),
    ("M13 a hint on every note", JS,
     "row.source === \"pacman\" && row.kind === \"note\" && PACNEW_SUFFIX.test(row.subject)",
     "row.kind === \"note\"",
     NODE, ""),
]


def run(selection, test, env):
    if selection is NODE:
        r = subprocess.run(["node", "tests/plugin/model.test.js"], capture_output=True, text=True)
        return r.returncode != 0 and "FAIL" in r.stdout, r
    r = subprocess.run(
        ["cargo", "test", "--manifest-path", "engine/Cargo.toml", *selection, test],
        capture_output=True, text=True, env=env,
    )
    return r.returncode != 0 and "test result: FAILED" in r.stdout, r


def main():
    only = sys.argv[1:]
    os.chdir(ROOT)
    target = os.environ.get("SELDON_MUTANTS_TARGET",
                            str(Path.home() / ".cache" / "seldon-target-wp141-mutants"))
    env = dict(os.environ, CARGO_TARGET_DIR=target)
    survivors = []
    for name, path, old, new, selection, test in M:
        if only and name.split()[0] not in only:
            continue
        orig = open(path).read()
        if orig.count(old) != 1:
            print(f"{name}: pattern found {orig.count(old)} times")
            survivors.append(name)
            continue
        try:
            open(path, "w").write(orig.replace(old, new))
            killed, r = run(selection, test, env)
            if r.returncode != 0 and not killed:
                print(f"{name}: did not compile or other error:\n{r.stderr[-1500:]}")
            print(f"{name}: {'killed' if killed else 'SURVIVED'}")
            if not killed:
                survivors.append(name)
        finally:
            open(path, "w").write(orig)
    print("survivors:", survivors)


if __name__ == "__main__":
    main()
