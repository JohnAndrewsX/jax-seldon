#!/usr/bin/env python3
"""Check the user guide in docs/user/ (WP-045).

Called by scripts/docs-check.sh, which builds the engine and passes it as
--seldon. Exit 0 when everything holds, 1 otherwise.

Checks (docs/user/STYLE.md, "Checks"):
  links      every relative link and image under docs/user/ resolves; an
             anchor in a link to a Markdown file names a heading there
             (GitHub's slug rules); every image has alt text
  pages      every language folder under docs/user/ (en, de, …) holds the
             same file names as en/
  structure  each en/de pair has the same heading levels in the same order
             and the same number of code blocks, tables and images
  source     every German page names the English page and commit it
             matches (`<!-- source: en/<page> @ <commit> -->`); a commit
             older than the English page's last change is a warning
  cli        05-cli-reference.md in each language carries the `--help` of
             every command between `<!-- help: seldon … -->` and
             `<!-- /help -->`, equal to the engine's output with the global
             options removed where their text matches `seldon --help`
  usage      every `seldon …` in a code span or code block of the guide
             names commands and options that the engine's help lists

--write regenerates the help blocks from the engine instead of comparing
them; the prose around them is left alone.
"""
import argparse
import os
import re
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
USER = os.path.join(ROOT, "docs", "user")
SOURCE_LANG = "en"


def languages():
    """The source language and every other folder under docs/user/."""
    found = sorted(d for d in os.listdir(USER) if os.path.isdir(os.path.join(USER, d)))
    return [SOURCE_LANG] + [d for d in found if d != SOURCE_LANG]


LANGS = languages()
CLI_PAGE = "05-cli-reference.md"

FENCE = re.compile(r"^\s*(```|~~~)")
HEADING = re.compile(r"^(#{1,6})\s+(.*?)\s*#*\s*$")
LINK = re.compile(r"(!?)\[((?:[^\[\]]|\[[^\]]*\])*)\]\(\s*<?([^)\s>]+)>?(?:\s+\"[^\"]*\")?\s*\)")
TABLE_SEPARATOR = re.compile(r"^\s*\|?\s*:?-{3,}:?\s*(\|\s*:?-{3,}:?\s*)*\|?\s*$")
SOURCE_LINE = re.compile(r"<!--\s*source:\s*(\S+)\s*@\s*([0-9a-f]{7,40})\s*-->")
HELP_BLOCK = re.compile(
    r"<!-- help: (seldon(?: [a-z-]+)*) -->\n```text\n(.*?)```\n<!-- /help -->",
    re.S,
)
OPTION_LINE = re.compile(r"^\s+((?:-[A-Za-z], )?--[a-z-]+(?: <[A-Z]+>)?)\s{2,}(.*)$")


class Report:
    def __init__(self, quiet):
        self.quiet = quiet
        self.errors = []
        self.warnings = []

    def error(self, msg):
        self.errors.append(msg)

    def warn(self, msg):
        self.warnings.append(msg)

    def finish(self, checked):
        for w in self.warnings:
            print(f"docs-check: warning: {w}")
        for e in self.errors:
            print(f"docs-check: {e}", file=sys.stderr)
        if self.errors:
            print(f"docs-check: {len(self.errors)} problem(s)", file=sys.stderr)
            return 1
        if not self.quiet:
            print(f"docs-check: ok ({checked})")
        return 0


def rel(path):
    return os.path.relpath(path, ROOT)


def read(path):
    with open(path, encoding="utf-8") as f:
        return f.read()


def prose_lines(text):
    """Yield (line, in_fence) for every line; fence lines count as in_fence."""
    in_fence = False
    marker = None
    for line in text.splitlines():
        m = FENCE.match(line)
        if m:
            if not in_fence:
                in_fence, marker = True, m.group(1)
            elif m.group(1) == marker:
                in_fence, marker = False, None
            yield line, True
            continue
        yield line, in_fence


def strip_code_spans(line):
    return re.sub(r"`+[^`]*`+", "", line)


def slug(text):
    """GitHub's heading anchor: lower case, punctuation dropped, spaces to hyphens."""
    text = re.sub(r"<[^>]+>", "", text)
    text = re.sub(r"\[([^\]]*)\]\([^)]*\)", r"\1", text)
    text = text.strip().lower()
    text = re.sub(r"[^\w\- ]", "", text)
    return text.replace(" ", "-")


_ANCHORS = {}


