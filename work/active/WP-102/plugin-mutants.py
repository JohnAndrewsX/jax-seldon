#!/usr/bin/env python3
"""WP-102b plugin mutants: run from the checkout with the trimmed harness dir as argument
(tests/plugin/desk-view.sh cut to the import scenarios; see HANDOVER ## 102b)."""
import os, subprocess, sys
WT = os.getcwd()
S = sys.argv[1]
M = [
 ("intentReviewed always true", "plugin/Model.js", "  return !!c && isObject(shown) && !shown.pending && shown.ok === true && shown.caseId === c.id", "  return true"),
 ("bar enables Start without the review", "plugin/sections/Work.qml", " && (!a.review || root.reviewed)", ""),
 ("Enter falls to Drop on an imported case", "plugin/Model.js", "  if (review(enter)) enter = \"\"\n", ""),
 ("press() ignores the review", "plugin/sections/Work.qml", "    if (action.review && !root.reviewed) return false\n", ""),
 ("validateArgs skips the path check", "plugin/Model.js", " && importPathError(free[0]) === \"\"\n      ? \"\" : \"import must be", "\n      ? \"\" : \"import must be"),
]
for name, f, a, b in M:
    p = os.path.join(WT, f); orig = open(p).read()
    if orig.count(a) != 1: print(name, "PATTERN", orig.count(a)); continue
    try:
        open(p, "w").write(orig.replace(a, b))
        m = subprocess.run(["node", "tests/plugin/model.test.js"], cwd=WT, capture_output=True, text=True)
        d = subprocess.run(["bash", S + "/desk-import.sh"], cwd=WT, capture_output=True, text=True, env=dict(os.environ, OUT_DIR=S + "/out"))
    finally:
        open(p, "w").write(orig)
    fails = [l for l in (m.stdout + m.stderr + d.stdout).splitlines() if l.startswith("FAIL")]
    print(name + ": " + ("killed (" + str(len(fails)) + " fails, e.g. " + fails[0][:110] + ")" if fails else "SURVIVED"))
