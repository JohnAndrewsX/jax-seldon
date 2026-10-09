#!/usr/bin/env python3
"""WP-166 manual mutants: each one undoes one rule of `seldon inbox add`;
`--test inbox` or the unit tests of `commands::inbox` must fail for every
one. Run it from a copy of the tree: it rewrites the source in place and
puts it back. Names given as arguments run only the mutants whose name
contains one of them; `--check` only applies each mutant."""
import os
import subprocess
import sys
from pathlib import Path

# the checkout this script lives in: work/active/WP-166/mutants.py
WT = str(Path(__file__).resolve().parents[3])
# a target dir of its own: a mutated build must never reach another run
# on disk, never under /tmp (a RAM tmpfs): the worktree's target when
# this copy lives elsewhere, else the copy's own
TARGET = os.environ.get("MUTANTS_TARGET", f"{WT}/engine/target/mutants-wp166")
INBOX = "engine/src/commands/inbox.rs"


def both(*pairs):
    """Every (a, b) of `pairs` replaced, each once."""
    def apply(src):
        for a, b in pairs:
            assert src.count(a) == 1, (a, src.count(a))
            src = src.replace(a, b)
        return src
    return apply


def plain(a, b, count=1):
    def apply(src):
        assert src.count(a) == count, (a, src.count(a))
        return src.replace(a, b)
    return apply


