#!/usr/bin/env python3
"""Every live desk case waits for its index or says why not (WP-205).

A live case (`run <case> "" …` in desk-view.sh) starts with no index file;
the fake engine's start-up `status` writes it. A case that reads the index
from its first step races that write: it passed on an idle host and failed
under load (gates n202m, n194b). Each live `run` therefore carries
`HARNESS_WAIT_INDEX=1`, or the line directly above it is an opt-out
marker with its reason:

    # no index wait: <reason>

for the cases that wait or settle on purpose, or have no index at all.
This fails on a live `run` with neither, or with both; it prints the
opt-outs. Then it proves itself on mutants of the same file: a wait
removed, a marker removed, and a new live case with neither must each
fail.

Usage: live-index-wait.py [desk-view.sh]
"""
import pathlib
import re
import sys

RUN = re.compile(r'^\s*run\s+"?([^"\s]+)"?\s+""\s')
MARKER = re.compile(r"^\s*# no index wait: (\S.*)$")
WAIT = "HARNESS_WAIT_INDEX=1"


def check(text):
    """Returns (errors, waiting case names, [(case, reason)] opt-outs)."""
    lines = text.split("\n")
    errors, waits, optouts = [], [], []
    for i, line in enumerate(lines):
        m = RUN.match(line)
        if not m:
            continue
        name = m.group(1)
        stmt, j = line, i
        while lines[j].rstrip().endswith("\\") and j + 1 < len(lines):
            j += 1
            stmt += "\n" + lines[j]
        marker = MARKER.match(lines[i - 1]) if i > 0 else None
        waits_here = re.search(r"(^|\s)" + re.escape(WAIT) + r"(\s|$)", stmt) is not None
        if waits_here and marker:
            errors.append(f"line {i + 1}: {name} has {WAIT} and an opt-out marker")
        elif waits_here:
            waits.append(name)
        elif marker:
            optouts.append((name, marker.group(1)))
        else:
            errors.append(f"line {i + 1}: live case {name} has no {WAIT} and no '# no index wait: <reason>' above it")
    return errors, waits, optouts


def self_test(text, waits, optouts):
    """Each mutant must fail, naming the case it broke."""
    failed = []
    first_wait = re.search(r" " + re.escape(WAIT), text)
    mutants = []
    if first_wait:
        mutants.append(("a wait removed", text[:first_wait.start()] + text[first_wait.end():], waits[0]))
    first_marker = re.search(r"^\s*# no index wait: .*\n", text, re.M)
    if first_marker:
        mutants.append(("a marker removed", text[:first_marker.start()] + text[first_marker.end():], optouts[0][0]))
    mutants.append(("a new live case with neither",
                    text + '\nrun mutant-live "" 1920x1080 "summon;view" HOME="$work/home-mutant-live"\n',
                    "mutant-live"))
    for label, mutant, name in mutants:
        errors, _, _ = check(mutant)
        if not any(f" {name} " in e for e in errors):
            failed.append(f"mutant not caught ({label}, {name})")
        else:
            print(f"ok   mutant caught: {label} ({name})")
    return failed


def main():
    root = pathlib.Path(__file__).resolve().parent
    path = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else root / "desk-view.sh"
    text = path.read_text(encoding="utf-8")
    errors, waits, optouts = check(text)
    for name, reason in optouts:
        print(f"opt-out {name}: {reason}")
    for e in errors:
        print(f"FAIL {e}")
    if not waits or not optouts:
        errors.append("found no waiting case or no opt-out: the pattern no longer matches desk-view.sh")
        print(f"FAIL {errors[-1]}")
    else:
        errors += self_test(text, waits, optouts)
    print(f"live-index-wait: {len(waits)} live cases wait, {len(optouts)} opt out, {len(errors)} problems")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
