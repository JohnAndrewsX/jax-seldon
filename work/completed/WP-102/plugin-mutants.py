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
 # round 2
 ("P2 no re-ask on a new index", "plugin/sections/Work.qml", "root.service.showCase(root.reviewKey.split(\"@\")[0], true)", "root.service.showCase(root.reviewKey.split(\"@\")[0], false)"),
 ("P4 the dry run ignores the area", "plugin/components/desk/ImportForm.qml", " && result.area === root.area", ""),
 ("P5 re-ask keeps the old Intent marked reviewed", "plugin/Service.qml", "{ ok: true, pending: true, text: \"Asking the engine again…\"", "{ ok: true, pending: false, text: \"Asking the engine again…\""),
 ("N2 re-ask clears the text", "plugin/Service.qml", "caseId: id, intent: c.intent, lines: c.lines,", "caseId: id, intent: \"\", lines: c.lines,"),
 ("B1 truncated counts as reviewed", "plugin/Model.js", "    && shown.truncated !== true && !(count(shown.hidden) > 0)", "    && !(count(shown.hidden) > 0)"),
 ("B2 hidden characters count as reviewed", "plugin/Model.js", "    && shown.truncated !== true && !(count(shown.hidden) > 0)", "    && shown.truncated !== true"),
 ("N3 separators allowed in plugin paths", "plugin/Model.js", "\\u2028-\\u202e", "\\u202a-\\u202e"),
 # stage 2
 ("re-ask while pending dropped", "plugin/Service.qml", "      if (again === true) c.reaskWanted = true\n", ""),
 ("validateArgs skips the path check", "plugin/Model.js", " && importPathError(free[0]) === \"\"\n      ? \"\" : \"import must be", "\n      ? \"\" : \"import must be"),
]
ONLY = sys.argv[2] if len(sys.argv) > 2 else ""
for name, f, a, b in [m for m in M if ONLY in m[0]]:
    p = os.path.join(WT, f); orig = open(p).read()
    if orig.count(a) != 1: print(name, "PATTERN", orig.count(a)); continue
    try:
        open(p, "w").write(orig.replace(a, b))
        m = subprocess.run(["node", "tests/plugin/model.test.js"], cwd=WT, capture_output=True, text=True)
        d = subprocess.run(["bash", S + "/desk-import.sh"], cwd=WT, capture_output=True, text=True,
                           env=dict(os.environ, OUT_DIR=S + "/out"))
    finally:
        open(p, "w").write(orig)
    fails = [l for l in (m.stdout + m.stderr + d.stdout).splitlines() if l.startswith("FAIL")]
    print(name + ": " + ("killed (" + str(len(fails)) + " fails, e.g. " + fails[0][:110] + ")" if fails else "SURVIVED"))
