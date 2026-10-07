#!/usr/bin/env python3
"""Mutation check for the guard (WP-130): each mutant breaks one rule of
scripts/guard.py; the table scripts/guard-test.sh must fail for every one.
A mutant that survives means a rule has no row that proves it."""

import os
import shutil
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor

HERE = os.path.dirname(os.path.abspath(__file__))

# (what the mutant drops, text in guard.py, replacement)
MUTANTS = [
    ("bash -c recursion",
     'self.run_script(rest[0], dict(scope), ctx.but(stdin=stdin), f"{name} -c")', "pass"),
    ("$(…) and backticks are checked", 'if kind in ("cmd", "proc"):', 'if kind in ("proc",):'),
    ("<(…) is checked", 'if kind in ("cmd", "proc"):', 'if kind in ("cmd",):'),
    ("a heredoc fed to a shell is code", "for text in stdin[1]:", "for text in []:"),
    ("a shell reading a pipe is blocked",
     """raise Unsure(f"{name} reads commands from a pipe; use a heredoc or {name} -c '…'")""", "return"),
    ("unquoted heredoc bodies run $(…)", "self.walk_parts(r.parts, scope, ctx)", "pass"),
    ("env is unwrapped", "return self.w_env(args, scope, ctx, stdin, node)", "return None"),
    ("eval strings are code", 'return self.run_script(" ".join(args), scope, ctx.but(stdin=stdin), "eval")',
     "return None"),
    ("find -exec is checked", "self.check_argv(sub, scope, ctx, None, False, node)", "pass"),
    ("the remote command of ssh is checked",
     'self.run_script(" ".join(rest), rscope, rctx, "ssh remote command", remote_top=True)', "pass"),
    ("the listed-host bypass needs the gate", "if self.host_gate and not ctx.remote and host in self.hosts:",
     "if not ctx.remote and host in self.hosts:"),
    ("the host gate matches the whole command", "host_regex(h).fullmatch(whole)", "host_regex(h).match(whole)"),
    ("parse errors are blocked", 'raise Unsure(f"cannot parse: {e}")', "return"),
    ("a computed command name is blocked",
     'raise Unsure("the command name is computed (a variable, $(…) or a glob); write it literally")',
     "return field"),
    ("pacman read-only queries pass", 'if name == "pacman" and pacman_read_only(args):', "if False:"),
    ("systemctl read-only verbs pass", 'if name == "systemctl" and systemctl_read_only(args):', "if False:"),
    ("omarchy --help passes", 'if "--help" in before or "-h" in before:', "if False:"),
    ("the theme sweep over ssh passes", "if theme and ctx.remote and ctx.ssh_single and ctx.r_single:",
     "if False:"),
    ("the plugin dir is writable", "if definitely_inside(comps, root + PLUGIN_DIR):", "if False:"),
    ("Seldon's config dir over ssh is writable", 'if seldon_ok and definitely_inside(comps, root + ["seldon"]):',
     "if False:"),
    ("variables are tracked (scratch HOME)", "for v in scope.get(name, [None]):", "for v in [None]:"),
    ("a prefix assignment does not change its own expansion",
     "for argv in self.expand_words(cmd.words, scope, ctx):", "for argv in self.expand_words(cmd.words, env, ctx):"),
    (".. is resolved", "            if out:\n                out.pop()", "            if False:\n                out.pop()"),
    ("a recursive delete above a protected dir is blocked",
     "        if not comp_match(c, base[k]):\n            return False\n    return True\n\n\ndef definitely_inside",
     "        if not comp_match(c, base[k]):\n            return False\n    return False\n\n\ndef definitely_inside"),
    ("~/Seldon and ~/.local/state/seldon are protected",
     'for d in (self.home_comps + ["Seldon"], self.home_comps + [".local", "state", "seldon"]):', "for d in ():"),
    # round 2
    ("hook input over 256 KB is blocked",
     "raw = sys.stdin.buffer.read(MAX_INPUT + 1)\n    if len(raw) > MAX_INPUT:",
     "raw = sys.stdin.buffer.read()\n    if False:"),
    ("more than 256 variables are blocked", 'if sum(1 for k in scope if not k.startswith(".")) > MAX_VARS:',
     "if False:"),
    ("the time budget", "    signal.setitimer(signal.ITIMER_REAL, budget)\n    try:", "    try:"),
    ("GUARD_HOSTS_FILE needs SELDON_TEST_GUARD",
     'if os.environ.get("SELDON_TEST_GUARD") and os.environ.get("GUARD_HOSTS_FILE"):',
     'if os.environ.get("GUARD_HOSTS_FILE"):'),
    ("omarchy plugin enable|disable is blocked", '"clone", "enable", "disable")),', '"clone")),'),
    ("omarchy hook is blocked", '("hook", None)', '("hook", ("install",))'),
    ("omarchy branch / channel set are blocked", '("branch", None), ("channel", ("set",))]', "]"),
    # round 3
    ("trap -- is read", 'targs = args[1:] if args and plain(args[0]) == "--" else args', "targs = args"),
    ("shred -u is a flag", 'operands(args, "ns" if name == "shred" else "")',
     'operands(args, "nsu" if name == "shred" else "")'),
    ("IFS changes fail closed", 'if name == "IFS" and not ifs_ok:', "if False:"),
    ("namerefs are followed", "                        self.namerefs[n] = target", "                        pass"),
    ("${HOME%…} may be the home", "opts.append(dedupe(self.lookup(payload, scope, ctx) + [UNK]))",
     "opts.append([UNK])"),
    ("a failed cd keeps the old directory", '            new[".cdfail"] = old  # if the cd fails, the old directory stays',
     "            pass"),
    ("cd - goes to OLDPWD", 'targets = self.lookup("OLDPWD", scope, ctx)', "targets = [UNK]"),
    ("a function runs at its call", "elif name in self.functions:", "elif False:"),
    ("writes follow links", "for comps in [c for full in fulls for c in self.via_links(normalize(full))]:",
     "for comps in [normalize(full) for full in fulls]:"),
    ("ssh to this machine gets the local rules", 'if wild(h) or h in local_names() or h.startswith("127."):',
     "if False:"),
    ("an Omarchy script path does not run",
     'if self.is_omarchy_path(first, scope, ctx) and not name.startswith("omarchy"):', "if False:"),
    ("a shell or source does not run an Omarchy script",
     '        if self.is_omarchy_path(path, scope, ctx):\n            raise Blocked',
     '        if False:\n            raise Blocked'),
    ("bash -n runs nothing", "if noexec and not cmode:", "if False:"),
    ("git writes its work tree", "elif verb in GIT_MUTATING:", "elif False:"),
    ("tar -x writes its directory", 'out += [(d.rstrip("/") + "/" + UNK, False) for d in (dirs or ["."])]',
     "out += []"),
    ("curl -o writes its file", 'if o in ("-o", "--output") and plain(v) != "-":', "if False:"),
    ("patch writes its file",
     'out.append((join_path(d, ops[0]), False) if ops else (d.rstrip("/") + "/" + UNK, False))',
     'out.append(("/tmp/x", False))'),
    ("script reads its shell's stdin", 'self.stdin_script(stdin, dict(scope), ctx, "script")  # its shell reads stdin',
     "pass"),
    ("aliases fail closed", 'raise Unsure("aliases change what a command name runs; the guard does not model them")',
     "pass"),
    ("hyprctl dispatch exec is checked", "return True, self.w_hyprctl(args, scope, ctx, stdin, node)",
     "return True, None"),
    ("tmux commands are checked", "return True, self.w_tmux(args, scope, ctx, stdin, node)", "return True, None"),
    ("terminal -e is checked", "if name in TERMINALS:", "if False:"),
    ("taskset is unwrapped", 'if name == "taskset":', "if False:"),
    ("rsync -e / scp -S are checked", "            self.w_copy(name, args)", "            pass"),
    ("git -c settings that run programs", "if GIT_EXEC_CONFIG.fullmatch(key):", "if False:"),
    ("git with GIT_SSH_COMMAND/EDITOR set", 'raise Unsure(f"git with {k} set in the command can run a program")',
     "pass"),
    ("the net for unknown wrappers", "if name not in DATA_SINKS:\n            self.net(name, args)",
     "if False:\n            self.net(name, args)"),
    ("loginctl read-only verbs pass", 'if name == "loginctl" and loginctl_read_only(args):', "if False:"),
    ('eval "$(ssh-agent -s)" passes', "if direct and self.is_ssh_agent_eval(node):", "if False:"),
    ("the round-2 binaries are blocked", '"hook", "dev-link", "branch",\n                      "channel-set")',
     '"hook-install", "dev-link")'),
]


