#!/usr/bin/env python3
"""WP-139 manual mutants: each change must make a test fail.

Run in a scratch copy, never the worktree (round 2b rule): the committed
HEAD is exported with `git archive` into MUTANT_ROOT (default the private
gates folder, on disk), and every mutant is applied and undone there.
Engine mutants run `cargo test --lib` plus the integration tests
`recent_config`, `index` and `preview` with a target dir on disk
(MUTANT_TARGET); plugin mutants run `node tests/plugin/model.test.js`.
Arguments, if any, keep only the mutants whose name contains one of them.
Service.qml and System.qml are covered by the desk harness (`desk-view.sh`
system, system-watch, system-watch-last, system-watch-locked,
system-partial), too slow to run per mutant.
"""
import os, subprocess, sys
from pathlib import Path

# the checkout this script lives in: work/active/WP-139/mutants.py
WT = str(Path(__file__).resolve().parents[3])
GATES = "/home/eandres/Work/johnandrewsx/jax-seldon-private/gates"
ROOT = os.environ.get("MUTANT_ROOT", f"{GATES}/wp139-mutant-root")
TARGET = os.environ.get("MUTANT_TARGET", f"{GATES}/target-wp139")
RECENT = "engine/src/collectors/recent.rs"
SCAN = "engine/src/config_scan.rs"
CMD = "engine/src/commands/config_cmd.rs"
MODEL = "plugin/Model.js"
ENGINE = [
    # the walk (config_scan, the one walker)
    ("breadth-first lost", SCAN, "queue.pop_front()", "queue.pop_back()"),
    ("file links not listed", SCAN, "} else if kind.is_file() || kind.is_symlink() {", "} else if kind.is_file() {"),
    ("folder links entered", SCAN, "            if kind.is_dir() {",
     "            if kind.is_dir() || (kind.is_symlink() && std::fs::metadata(&path).is_ok_and(|m| m.is_dir())) {"),
    ("exclusions ignored", SCAN, "if skip.matches(&path) || excluded(&path) {", "if skip.matches(&path) {"),
    ("skipPaths ignored", SCAN, "if skip.matches(&path) || excluded(&path) {", "if excluded(&path) {"),
    ("keep not asked for folders", SCAN, "if ignored_dir(&name) || !keep(&path) {", "if ignored_dir(&name) {"),
    ("keep not asked before the cut", SCAN, "if modified >= limits.since && keep(&path) {", "if modified >= limits.since {"),
    ("cache folders entered", SCAN, 'lower.contains("cache") ||', ""),
    ("node_modules entered", SCAN, '\n    "node_modules",\n', '\n    "node_modulez",\n'),
    ("history folders entered", SCAN, '\n    "history",\n', '\n    "historz",\n'),
    ("browser profiles entered", SCAN, "PROFILE_MARKERS.iter().any(|m| name == *m)", "false"),
    ("shell.json listed", SCAN, '        || lower == "shell.json"\n', ""),
    ("history.json listed", SCAN, '        || lower == "history.json"\n', ""),
    ("rotated logs listed", SCAN, '        || lower.contains(".log.")\n', ""),
    ("temp files listed", SCAN, '        || lower.contains(".tmp-")\n', ""),
    ("key stores listed", SCAN, '\n    "kdbx",\n', '\n    "kdbz",\n'),
    ("files older than 7 days listed", RECENT, "since: SystemTime::from(now - chrono::Duration::days(DAYS)),",
     "since: SystemTime::UNIX_EPOCH,"),
    ("more than 80 kept", RECENT, "        max_files: MAX_FILES,\n", "        max_files: usize::MAX,\n"),
    ("entry budget ignored", SCAN, "out.entries > limits.max_entries", "false"),
    ("deadline ignored", SCAN, "|| limits.deadline.is_some_and(|d| Instant::now() >= d)", ""),
    ("depth unbounded", SCAN, "if depth + 1 > MAX_DEPTH {", "if false {"),
    ("partial lost on the way", RECENT, "        partial: walked.partial,\n", "        partial: false,\n"),
    ("watch paths not excluded", RECENT, "    let mut out = watched(dirs, config);", "    let mut out = Vec::new();"),
    ("plugin folder not built in", RECENT, "        dirs.home.join(super::plugins::PLUGINS_DIR),\n", ""),
    ("seldon config folder not built in", RECENT,
     '        dirs.home.join(ROOT).join("seldon"),\n        dirs.config_dir(),\n', ""),
    ("a time after the scan kept", RECENT, "    t.with_nanosecond(0).unwrap_or(t).min(now)\n", "    t.with_nanosecond(0).unwrap_or(t)\n"),
    ("times not whole seconds", RECENT, "    t.with_nanosecond(0).unwrap_or(t).min(now)\n", "    t.min(now)\n"),
    ("preview walks $XDG_CONFIG_HOME", "engine/src/commands/preview.rs",
     "config_scan::scan(&ctx.dirs.home.join(config_scan::ROOT), &skip, &limits)",
     "config_scan::scan(&ctx.dirs.xdg_config_home, &skip, &limits)"),
    # the path rules
    ("redaction not asked", RECENT, "        && redactor.redact(&key) == key;", ";"),
    ("format characters pass", RECENT,
     "!key.chars().any(crate::import::bad_path_char)", "!key.chars().any(char::is_control)"),
    ("dot folders pass", RECENT, '.all(|c| !c.is_empty() && c != "." && c != "..")', ".all(|_| true)"),
    ("paths outside ~/.config pass", RECENT, 'let ok = key.starts_with("~/.config/")', 'let ok = key.starts_with("~/")'),
    # the index side
    ("watched since the scan stays", RECENT, "                && !watched.iter().any(|w| path.starts_with(w))\n", ""),
    ("skipped since the scan stays", RECENT, "                && !skipped(&skip, &dirs.home, &path)\n", ""),
    ("old by now stays", RECENT, "DateTime::parse_from_rfc3339(&f.mtime).is_ok_and(|t| t >= since)",
     "DateTime::parse_from_rfc3339(&f.mtime).is_ok()"),
    ("a hand-written path stays", RECENT,
     "shown_path(dirs, redactor, &path).as_deref() == Some(f.path.as_str())", "true"),
    ("folder names not matched by skipPaths", RECENT, "    path.ancestors()\n", "    std::iter::once(path)\n"),
    ("no list in the index", "engine/src/index/mod.rs",
     "built.index.system.recent_config = crate::collectors::recent::shown(", "let _ = crate::collectors::recent::shown("),
    ("an unreadable state file is no warning", RECENT, "            warnings.push(e);\n", ""),
    # capture
    ("scan without the config collector", "engine/src/commands/capture.rs",
     '    if reports.iter().any(|r| r.name == "config" && r.ran) {\n        let excluded',
     "    if true {\n        let excluded"),
    ("the config file listed", "engine/src/commands/capture.rs",
     "            ctx.config_file.clone(),\n            // its rows", "            // its rows"),
    # config watch
    ("outside home accepted", CMD, "if !path.starts_with(&dirs.home) || path == dirs.home {", "if path == dirs.home {"),
    ("home itself accepted", CMD, "if !path.starts_with(&dirs.home) || path == dirs.home {",
     "if !path.starts_with(&dirs.home) {"),
    ("a folder holding own files accepted", CMD, ".find(|o| o.starts_with(&path) || path.starts_with(o))",
     ".find(|o| path.starts_with(o))"),
    ("a path in own files accepted", CMD, ".find(|o| o.starts_with(&path) || path.starts_with(o))",
     ".find(|o| o.starts_with(&path))"),
    ("the logbook is not own", CMD, "    let own = [\n        logbook,\n", "    let _ = logbook;\n    let own = [\n"),
    ("skipPaths accepted", CMD, "if recent::skipped(&skip, &dirs.home, &path) {", "if false {"),
    ("covered not seen", CMD, ".find(|w| dirs.expand_config(w).is_some_and(|w| path.starts_with(w)))", ".find(|_| false)"),
    ("format characters accepted", CMD,
     "if key.chars().any(crate::import::bad_path_char)", "if key.chars().any(char::is_control)"),
    ("long paths accepted", CMD, " || key.chars().count() > SUBJECT_MAX {", " {"),
    ("no rebuild", CMD, "    crate::index::rebuild_if_initialised(ctx);\n", ""),
    ("no lock", CMD, "    let lock = ctx.lock()?;", "    let lock = ();"),
    ("no defaults without a file", CMD, "            let mut config = Config::default();",
     "            let mut config = Config { watch_paths: Vec::new(), ..Config::default() };"),
    ("the defaults hint lost", CMD, '.is_ok_and(|t| t.contains_key("watchPaths"))', ".is_ok()"),
    ("empty array refused", "engine/src/config.rs", "    let at = last_end.unwrap_or(open + 1);", "    let at = last_end?;"),
    # round 3 (review 2: B1, B2, B3, N1)
    ("r3 a link out of the root by ..", SCAN, "            resolved.pop()?;\n", "            resolved.pop();\n"),
    ("r3 a link's target not checked", SCAN,
     "Some(target) if listable_target(config_dir, &target, skip, &excluded) =>", "Some(target) =>"),
    ("r3 a skipped target listed", SCAN, "                skip.matches(p)\n", "                false\n"),
    ("r3 an excluded target listed", SCAN, "                    || excluded(p)\n", "\n"),
    ("r3 an ignored target name listed", SCAN, "!name.is_some_and(|n| ignored_file(&n))", "true"),
    ("r3 a watch path's link not checked", "engine/src/collectors/config.rs",
     "            if !self.system.contains(root) {\n", "            if false {\n"),
    ("r3 a link in a watched folder to a skipped file read", "engine/src/collectors/config.rs",
     "                    Ok(target) if target.is_file() && refusal.is_some() => scan.refused += 1,\n", ""),
    ("r3 link_refusal ignores skipPaths", "engine/src/collectors/config.rs", "                .any(ignored)\n", "                .any(|_| false)\n"),
    ("r3 link_refusal ignores own files", "engine/src/collectors/config.rs",
     ".any(|o| canonical.starts_with(o) || o.starts_with(&canonical))", ".any(|_| false)"),
    ("r3 link_refusal lets out of the home", "engine/src/collectors/config.rs",
     "        Err(_) if outside_home => Some(LinkRefusal::OutsideHome),\n", ""),
    ("r3 config watch skips the link check", CMD, "    if let Some(why) = refusal {", "    if let Some(why) = None::<LinkRefusal> {"),
    ("r3 the list misses U+2028", RECENT, "!key.chars().any(crate::import::bad_path_char)",
     "!key.chars().any(|c| c.is_control() || crate::redact::is_invisible(c))"),
    ("r3 config watch misses U+2028", CMD, "if key.chars().any(crate::import::bad_path_char)",
     "if key.chars().any(|c| c.is_control() || crate::redact::is_invisible(c))"),
    ("r3b a linked ~/.config is outside the home", "engine/src/collectors/config.rs",
     "match under_dot_config.ok_or(()).or(under_home.map_err(|_| ())) {", "match under_home.map_err(|_| ()) {"),
    # stage 2 (Fable B1, N1)
    ("s2 a folder link to a skipped folder entered", "engine/src/collectors/config.rs",
     "&& refusal.is_some_and(|r| r != LinkRefusal::Own) =>",
     "&& false =>"),
    ("s2 a file below a followed link read by its spelling", "engine/src/collectors/config.rs",
     "                match self.refused_below_link(&path, follow) {", "                match None::<LinkRefusal> {"),
    ("s2 a folder below a followed link entered by its spelling", "engine/src/collectors/config.rs",
     "            } else if let Some(why) = self.refused_below_link(&path, follow) {",
     "            } else if let Some(why) = None::<LinkRefusal> {"),
    ("s2 the state directory listed", RECENT, "        dirs.state_dir.clone(),\n", ""),
    # round 2 (stage-1 review B1–B3, N1, N4)
    ("r2 shown_path takes a lossy path", RECENT, "    path.to_str()?;\n", ""),
    ("r2 state file read unbounded", RECENT, "match sys::read_small_file(&path, sys::STATE_FILE_MAX) {",
     "match std::fs::read_to_string(&path).map(Some).map_err(|e| e.to_string()) {"),
    ("r2 partial not saved", RECENT, "            partial: scan.partial,\n", "            partial: false,\n"),
    ("r2 partial not in the index", RECENT, "        partial: saved.partial,\n", "        partial: false,\n"),
    ("r2 logbook walked", "engine/src/commands/capture.rs", "            logbook.root.clone(),\n", ""),
    ("r2 a missing path watched", CMD, "    if std::fs::symlink_metadata(&path).is_err() {", "    if false {"),
    ("empty array gets a leading comma", "engine/src/config.rs",
     "        if last_end.is_some() || !insert.is_empty() {", "        {"),
]
PLUGIN = [
    ("any path in config watch", MODEL,
     '    return withText && n === 2 && a[1] === "watch" && json && watchPathError(free[0]) === ""',
     '    return withText && n === 2 && a[1] === "watch" && json'),
    ("config watch without --json", MODEL,
     '    return withText && n === 2 && a[1] === "watch" && json && watchPathError(free[0]) === ""',
     '    return withText && n === 2 && a[1] === "watch" && watchPathError(free[0]) === ""'),
    ("dot folders pass in the plugin", MODEL,
     '  if (/\\/\\.\\.?(\\/|$)/.test(p.slice(1)) || /\\/\\//.test(p)) return "The path holds a . or .. folder"\n', ""),
    ("bad characters pass in the plugin", MODEL,
     '  if (BAD_PATH_CHARS.test(p)) return "The path holds a control, text-direction or invisible character"\n'
     '  if (Array.from(p).length',
     '  if (Array.from(p).length'),
    ("long paths pass in the plugin", MODEL, "if (Array.from(p).length > RECENT_PATH_MAX)", "if (false)"),
    ("a path outside ~/.config passes", MODEL, "if (p.indexOf(RECENT_ROOT) !== 0 ||", "if ("),
    ("unwatchable rows shown", MODEL, '    if (!isObject(f) || watchPathError(f.path) !== "") continue', "    if (!isObject(f)) continue"),
    ("the tile counts the raw list", MODEL, 'big: files === null ? "—" : String(files.length)',
     'big: files === null ? "—" : String(rc.files.length)'),
    ("added read as already", MODEL, "  var added = !!data && data.added === true", "  var added = !!data"),
    ("r2 partial ignored", MODEL, "  var partial = rc !== null && rc.partial === true", "  var partial = false"),
    ("r2 any partial value", MODEL, "  var partial = rc !== null && rc.partial === true", "  var partial = rc !== null && !!rc.partial"),
    ("no Scanned row", MODEL, '    rows: isFinite(scanned) ? [["Scanned", relativeAge(scanned, nowMs)]] : [],', "    rows: [],"),
]
CARGO = ["cargo", "test", "--manifest-path", "engine/Cargo.toml", "--locked", "--lib",
         "--test", "recent_config", "--test", "index", "--test", "preview"]
