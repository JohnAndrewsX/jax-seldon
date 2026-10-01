#!/usr/bin/env python3
"""Check Style/Color/Border/Util references in plugin QML against the shell.

qmllint types the shell's nested token objects (Style.font, Color.popups) as
plain QObject, so the `just qmllint` gate demotes missing-property to info and
cannot see a typo like `Style.font.bodySmal` (SPEC-PLUGIN §9). This reads the
singletons in $OMARCHY_PATH/shell/Commons and fails on any reference to a
member they do not declare.

Usage: check-tokens.py <shell dir> <file.qml>...
"""
import pathlib
import re
import sys

SINGLETONS = ("Style", "Color", "Border", "Util")
MEMBER = re.compile(r"^\s*(?:readonly\s+|required\s+|default\s+)*property\s+[\w<>.]+\s+(\w+)|^\s*function\s+(\w+)\s*\(|^\s*signal\s+(\w+)")
NESTED = re.compile(r"^\s*(?:readonly\s+)?property\s+QtObject\s+(\w+)\s*:\s*QtObject\s*\{")
REF = re.compile(r"\b(" + "|".join(SINGLETONS) + r")\.(\w+)(?:\.(\w+))?")


def strip_comment(line):
    # Good enough for the shell's singletons and our plugin: no "//" in strings
    # we care about except URLs, which never contain a token reference.
    return re.sub(r"(?<!:)//.*$", "", line)


def declared(path):
    """Top-level members and the members of nested QtObject tokens."""
    top, nested = set(), {}
    depth, current, current_depth = 0, None, 0
    for raw in path.read_text(encoding="utf-8").splitlines():
        line = strip_comment(raw)
        if depth == 1:
            m = NESTED.match(line)
            if m:
                current = m.group(1)
                current_depth = 2
                top.add(current)
                nested.setdefault(current, set())
            else:
                m = MEMBER.match(line)
                if m:
                    top.add(next(g for g in m.groups() if g))
        elif current and depth == current_depth:
            m = MEMBER.match(line)
            if m:
                nested[current].add(next(g for g in m.groups() if g))
        depth += line.count("{") - line.count("}")
        if current and depth < current_depth:
            current = None
    return top, nested


def main():
    if len(sys.argv) < 3:
        print(__doc__.strip().splitlines()[-1], file=sys.stderr)
        return 2
    commons = pathlib.Path(sys.argv[1]) / "Commons"
    tables = {name: declared(commons / f"{name}.qml") for name in SINGLETONS}
    errors = []
    checked = 0
    for name in sys.argv[2:]:
        path = pathlib.Path(name)
        for number, raw in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            for m in REF.finditer(strip_comment(raw)):
                singleton, member, sub = m.groups()
                top, nested = tables[singleton]
                checked += 1
                if member not in top:
                    errors.append(f"{path}:{number}: {singleton}.{member} is not declared in Commons/{singleton}.qml")
                elif sub and member in nested and sub not in nested[member]:
                    errors.append(f"{path}:{number}: {singleton}.{member}.{sub} is not declared in Commons/{singleton}.qml")
    for error in errors:
        print(error, file=sys.stderr)
    if errors:
        return 1
    print(f"tokens: ok ({checked} references)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
