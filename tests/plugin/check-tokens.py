#!/usr/bin/env python3
"""Check Style/Color/Border/Util references in plugin QML against the shell.

qmllint types the shell's nested token objects (Style.font, Color.popups) as
plain QObject, so the `just qmllint` gate demotes missing-property to info and
cannot see a typo like `Style.font.bodySmal` (SPEC-PLUGIN §9). This reads the
singletons in $OMARCHY_PATH/shell/Commons and fails on any reference to a
member they do not declare.

Two house rules of SPEC-PLUGIN §7 (WP-177) need no shell and run with
`--rules` everywhere (`just plugin-test`, CI included), and with the shell
check on the dev host:
  - no `Color.muted` as the colour of a Text (also not through a colour
    property set to it): text takes Tone's `dim` (components/Tone.qml);
  - `Util.alpha(…, <number>)` only where ALPHA_ALLOWED lists it: fills and
    borders come from Omarchy's state tokens, text and lines from Tone.

Usage: check-tokens.py <shell dir> <file.qml>...
       check-tokens.py --rules <file.qml>...
"""
import pathlib
import re
import sys

SINGLETONS = ("Style", "Color", "Border", "Util")
MEMBER = re.compile(r"^\s*(?:readonly\s+|required\s+|default\s+)*property\s+[\w<>.]+\s+(\w+)|^\s*function\s+(\w+)\s*\(|^\s*signal\s+(\w+)")
NESTED = re.compile(r"^\s*(?:readonly\s+)?property\s+QtObject\s+(\w+)\s*:\s*QtObject\s*\{")
REF = re.compile(r"\b(" + "|".join(SINGLETONS) + r")\.(\w+)(?:\.(\w+))?")


# Literal alphas that are data colours, not states or text: the Prime
# Radiant's charts and the graph draw the theme's roles at opacities
# (SPEC-PLUGIN §7: "charts use accent, foreground at opacities"; the graph's
# shapes and opacities tell kinds apart). Path relative to plugin/ → alphas.
ALPHA_ALLOWED = {
    "components/overlay/Series.qml": {"0.7"},
    "components/overlay/Timeline.qml": {"0.4", "0.7"},
    "components/overlay/RiskDonut.qml": {"0.3", "0.6"},
    "components/overlay/DriftBars.qml": {"0.45"},
    # the Timeline legend's snapshot marker, in Timeline.qml's colour
    "sections/Radiant.qml": {"0.7"},
    "components/graph/GraphCanvas.qml": {"0.14", "0.22", "0.3", "0.42", "0.5", "0.6", "0.65", "0.72", "0.9"},
}
TEXT_TYPES = {"Text"}
INLINE = re.compile(r"\bcomponent\s+(\w+)\s*:\s*(\w+)\s*\{")
TOKEN = re.compile(r"(?:\bcomponent\s+\w+\s*:\s*)?\b(?P<element>[A-Z]\w*)\s*\{|(?P<key>(?<![.\w])color\s*:)|\{|\}")
MUTED_PROPERTY = re.compile(r"\bproperty\s+color\s+(\w+)\s*:\s*Color\.muted\b")
ALPHA = "Util.alpha("


def alpha_literals(line):
    """The second arguments of Util.alpha(…) calls on a line that are plain numbers."""
    out = []
    start = line.find(ALPHA)
    while start != -1:
        i, depth, comma = start + len(ALPHA), 1, -1
        while i < len(line) and depth:
            ch = line[i]
            if ch in "([":
                depth += 1
            elif ch in ")]":
                depth -= 1
            elif ch == "," and depth == 1:
                comma = i
            i += 1
        if depth == 0 and comma != -1:
            arg = line[comma + 1:i - 1].strip()
            if re.fullmatch(r"[0-9]*\.?[0-9]+", arg):
                out.append(arg)
        start = line.find(ALPHA, i)
    return out


def house_rules(path, plugin_root):
    """Errors of the two SPEC-PLUGIN §7 rules in one QML file."""
    errors = []
    try:
        rel = path.resolve().relative_to(plugin_root.resolve()).as_posix()
    except ValueError:
        rel = path.as_posix()
    lines = [strip_comment(l) for l in path.read_text(encoding="utf-8").splitlines()]
    text_types = set(TEXT_TYPES)
    for line in lines:
        for m in INLINE.finditer(line):
            if m.group(2) in text_types:
                text_types.add(m.group(1))
    stack, in_color, color_of = [], False, ""
    for number, line in enumerate(lines, 1):
        stripped = line.strip()
        # a colour binding that goes on over the next lines (`? …`, `: …`)
        if in_color and not stripped.startswith(("?", ":")):
            in_color = False
        if in_color and color_of in text_types and "Color.muted" in line:
            errors.append(f"{path}:{number}: Color.muted as the colour of a {color_of}; text takes Tone's dim (SPEC-PLUGIN §7)")
        # walk the line: element braces and `color:` keys, in order
        for m in TOKEN.finditer(line):
            if m.group(0) == "}":
                if stack:
                    stack.pop()
            elif m.group("key"):
                color_of = next((e for e in reversed(stack) if e), "")
                in_color = True
                value = line[m.end():]
                value = value[:value.find("}")] if "}" in value else value
                if color_of in text_types and "Color.muted" in value:
                    errors.append(f"{path}:{number}: Color.muted as the colour of a {color_of}; text takes Tone's dim (SPEC-PLUGIN §7)")
            else:
                stack.append(m.group("element") or "")
        m = MUTED_PROPERTY.search(line)
        if m:
            errors.append(f"{path}:{number}: colour property {m.group(1)} set to Color.muted; text takes Tone's dim (SPEC-PLUGIN §7)")
        for value in alpha_literals(line):
            if value not in ALPHA_ALLOWED.get(rel, set()):
                errors.append(f"{path}:{number}: Util.alpha(…, {value}): a literal alpha; use Omarchy's state tokens or Tone, "
                              "or list a data colour in ALPHA_ALLOWED (SPEC-PLUGIN §7)")
    return errors


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
    plugin_root = pathlib.Path(__file__).resolve().parent.parent.parent / "plugin"
    if len(sys.argv) >= 3 and sys.argv[1] == "--rules":
        errors = [e for name in sys.argv[2:] for e in house_rules(pathlib.Path(name), plugin_root)]
        for error in errors:
            print(error, file=sys.stderr)
        if errors:
            return 1
        print(f"tokens: house rules ok ({len(sys.argv) - 2} files)")
        return 0
    if len(sys.argv) < 3:
        print("\n".join(__doc__.strip().splitlines()[-2:]), file=sys.stderr)
        return 2
    commons = pathlib.Path(sys.argv[1]) / "Commons"
    tables = {name: declared(commons / f"{name}.qml") for name in SINGLETONS}
    errors = []
    checked = 0
    for name in sys.argv[2:]:
        path = pathlib.Path(name)
        errors.extend(house_rules(path, plugin_root))
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
    print(f"tokens: ok ({checked} references; house rules ok)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