env = dict(os.environ, CARGO_TARGET_DIR=TARGET)
# a fresh export of HEAD; the scratch root is this script's own folder
if os.path.isdir(ROOT):
    subprocess.run(["rm", "-rf", "--", ROOT], check=True)
os.makedirs(ROOT)
archive = subprocess.run(["git", "-C", WT, "archive", "HEAD"], check=True, capture_output=True).stdout
subprocess.run(["tar", "-x", "-C", ROOT], input=archive, check=True)
results = []
for kind, mutants, cmd in (("engine", ENGINE, CARGO), ("plugin", PLUGIN, ["node", "tests/plugin/model.test.js"])):
    for name, rel, a, b, *own in mutants:
        if sys.argv[1:] and not any(w in name for w in sys.argv[1:]):
            continue
        path = os.path.join(ROOT, rel)
        orig = open(path, encoding="utf-8").read()
        if orig.count(a) != 1:
            results.append((kind, name, f"PATTERN COUNT {orig.count(a)}"))
            print(f"{kind}: {name}: PATTERN COUNT {orig.count(a)}", flush=True)
            continue
        try:
            open(path, "w", encoding="utf-8").write(orig.replace(a, b))
            r = subprocess.run(own[0] if own else cmd, cwd=ROOT, env=env, capture_output=True, text=True)
        finally:
            open(path, "w", encoding="utf-8").write(orig)
        out = r.stdout + r.stderr
        failed = [l.split()[1] for l in out.splitlines() if l.startswith("test ") and l.endswith("FAILED")]
        failed += [l[5:].strip() for l in out.splitlines() if l.startswith("FAIL ")]
        verdict = ("killed by " + ", ".join(failed)) if r.returncode != 0 and failed else \
            "SURVIVED" if r.returncode == 0 else "build error:\n" + out[-800:]
        results.append((kind, name, verdict))
        print(f"{kind}: {name}: {verdict}", flush=True)
survived = [n for _, n, v in results if not v.startswith("killed")]
print(f"\n{len(results) - len(survived)} of {len(results)} killed" + (f"; not killed: {survived}" if survived else ""))
