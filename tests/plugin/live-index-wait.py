#!/usr/bin/env python3
"""Every live harness case waits for its index or says why not (WP-205,
WP-216).

A live case starts with no index file; the fake engine's start-up `status`
writes it. In desk-view.sh that is `run <case> "" …`; in service-states.sh
a `run <case> <ms> …` without `SELDON_INDEX=`. A case that reads the index
before it is loaded races that write: it passed on an idle host and failed
under load (gates n202m, n194b); service-states.sh's snapshot came after
the start-up engine calls, a narrower gap of the same kind (WP-205 review,
Q3). Each live `run` therefore waits, with `HARNESS_WAIT_INDEX=1` (in
service-states.sh also `HARNESS_UNTIL=status=ok`, which needs the index),
or the line directly above it is an opt-out marker with its reason:

    # no index wait: <reason>

for the cases that wait or settle on purpose, or have no index at all.
This fails on a live `run` with neither, or with both; it prints the
opt-outs. Then it proves itself on mutants of the same file: a wait
removed, a marker removed, and a new live case with neither must each
fail.

Usage: live-index-wait.py [desk-view.sh | service-states.sh ...]
(both when none is named; a file named service-states.sh gets that
harness's rules, any other desk-view.sh's)
"""
import pathlib
import re
import sys

MARKER = re.compile(r"^\s*# no index wait: (\S.*)$")
WAIT = "HARNESS_WAIT_INDEX=1"
UNTIL_OK = "HARNESS_UNTIL=status=ok"


class Desk:
    """desk-view.sh: `run <case> <index file> …`, live with an empty one."""

    run = re.compile(r'^\s*run\s+"?([^"\s]+)"?\s+""\s')
    waits = (WAIT,)
    new_live = '\nrun mutant-live "" 1920x1080 "summon;view" HOME="$work/home-mutant-live"\n'

    @staticmethod
    def live(stmt):
        return True


class Service:
    """service-states.sh: `run <case> <ms> …`, live without SELDON_INDEX."""

    run = re.compile(r"^\s*run\s+(\S+)\s+\d+\s")
    waits = (WAIT, UNTIL_OK)
    new_live = ('\nrun mutant-live 2500 PATH="$fake_path" HOME="$work/home-mutant-live" '
                'FAKE_SELDON_FIXTURE="$fx/index.sample.json"\n')

    @staticmethod
    def live(stmt):
        return re.search(r"(^|\s)SELDON_INDEX=", stmt) is None


def check(text, kind=Desk):
    """Returns (errors, waiting case names, [(case, reason)] opt-outs)."""
    lines = text.split("\n")
    errors, waits, optouts = [], [], []
    for i, line in enumerate(lines):
        m = kind.run.match(line)
        if not m:
            continue
        name = m.group(1)
        stmt, j = line, i
        while lines[j].rstrip().endswith("\\") and j + 1 < len(lines):
            j += 1
            stmt += "\n" + lines[j]
        if not kind.live(stmt):
            continue
        marker = MARKER.match(lines[i - 1]) if i > 0 else None
        waits_here = any(re.search(r"(^|\s)" + re.escape(w) + r"(\s|$)", stmt) for w in kind.waits)
        if waits_here and marker:
            errors.append(f"line {i + 1}: {name} has {WAIT} and an opt-out marker")
        elif waits_here:
            waits.append(name)
        elif marker:
            optouts.append((name, marker.group(1)))
        else:
            errors.append(f"line {i + 1}: live case {name} has no {WAIT} and no '# no index wait: <reason>' above it")
    return errors, waits, optouts


def first_live_with(text, kind, setting):
    """The name of the first live case whose `run` holds `setting` (and no
    other wait), and the offset of ` <setting>` in `text`, or None."""
    lines = text.split("\n")
    starts = [0]
    for line in lines:
        starts.append(starts[-1] + len(line) + 1)
    for i, line in enumerate(lines):
        m = kind.run.match(line)
        if not m:
            continue
        j = i
        while lines[j].rstrip().endswith("\\") and j + 1 < len(lines):
            j += 1
        stmt = "\n".join(lines[i:j + 1])
        hit = stmt.find(" " + setting)
        if kind.live(stmt) and hit >= 0 and sum(f" {w}" in stmt for w in kind.waits) == 1:
            return m.group(1), starts[i] + hit
    return None


def self_test(text, kind, waits, optouts):
    """Each mutant must fail, naming the case it broke."""
    failed = []
    mutants = []
    for setting in kind.waits:
        found = first_live_with(text, kind, setting)
        if found:
            name, at = found
            mutant = text[:at] + text[at + 1 + len(setting):]
            mutants.append((f"a wait removed ({setting})", mutant, name))
    first_marker = re.search(r"^\s*# no index wait: .*\n", text, re.M)
    if first_marker:
        mutants.append(("a marker removed", text[:first_marker.start()] + text[first_marker.end():], optouts[0][0]))
    mutants.append(("a new live case with neither", text + kind.new_live, "mutant-live"))
    if len(mutants) < len(kind.waits) + 2:
        failed.append(f"a mutant could not be made ({len(mutants)} of {len(kind.waits) + 2})")
    for label, mutant, name in mutants:
        errors, _, _ = check(mutant, kind)
        if not any(f" {name} " in e for e in errors):
            failed.append(f"mutant not caught ({label}, {name})")
        else:
            print(f"ok   mutant caught: {label} ({name})")
    return failed


def check_file(path):
    """Checks one harness script and proves the check on its mutants;
    returns the number of problems."""
    kind = Service if path.name == "service-states.sh" else Desk
    text = path.read_text(encoding="utf-8")
    errors, waits, optouts = check(text, kind)
    for name, reason in optouts:
        print(f"opt-out {path.name} {name}: {reason}")
    for e in errors:
        print(f"FAIL {path.name} {e}")
    if not waits or not optouts:
        errors.append(f"found no waiting case or no opt-out: the pattern no longer matches {path.name}")
        print(f"FAIL {errors[-1]}")
    else:
        errors += self_test(text, kind, waits, optouts)
    print(f"live-index-wait: {path.name}: {len(waits)} live cases wait, {len(optouts)} opt out, {len(errors)} problems")
    return len(errors)


def main():
    root = pathlib.Path(__file__).resolve().parent
    paths = [pathlib.Path(a) for a in sys.argv[1:]] or [root / "desk-view.sh", root / "service-states.sh"]
    problems = sum(check_file(p) for p in paths)
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