def anchors(path):
    cache = _ANCHORS
    if path not in cache:
        seen = {}
        found = set()
        for line, in_fence in prose_lines(read(path)):
            if in_fence:
                continue
            m = HEADING.match(line)
            if not m:
                continue
            base = slug(m.group(2))
            n = seen.get(base, 0)
            seen[base] = n + 1
            found.add(base if n == 0 else f"{base}-{n}")
        for m in re.finditer(r'<a\s+(?:name|id)="([^"]+)"', read(path)):
            found.add(m.group(1))
        cache[path] = found
    return cache[path]


def markdown_files():
    for dirpath, _, files in os.walk(USER):
        for name in sorted(files):
            if name.endswith(".md"):
                yield os.path.join(dirpath, name)


def check_links(report):
    count = 0
    for path in markdown_files():
        for line, in_fence in prose_lines(read(path)):
            if in_fence:
                continue
            for m in LINK.finditer(strip_code_spans(line)):
                is_image, label, target = m.group(1) == "!", m.group(2), m.group(3)
                count += 1
                where = rel(path)
                if is_image and not label.strip():
                    report.error(f"{where}: image {target} has no alt text")
                if re.match(r"^[a-z][a-z0-9+.-]*:", target):
                    continue
                file_part, _, anchor = target.partition("#")
                dest = path if not file_part else os.path.normpath(
                    os.path.join(os.path.dirname(path), file_part))
                if not os.path.exists(dest):
                    report.error(f"{where}: link to {target}: {rel(dest)} does not exist")
                    continue
                if not dest.startswith(ROOT + os.sep):
                    report.error(f"{where}: link to {target} leaves the repository")
                    continue
                if anchor and dest.endswith(".md") and anchor not in anchors(dest):
                    report.error(f"{where}: link to {target}: no heading with anchor #{anchor} in {rel(dest)}")
    return count


def fenced_blocks(text):
    return sum(1 for line, _ in prose_lines(text) if FENCE.match(line)) // 2


def page_shape(text):
    headings, tables, images = [], 0, 0
    for line, in_fence in prose_lines(text):
        if in_fence:
            continue
        m = HEADING.match(line)
        if m:
            headings.append(len(m.group(1)))
        if "|" in line and TABLE_SEPARATOR.match(line):
            tables += 1
        images += sum(1 for l in LINK.finditer(strip_code_spans(line)) if l.group(1) == "!")
    return {
        "heading levels": headings,
        "code blocks": fenced_blocks(text),
        "tables": tables,
        "images": images,
    }


def git(*args):
    try:
        out = subprocess.run(["git", "-C", ROOT, *args], capture_output=True, text=True, timeout=30)
    except (OSError, subprocess.TimeoutExpired):
        return None
    return out


def check_pages(report):
    sets = {}
    for lang in LANGS:
        d = os.path.join(USER, lang)
        sets[lang] = sorted(n for n in os.listdir(d) if n.endswith(".md")) if os.path.isdir(d) else []
        if not sets[lang]:
            report.error(f"docs/user/{lang}/ holds no pages")
    base = set(sets[SOURCE_LANG])
    for lang in LANGS:
        if lang == SOURCE_LANG:
            continue
        for name in sorted(base - set(sets[lang])):
            report.error(f"docs/user/{lang}/{name} is missing (docs/user/{SOURCE_LANG}/{name} exists)")
        for name in sorted(set(sets[lang]) - base):
            report.error(f"docs/user/{lang}/{name} has no English source docs/user/{SOURCE_LANG}/{name}")
    pairs = 0
    unchecked = []
    for lang in LANGS:
        if lang == SOURCE_LANG:
            continue
        for name in sorted(base & set(sets[lang])):
            pairs += 1
            src_path = os.path.join(USER, SOURCE_LANG, name)
            tr_path = os.path.join(USER, lang, name)
            src, tr = read(src_path), read(tr_path)
            a, b = page_shape(src), page_shape(tr)
            for key in a:
                if a[key] != b[key]:
                    report.error(
                        f"docs/user/{lang}/{name}: {key} differ from the English page "
                        f"({b[key]} vs {a[key]})")
            m = SOURCE_LINE.search("\n".join(tr.splitlines()[:6]))
            if not m:
                report.error(
                    f"docs/user/{lang}/{name}: no source line "
                    f"`<!-- source: {SOURCE_LANG}/{name} @ <commit> -->` after the title")
                continue
            if m.group(1) != f"{SOURCE_LANG}/{name}":
                report.error(f"docs/user/{lang}/{name}: source line names {m.group(1)}, not {SOURCE_LANG}/{name}")
            commit = m.group(2)
            known = git("rev-parse", "--verify", "--quiet", f"{commit}^{{commit}}")
            if known is None or known.returncode != 0:
                unchecked.append(f"{lang}/{name}")
                continue
            diff = git("diff", "--quiet", commit, "--", rel(src_path))
            if diff is not None and diff.returncode == 1:
                report.warn(
                    f"docs/user/{lang}/{name} matches {SOURCE_LANG}/{name} at {commit}; "
                    f"the English page has changed since (update the translation and its source line)")
    if unchecked:
        report.warn(
            f"source commits not in this checkout (shallow clone?), freshness not checked: "
            f"{', '.join(unchecked)}")
    return pairs


