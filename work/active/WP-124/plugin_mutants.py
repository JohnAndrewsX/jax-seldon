#!/usr/bin/env python3
"""WP-124b plugin mutants: each patch must make its check fail. Restores
every file.

Run from anywhere: python3 work/active/WP-124/plugin_mutants.py [P1 P3 …]
"node" runs tests/plugin/model.test.js; "desk" runs the triage scenarios of
tests/plugin/desk-view.sh alone (a copy made in a temp dir: its setup, its
helpers, the "Bulk triage" block). Needs what desk-view.sh needs.
"""
import os
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
SVC = "plugin/Service.qml"
TD = "plugin/components/desk/TriageDetail.qml"
CL = "plugin/sections/Changelog.qml"
MJ = "plugin/Model.js"

M = [
    ("P1 Q3 the service's id guard never fires", SVC,
     "    if (current !== id) return fail(", "    if (false) return fail(", "desk"),
    ("P2 Q4 a result shown for any proposal", TD,
     "    && root.service.triageResult.proposalId === root.seenId ? root.service.triageResult : null",
     "    ? root.service.triageResult : null", "desk"),
    ("P3 Q5 StyledText", TD,
     "component Line: Text {\n    width: parent ? parent.width : 0\n    textFormat: Text.PlainText",
     "component Line: Text {\n    width: parent ? parent.width : 0\n    textFormat: Text.StyledText", "node"),
    ("P4 Q6 an evidence text elided on the left", TD,
     "          wrapMode: Text.WrapAtWordBoundaryOrAnywhere\n",
     "          wrapMode: Text.WrapAtWordBoundaryOrAnywhere\n          elide: Text.ElideLeft\n", "node"),
    ("P5 the author regex unanchored", MJ,
     "  var m = /^by (.+?) · /.exec(String(text || \"\"))",
     "  var m = /by (.+?) · /.exec(String(text || \"\"))", "node"),
    ("P6 itemOutcome ignores refused", MJ,
     "  var lists = [[\"done\", result.done], [\"skipped\", result.skipped], [\"refused\", result.refused]]",
     "  var lists = [[\"done\", result.done], [\"skipped\", result.skipped]]", "node"),
    ("P7 the opened id is not kept", CL,
     "    root.seenProposalId = root.triageView.id\n", "", "desk"),
    ("P8 a replaced proposal keeps its bar", TD,
     "    if (root.seen.state === \"replaced\")\n", "    if (false)\n", "desk"),
    ("P9 Discard in one click", TD,
     "      if (root.arm && root.arm.press(root.discardKey,",
     "      if (true || root.arm.press(root.discardKey,", "desk"),
    ("P10 unknown properties accepted", MJ,
     "  for (var k in obj) if (keys.indexOf(k) === -1) return false\n",
     "", "node"),
    ("P11 no size limit", MJ,
     "  if (raw.length > PROPOSAL_TEXT_MAX) return null\n", "", "node"),
    ("P12 the items built while hidden", TD,
     "      active: root.shown && !!root.proposal\n", "      active: !!root.proposal\n", "desk"),
]


def triage_harness(tmp):
    """desk-view.sh cut to its setup, its helpers and the triage block."""
    lines = (ROOT / "tests/plugin/desk-view.sh").read_text().split("\n")
    find = lambda pred: next(i for i, l in enumerate(lines) if pred(l))
    first_run = find(lambda l: l.startswith("run thresholds"))
    helpers = [i for i, l in enumerate(lines) if l.startswith("q() ")][-1]
    sel = find(lambda l: l.startswith("sel() "))
    block = find(lambda l: l.startswith("# Bulk triage and Ask agent"))
    shots = find(lambda l: l.startswith("# Offscreen renders in three themes"))
    head = [f"root={ROOT}" if l.startswith("root=") else l for l in lines[:first_run - 7]]
    body = head + lines[helpers:sel + 1] + lines[block - 1:shots - 1]
    body.append('echo "desk-triage: $pass passed, $fail failed"; ((fail == 0))')
    path = Path(tmp) / "desk-triage.sh"
    path.write_text("\n".join(body) + "\n")
    return path


def check(kind, script):
    if kind == "node":
        r = subprocess.run(["node", "tests/plugin/model.test.js"], capture_output=True, text=True)
    else:
        r = subprocess.run(["bash", str(script)], capture_output=True, text=True, timeout=900)
    return r.returncode != 0


def main():
    only = sys.argv[1:]
    os.chdir(ROOT)
    survivors = []
    with tempfile.TemporaryDirectory() as tmp:
        script = triage_harness(tmp)
        assert not check("node", script), "model.test.js fails without a mutant"
        for name, path, old, new, kind in M:
            if only and name.split()[0] not in only:
                continue
            orig = open(path).read()
            if orig.count(old) != 1:
                print(f"{name}: pattern found {orig.count(old)} times")
                survivors.append(name)
                continue
            try:
                open(path, "w").write(orig.replace(old, new))
                killed = check(kind, script)
                print(f"{name}: {'killed' if killed else 'SURVIVED'}")
                if not killed:
                    survivors.append(name)
            finally:
                open(path, "w").write(orig)
    print("survivors:", survivors)


if __name__ == "__main__":
    main()