def run_mutant(src, root, k, what, old, new):
    if src.count(old) != 1:
        return False, f"BROKEN   {what}: the mutated text occurs {src.count(old)} times in guard.py"
    tmp = os.path.join(root, str(k))
    os.mkdir(tmp)
    shutil.copy(os.path.join(HERE, "guard.sh"), tmp)
    with open(os.path.join(tmp, "guard.py"), "w", encoding="utf-8") as f:
        f.write(src.replace(old, new))
    env = dict(os.environ, GUARD_SH=os.path.join(tmp, "guard.sh"), GUARD_TEST_FAILFAST="1", GUARD_TEST_QUIET="1")
    run = subprocess.run(["bash", os.path.join(HERE, "guard-test.sh")], env=env, capture_output=True, text=True)
    if run.returncode == 0:
        return False, f"SURVIVED {what}"
    first = next((line for line in run.stdout.splitlines() if line.startswith("FAIL")), "(no FAIL row)")
    return True, f"killed   {what} :: {first[:110]}"


def main():
    src = open(os.path.join(HERE, "guard.py"), encoding="utf-8").read()
    with tempfile.TemporaryDirectory(prefix="guard-mutants.") as root:
        # half the cores: an overloaded host would push rows past the guard's time budget
        with ThreadPoolExecutor(max_workers=max(2, (os.cpu_count() or 4) // 2)) as pool:
            results = list(pool.map(lambda a: run_mutant(src, root, *a),
                                    [(k, *m) for k, m in enumerate(MUTANTS)]))
    for _, line in results:
        print(line)
    print(f"mutants: {len(MUTANTS)}")
    return 0 if all(ok for ok, _ in results) else 1


if __name__ == "__main__":
    sys.exit(main())