MUTANTS = [
    # the text, as `import task` treats a task file
    ("text: format characters kept", INBOX, plain('scrubber.text_dropping_invisible("text", &text)', 'scrubber.text("text", &text)')),
    ("text: format characters not counted", INBOX, plain("text.chars().count() - kept.chars().count()", "0")),
    ("text: CRLF kept", INBOX, plain('raw.replace("\\r\\n", "\\n")', "raw.clone()")),
    ("text: not scrubbed", INBOX, plain('scrubber.text_dropping_invisible("text", &text)', 'crate::redact::without_invisible(&text).into_owned()')),
    ("text: outer blank lines kept", INBOX, plain("    let text = trim_blank_lines(&text);\n", "")),
    ("text: blank text filed", INBOX, plain("    if text.is_empty() {\n        return Err(Error::user(\"the text must not be empty\"));\n    }\n", "")),
    ("text: stdin size not checked", INBOX, plain("if bytes.len() as u64 > MAX_TEXT_BYTES {", "if false {")),
    ("text: stdin size off by one", INBOX, plain("if bytes.len() as u64 > MAX_TEXT_BYTES {", "if bytes.len() as u64 >= MAX_TEXT_BYTES {")),
    ("text: stdin not UTF-8 accepted", INBOX, plain('return String::from_utf8(bytes).map_err(|_| Error::user("the text on stdin is not UTF-8"));', "return Ok(String::from_utf8_lossy(&bytes).into_owned());")),
    ("text: a file read through its link", INBOX, plain("match sys::read_small_file(file, MAX_TEXT_BYTES) {", "match std::fs::read_to_string(file).map(Some).map_err(|e| e.to_string()) {")),
    # the title
    ("title: format characters kept", INBOX, plain('scrubber.text_dropping_invisible("title", &title)', 'scrubber.text("title", &title)')),
    ("title: several lines", INBOX, plain('let title = one_line("the title", &title)?;', 'let title = super::required_text("the title", &title)?;')),
    ("title: not scrubbed", INBOX, both(('let title = scrubber.text("title", &args.title);', "let title = args.title.clone();"), ('scrubber.text_dropping_invisible("title", &title)', "crate::redact::without_invisible(&title).into_owned()"))),
    ("title: no length limit", INBOX, plain("if title.chars().count() > MAX_TITLE_CHARS {", "if false {")),
    ("title: length off by one", INBOX, plain("if title.chars().count() > MAX_TITLE_CHARS {", "if title.chars().count() >= MAX_TITLE_CHARS {")),
    ("title: the fallback slug", INBOX, plain('const FALLBACK_SLUG: &str = "note";', 'const FALLBACK_SLUG: &str = "";')),
    # counts
    ("count: hits, not lines", INBOX, plain("    lines.len()\n}", "    scrubber.hits.len()\n}")),
    ("count: lines of title and text merged", INBOX, plain(".map(|h| (h.file.as_str(), h.line))", '.map(|h| ("", h.line))')),
    ("count: no private paths", INBOX, plain('"privatePaths": scrubber.private_paths,', '"privatePaths": 0,')),
    ("tags: not redacted", INBOX, plain("args.tags.iter().map(|t| redactor.redact(t)).collect()", "args.tags.clone()")),
    # the file
    ("file: frontmatter without the actor", INBOX, plain('        ("actor", FmValue::str(actor)),\n', "")),
    ("file: the date of the name", INBOX, plain('ctx.now.format("%Y-%m-%d")', 'ctx.now.format("%Y%m%d")')),
    ("file: the first name numbered", INBOX, plain("            1 => format!(\"{stem}.md\"),\n", "")),
    ("file: one name fewer", INBOX, plain("const MAX_SUFFIX: u32 = 99;", "const MAX_SUFFIX: u32 = 98;")),
    ("file: one name more", INBOX, plain("const MAX_SUFFIX: u32 = 99;", "const MAX_SUFFIX: u32 = 100;")),
    ("file: a taken name is an error", INBOX, plain("Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => return Ok(false),", "Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => return Err(Error::user(\"taken\")),")),
    ("file: a taken name overwritten", INBOX, plain("let mut file = match sys::create_new_private(path) {", "let _ = std::fs::remove_file(path);\n    let mut file = match sys::create_new_private(path) {")),
    ("file: inbox/ not created", INBOX, plain("    sys::create_dir_private(&dir).with_context(|| format!(\"cannot create {}\", dir.display()))?;\n", "")),
    # idempotency
    ("filed: never found", INBOX, plain("    let (path, filed) = match already_filed(&logbook, &body) {", "    let (path, filed) = match None::<String> {")),
    ("filed: the frontmatter counts", INBOX, plain("(doc.body == body).then(", "(text == body).then(")),
    ("filed: only the first name looked at", INBOX, plain("    names.into_iter().find_map(|name| {", "    names.into_iter().take(1).find_map(|name| {")),
    ("filed: committed anyway", INBOX, plain("    let commit = if filed {", "    let commit = if true {")),
    # the record
    ("record: the user's edits committed too", INBOX, plain('autocommit_paths(ctx, &config, &logbook, &[&path], "inbox add")', 'super::autocommit(ctx, &config, &logbook, "inbox add")')),
    ("record: no index rebuild", INBOX, plain("        crate::index::rebuild_if_initialised(ctx);\n", "")),
    ("record: no lock", INBOX, plain("    let lock = ctx.lock()?;\n", "    let lock = ();\n")),
    # round 2
    ("r2: a linked inbox written through", INBOX, plain("    logbook.checked_dir(INBOX)?;\n", "")),
    ("r2: the filed scan reads through links", INBOX, plain("let text = sys::read_small_file(&path, 2 * MAX_TEXT_BYTES).ok()??;", "let text = std::fs::read_to_string(&path).ok()?;")),
    ("r2: title controls kept", INBOX, plain("let (title, _) = drop_chars(&title, is_title_control);", "let (title, _) = (title.clone(), 0);")),
    ("r2: title controls not counted", INBOX, plain(".filter(|c| is_invisible(*c) || is_title_control(*c))", ".filter(|c| is_invisible(*c))")),
    ("r2: the title's drops not counted", INBOX, plain('"droppedCharacters": title_dropped + text_dropped,', '"droppedCharacters": text_dropped,')),
    ("r2: a proc view filed", INBOX, plain("&& std::fs::symlink_metadata(file).is_ok_and(|m| m.len() == 0) =>", "&& false =>")),
    ("r2: an empty file is a view", INBOX, plain("if !text.is_empty() && std::fs::symlink_metadata", "if true && std::fs::symlink_metadata")),
    ("r2: a terminal is read", INBOX, plain("        if stdin.is_terminal() {", "        if false && stdin.is_terminal() {")),
    # round 3
    ("r3: text controls kept", INBOX, plain("let (text, _) = drop_chars(&text, is_text_control);", "let text = text.clone();")),
    ("r3: text controls not counted", INBOX, plain(".filter(|c| is_invisible(*c) || is_text_control(*c))", ".filter(|c| is_invisible(*c))")),
    # round 3b: redaction before the invisible characters go (WP-159)
    ("r3b: the text stripped before its redaction", INBOX, plain('scrubber.text_dropping_invisible("text", &text)', 'scrubber.text_dropping_invisible("text", &crate::redact::without_invisible(&text))')),
    ("r3b: the title stripped before its redaction", INBOX, plain('scrubber.text_dropping_invisible("title", &title)', 'scrubber.text_dropping_invisible("title", &crate::redact::without_invisible(&title))')),
    ("r3b: the text's invisible ones not counted", INBOX, plain(".filter(|c| is_invisible(*c) || is_text_control(*c))", ".filter(|c| is_text_control(*c))")),
    # round 3c: two passes, the controls dropped between them
    ("r3c: no first pass over the text", INBOX, plain('    let text = scrubber.text("text", &text);\n', "")),
    ("r3c: no first pass over the title", INBOX, plain('let title = scrubber.text("title", &args.title);', "let title = args.title.clone();")),
    ("r3c: the text's controls dropped after the scrubber", INBOX, plain('    let (text, _) = drop_chars(&text, is_text_control);\n    let text = scrubber.text_dropping_invisible("text", &text);\n', '    let text = scrubber.text_dropping_invisible("text", &text);\n    let (text, _) = drop_chars(&text, is_text_control);\n')),
    ("r3c: the title's controls dropped after the scrubber", INBOX, plain('    let (title, _) = drop_chars(&title, is_title_control);\n    let title = scrubber.text_dropping_invisible("title", &title);\n', '    let title = scrubber.text_dropping_invisible("title", &title);\n    let (title, _) = drop_chars(&title, is_title_control);\n')),
    ("r3c: the title keeps its white-space controls", INBOX, plain("c.is_control() && c != '\\n' && c != '\\r'", "c.is_control() && !c.is_whitespace()")),
    ("r3: a tab dropped", INBOX, plain("c.is_control() && c != '\\n' && c != '\\t'", "c.is_control() && c != '\\n'")),
]

