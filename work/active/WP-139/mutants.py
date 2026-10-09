#!/usr/bin/env python3
"""WP-139 manual mutants: each change must make a test fail.

Engine mutants run `cargo test --lib collectors::recent` plus the
integration tests `recent_config` and `index` in a target dir of their own;
plugin mutants run `node tests/plugin/model.test.js`. Every file is
restored after its run. Arguments, if any, keep only the mutants whose name
contains one of them. Service.qml and System.qml are covered by the desk
harness (`desk-view.sh` system, system-watch, system-watch-locked), too
slow to run per mutant.
"""
import os, subprocess, sys
from pathlib import Path

# the checkout this script lives in: work/active/WP-139/mutants.py
WT = str(Path(__file__).resolve().parents[3])
# a target dir of its own: a mutated binary must never reach another run
TARGET = f"{WT}/engine/target/mutants-wp139"
RECENT = "engine/src/collectors/recent.rs"
CMD = "engine/src/commands/config_cmd.rs"
MODEL = "plugin/Model.js"
ENGINE = [
    # the walk
    ("watch paths not left out", RECENT,
     "        self.watched.iter().any(|w| path.starts_with(w))\n            || self.excluded",
     "        false\n            || self.excluded"),
    ("own and plugin folders entered", RECENT,
     "            || self.excluded.iter().any(|x| path.starts_with(x))\n", ""),
    ("skipPaths ignored by the walk", RECENT, "            || self.skip.matches(path)\n", ""),
    ("plugin folder not built in", RECENT, "        dirs.home.join(super::plugins::PLUGINS_DIR),\n", ""),
    ("seldon config folder not built in", RECENT,
     '        dirs.home.join(ROOT).join("seldon"),\n        dirs.config_dir(),\n', ""),
    ("cache folders entered", RECENT, ' || lower.contains("cache")', ""),
    ("named folders entered", RECENT, "DIR_NAMES.contains(&lower.as_str()) ||", "false ||"),
    ("browser profiles entered", RECENT, ".any(|e| PROFILE_MARKS.iter().any(|m| e.file_name() == *m))",
     ".any(|_| false)"),
    ("ignored endings listed", RECENT, "        || FILE_ENDINGS.iter().any(|e| lower.ends_with(e))\n", "\n"),
    ("shell.json listed", RECENT, '    lower == "shell.json"\n        ||', "    false\n        ||"),
    ("rotated logs listed", RECENT, '        || lower.contains(".log.")\n', ""),
    ("files older than 7 days listed", RECENT, "                    && t >= self.since\n", ""),
    ("more than 80 kept", RECENT, "    found.truncate(MAX_FILES);\n", ""),
    ("oldest first", RECENT, "Reverse(a.0).cmp(&Reverse(b.0))", "a.0.cmp(&b.0)"),
    ("entry budget ignored", RECENT, "if self.entries >= self.max_entries || Instant::now()", "if Instant::now()"),
    ("depth unbounded", RECENT, "        if depth >= MAX_DEPTH {\n", "        if false {\n"),
    ("directory links followed", RECENT, "            if kind.is_dir() {",
     "            if kind.is_dir() || std::fs::metadata(&path).is_ok_and(|m| m.is_dir()) {"),
    ("links to files not followed", RECENT, "(kind.is_file() || kind.is_symlink())", "kind.is_file()"),
    ("a time after the scan kept", RECENT, "self.found.push((t.min(SystemTime::from(self.now)), key));",
     "self.found.push((t, key));"),
    ("times not whole seconds", RECENT, "    t.with_nanosecond(0).unwrap_or(t)\n", "    t\n"),
    # the path rules
    ("redaction not asked", RECENT, "        && redactor.redact(&key) == key;", ";"),
    ("format characters pass", RECENT,
     ".any(|c| c.is_control() || crate::redact::is_invisible(c))", ".any(|c| c.is_control())"),
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
     ".any(|c| c.is_control() || crate::redact::is_invisible(c))", ".any(|c| c.is_control())"),
    ("long paths accepted", CMD, "        || key.chars().count() > SUBJECT_MAX\n", "\n"),
    ("no rebuild", CMD, "    crate::index::rebuild_if_initialised(ctx);\n", ""),
    ("no lock", CMD, "    let lock = ctx.lock()?;", "    let lock = ();"),
    ("no defaults without a file", CMD, "            let mut config = Config::default();",
     "            let mut config = Config { watch_paths: Vec::new(), ..Config::default() };"),
    ("the defaults hint lost", CMD, '.is_ok_and(|t| t.contains_key("watchPaths"))', ".is_ok()"),
    ("empty array refused", "engine/src/config.rs", "    let at = last_end.unwrap_or(open + 1);", "    let at = last_end?;"),
    # round 2 (stage-1 review B1–B3, N1, N4)
    ("r2 a name that is not UTF-8 walked lossily", RECENT,
     "            let Some(name) = name.to_str() else {\n                continue;\n            };",
     "            let name = &*name.to_string_lossy();"),
    ("r2 shown_path takes a lossy path", RECENT, "    path.to_str()?;\n", ""),
    ("r2 state file read unbounded", RECENT, "match sys::read_small_file(&path, sys::STATE_FILE_MAX) {",
     "match std::fs::read_to_string(&path).map(Some).map_err(|e| e.to_string()) {"),
    ("r2 deadline ignored", RECENT, " || Instant::now() >= self.deadline", ""),
    ("r2 depth not partial", RECENT, "            self.partial |= !subdirs.is_empty();\n", ""),
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
         "--test", "recent_config", "--test", "index"]
env = dict(os.environ, CARGO_TARGET_DIR=TARGET)
results = []
for kind, mutants, cmd in (("engine", ENGINE, CARGO), ("plugin", PLUGIN, ["node", "tests/plugin/model.test.js"])):
    for name, rel, a, b, *own in mutants:
        if sys.argv[1:] and not any(w in name for w in sys.argv[1:]):
            continue
        path = os.path.join(WT, rel)
        orig = open(path, encoding="utf-8").read()
        if orig.count(a) != 1:
            results.append((kind, name, f"PATTERN COUNT {orig.count(a)}"))
            print(f"{kind}: {name}: PATTERN COUNT {orig.count(a)}", flush=True)
            continue
        try:
            open(path, "w", encoding="utf-8").write(orig.replace(a, b))
            r = subprocess.run(own[0] if own else cmd, cwd=WT, env=env, capture_output=True, text=True)
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
