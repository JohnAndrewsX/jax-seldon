#!/usr/bin/env python3
"""WP-135 plugin mutants: each must fail model.test.js or the trimmed desk harness."""
import subprocess, sys
root, trim = sys.argv[1], sys.argv[2]
DQ = "plugin/sections/Decisions.qml"; SQ = "plugin/Service.qml"; MJ = "plugin/Model.js"
H = ["bash", trim]; U = ["node", "tests/plugin/model.test.js"]
M = [
 ("P1 Accept runs on the first click", DQ,
  "    if (!root.arm.press(root.armId, Model.acceptArmHint(row.id))) return false\n",
  "    root.arm.press(root.armId, Model.acceptArmHint(row.id))\n", H),
 ("P2 a new selection keeps it armed", DQ, "  onSelectedIdChanged: root.disarm()\n", "", H),
 ("P3 Accept enabled without a writer", DQ,
  "      enabled: a.enabled && (!a.write || (root.canWrite && !root.accepting))", "      enabled: a.enabled", H),
 ("P4 the answer goes to decideResult", SQ,
  "      root.acceptResult = result\n    } else if (args[0] === \"decide\") {",
  "      root.decideResult = result\n    } else if (args[0] === \"decide\") {", H),
 ("P5 an accepted decision opens in the editor", SQ,
  "if (args[0] === \"decide\" && args[1] !== \"accept\" && result", "if (args[0] === \"decide\" && result", H),
 ("P6 the answer on every decision", DQ,
  "root.acceptResult.decisionId === root.current.id", "true", H),
 ("P7 accept without --json admitted", MJ,
  "a[1] === \"accept\" && DECISION_ID.test(a[2]) && json) return \"\"", "a[1] === \"accept\" && DECISION_ID.test(a[2])) return \"\"", U),
 ("P8 accept takes any id", MJ,
  "a[1] === \"accept\" && DECISION_ID.test(a[2]) && json", "a[1] === \"accept\" && json", U),
 ("P9 already ignored", MJ, "  var already = !!data && data.already === true\n", "  var already = false\n", U),
 ("P10 the label stays Accept when armed", DQ,
  "label: a.id === \"accept\" && root.armed ? \"Confirm accept\" : a.label,", "label: a.label,", H),
 ("P11 a key keeps it armed", "plugin/Desk.qml",
  "            if (!root.arm.touched) root.arm.disarm()\n", "", H),
]
surv = []
for name, path, old, new, cmd in M:
    p = f"{root}/{path}"; orig = open(p).read()
    if orig.count(old) != 1:
        print(f"{name}: pattern found {orig.count(old)} times"); surv.append(name); continue
    try:
        open(p, "w").write(orig.replace(old, new))
        if cmd is H:
            subprocess.run(["bash", sys.argv[3], root, trim], check=True)
        r = subprocess.run(cmd, cwd=root, capture_output=True, text=True)
        killed = r.returncode != 0
        print(f"{name}: {'killed' if killed else 'SURVIVED'}", flush=True)
        if killed:
            print("   " + "\n   ".join([l for l in r.stdout.splitlines() if l.startswith("FAIL")][:3] or r.stdout.splitlines()[-2:]))
        else: surv.append(name)
    finally:
        open(p, "w").write(orig)
print("survivors:", surv)