check = "--check" in sys.argv
only = [a for a in sys.argv[1:] if a != "--check"]
env = dict(os.environ, CARGO_TARGET_DIR=TARGET)
cargo = ["cargo", "test", "--manifest-path", "engine/Cargo.toml", "--no-fail-fast"]
runs = [
    cargo + ["--lib", "--", "commands::inbox", "--test-threads=4"],
    cargo + ["--test", "inbox", "--", "--test-threads=4"],
]
results = []
for name, file, mutate in MUTANTS:
    if only and not any(o in name for o in only):
        continue
    path = os.path.join(WT, file)
    orig = open(path).read()
    try:
        mutated = mutate(orig)
    except (AssertionError, ValueError) as e:
        results.append((name, f"NOT APPLIED {e}"))
        continue
    if mutated == orig:
        results.append((name, "NOT APPLIED (no change)"))
        continue
    if check:
        results.append((name, "killed (not run)"))
        continue
    try:
        open(path, "w").write(mutated)
        failed = []
        for cmd in runs:
            r = subprocess.run(cmd, cwd=WT, env=env, capture_output=True, text=True)
            if "error[E" in r.stderr or "error: could not compile" in r.stderr:
                failed.append("does not compile")
                break
            if r.returncode != 0:
                failed.append(cmd[6] if cmd[5] == "--test" else "lib")
    finally:
        open(path, "w").write(orig)
    results.append((name, f"killed ({', '.join(failed)})" if failed else "SURVIVED"))
    print(f"{name}: {results[-1][1]}", flush=True)

print()
killed = sum(r.startswith("killed") for _, r in results)
print(f"{killed}/{len(results)} killed")
for name, r in results:
    if not r.startswith("killed"):
        print(f"  {name}: {r}")