def run_help(seldon, words, env):
    out = subprocess.run([seldon, *words, "--help"], capture_output=True, text=True, env=env, timeout=30)
    if out.returncode != 0:
        raise RuntimeError(f"`seldon {' '.join(words)} --help` exited {out.returncode}: {out.stderr.strip()}")
    return [line.rstrip() for line in out.stdout.rstrip("\n").splitlines()]


def subcommands(lines):
    names, inside = [], False
    for line in lines:
        if line == "Commands:":
            inside = True
            continue
        if inside:
            if not line.strip():
                break
            name = line.split()[0]
            if name != "help":
                names.append(name)
    return names


def global_options(lines):
    opts, inside = {}, False
    for line in lines:
        if line == "Options:":
            inside = True
            continue
        if inside:
            m = OPTION_LINE.match(line)
            if m:
                opts[m.group(1)] = m.group(2)
    opts.pop("-V, --version", None)
    return opts


def option_names(lines):
    """Every option a help text lists: `--json`, `-V`, `--version`, …"""
    names = set()
    for line in lines:
        m = OPTION_LINE.match(line)
        if m:
            names.update(re.findall(r"-{1,2}[A-Za-z][a-z-]*", m.group(1)))
    return names


def help_tree(seldon):
    """{'seldon plan new': {'lines', 'subs', 'opts'}} for every command."""
    with tempfile.TemporaryDirectory(prefix="seldon-docs-check-") as home:
        env = {
            "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
            "HOME": home,
            "XDG_CONFIG_HOME": os.path.join(home, ".config"),
            "XDG_STATE_HOME": os.path.join(home, ".local", "state"),
            "XDG_DATA_HOME": os.path.join(home, ".local", "share"),
            "SELDON_TEST_GUARD": home,
            "LC_ALL": "C.UTF-8",
        }
        tree = {}
        queue = [[]]
        while queue:
            words = queue.pop(0)
            lines = run_help(seldon, words, env)
            subs = subcommands(lines)
            tree[" ".join(["seldon", *words])] = {"lines": lines, "subs": subs, "opts": option_names(lines)}
            queue.extend(words + [name] for name in subs)
        return tree


def help_texts(tree):
    """{'seldon plan new': text}, the global options stripped below the root."""
    root = tree["seldon"]["lines"]
    globals_ = global_options(root)
    texts = {}
    for cmd, node in tree.items():
        kept = []
        for line in node["lines"]:
            m = OPTION_LINE.match(line)
            if cmd != "seldon" and m and globals_.get(m.group(1)) == m.group(2):
                continue
            kept.append(line)
        texts[cmd] = "\n".join(kept) + "\n"
    return texts


# `seldon` as a command word: not part of a path, an id or a commit message
COMMAND = re.compile(r"(?<![\w./~-])seldon(?=\s|$)")
CODE_SPAN = re.compile(r"(`+)(.+?)\1")


# Fenced blocks that hold commands; `text` blocks are output, `toml` config.
COMMAND_FENCES = ("", "sh", "bash", "console", "shell")

# Commands the guide names as planned, not as available (08: update-impact,
# WP-033). An entry the engine has gained is an error, so it gets removed.
PLANNED = {"seldon update-impact"}


def command_snippets(text):
    """Yield (line number, words after `seldon`) for every command in code."""
    lang = None
    for n, line in enumerate(text.splitlines(), 1):
        m = FENCE.match(line)
        if m:
            lang = None if lang is not None else line.strip()[3:].strip().lower()
            continue
        if lang is not None:
            if lang not in COMMAND_FENCES:
                continue
            parts = [line]
        else:
            parts = [m.group(2) for m in CODE_SPAN.finditer(line)]
        for part in parts:
            for m in COMMAND.finditer(part):
                rest = re.split(r"\s(?:\||&&|;|#)\s|\s#|\s\|\s*$", part[m.end():])[0]
                yield n, rest.split()


def check_usage(report, tree):
    """Every `seldon …` in the guide names a real command and real options."""
    root_opts = tree["seldon"]["opts"]
    for cmd in sorted(PLANNED & set(tree)):
        report.error(f"`{cmd}` exists now; remove it from PLANNED in scripts/docs-check.py and document it")
    count = 0
    for path in markdown_files():
        if os.path.basename(path) == CLI_PAGE:
            text = HELP_BLOCK.sub("", read(path))
        else:
            text = read(path)
        for n, words in command_snippets(text):
            count += 1
            paths = ["seldon"]
            for word in words:
                if word == "--":
                    break
                if word.startswith("-"):
                    flag = word.split("=")[0]
                    for cmd in paths:
                        if flag not in tree[cmd]["opts"] | root_opts:
                            report.error(f"{rel(path)}:{n}: `{cmd}` has no option {flag}")
                    continue
                subs = tree[paths[0]]["subs"]
                if not subs:
                    continue  # an argument
                if word.startswith(("<", "…", '"', "'")) or word in ("help",):
                    break
                names = word.split("|")
                if all(f"{paths[0]} {w}" in PLANNED for w in names):
                    break
                unknown = [w for w in names if w not in subs]
                if unknown:
                    report.error(f"{rel(path)}:{n}: `{paths[0]}` has no command {'|'.join(unknown)}")
                    break
                paths = [f"{paths[0]} {w}" for w in names]
    return count


def check_cli(report, tree, write):
    texts = help_texts(tree)
    for lang in LANGS:
        path = os.path.join(USER, lang, CLI_PAGE)
        if not os.path.exists(path):
            if lang == SOURCE_LANG:
                report.error(f"{rel(path)} is missing")
            continue
        page = read(path)
        found = [m.group(1) for m in HELP_BLOCK.finditer(page)]
        for cmd in sorted(set(found) - set(texts)):
            report.error(f"{rel(path)}: help block for `{cmd}`, which the engine does not have")
        for cmd in sorted(set(texts) - set(found)):
            report.error(f"{rel(path)}: no help block for `{cmd}` (add `<!-- help: {cmd} -->` … `<!-- /help -->`)")
        for cmd in sorted({c for c in found if found.count(c) > 1}):
            report.error(f"{rel(path)}: more than one help block for `{cmd}`")
        if write:
            new = HELP_BLOCK.sub(
                lambda m: f"<!-- help: {m.group(1)} -->\n```text\n"
                f"{texts.get(m.group(1), m.group(2))}```\n<!-- /help -->",
                page,
            )
            if new != page:
                with open(path, "w", encoding="utf-8") as f:
                    f.write(new)
                print(f"docs-check: wrote the help blocks of {rel(path)}")
            continue
        for m in HELP_BLOCK.finditer(page):
            cmd, shown = m.group(1), m.group(2)
            want = texts.get(cmd)
            if want is not None and shown != want:
                report.error(
                    f"{rel(path)}: the help block for `{cmd}` differs from `{cmd} --help` "
                    f"(run `bash scripts/docs-check.sh --write`)")
    return len(texts)


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--seldon", help="the engine binary whose --help the CLI reference must match")
    ap.add_argument("--write", action="store_true", help="regenerate the help blocks of the CLI reference")
    ap.add_argument("-q", "--quiet", action="store_true")
    args = ap.parse_args()

    report = Report(args.quiet)
    links = check_links(report)
    pairs = check_pages(report)
    if args.seldon:
        try:
            tree = help_tree(args.seldon)
            commands = check_cli(report, tree, args.write)
            usages = check_usage(report, tree)
        except (OSError, RuntimeError, subprocess.TimeoutExpired) as e:
            report.error(f"cannot read the engine's help: {e}")
            commands = usages = 0
    else:
        report.error("no engine given (--seldon); the CLI reference was not checked")
        commands = usages = 0
    return report.finish(
        f"{links} links, {pairs} translated pages, {commands} commands, {usages} command lines")


if __name__ == "__main__":
    sys.exit(main())
