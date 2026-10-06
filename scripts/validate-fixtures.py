#!/usr/bin/env python3
"""Validate every JSON fixture against its schema and check that the sample
index derives from the sample logbook (WP-002, WP-014, WP-015).

Called by scripts/validate-fixtures.sh. Exit 0 when everything holds, 1 otherwise.

Schema backends, picked in this order unless --validator / SELDON_SCHEMA_VALIDATOR says otherwise:
  jsonschema        python module (Draft 2020-12, local registry, format checks)
  check-jsonschema  CLI, fed with local copies of the schemas
  builtin           the small Draft 2020-12 subset below; fails closed on any
                    keyword it does not implement, so a schema change that needs
                    more is noticed instead of silently skipped

The derivation check implements the index rules of ADR-0012 and the drift
grouping of ADR-0013 for the parts that come from the logbook. It is a fixture
consistency check, not the engine; the engine's golden test (WP-007) compares
real `seldon index` output with the same fixture, and runs `--derive` on scratch logbooks
(the text clip of ADR-0025, WP-077). `--write-index` rewrites those
parts of fixtures/index.sample.json and regenerates fixtures/index-variants/ from
the sample (VARIANTS). Every case's Log is walked through the SPEC-LOGBOOK §3
state machine, and index times must not precede the events they list. Every run
also executes the mutation self-checks of ADR-0013 (drift groups), ADR-0015 §4
(proposal token rule) and the case walk.
"""
import argparse
import copy
import datetime as dt
import fnmatch
import glob
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import urllib.parse

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SCHEMA_DIR = os.path.join(ROOT, "schema")
FIX = os.path.join(ROOT, "fixtures")
LOGBOOK = os.path.join(FIX, "logbook")
SAMPLE = os.path.join(FIX, "index.sample.json")
# ADR-0028 §5: the sample logbook indexed with `[drift] attention = "all"` (the rollback, the drift
# derivation before ADR-0028); the engine's golden test holds `seldon index` to it.
ATTENTION_ALL = os.path.join(FIX, "index.attention-all.json")
ID = "https://github.com/JohnAndrewsX/jax-seldon/schema/"

EVENT, CASE, INDEX = ID + "event.schema.json", ID + "case.schema.json", ID + "index.schema.json"
EXT = {
    "snapper": ID + "external/snapper-list.schema.json",
    "plugin-list": ID + "external/omarchy-plugin-list.schema.json",
    "plugin-catalog": ID + "external/omarchy-plugin-catalog.schema.json",
    "hook": ID + "external/claude-code-hook.schema.json",
}

DRIFT_SOURCES = {"pacman", "omarchy", "plugins", "theme", "config"}
EVENT_KEYS = ["id", "ts", "source", "kind", "subject", "detail", "actor", "case", "zone",
              "explicit", "txId", "refersTo", "resolution", "resolutionDetail", "meta"]
DRIFT_KEYS = ["eventId", "ts", "source", "kind", "subject", "detail", "actor", "zone", "crisis",
              "proposedCase", "txId", "members"]

# ADR-0013 §3, ADR-0023 (WP-050): default of config.toml [drift] alwaysRed (fnmatch globs,
# case-sensitive); keep in step with engine/src/config.rs DriftConfig::default.
ALWAYS_RED = ["linux", "linux-lts", "linux-zen", "linux-hardened", "linux-rt",
              "linux-rt-lts", "linux-omarchy", "systemd", "glibc", "hyprland", "omarchy",
              "omarchy-settings", "quickshell", "limine*", "grub", "mkinitcpio*", "filesystem",
              "pam", "sddm", "uwsm"]

# ADR-0025 (WP-077): the index clips `detail`, `resolutionDetail` and every string of `meta` of
# its events and drift items to TEXT_MAX bytes of JSON (escapes counted, marker included); keep
# in step with engine/src/index/build.rs TEXT_MAX and clip().
TEXT_MAX = 256
# Rust's char::is_whitespace (Unicode White_Space), which `trim_end` strips; Python's str.rstrip()
# would strip U+001C..U+001F too.
WHITE_SPACE = ("\t\n\x0b\x0c\r \x85\xa0\u1680\u2000\u2001\u2002\u2003\u2004\u2005\u2006"
               "\u2007\u2008\u2009\u200a\u2028\u2029\u202f\u205f\u3000")

# ADR-0015 §4 (supersedes ADR-0012 §13): whole-word, case-sensitive; word characters are
# [A-Za-z0-9._+-], except that a final `.` not followed by a word character is punctuation.
TOKEN_CHARS = r"A-Za-z0-9._+\-"


def token_pattern(subject):
    return re.compile(f"(?<![{TOKEN_CHARS}])" + re.escape(subject) + f"(?=\\.?(?![{TOKEN_CHARS}]))")

# pacman options that take a separate argument word (`--opt value`; `--opt=value` is one word).
# An unknown option is assumed to take none, so its argument counts as a package name and the
# transaction is not routine: unknown input errs towards red.
PACMAN_LONG_OPS = {"--sync": "S", "--database": "D", "--files": "F", "--query": "Q", "--remove": "R",
                   "--deptest": "T", "--upgrade": "U", "--version": "V"}
PACMAN_LONG_WITH_ARG = {"--arch", "--ask", "--assume-installed", "--cachedir", "--color", "--config",
                        "--dbpath", "--gpgdir", "--hookdir", "--ignore", "--ignoregroup", "--logfile",
                        "--overwrite", "--print-format", "--root", "--sysroot"}
PACMAN_SHORT_WITH_ARG = "br"  # -b/--dbpath, -r/--root


def full_upgrade_argv(command):
    """ADR-0013 §3: the command, split into argv the way pacman logs it (words joined by single
    spaces, no quoting), is the sync operation with -u/--sysupgrade and names no package."""
    argv = (command or "").split()
    if not argv or os.path.basename(argv[0]) != "pacman":
        return False
    ops, sysupgrade, targets = set(), False, []
    i, end_of_opts = 1, False
    while i < len(argv):
        a = argv[i]
        i += 1
        if end_of_opts or not a.startswith("-") or a == "-":
            targets.append(a)
        elif a == "--":
            end_of_opts = True
        elif a.startswith("--"):
            name = a.split("=", 1)[0]
            if name in PACMAN_LONG_OPS:
                ops.add(PACMAN_LONG_OPS[name])
            elif name == "--sysupgrade":
                sysupgrade = True
            elif name in PACMAN_LONG_WITH_ARG and "=" not in a:
                i += 1
        else:
            for j, ch in enumerate(a[1:], 2):
                if ch.isupper():
                    ops.add(ch)
                elif ch == "u":
                    sysupgrade = True
                elif ch in PACMAN_SHORT_WITH_ARG:
                    if j == len(a):  # the argument is the next word, else the rest of this one
                        i += 1
                    break
    return ops == {"S"} and sysupgrade and not targets


def always_red(subject):
    return any(fnmatch.fnmatchcase(subject, p) for p in ALWAYS_RED)


def routine(e):
    """ADR-0013 §3: a group member that keeps the group yellow."""
    return (e["kind"] in ("upgrade", "reinstall") and e.get("explicit") is False
            and full_upgrade_argv(e.get("meta", {}).get("command")) and not always_red(e["subject"]))
# ADR-0028 §2 (WP-109): the class of a drift-eligible event, routine < attention < crisis. Keep in
# step with engine/src/index/class.rs and the [drift] defaults of engine/src/config.rs.
ROUTINE_RULES = ["sysupgrade", "upgrade", "keyring", "omarchy-update", "plugin-toggle", "theme",
                 "omarchy-default", "system-link", "routine-paths", "theme-assets", "theme-repo"]
ROUTINE_PATHS = ["~/.config/omarchy/shell.json", "**/*.bak.*"]
ROUTINE_PACKAGES = ["archlinux-keyring", "omarchy-keyring"]
ALWAYS_RED_PATHS = ["~/.config/systemd/user/**", "~/.config/omarchy/hooks/**", "~/.config/autostart/**",
                    "~/.config/environment.d/**", "~/.config/uwsm/**", "~/.profile", "~/.bash_profile"]
CLASS_ORDER = {"routine": 0, "attention": 1, "crisis": 2}
THEME_CODE = {"alacritty.toml", "foot.ini", "ghostty.conf", "kitty.conf", "vscode.json"}
OMARCHY_LOOKBACK = dt.timedelta(days=31)


def glob_path(subject):
    """A `~`-path as the path globs read it: `~/x` is `/~/x` (engine: class.rs glob_path)."""
    if subject == "~" or subject.startswith("~/"):
        return "/~" + subject[1:]
    return subject


def path_globs(patterns):
    """`[redaction] skipPaths` glob semantics (engine: collectors/config.rs SkipPaths) with the home
    directory as `/~`: a pattern with a `/` matches the whole path, at a directory boundary when it
    is not absolute; one without matches a name; `*`/`?` within a component, `**` across; a match
    on a directory covers everything below it."""
    def rx(p):
        out = ""
        i = 0
        while i < len(p):
            if p.startswith("**", i):
                out += ".*"
                i += 2
                continue
            out += {"*": "[^/]*", "?": "[^/]"}.get(p[i], re.escape(p[i]))
            i += 1
        return out
    globs = []
    for p in (p.strip() for p in patterns):
        if not p:
            continue
        p = p.rstrip("/")
        if "/" not in p and p != "~":
            globs.append(("name", re.compile("^" + rx(p) + "$")))
            continue
        p = glob_path(p)
        globs.append(("path", re.compile(("^" if p.startswith("/") else "(?:^|/)") + rx(p) + "(?:/.*)?$")))
    return globs


def path_match(globs, subject):
    path = glob_path(subject)
    name = path.rsplit("/", 1)[-1]
    return any(r.search(path if kind == "path" else name) for kind, r in globs)


def parse_pacman(command):
    """The pacman-like command line as argv (engine: pkgcmd::parse_command): program, the last
    operation letter, sysupgrade, target names. None for another program."""
    argv = (command or "").split()
    if not argv or os.path.basename(argv[0]) not in ("pacman", "yay", "paru"):
        return None
    prog = os.path.basename(argv[0])
    op, sysupgrade, prints_only, words = None, False, False, []
    i = 1
    while i < len(argv):
        a = argv[i]
        i += 1
        if a == "--":
            words += argv[i:]
            break
        if a.startswith("--"):
            name = a[2:].split("=", 1)[0]
            ops = {"sync": "S", "remove": "R", "upgrade": "U", "database": "D", "query": "Q",
                   "deptest": "T", "files": "F", "yay": "Y", "show": "P", "getpkgbuild": "G"}
            if name in ops:
                op = ops[name]
            elif name in ("help", "version"):
                prints_only = True
            elif name == "sysupgrade":
                sysupgrade = True
            elif "--" + name in PACMAN_LONG_WITH_ARG and "=" not in a:
                i += 1
        elif a.startswith("-") and a != "-":
            for j, ch in enumerate(a[1:], 2):
                if ch in "SRUDQTFYPG":
                    op = ch
                elif ch in "hV":
                    prints_only = True
                elif ch == "u":
                    sysupgrade = True
                elif ch in PACMAN_SHORT_WITH_ARG:
                    if j == len(a):
                        i += 1
                    break
        else:
            words.append(a)
    if op is None and prog != "pacman" and not prints_only:
        op, sysupgrade = "S", sysupgrade or not words

    def name(w):
        if op == "U" and ".pkg.tar" in w:
            f = w.rsplit("/", 1)[-1]
            f = f[:f.index(".pkg.tar")]
            parts = f.rsplit("-", 3)
            return parts[0] if len(parts) == 4 else f
        return re.split(r"[<>=]", w.rsplit("/", 1)[-1])[0]
    targets = [n for n in (name(w) for w in words if w != "-") if n]
    cache = lambda w: (".pkg.tar" in w and "/../" not in w and (w.startswith("/var/cache/pacman/pkg/")
                       or "/.cache/yay/" in w or "/.cache/paru/" in w))
    return {"program": prog, "op": op, "sysupgrade": sysupgrade and op == "S" and not prints_only,
            "targets": targets, "stdin": "-" in words,
            "from_cache": op == "U" and bool(words) and all(cache(w) for w in words)}


def plain_full_upgrade(cmd):
    """No target, none from stdin either (`-`, WP-109 round 2)."""
    return cmd is not None and cmd["op"] == "S" and cmd["sysupgrade"] and not cmd["targets"] and not cmd["stdin"]


def package_shaped(v):
    """`N…-N` (engine: class.rs package_shaped)."""
    if not isinstance(v, str) or "-" not in v:
        return False
    ver, rel_ = v.rsplit("-", 1)
    parts = rel_.split(".")
    return (bool(rel_) and len(parts) <= 2 and all(p.isdigit() and p.isascii() for p in parts)
            and ver[:1].isdigit() and ver[:1].isascii() and not re.search(r"[\s-]", ver))


class Classifier:
    """engine: class.rs Rules + Classifier with the default [drift] config."""
    def __init__(self, events):
        self.routine_paths = path_globs(ROUTINE_PATHS)
        self.red_paths = path_globs(ALWAYS_RED_PATHS)
        self.events = events
        self.explicit = {}
        self.omarchy = []
        for e in events:
            if e["source"] == "pacman" and e.get("explicit") is True and e.get("txId"):
                self.explicit.setdefault(e["txId"], []).append(e)
            if (e["source"] == "pacman" and e["subject"] in ("omarchy", "omarchy-dev")
                    and e["kind"] in ("install", "upgrade")
                    and plain_full_upgrade(parse_pacman(e.get("meta", {}).get("command")))):
                v = e.get("meta", {}).get("to") or e.get("meta", {}).get("version")
                if v:
                    self.omarchy.append((v, instant(e["ts"])))

    def event(self, e, cmd):
        """(class, rule), or None for a dependency of a named transaction (it follows)."""
        src, kind, subject = e["source"], e["kind"], e["subject"]
        meta = e.get("meta", {})
        if src == "pacman":
            red = always_red(subject)
            if plain_full_upgrade(cmd):
                if kind in ("upgrade", "reinstall", "install") or (kind == "remove" and not red):
                    return ("routine", "sysupgrade")
                if red and kind in ("downgrade", "remove"):
                    return ("attention", "sysupgrade-red")
                return ("attention", "downgrade") if kind == "downgrade" else ("attention", "other")
            if (cmd and cmd["op"] in ("S", "U") and not cmd["stdin"] and cmd["targets"]
                    and all(t in ROUTINE_PACKAGES for t in cmd["targets"])):
                return ("routine", "keyring")
            if e.get("explicit") is False:
                return None
            if e.get("explicit") is True:
                if kind in ("upgrade", "reinstall"):
                    if red:
                        return ("attention", "upgrade-red")
                    # `-U` is an upgrade of what is installed only from a package cache (N1)
                    if cmd and cmd["op"] == "U" and not cmd["from_cache"]:
                        return ("attention", "package")
                    return ("routine", "upgrade")
                if kind in ("install", "remove", "downgrade"):
                    return ("crisis", "always-red") if red else ("attention", "package")
            return ("attention", "other")
        if src == "omarchy":
            to, at = meta.get("to"), instant(e["ts"])
            if (kind == "update" and package_shaped(meta.get("from")) and package_shaped(to)
                    and any(v == to and at - OMARCHY_LOOKBACK <= ts <= at for v, ts in self.omarchy)):
                return ("routine", "omarchy-update")
            return ("attention", "omarchy-other")
        if src == "plugins":
            return ("routine", "plugin-toggle") if kind in ("plugin-enable", "plugin-disable") else ("attention", "plugin")
        if src == "theme":
            return ("routine", "theme") if kind == "theme-set" else ("attention", "other")
        if src == "config":
            mark = meta.get("matches")
            removed = kind == "config-remove"
            if not removed and mark in ("omarchy-default", "system-link"):
                return ("routine", mark)
            # the persistence paths right after the evidence rows; a backup there needs evidence
            inert = subject.startswith("~/.config/omarchy/hooks/") and subject.endswith(".sample")
            if not removed and path_match(self.red_paths, subject) and not inert:
                # not under the hooks dir: omarchy-hook runs every file not named *.sample (B5)
                if (path_match(self.routine_paths, subject) and not subject.startswith("~/.config/omarchy/hooks/")
                        and self.holds_base_content(e)):
                    return ("routine", "routine-paths")
                return ("crisis", "always-red-paths")
            if path_match(self.routine_paths, subject):
                return ("routine", "routine-paths")
            if removed:
                return ("attention", "config-remove")
            theme = "~/.config/omarchy/themes/"
            if subject.startswith(theme) and "/" in subject[len(theme):]:
                if mark == "theme-repo":
                    return ("routine", "theme-repo")
                name = subject.rsplit("/", 1)[-1]
                if not name.endswith(".lua") and name not in THEME_CODE:
                    return ("routine", "theme-assets")
            if subject.startswith("~/.config/omarchy/backgrounds/"):
                return ("routine", "theme-assets")
            return ("attention", "config")
        return ("attention", "other")

    def holds_base_content(self, e):
        """engine: class.rs History::holds_base_content."""
        h = e.get("meta", {}).get("hashTo")
        at = e["subject"].rfind(".bak.")
        if not h or at < 0:
            return False
        base = e["subject"][:at]
        if not base or base.endswith("/") or at + 5 == len(e["subject"]):
            return False
        t = instant(e["ts"])
        xs = [x for x in self.events if x["source"] == "config" and x["subject"] == base]
        before = max((x for x in xs if instant(x["ts"]) < t), key=lambda x: (instant(x["ts"]), x["id"]), default=None)
        after = min((x for x in xs if instant(x["ts"]) >= t), key=lambda x: (instant(x["ts"]), x["id"]), default=None)
        return bool((before and before.get("meta", {}).get("hashTo") == h)
                    or (after and after.get("meta", {}).get("hashFrom") == h))

    def follow(self, dep, cmd):
        vs = [v for v in (self.event(x, cmd) for x in self.explicit.get(dep.get("txId"), [])) if v]
        return max(vs, key=lambda v: CLASS_ORDER[v[0]]) if vs else ("attention", "other")

    def group(self, members, lead):
        """The highest class of the members; the rule of the lead, else of the lowest id with it."""
        cmd = parse_pacman(lead.get("meta", {}).get("command")) if lead["source"] == "pacman" else None
        best = None
        for m in members:
            v = self.event(m, cmd) or self.follow(m, cmd)
            key = (CLASS_ORDER[v[0]], m["id"] == lead["id"], [-ord(c) for c in m["id"]])
            if best is None or key > best[0]:
                best = (key, v)
        return best[1]


def group_lead(members):
    explicit = [m for m in members if m.get("explicit") is True]
    return min(explicit or members, key=lambda m: m["id"])


CASE_KEYS = ["id", "title", "status", "zone", "risk", "priority", "area", "created", "started", "closed",
             "snapshotBefore", "agents", "events", "tags", "path", "steps", "proposedEvents"]


class Fail(Exception):
    pass


# --------------------------------------------------------------------------- schemas

def load_schemas():
    out = {}
    for path in sorted(glob.glob(os.path.join(SCHEMA_DIR, "**", "*.json"), recursive=True)):
        with open(path, encoding="utf-8") as f:
            s = json.load(f)
        sid = s.get("$id")
        if not sid:
            raise Fail(f"{rel(path)}: schema has no $id")
        out[sid] = (s, path)
    return out


def rel(p):
    return os.path.relpath(p, ROOT)


DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")
DATETIME_RE = re.compile(r"^\d{4}-\d{2}-\d{2}[Tt]\d{2}:\d{2}:\d{2}(\.\d+)?([Zz]|[+-]\d{2}:\d{2})$")


def check_format(fmt, value):
    if not isinstance(value, str):
        return True
    try:
        if fmt == "date":
            return bool(DATE_RE.match(value)) and dt.date.fromisoformat(value) is not None
        if fmt == "date-time":
            return bool(DATETIME_RE.match(value)) and dt.datetime.fromisoformat(value.replace("Z", "+00:00")) is not None
    except ValueError:
        return False
    return True  # unknown formats are annotations


class Builtin:
    """Draft 2020-12 subset sufficient for schema/**. Unknown keywords are errors."""
    name = "builtin"
    ANNOTATIONS = {"$schema", "$id", "$defs", "title", "description", "default", "$comment", "examples"}

    def __init__(self, schemas):
        self.docs = {sid: s for sid, (s, _) in schemas.items()}

    def validate(self, instance, sid):
        errs = []
        self._v(instance, self.docs[sid], sid, "", errs)
        return errs

    def _resolve(self, base, ref):
        target = urllib.parse.urljoin(base, ref)
        doc_id, _, frag = target.partition("#")
        if doc_id not in self.docs:
            raise Fail(f"builtin validator: unresolvable $ref {ref} (from {base})")
        node = self.docs[doc_id]
        for part in [p for p in frag.split("/") if p]:
            part = part.replace("~1", "/").replace("~0", "~")
            node = node[int(part)] if isinstance(node, list) else node[part]
        return node, doc_id

    @staticmethod
    def _type_ok(t, x):
        if t == "integer":
            return (isinstance(x, int) and not isinstance(x, bool)) or (isinstance(x, float) and x.is_integer())
        if t == "number":
            return isinstance(x, (int, float)) and not isinstance(x, bool)
        return isinstance(x, {"string": str, "boolean": bool, "null": type(None), "array": list, "object": dict}[t])

    @staticmethod
    def _same(a, b):
        """JSON equality: booleans are not numbers."""
        if isinstance(a, bool) or isinstance(b, bool):
            return isinstance(a, bool) and isinstance(b, bool) and a == b
        return a == b

    def _v(self, x, s, base, path, errs):
        if s is True or s == {}:
            return
        if s is False:
            errs.append(f"{path or '/'}: not allowed")
            return
        where = path or "/"
        for k in s:
            if k in self.ANNOTATIONS or k in ("then", "else"):
                continue
            v = s[k]
            if k == "$ref":
                node, doc = self._resolve(base, v)
                self._v(x, node, doc, path, errs)
            elif k == "type":
                ts = v if isinstance(v, list) else [v]
                if not any(self._type_ok(t, x) for t in ts):
                    errs.append(f"{where}: expected type {v}, got {type(x).__name__}")
            elif k == "enum":
                if not any(self._same(x, e) for e in v):
                    errs.append(f"{where}: {json.dumps(x, ensure_ascii=False)[:60]} not in enum")
            elif k == "const":
                if not self._same(x, v):
                    errs.append(f"{where}: expected const {v!r}")
            elif k == "required":
                if isinstance(x, dict):
                    for r in v:
                        if r not in x:
                            errs.append(f"{where}: missing required '{r}'")
            elif k == "properties":
                if isinstance(x, dict):
                    for pk, ps in v.items():
                        if pk in x:
                            self._v(x[pk], ps, base, f"{path}/{pk}", errs)
            elif k == "additionalProperties":
                if isinstance(x, dict):
                    known = set(s.get("properties", {}))
                    for pk in x:
                        if pk not in known:
                            if v is False:
                                errs.append(f"{where}: unexpected property '{pk}'")
                            else:
                                self._v(x[pk], v, base, f"{path}/{pk}", errs)
            elif k == "propertyNames":
                if isinstance(x, dict):
                    for pk in x:
                        self._v(pk, v, base, f"{path}/{pk}(name)", errs)
            elif k == "items":
                if isinstance(x, list):
                    for i, item in enumerate(x):
                        self._v(item, v, base, f"{path}/{i}", errs)
            elif k == "maxItems":
                if isinstance(x, list) and len(x) > v:
                    errs.append(f"{where}: more than {v} items")
            elif k == "minItems":
                if isinstance(x, list) and len(x) < v:
                    errs.append(f"{where}: fewer than {v} items")
            elif k == "maxLength":
                if isinstance(x, str) and len(x) > v:
                    errs.append(f"{where}: longer than {v}")
            elif k == "minLength":
                if isinstance(x, str) and len(x) < v:
                    errs.append(f"{where}: shorter than {v}")
            elif k == "minimum":
                if self._type_ok("number", x) and x < v:
                    errs.append(f"{where}: below minimum {v}")
            elif k == "pattern":
                if isinstance(x, str) and not re.search(v, x):
                    errs.append(f"{where}: {x[:60]!r} does not match {v}")
            elif k == "format":
                if not check_format(v, x):
                    errs.append(f"{where}: {x!r} is not a valid {v}")
            elif k == "allOf":
                for sub in v:
                    self._v(x, sub, base, path, errs)
            elif k in ("anyOf", "oneOf"):
                ok = 0
                for sub in v:
                    e = []
                    self._v(x, sub, base, path, e)
                    ok += not e
                if (k == "anyOf" and ok == 0) or (k == "oneOf" and ok != 1):
                    errs.append(f"{where}: does not match {k}")
            elif k == "not":
                e = []
                self._v(x, v, base, path, e)
                if not e:
                    errs.append(f"{where}: matches a 'not' schema")
            elif k == "if":
                e = []
                self._v(x, v, base, path, e)
                branch = s.get("then") if not e else s.get("else")
                if branch is not None:
                    self._v(x, branch, base, path, errs)
            else:
                raise Fail(f"builtin validator does not implement keyword '{k}' — install python-jsonschema "
                           f"or check-jsonschema, or extend scripts/validate-fixtures.py")


class LibJsonschema:
    name = "jsonschema"

    def __init__(self, schemas):
        import jsonschema
        self.js = jsonschema
        self.schemas = {sid: s for sid, (s, _) in schemas.items()}
        for s in self.schemas.values():
            jsonschema.Draft202012Validator.check_schema(s)
        cls = jsonschema.Draft202012Validator
        fc = cls.FORMAT_CHECKER
        try:
            from referencing import Registry, Resource
            reg = Registry().with_resources(
                [(sid, Resource.from_contents(s)) for sid, s in self.schemas.items()])
            self.make = lambda sid: cls(self.schemas[sid], registry=reg, format_checker=fc)
        except ImportError:  # jsonschema < 4.18
            def make(sid):
                res = jsonschema.RefResolver.from_schema(self.schemas[sid], store=dict(self.schemas))
                return cls(self.schemas[sid], resolver=res, format_checker=fc)
            self.make = make

    def validate(self, instance, sid):
        errs = []
        for e in self.make(sid).iter_errors(instance):
            p = "/" + "/".join(str(x) for x in e.absolute_path)
            errs.append(f"{p}: {e.message[:200]}")
        return errs


class CheckJsonschema:
    """check-jsonschema CLI. Schemas are copied to a temp dir with file:// ids so no $ref goes to the network."""
    name = "check-jsonschema"

    def __init__(self, schemas, exe):
        self.exe = exe
        self.tmp = tempfile.mkdtemp(prefix="seldon-schemas-")
        self.paths = {}
        for sid, (s, path) in schemas.items():
            r = os.path.relpath(path, SCHEMA_DIR)
            dst = os.path.join(self.tmp, "schema", r)
            os.makedirs(os.path.dirname(dst), exist_ok=True)
            s2 = dict(s)
            s2["$id"] = "file://" + dst
            with open(dst, "w", encoding="utf-8") as f:
                json.dump(s2, f)
            self.paths[sid] = dst
        self.n = 0

    def validate(self, instance, sid):
        self.n += 1
        inst = os.path.join(self.tmp, f"instance-{self.n}.json")
        with open(inst, "w", encoding="utf-8") as f:
            json.dump(instance, f, ensure_ascii=False)
        r = subprocess.run([self.exe, "--schemafile", self.paths[sid], inst], capture_output=True, text=True)
        if r.returncode == 0:
            return []
        return [(r.stdout + r.stderr).strip().replace(inst, "<instance>")[:2000]]


def pick_backend(schemas, wanted):
    wanted = wanted or os.environ.get("SELDON_SCHEMA_VALIDATOR", "auto")
    if wanted in ("auto", "jsonschema"):
        try:
            return LibJsonschema(schemas)
        except ImportError:
            if wanted == "jsonschema":
                raise Fail("python module jsonschema not available")
    if wanted in ("auto", "check-jsonschema"):
        exe = shutil.which("check-jsonschema")
        if exe:
            return CheckJsonschema(schemas, exe)
        if wanted == "check-jsonschema":
            raise Fail("check-jsonschema not on PATH")
    if wanted in ("auto", "builtin"):
        return Builtin(schemas)
    raise Fail(f"unknown validator '{wanted}'")


# --------------------------------------------------------------------------- logbook parsing

FM_KEY = re.compile(r"^([A-Za-z][A-Za-z0-9]*):(?:\s+(.*))?$")


def scalar(v):
    v = v.strip()
    if v == "":
        return None
    if v.startswith('"'):
        if not v.endswith('"') or len(v) < 2:
            raise Fail(f"unterminated string {v!r}")
        return json.loads(v)
    if v in ("true", "false"):
        return v == "true"
    if re.fullmatch(r"-?\d+", v):
        return int(v)
    return v


def frontmatter(path):
    """Flat YAML frontmatter as Seldon writes it: key: scalar | [a, b] | empty. Dates stay strings."""
    with open(path, encoding="utf-8") as f:
        text = f.read()
    if not text.startswith("---\n"):
        raise Fail(f"{rel(path)}: no frontmatter")
    head, sep, body = text[4:].partition("\n---\n")
    if not sep:
        raise Fail(f"{rel(path)}: unterminated frontmatter")
    out = {}
    for n, line in enumerate(head.splitlines(), 2):
        m = FM_KEY.match(line)
        if not m:
            raise Fail(f"{rel(path)}:{n}: unsupported frontmatter line {line!r}")
        k, v = m.group(1), (m.group(2) or "").strip()
        if v.startswith("["):
            if not v.endswith("]"):
                raise Fail(f"{rel(path)}:{n}: unsupported list {v!r}")
            inner = v[1:-1].strip()
            out[k] = [str(scalar(x)) for x in inner.split(",")] if inner else []
        else:
            out[k] = scalar(v)
    return out, body


def fences(path):
    with open(path, encoding="utf-8") as f:
        text = f.read()
    out = {}
    for m in re.finditer(r"<!-- seldon:begin ([a-z0-9.-]+) -->\n(.*?)<!-- seldon:end -->", text, re.S):
        out[m.group(1)] = m.group(2)
    return out


def fence_kv(text):
    out = {}
    for line in text.splitlines():
        m = re.match(r"^- ([A-Za-z][A-Za-z0-9]*): (.*)$", line)
        if m:
            out[m.group(1)] = m.group(2).strip()
    return out


def fence_table(text):
    rows = [l for l in text.splitlines() if l.startswith("|")]
    if len(rows) < 2:
        return []
    head = [c.strip() for c in rows[0].strip("|").split("|")]
    return [dict(zip(head, [c.strip() for c in r.strip("|").split("|")])) for r in rows[2:]]


HEADING = re.compile(r"^## ([0-2][0-9]:[0-5][0-9]) · (human|system|agent:[a-z0-9-]+)(?: · (C-[0-9]{4}-[0-9]{3,}))?\s*$")


def journal(path):
    if not os.path.exists(path):
        return []
    _, body = frontmatter(path)
    entries, cur = [], None
    for line in body.splitlines():
        if line.startswith("## "):
            m = HEADING.match(line)
            if not m:
                raise Fail(f"{rel(path)}: bad journal heading {line!r}")
            cur = {"time": m.group(1), "actor": m.group(2), "case": m.group(3), "text": []}
            entries.append(cur)
        elif cur is not None:
            cur["text"].append(line)
    for e in entries:
        e["text"] = "\n".join(e["text"]).strip()
    return entries


def strip_comments(text):
    """engine: cases::strip_comments (an unclosed comment runs to the end)."""
    return re.sub(r"<!--.*?(?:-->|\Z)", "", text, flags=re.S)


def plan_section(body):
    """The `## Plan` text without HTML comments: a template placeholder is no plan (rules 3 and 9,
    WP-115 round 2)."""
    m = re.search(r"^## Plan\n(.*?)(?=^## |\Z)", body, re.S | re.M)
    return strip_comments(m.group(1)) if m else ""


LOG_RISK = re.compile(r"^- (\d{4}-\d{2}-\d{2} \d{2}:\d{2}) · (.*) · \S+\s*$")


def risk_timeline(body):
    """engine: reconcile::risk_timeline — [(minute, risk)] from the `created` Log line and every
    `set … risk A → B`; None without a `created` line that names a risk."""
    m = re.search(r"^## Log\n(.*?)(?=^## |\Z)", body, re.S | re.M)
    out = []
    for line in (m.group(1) if m else "").splitlines():
        lm = LOG_RISK.match(line)
        if not lm:
            continue
        at = dt.datetime.strptime(lm.group(1), "%Y-%m-%d %H:%M")
        r = re.match(r"created\b.*\brisk (R[0-3])\b", lm.group(2)) if not out else \
            re.match(r"set .*\brisk R[0-3] → (R[0-3])\b", lm.group(2))
        if r:
            out.append((at, r.group(1)))
    return out or None


def risk_at(risks, at):
    """engine: PlanningCase::risk_at — the risk at local time `at`, None when it cannot be told."""
    if risks is None:
        return None
    minute = at.replace(second=0, microsecond=0)
    before = [r for m, r in risks if m < minute][-1:]
    values = before + [r for m, r in risks if m == minute]
    return values[0] if values and all(v == values[0] for v in values) else None


def instant(ts):
    return dt.datetime.fromisoformat(ts.replace("Z", "+00:00"))


def order_event(e):
    return {k: e[k] for k in EVENT_KEYS if k in e}


# --------------------------------------------------------------------------- derivation (ADR-0012)

def json_len(c):
    """Bytes of one character in a JSON string as serde_json writes it (UTF-8; it escapes only
    these)."""
    if c in '"\\\n\r\t\b\f':
        return 2
    if ord(c) < 0x20:
        return 6
    return len(c.encode("utf-8"))


def clip(text):
    """`text` as the index carries it (ADR-0025): unchanged when it takes at most TEXT_MAX bytes
    in JSON, else its start, cut on a character boundary and stripped of trailing white space,
    followed by `… (N more characters in the ledger)`, N the characters left out."""
    if sum(json_len(c) for c in text) <= TEXT_MAX:
        return text

    def marker(left):
        return f"… ({left} more {'character' if left == 1 else 'characters'} in the ledger)"
    # the marker for every character is at least as long as the real one
    room = TEXT_MAX - len(marker(len(text)).encode("utf-8"))
    used, end = 0, 0
    for i, c in enumerate(text):
        used += json_len(c)
        if used > room:
            break
        end = i + 1
    head = text[:end].rstrip(WHITE_SPACE)
    return head + marker(len(text) - len(head))


def clipped(e):
    """An event as `index.events` lists it: every free text clipped (ADR-0025)."""
    e = copy.deepcopy(e)
    for k in ("detail", "resolutionDetail"):
        if isinstance(e.get(k), str):
            e[k] = clip(e[k])
    for k, v in e.get("meta", {}).items():
        if isinstance(v, str):
            e["meta"][k] = clip(v)
    return e


def load_logbook(lb, problems, mutate=None, mutate_cases=None):
    """Self-checks only: mutate(ledger) may edit or append (where, event) pairs, and
    mutate_cases(cases) may return changed (path, frontmatter, body) triples, before derivation."""
    ledger = []
    for f in sorted(glob.glob(os.path.join(lb, "ledger", "*.jsonl"))):
        month = os.path.basename(f)[:-6]
        with open(f, encoding="utf-8") as fh:
            for n, line in enumerate(fh, 1):
                if not line.strip():
                    problems.append(f"{rel(f)}:{n}: empty line")
                    continue
                e = json.loads(line)
                if e.get("ts", "")[:7] != month:
                    problems.append(f"{rel(f)}:{n}: ts {e.get('ts')} not in month file {month}")
                ledger.append((f"{rel(f)}:{n}", e))
    if mutate:
        mutate(ledger)
    cases = []
    for f in sorted(glob.glob(os.path.join(lb, "work", "*", "C-*.md"))):
        fm, body = frontmatter(f)
        cases.append((f, fm, body))
    if mutate_cases:
        cases = mutate_cases(cases)
    return ledger, cases


def derive(lb, today, problems, mutate=None, mutate_cases=None, legacy=False):
    """The index parts that come from the logbook. `legacy`: `[drift] attention = "all"`, the drift
    derivation before ADR-0028 (computed pacman zone, crisis iff red, every caseless event open)."""
    ledger, case_files = load_logbook(lb, problems, mutate, mutate_cases)
    events = [e for _, e in ledger]
    by_id = {}
    for where, e in ledger:
        if e["id"] in by_id:
            problems.append(f"{where}: duplicate id {e['id']}")
        by_id[e["id"]] = e
    seen = set()
    resolutions = {}
    for where, e in ledger:
        if "resolution" in e and e["kind"] != "resolution":
            problems.append(f"{where}: 'resolution' field on a {e['kind']} event (ledger lines carry it only on kind resolution)")
        if "resolutionDetail" in e:
            problems.append(f"{where}: 'resolutionDetail' is index-only (ADR-0012 §11)")
        if "refersTo" in e:
            if e["refersTo"] not in seen:
                problems.append(f"{where}: refersTo {e['refersTo']} is not an earlier ledger event")
            elif e["kind"] == "resolution":
                tgt = by_id[e["refersTo"]]
                if tgt["source"] not in DRIFT_SOURCES or "case" in tgt:
                    problems.append(f"{where}: resolves {tgt['id']}, which was never drift")
                if e["subject"] != tgt["subject"]:
                    problems.append(f"{where}: resolution subject differs from its target's")
                tx = e.get("meta", {}).get("txId")
                if tx is not None and tx != tgt.get("txId"):
                    problems.append(f"{where}: meta.txId {tx} != its target's txId {tgt.get('txId')} (ADR-0013 §4)")
                resolutions[e["refersTo"]] = e
        seen.add(e["id"])

    # cases
    cases_by_id = {}
    for f, fm, body in case_files:
        r = os.path.relpath(f, lb)
        folder = r.split(os.sep)[1]
        want = {"queued": "queued", "active": "active", "verification": "active",
                "completed": "completed", "dropped": "completed"}.get(fm.get("status"))
        if folder != want:
            problems.append(f"{rel(f)}: status {fm.get('status')} belongs in work/{want}/")
        if not os.path.basename(f).startswith(str(fm.get("id")) + "-"):
            problems.append(f"{rel(f)}: file name does not start with its id")
        cases_by_id[fm["id"]] = (r, fm, body)
    for where, e in ledger:
        if e.get("case") and e["case"] not in cases_by_id:
            problems.append(f"{where}: unknown case {e['case']}")

    # events, resolutions folded (newest first)
    folded = []
    for e in events:
        if e["kind"] == "resolution":
            continue
        e = copy.deepcopy(e)
        r = resolutions.get(e["id"])
        if r:
            e["resolution"] = r["resolution"]
            if "detail" in r:
                e["resolutionDetail"] = r["detail"]
            if r.get("case"):  # ADR-0021: linked or explained, whenever the line carries one
                e["case"] = r["case"]
        folded.append(order_event(e))
    folded.sort(key=lambda e: (instant(e["ts"]), e["id"]), reverse=True)

    open_cases = sorted((cid for cid, (_, fm, _) in cases_by_id.items()
                         if fm["status"] in ("queued", "active", "verification")))

    def proposed(subject):
        # ADR-0012 §7, ADR-0015 §4 (the reference implementation of the token rule)
        pat = token_pattern(subject)
        for cid in open_cases:
            if pat.search(plan_section(cases_by_id[cid][2])):
                return cid
        return None

    # linkable events (drift eligible, no case, no resolution); caseless pacman events of one
    # transaction form one item (ADR-0013 §1); its class is the highest of its members (ADR-0028 §2)
    classifier = Classifier(events)
    items, by_tx = [], {}
    for e in folded:
        if e["source"] in DRIFT_SOURCES and "case" not in e and "resolution" not in e:
            if e["source"] == "pacman" and e.get("txId"):
                if e["txId"] not in by_tx:
                    by_tx[e["txId"]] = []
                    items.append(by_tx[e["txId"]])
                by_tx[e["txId"]].append(e)
            else:
                items.append([e])
    drift = []
    for members in items:
        lead = group_lead(members)
        d = {"eventId": lead["id"], "ts": lead["ts"], "source": lead["source"], "kind": lead["kind"],
             "subject": lead["subject"], "detail": lead.get("detail"), "actor": lead["actor"]}
        if d["detail"] is not None:
            d["detail"] = clip(d["detail"])
        d["proposedCase"] = proposed(lead["subject"])
        if legacy:
            if lead["source"] == "pacman":
                d["zone"] = "yellow" if all(routine(m) for m in members) else "red"
            elif "zone" in lead:
                d["zone"] = lead["zone"]
            d["crisis"] = d.get("zone") == "red"
        else:
            cls, _ = classifier.group(members, lead)
            if cls == "routine" and d["proposedCase"] is None:
                continue  # history, not drift (ADR-0028 §3)
            if "zone" in lead:
                d["zone"] = lead["zone"]  # the ledger zone (ADR-0028 §7)
            d["crisis"] = cls == "crisis"
        if len(members) > 1:
            d["txId"], d["members"] = lead["txId"], len(members)
        drift.append({k: d[k] for k in DRIFT_KEYS if d.get(k) is not None or k == "proposedCase"})
    drift.sort(key=lambda d: (instant(d["ts"]), d["eventId"]), reverse=True)

    # case objects
    attributed = {}
    for e in sorted(folded, key=lambda e: (instant(e["ts"]), e["id"])):
        if e["source"] != "seldon" and e.get("case"):
            attributed.setdefault(e["case"], []).append(e["id"])
    groups = {"queued": [], "active": [], "verification": [], "completed": []}
    for cid in sorted(cases_by_id):
        r, fm, body = cases_by_id[cid]
        if fm.get("type") != "case":
            problems.append(f"{r}: frontmatter type must be 'case'")
        if fm.get("events", []) != attributed.get(cid, []):
            problems.append(f"{r}: frontmatter events {fm.get('events')} != ledger-attributed {attributed.get(cid, [])}")
        plan = plan_section(body)
        c = {k: v for k, v in fm.items() if k != "type"}
        c["path"] = r.replace(os.sep, "/")
        c["steps"] = {"total": len(re.findall(r"^\s*- \[[ xX]\] ", plan, re.M)),
                      "done": len(re.findall(r"^\s*- \[[xX]\] ", plan, re.M))}
        prop = [d["eventId"] for d in drift if d["proposedCase"] == cid]
        if prop:
            c["proposedEvents"] = prop
        c = {k: c[k] for k in CASE_KEYS if k in c}
        g = "completed" if fm["status"] in ("completed", "dropped") else fm["status"]
        groups[g].append(c)
    groups["completed"].sort(key=lambda c: (c.get("closed") or "", c["id"]), reverse=True)
    groups["completed"] = groups["completed"][:50]
    all_cases = [c for g in groups.values() for c in g]

    # summary
    day = lambda e: dt.date.fromisoformat(e["ts"][:10])
    week_ago = today - dt.timedelta(days=6)
    summary = {
        "activeCases": len(groups["active"]), "queuedCases": len(groups["queued"]),
        "openDrift": len(drift), "crisis": sum(d["crisis"] for d in drift),
        "eventsToday": sum(day(e) == today for e in folded),
        "events7d": sum(week_ago <= day(e) <= today for e in folded),
    }

    # today
    def jpath(d):
        return f"journal/{d.year:04d}/{d.isoformat()}.md"
    tpath = jpath(today)
    today_obj = {"date": today.isoformat(), "path": tpath,
                 "entries": journal(os.path.join(lb, tpath)),
                 "yesterday": journal(os.path.join(lb, jpath(today - dt.timedelta(days=1))))}

    # decisions
    decisions = []
    for f in sorted(glob.glob(os.path.join(lb, "decisions", "ADR-*.md")), reverse=True):
        fm, _ = frontmatter(f)
        decisions.append({"id": fm["id"], "title": fm["title"], "status": fm["status"], "date": fm["date"],
                          "path": os.path.relpath(f, lb).replace(os.sep, "/")})

    # system
    sysdir = os.path.join(lb, "system")
    om = fence_kv(fences(os.path.join(sysdir, "omarchy.md"))["omarchy.summary"])
    pk = {k: int(v) for k, v in fence_kv(fences(os.path.join(sysdir, "packages.md"))["packages.summary"]).items()}
    pl = fence_table(fences(os.path.join(sysdir, "plugins.md"))["plugins.list"])
    dev = fence_table(fences(os.path.join(sysdir, "deviations.md"))["deviations.table"])
    deleted = {e["subject"] for e in events if e["source"] == "snapper" and e["kind"] == "snapshot-delete"}
    snaps = []
    for e in sorted(events, key=lambda e: (instant(e["ts"]), e["id"]), reverse=True):
        if e["source"] == "snapper" and e["kind"] == "snapshot" and e["subject"] not in deleted:
            s = {"number": int(e["subject"]), "ts": e["ts"]}
            if "detail" in e:
                s["description"] = e["detail"]
            if "type" in e.get("meta", {}):
                s["type"] = e["meta"]["type"]
            snaps.append(s)
    areas = []
    for d in sorted(glob.glob(os.path.join(lb, "areas", "*", "README.md"))):
        name = os.path.basename(os.path.dirname(d))
        areas.append({"name": name, "hasAgentsMd": os.path.exists(os.path.join(os.path.dirname(d), "AGENTS.md")),
                      "cases": sum(c.get("area") == name for c in all_cases)})
    system = {
        "omarchy": {"version": om["version"], "theme": om["theme"], "lastUpdate": om.get("lastUpdate") or None},
        "packages": pk,
        "deviations": len(dev),
        "snapshots": snaps[:10],
        "plugins": {"enabled": sum(r["enabled"] == "yes" for r in pl), "installed": len(pl)},
        "areas": areas,
    }
    upd = [e for e in folded if e["source"] == "omarchy" and e["kind"] == "update"]
    if upd and upd[0].get("meta", {}).get("to") != om["version"]:
        problems.append(f"dossier omarchy.version {om['version']} != last update event {upd[0]['meta'].get('to')}")
    if upd and upd[0]["ts"] != om.get("lastUpdate"):
        problems.append(f"dossier omarchy.lastUpdate {om.get('lastUpdate')} != last update event {upd[0]['ts']}")
    th = [e for e in folded if e["source"] == "theme" and e["kind"] == "theme-set"]
    if th and th[0]["subject"] != om["theme"]:
        problems.append(f"dossier omarchy.theme {om['theme']} != last theme-set {th[0]['subject']}")

    # memory
    memdir = os.path.join(lb, "memory")
    _, lbody = frontmatter(os.path.join(memdir, "lessons.md"))
    lessons = [l[3:].strip() for l in lbody.splitlines() if l.startswith("## ")]
    topics = []
    for f in sorted(glob.glob(os.path.join(memdir, "*.md"))):
        if os.path.basename(f) == "lessons.md":
            continue
        fm, _ = frontmatter(f)
        topics.append({"topic": fm["topic"], "updated": fm["updated"], "path": os.path.relpath(f, lb).replace(os.sep, "/")})
    topics.sort(key=lambda t: t["topic"])
    topics.sort(key=lambda t: t["updated"], reverse=True)

    # series
    heat = []
    for i in range(365, -1, -1):
        d = today - dt.timedelta(days=i)
        evs = [e for e in folded if day(e) == d]
        h = {"date": d.isoformat(), "total": len(evs)}
        if evs:
            by = {}
            for e in evs:
                by[e["source"]] = by.get(e["source"], 0) + 1
            h["bySource"] = dict(sorted(by.items()))
        heat.append(h)
    hist = [{"date": r["date"], "explicit": int(r["explicit"]), "total": int(r["total"])}
            for r in fence_table(fences(os.path.join(sysdir, "packages.md"))["packages.history"])]

    def week(d):
        y, w, _ = d.isocalendar()
        return f"{y}-W{w:02d}"
    weeks = []
    if events:
        d = min(day(e) for e in events)
        d -= dt.timedelta(days=d.weekday())
        while d <= today:
            weeks.append(week(d))
            d += dt.timedelta(days=7)
    # ADR-0013 §4: drift items, not lines. A caseless pacman transaction opens one item (in the
    # week of its earliest line); the resolution lines of one group write (same meta.txId, ts and
    # actor) count as one. ADR-0028 §5: a routine group opens nothing (proposals aside), and a
    # resolution counts only when its target opened an item.
    sgroups, gby = [], {}
    for e in sorted(events, key=lambda e: (instant(e["ts"]), e["id"])):
        if e["source"] in DRIFT_SOURCES and "case" not in e and e["kind"] != "resolution":
            k = ("tx", e["txId"]) if e["source"] == "pacman" and e.get("txId") else e["id"]
            if k not in gby:
                gby[k] = []
                sgroups.append(gby[k])
            gby[k].append(e)
    opened_ids = {m["id"] for g in sgroups
                  if legacy or classifier.group(g, group_lead(g))[0] != "routine" for m in g}
    first = {}
    for e in sorted(events, key=lambda e: (instant(e["ts"]), e["id"])):
        if e["source"] in DRIFT_SOURCES and "case" not in e and e["kind"] != "resolution":
            if e["id"] in opened_ids:
                first.setdefault(("tx", e["txId"]) if e["source"] == "pacman" and e.get("txId") else e["id"], e)
        elif e["kind"] == "resolution" and e.get("refersTo") in opened_ids:
            tx = e.get("meta", {}).get("txId")
            first.setdefault(("res", tx, e["ts"], e["actor"]) if tx else e["id"], e)
    opened = [week(day(e)) for e in first.values() if e["kind"] != "resolution"]
    resolved_w = [week(day(e)) for e in first.values() if e["kind"] == "resolution"]
    drift_series = [{"week": w, "opened": opened.count(w), "resolved": resolved_w.count(w)} for w in weeks]
    risk = {r: sum(c["risk"] == r for c in all_cases) for r in ("R0", "R1", "R2", "R3")}
    timeline = []
    for e in upd:
        to = e.get("meta", {}).get("to", "")
        timeline.append({"kind": "release", "ts": e["ts"], "label": f"Omarchy {to}", "ref": to})
    for s in system["snapshots"]:
        timeline.append({"kind": "snapshot", "ts": s["ts"],
                         "label": f"{s['number']} {s.get('description', '')}".strip(), "ref": str(s["number"])})
    for c in all_cases:
        if c["status"] != "dropped":
            timeline.append({"kind": "case", "ts": c["created"], "end": c.get("closed"),
                             "label": f"{c['id']} {c['title']}", "ref": c["id"]})
    for d in drift:
        if d["crisis"]:
            timeline.append({"kind": "crisis", "ts": d["ts"], "label": f"{d['source']} {d['kind']} {d['subject']}",
                             "ref": d["eventId"]})
    korder = {"release": 0, "snapshot": 1, "case": 2, "crisis": 3}
    timeline.sort(key=lambda t: (t["ts"], korder[t["kind"]], t["ref"]))

    pfm, _ = frontmatter(os.path.join(lb, "PROJECT.md"))
    return {
        "logbook": {"language": pfm["language"], "machine": pfm["machineId"]},
        "summary": summary,
        "today": today_obj,
        "events": [clipped(e) for e in folded[:500]],
        "drift": drift,
        "cases": groups,
        "decisions": decisions,
        "system": system,
        "memory": {"lessons": lessons, "topics": topics},
        "series": {"heatmap": heat, "packages": hist, "drift": drift_series, "risk": risk, "timeline": timeline},
    }, ledger, case_files


# --------------------------------------------------------------------------- case lifecycle (SPEC-LOGBOOK §3)

LOG_LINE = re.compile(r"^- (\d{4}-\d{2}-\d{2}) (\d{2}:\d{2}) · (.*) · (human|system|agent:[a-z0-9-]+)$")
# Log word → (ledger kind, statuses it may start from, status after); the engine's
# `Transition::target`. `created` opens a case as queued.
CASE_STEPS = {
    "started": ("case-started", ("queued",), "active"),
    "verification": ("case-verified", ("active",), "verification"),
    "completed": ("case-completed", ("verification",), "completed"),
    "dropped": ("case-dropped", ("queued", "active", "verification"), "dropped"),
}


def check_case_logs(ledger, case_files):
    """Walk every case's Log lines through the state machine `queued → active → verification →
    completed`, open → `dropped`. The walk must end in the frontmatter status; its steps must be
    the case's `case-*` ledger events (same kind, minute and actor, in order); `created`, `started`
    and `closed` are the dates of their steps; `started (snapshot N)` is `snapshotBefore`."""
    out = []
    for f, fm, body in case_files:
        where = rel(f)
        m = re.search(r"^## Log\n(.*?)(?=^## |\Z)", body, re.S | re.M)
        status, steps, dates, snapshot = None, [], {}, None
        for line in (m.group(1) if m else "").splitlines():
            if not line.startswith("- "):
                continue
            lm = LOG_LINE.match(line)
            if not lm:
                out.append(f"{where}: bad Log line {line!r}")
                continue
            day, hm, text, actor = lm.groups()
            word = re.match(r"[a-z]*", text).group(0)
            if word == "created":
                if status is not None:
                    out.append(f"{where}: Log 'created' twice")
                status = "queued"
                steps.append(("case-created", f"{day} {hm}", actor))
                dates["created"] = day
            elif word in CASE_STEPS:
                kind, allowed, to = CASE_STEPS[word]
                if status not in allowed:
                    out.append(f"{where}: Log '{word}' from {status}; SPEC-LOGBOOK §3 allows it only from "
                               f"{' | '.join(allowed)}")
                status = to
                steps.append((kind, f"{day} {hm}", actor))
                if word == "started":
                    dates["started"] = day
                    sm = re.match(r"started \(snapshot (\d+)\)$", text)
                    snapshot = int(sm.group(1)) if sm else None
                elif word in ("completed", "dropped"):
                    dates["closed"] = day
        if status != fm.get("status"):
            out.append(f"{where}: the Log ends in {status}, frontmatter says {fm.get('status')}")
        events = [(e["kind"], e["ts"][:16].replace("T", " "), e["actor"]) for _, e in ledger
                  if e["source"] == "seldon" and e["kind"].startswith("case-") and e["subject"] == fm.get("id")]
        if events != steps:
            out.append(f"{where}: Log steps {steps} != ledger case events {events}")
        for k in ("created", "started", "closed"):
            if fm.get(k) != dates.get(k):
                out.append(f"{where}: frontmatter {k} {fm.get(k)} != Log {dates.get(k)}")
        if "started" in dates and fm.get("snapshotBefore") != snapshot:
            out.append(f"{where}: snapshotBefore {fm.get('snapshotBefore')} != Log 'started' snapshot {snapshot}")
    return out


# SPEC-ENGINE §5 rule 9 (ADR-0029 §1): the detail of the engine's planned-and-active link.
PLANNED_DETAIL = "planned by {}; active at the time"


def case_windows(events):
    """ADR-0029 §1a: every case's windows [start, end] (end None while open), from its
    `case-started` to the next `case-completed`/`case-dropped`, read from the ledger alone."""
    steps = sorted((e for e in events if e["source"] == "seldon"
                    and e["kind"] in ("case-started", "case-completed", "case-dropped")),
                   key=lambda e: instant(e["ts"]))
    out = {}
    for e in steps:
        ws = out.setdefault(e["subject"], [])
        if e["kind"] == "case-started":
            if not ws or ws[-1][1] is not None:
                ws.append([instant(e["ts"]), None])
        elif ws and ws[-1][1] is None:
            ws[-1][1] = instant(e["ts"])
    return out


def planned_links(events, case_files):
    """Rule 9, the reference of engine/src/reconcile.rs planned_links(): ([(event, case id,
    txId or None)] oldest first, {case id: [Log line text]}) for the ledger `events` and the case
    files [(path, frontmatter, body)] as they are."""
    windows = case_windows(events)
    resolved = {e["refersTo"] for e in events if e["kind"] == "resolution" and "refersTo" in e}

    def open_(e):
        return (e["source"] in DRIFT_SOURCES and e["kind"] != "resolution" and "case" not in e
                and e["id"] not in resolved)

    def tx(e):
        return e.get("txId") if e["source"] == "pacman" else None

    explicit_tx = {tx(e) for e in events if e.get("explicit") is True and tx(e)}

    def follows(e):
        return e.get("explicit") is not True and tx(e) in explicit_tx

    def named(e):
        return f"{e['source']} {e['kind']} {e['subject']} at {e['ts'][11:19]}"

    cases = sorted(((fm["id"], fm, body) for _, fm, body in case_files), key=lambda c: c[0])
    known = {cid for cid, _, _ in cases}
    by_case = {cid: body for cid, _, body in cases}
    classifier = Classifier(events)
    logs, links = {}, []
    for e in events:
        if not open_(e) or follows(e):
            continue
        t, pat = instant(e["ts"]), token_pattern(e["subject"])
        # a case whose file does not load might have planned it: no link (round 2, B2)
        if any(s <= t and (end is None or t <= end)
               for cid, ws in windows.items() if cid not in known for s, end in ws):
            continue
        planned = [(cid, fm) for cid, fm, body in cases
                   if any(s <= t and (end is None or t <= end) for s, end in windows.get(cid, []))
                   and pat.search(plan_section(body))]
        if len(planned) == 1:
            cid, fm = planned[0]
            package = always_red(e["subject"])
            crisis = classifier.group([e], e)[0] == "crisis"
            # the Log's local time; the fixture's events carry the same offset
            risk = risk_at(risk_timeline(by_case[cid]), t.replace(tzinfo=None))
            if (package or crisis) and risk != "R3":
                if package and risk:
                    logs.setdefault(cid, []).append(
                        f"advisory: {cid} is {risk}, but its red change `{e['subject']}` is R3 "
                        f"(`[drift] alwaysRed`): an R3 step needs the user's explicit go and a snapshot; "
                        f"raise it with `seldon plan set {cid} --risk R3` (ADR-0027 §2c)")
                else:
                    what = ("is `alwaysRed`" if package
                            else "can affect boot, login or the shell (`[drift] alwaysRedPaths`)")
                    was = (f"{cid} was {risk} at the time" if risk
                           else f"the record of {cid} does not tell its risk at the time")
                    logs.setdefault(cid, []).append(
                        f"advisory: not linked: {named(e)} {what}, which only an R3 case takes, and "
                        f"{was} (ADR-0027 §2c); if this case made it: `seldon drift link {e['id']} {cid}`")
            else:
                links.append((e, cid))
                closed = fm["status"] in ("completed", "dropped")
                logs.setdefault(cid, []).append(
                    f"linked after the fact: {named(e)} ("
                    + ("planned here, no capture ran before the close" if closed else "planned here") + ")")
        elif len(planned) > 1:
            for cid, _ in planned:
                others = [o for o, _ in planned if o != cid]
                logs.setdefault(cid, []).append(
                    f"not linked: {named(e)} is planned here and in {', '.join(others)}, "
                    f"{'both' if len(others) == 1 else 'all'} active at the time; "
                    f"`seldon drift link {e['id']} <CASE>` links it")
    tx_cases = {}
    for e, cid in links:
        if tx(e):
            tx_cases.setdefault(tx(e), set()).add(cid)
    members = list(links)
    for e in events:
        if open_(e) and follows(e) and len(tx_cases.get(tx(e), ())) == 1:
            members.append((e, next(iter(tx_cases[tx(e)]))))
    members.sort(key=lambda m: (instant(m[0]["ts"]), m[0]["id"]))
    per_tx = {}
    for e, _ in members:
        if tx(e):
            per_tx[tx(e)] = per_tx.get(tx(e), 0) + 1
    return [(e, cid, tx(e) if per_tx.get(tx(e), 0) > 1 else None) for e, cid in members], logs


def check_planned_links(ledger, case_files):
    """The sample logbook is the state after a capture: its rule-9 lines (`linked` by `system`,
    detail `planned by …`) are exactly what rule 9 writes on the ledger without them, each case
    lists the linked ids in `events:` and has every Log line rule 9 writes."""
    out = []
    events = [e for _, e in ledger]
    engine = [e for e in events if e["kind"] == "resolution" and e["actor"] == "system"
              and e.get("resolution") == "linked" and e.get("detail", "").startswith("planned by ")]
    ids = {e["id"] for e in engine}
    lines, logs = planned_links([e for e in events if e["id"] not in ids], case_files)
    want = sorted((e["id"], cid, t) for e, cid, t in lines)
    have = sorted((e["refersTo"], e.get("case"), e.get("meta", {}).get("txId")) for e in engine)
    if want != have:
        out.append(f"rule 9 (ADR-0029): the ledger's engine links {have} != what rule 9 writes {want}")
    for e in engine:
        if e.get("detail") != PLANNED_DETAIL.format(e.get("case")):
            out.append(f"rule 9 line {e['id']}: detail {e.get('detail')!r}")
    by_case = {fm["id"]: (f, fm, body) for f, fm, body in case_files}
    for e, cid, _ in lines:
        if e["id"] not in (by_case[cid][1].get("events") or []):
            out.append(f"{rel(by_case[cid][0])}: rule 9 linked {e['id']}, not in events:")
    for cid, texts in logs.items():
        f, _, body = by_case[cid]
        for text in texts:
            if f" · {text} · system" not in body:
                out.append(f"{rel(f)}: rule 9 Log line missing: {text}")
    return out


def check_times(index, name):
    """An index is not older than what it lists: generatedAt is not before any event, lastCapture
    not before any collector event (the engine stamps both at write time)."""
    out = []
    evs = index.get("events") or []
    gen = instant(index["generatedAt"])
    newest = max(evs, key=lambda e: instant(e["ts"]), default=None)
    if newest and instant(newest["ts"]) > gen:
        out.append(f"{name}: generatedAt {index['generatedAt']} is before event {newest['id']} ({newest['ts']})")
    cap = index.get("state", {}).get("lastCapture")
    collected = [e for e in evs if e["source"] in DRIFT_SOURCES | {"snapper"}]
    newest = max(collected, key=lambda e: instant(e["ts"]), default=None)
    if cap and newest and instant(newest["ts"]) > instant(cap):
        out.append(f"{name}: state.lastCapture {cap} is before collector event {newest['id']} ({newest['ts']})")
    return out


def diff(a, b, path=""):
    """First few differences between fixture (a) and derived (b)."""
    out = []
    if type(a) is not type(b):
        return [f"{path or '/'}: fixture {json.dumps(a, ensure_ascii=False)[:80]} != derived {json.dumps(b, ensure_ascii=False)[:80]}"]
    if isinstance(a, dict):
        for k in sorted(set(a) | set(b)):
            if k not in a:
                out.append(f"{path}/{k}: missing in fixture")
            elif k not in b:
                out.append(f"{path}/{k}: not derivable from the logbook")
            else:
                out += diff(a[k], b[k], f"{path}/{k}")
    elif isinstance(a, list):
        if len(a) != len(b):
            out.append(f"{path}: fixture has {len(a)} items, derived {len(b)}")
        for i, (x, y) in enumerate(zip(a, b)):
            out += diff(x, y, f"{path}/{i}")
    elif a != b:
        out.append(f"{path or '/'}: fixture {json.dumps(a, ensure_ascii=False)[:80]} != derived {json.dumps(b, ensure_ascii=False)[:80]}")
    return out[:20]


# --------------------------------------------------------------------------- index variants

# fixtures/index-variants/<name>.json = index.sample.json with these operations applied (an RFC 6902
# subset: test, add, replace, remove), so a variant never drifts from the sample. `--write-index`
# regenerates them; the check fails when a variant file differs from sample + overlay.
VARIANTS = {
    # ADR-0026: snapper can neither list nor read the snapshot directory; everything else as in the sample.
    "snapper-degraded": [
        {"op": "test", "path": "/state/collectors/1/name", "value": "snapper"},
        {"op": "replace", "path": "/state/collectors/1/ok", "value": False},
        {"op": "add", "path": "/state/collectors/1/message",
         "value": "snapper: No permissions. This user can neither list the snapshots nor read the snapshot directory; `seldon doctor` prints the read grant."},
    ],
    # `seldon index` before `seldon init`: no logbook, every section empty.
    "not-initialised": [
        {"op": "replace", "path": "/logbook", "value": {"path": "/home/user/Seldon", "language": "en", "machine": ""}},
        {"op": "replace", "path": "/state", "value": {"status": "notInitialised", "lastCapture": None, "collectors": []}},
        {"op": "replace", "path": "/summary", "value": {"activeCases": 0, "queuedCases": 0, "openDrift": 0, "crisis": 0,
                                                        "eventsToday": 0, "events7d": 0}},
        {"op": "remove", "path": "/today/path"},
        {"op": "replace", "path": "/today/entries", "value": []},
        {"op": "remove", "path": "/today/yesterday"},
        {"op": "replace", "path": "/events", "value": []},
        {"op": "replace", "path": "/drift", "value": []},
        {"op": "replace", "path": "/cases", "value": {"queued": [], "active": [], "verification": [], "completed": []}},
        {"op": "replace", "path": "/decisions", "value": []},
        {"op": "replace", "path": "/system", "value": {}},
        {"op": "replace", "path": "/memory", "value": {}},
        {"op": "replace", "path": "/series", "value": {"heatmap": [], "packages": [], "drift": []}},
    ],
    # The engine never writes indexStale; plugin/Model.js derives it from the clock or takes it
    # from state.status. This variant exercises that data-driven branch. With SELDON_NOW =
    # STALE_NOW the clock agrees: the check below requires generatedAt and lastCapture to lie
    # more than 2 h before it.
    "index-stale": [
        {"op": "replace", "path": "/state/status", "value": "indexStale"},
    ],
    # A non-snapper collector failed (SPEC-ENGINE §4 plugins: the shell IPC call timed out).
    "plugins-degraded": [
        {"op": "test", "path": "/state/collectors/3/name", "value": "plugins"},
        {"op": "replace", "path": "/state/collectors/3/ok", "value": False},
        {"op": "add", "path": "/state/collectors/3/message", "value": "omarchy plugin list --json: timed out"},
    ],
    # Omarchy run from a git checkout of $OMARCHY_PATH: the dossier carries its HEAD (short hash).
    "omarchy-git-checkout": [
        {"op": "test", "path": "/system/omarchy/version", "value": "4.0.7-1"},
        {"op": "add", "path": "/system/omarchy/repoHead", "value": "3f9c2e1"},
    ],
    # ADR-0021: an `explained` resolution that carries a case folds it onto the event. The sample's
    # explained lines carry none; this folds C-2026-002 onto btop (index only, the logbook is not
    # touched), so the row reads "explained · C-2026-002: …".
    "drift-explained-case": [
        {"op": "test", "path": "/events/67/id", "value": "01M1MB2M1GWZYF485HTGVZ1KS3"},
        {"op": "test", "path": "/events/67/resolution", "value": "explained"},
        {"op": "add", "path": "/events/67/case", "value": "C-2026-002"},
    ],
    # ADR-0020: the index lists at most 200 open drift items, the summary counts all of them. The
    # list stays the sample's six, so the plugin shows "+244 more open drift items not listed here".
    "drift-capped": [
        {"op": "test", "path": "/summary/openDrift", "value": 6},
        {"op": "replace", "path": "/summary/openDrift", "value": 250},
    ],
    # ADR-0027 §5 (WP-101): the user reopened the agent-closed C-2026-002 (`seldon plan reopen`):
    # a new active case with the tag `reopens:C-2026-002`, its Intent copied. Index only, like
    # drift-explained-case: in the logbook it would move every list the plugin harness walks.
    "case-reopened": [
        {"op": "test", "path": "/cases/completed/0/id", "value": "C-2026-002"},
        {"op": "test", "path": "/cases/completed/0/tags", "value": ["closed-by-agent"]},
        {"op": "add", "path": "/cases/active/2", "value": {
            "id": "C-2026-009", "title": "Reopen: Hyprland-Monitorlayout für Dual-WQHD",
            "status": "active", "zone": "yellow", "risk": "R1", "priority": "normal", "area": "hyprland",
            "created": "2026-10-01", "started": "2026-10-01", "closed": None, "snapshotBefore": None,
            "agents": [], "events": [], "tags": ["reopens:C-2026-002"],
            "path": "work/active/C-2026-009-reopen-hyprland-monitorlayout-fuer-dual.md",
            "steps": {"total": 0, "done": 0}}},
        {"op": "test", "path": "/summary/activeCases", "value": 2},
        {"op": "replace", "path": "/summary/activeCases", "value": 3},
    ],
    # CONTRACT.md rule 4: index.events may omit members of an open group. lib32-mesa leaves events,
    # the mesa downgrade group keeps `members: 3`, so the drift sheet lists two and asks `seldon drift show`.
    "drift-members-capped": [
        {"op": "test", "path": "/drift/5/members", "value": 3},
        {"op": "test", "path": "/events/46/id", "value": "01M3H6M8184NVTFDTEGPD71P5H"},
        {"op": "test", "path": "/events/46/subject", "value": "lib32-mesa"},
        {"op": "remove", "path": "/events/46"},
    ],
}

# SELDON_NOW for index-variants/index-stale.json (fixtures/README.md); the plugin harness pins
# the same clock for its clock-driven stale case.
STALE_NOW = "2026-10-01T20:05:12+02:00"
STALE_AFTER = dt.timedelta(hours=2)  # SPEC-PLUGIN §3


def apply_overlay(doc, ops, name):
    doc = copy.deepcopy(doc)
    for op in ops:
        parts = [p.replace("~1", "/").replace("~0", "~") for p in op["path"].split("/")[1:]]
        parent = doc
        for p in parts[:-1]:
            parent = parent[int(p)] if isinstance(parent, list) else parent[p]
        last = int(parts[-1]) if isinstance(parent, list) else parts[-1]
        if op["op"] == "test":
            if parent[last] != op["value"]:
                raise Fail(f"index-variants/{name}: overlay test {op['path']} == {op['value']!r} failed")
        elif op["op"] in ("remove", "replace"):
            exists = last in parent if isinstance(parent, dict) else 0 <= last < len(parent)
            if not exists:
                raise Fail(f"index-variants/{name}: overlay {op['op']} {op['path']}: no such member")
            if op["op"] == "remove":
                del parent[last]
            else:
                parent[last] = copy.deepcopy(op["value"])
        elif op["op"] == "add":
            if isinstance(parent, list):
                parent.insert(last, copy.deepcopy(op["value"]))
            else:
                parent[last] = copy.deepcopy(op["value"])
        else:
            raise Fail(f"index-variants/{name}: unsupported overlay op {op['op']}")
    return doc


def dump_json(path, doc):
    with open(path, "w", encoding="utf-8") as f:
        json.dump(doc, f, ensure_ascii=False, indent=2)
        f.write("\n")


# --------------------------------------------------------------------------- self-checks

GROUP_TX = "tx-20260930T214115"  # the open caseless `-Syu` of 2026-09-30 in the sample logbook


def self_checks(today):
    """Mutation tests of the ADR-0013 drift rules and the ADR-0015 §4 token rule on in-memory
    copies of the sample logbook.
    Each case: (label, mutate(ledger), expect(group item or None, derived) -> error or None)."""
    def members(ledger):
        return [e for _, e in ledger if e.get("txId") == GROUP_TX]

    def set_on_first(**kv):
        def m(ledger):
            members(ledger)[0].update(kv)
        return m

    def set_command(cmd):
        def m(ledger):
            for e in members(ledger):
                e["meta"]["command"] = cmd
        return m

    def resolve(n, fan_out):
        def m(ledger):
            for i, e in enumerate(members(ledger)[:n]):
                r = {"id": "7" + "Z" * 23 + f"{i:02d}", "ts": "2026-10-01T16:50:00+02:00",
                     "source": "seldon", "kind": "resolution", "subject": e["subject"], "detail": "Routine.",
                     "actor": "human", "refersTo": e["id"], "resolution": "explained"}
                if fan_out:
                    r["meta"] = {"txId": GROUP_TX}
                ledger.append((f"<self-check>:{i}", r))
        return m

    def zone(z, crisis, n=3):
        def x(g, _):
            got = g and (g.get("zone"), g["crisis"], g.get("members", 1))
            return None if got == (z, crisis, n) else f"group (zone, crisis, members) = {got}, want {(z, crisis, n)}"
        return x

    def resolved_once(g, derived):
        w = [s for s in derived["series"]["drift"] if s["week"] == "2026-W40"][0]
        return None if g is None and w["resolved"] == 3 else f"group {g}, W40 resolved {w['resolved']} (want gone, 3)"

    # ADR-0013 §3: the group rules of the rollback (`attention = "all"`)
    cases = [
        ("unchanged", None, zone("yellow", False)),
        ("one member explicit", set_on_first(explicit=True), zone("red", True)),
        ("member subject linux", set_on_first(subject="linux"), zone("red", True)),
        ("member subject linux-firmware (not a kernel)", set_on_first(subject="linux-firmware"), zone("yellow", False)),
        ("member subject limine-snapper-sync (glob)", set_on_first(subject="limine-snapper-sync"), zone("red", True)),
        ("member subject sddm (login)", set_on_first(subject="sddm"), zone("red", True)),
        ("member subject quickshell", set_on_first(subject="quickshell"), zone("red", True)),
        ("member kind install", set_on_first(kind="install"), zone("red", True)),
        ("member kind reinstall", set_on_first(kind="reinstall"), zone("yellow", False)),
        ("command names a package", set_command("pacman -Syu ollama"), zone("red", True)),
        ("command after --", set_command("pacman -Syu -- ollama"), zone("red", True)),
        ("command without -u", set_command("pacman -Sy"), zone("red", True)),
        ("command -S -u --needed", set_command("pacman -S -u --needed"), zone("yellow", False)),
        ("command --sync --sysupgrade --refresh", set_command("pacman --sync --sysupgrade --refresh"), zone("yellow", False)),
        ("command --overwrite takes its argument", set_command("pacman -Syu --noconfirm --overwrite /usr/share/omarchy/*"),
         zone("yellow", False)),
        ("command -r takes its argument", set_command("pacman -Syur /mnt"), zone("yellow", False)),
        ("command with an unknown option and a word", set_command("pacman -Syu --frobnicate ollama"), zone("red", True)),
        ("command yay", set_command("yay -Syu"), zone("red", True)),
        ("--only resolves one member", resolve(1, False), zone("yellow", False, 2)),
        ("fan-out resolves the group, counted once", resolve(3, True), resolved_once),
    ]
    cases = [(f"all: {label}", m, x, True) for label, m, x in cases]

    # ADR-0028 §2: the same group under the default rules; gone = routine, history, not drift
    def gone(g, _):
        return None if g is None else f"group {g}, want none (routine)"

    def both(*ms):
        def m(ledger):
            for f in ms:
                f(ledger)
        return m

    cases += [(f"normal: {label}", m, x, False) for label, m, x in [
        ("plain -Syu is routine", None, gone),
        ("plain -Syu with a kernel upgrade is routine", set_on_first(subject="linux"), gone),
        ("plain -Syu with an install is routine", set_on_first(kind="install"), gone),
        ("plain -Syyuu is routine", set_command("pacman -Syyuu"), gone),
        ("plain -Su is routine", set_command("pacman -Su"), gone),
        ("bare yay is routine", set_command("yay"), gone),
        ("Omarchy's update line is routine", set_command("pacman -Syu --noconfirm --overwrite /usr/share/omarchy/*"), gone),
        ("a :: Replace removal is routine", set_on_first(kind="remove"), gone),
        ("a downgrade in a full upgrade is attention", set_on_first(kind="downgrade"), zone("red", False)),
        ("a kernel removal in a full upgrade is attention", both(set_on_first(subject="linux"), set_on_first(kind="remove")),
         zone("red", False)),
        ("a full upgrade naming a package is attention", set_command("pacman -Syu ollama"), zone("red", False)),
        ("a named upgrade is routine", both(set_command("pacman -S firefox"), set_on_first(explicit=True)), gone),
        ("a named kernel install is a crisis", both(set_command("pacman -S linux"), set_on_first(explicit=True),
                                                    set_on_first(subject="linux"), set_on_first(kind="install")),
         zone("red", True)),
        ("a named kernel upgrade is attention", both(set_command("pacman -S linux"), set_on_first(explicit=True),
                                                     set_on_first(subject="linux")), zone("red", False)),
        ("a keyring transaction is routine", both(set_command("pacman -Sy --noconfirm archlinux-keyring"),
                                                  set_on_first(explicit=True), set_on_first(kind="install"),
                                                  set_on_first(subject="archlinux-keyring")), gone),
        ("targets from stdin are no plain upgrade (B4)", set_command("pacman -Syu -"), zone("red", False)),
        ("-U outside a cache is no upgrade (N1)", both(set_command("pacman -U /tmp/x/firefox-1-1-x86_64.pkg.tar.zst"),
                                                        set_on_first(explicit=True)), zone("red", False)),
        ("a keyring removal is no keyring (N2)", both(set_command("pacman -Rdd archlinux-keyring"),
                                                      set_on_first(explicit=True), set_on_first(kind="remove"),
                                                      set_on_first(subject="archlinux-keyring")), zone("red", False)),
    ]]
    out = []
    for label, mutate, expect, legacy in cases:
        problems = []
        derived, _, _ = derive(LOGBOOK, today, problems, mutate, legacy=legacy)
        tx_items = [d for d in derived["drift"] if d.get("txId") == GROUP_TX
                    or (d["source"] == "pacman" and d["ts"].startswith("2026-09-30"))]
        err = problems[:1] or ([f"{len(tx_items)} items for the group"] if len(tx_items) > 1 else [])
        if not err:
            g = tx_items[0] if tx_items else None
            e = expect(g, derived)
            err = [e] if e else []
        out += [f"self-check '{label}': {e}" for e in err]

    # ADR-0015 §4 token rule, end to end: an open caseless `zed` install, every open case's Plan
    # replaced by a neutral one (the sample's Plans name zed in other forms), and one Plan line
    # in C-2026-007.
    def add_zed(ledger):
        ledger.append(("<self-check>:zed", {
            "id": "7" + "Z" * 23 + "ZD", "ts": "2026-10-01T16:55:00+02:00", "source": "pacman", "kind": "install",
            "subject": "zed", "detail": "0.198.5-1", "actor": "system", "zone": "red", "explicit": True,
            "txId": "tx-20261001T165500", "meta": {"command": "pacman -S zed", "version": "0.198.5-1"}}))

    def plan_line(line):
        def m(cases):
            out = []
            for f, fm, body in cases:
                if fm["status"] in ("queued", "active", "verification"):
                    plan = f"- [ ] {line}\n" if fm["id"] == "C-2026-007" else "- [ ] nothing to see\n"
                    body = re.sub(r"^## Plan\n.*?(?=^## |\Z)", lambda _: f"## Plan\n{plan}\n", body, flags=re.S | re.M)
                out.append((f, fm, body))
            return out
        return m

    proposals = [
        ("Plan 'Install zed.' proposes zed", "Install zed.", "C-2026-007"),
        ("Plan 'Edit zed.conf' does not propose zed", "Edit zed.conf", None),
        ("Plan 'extra/zed' proposes zed", "`extra/zed` from the repo", "C-2026-007"),
    ]
    for label, line, want in proposals:
        problems = []
        derived, _, _ = derive(LOGBOOK, today, problems, add_zed, plan_line(line))
        got = [d.get("proposedCase") for d in derived["drift"] if d["subject"] == "zed"]
        err = problems[:1] or ([] if got == [want] else [f"proposedCase {got}, want [{want!r}]"])
        out += [f"self-check '{label}': {e}" for e in err]

    # ADR-0021: the winning resolution folds its case whether linked or explained; an explained
    # line without a case (the sample's btop) folds none.
    OLLAMA = "01M3VNFTF8EVHWFFZ687N14Q0C"

    def explain_with_case(ledger):
        ledger.append(("<self-check>:explain", {
            "id": "7" + "Z" * 23 + "EX", "ts": "2026-10-01T16:58:00+02:00", "source": "seldon",
            "kind": "resolution", "subject": "ollama", "detail": "Lokale Modelle.", "actor": "human",
            "case": "C-2026-004", "refersTo": OLLAMA, "resolution": "explained"}))

    def list_in_case(cases):
        # the engine records the event in the case's `events:`, in time order (ADR-0012 §10)
        out = []
        for f, fm, body in cases:
            if fm["id"] == "C-2026-004":
                fm = dict(fm)
                ev = list(fm["events"])
                ev.insert(ev.index("01M3VZNFC0M2FQVGJBZGX9KDF7"), OLLAMA)
                fm["events"] = ev
            out.append((f, fm, body))
        return out

    problems = []
    derived, _, _ = derive(LOGBOOK, today, problems, explain_with_case, list_in_case)
    by_id = {e["id"]: e for e in derived["events"]}
    got = [(by_id[i].get("resolution"), by_id[i].get("case"))
           for i in (OLLAMA, "01M1MB2M1GWZYF485HTGVZ1KS3")]
    want = [("explained", "C-2026-004"), ("explained", None)]
    open_ids = [d["eventId"] for d in derived["drift"]]
    err = problems[:1] or ([] if got == want and OLLAMA not in open_ids
                           else [f"(resolution, case) {got}, want {want}; drift {open_ids}"])
    out += [f"self-check 'explained with a case folds it (ADR-0021)': {e}" for e in err]

    # SPEC-LOGBOOK §3: C-2026-001 without its verification step (as before WP-015) must fail the walk.
    def drop_verified(ledger):
        ledger[:] = [(w, e) for w, e in ledger if not (e["kind"] == "case-verified" and e["subject"] == "C-2026-001")]

    def drop_verification_line(cases):
        return [(f, fm, re.sub(r"^- \S+ \S+ · verification · human\n", "", body, flags=re.M)
                 if fm["id"] == "C-2026-001" else body) for f, fm, body in cases]

    ledger, case_files = load_logbook(LOGBOOK, [], drop_verified, drop_verification_line)
    errs = check_case_logs(ledger, case_files)
    if not any("C-2026-001" in e and "'completed' from active" in e for e in errs):
        out.append(f"self-check 'C-2026-001 active -> completed is rejected': walker reported {errs}")

    # ADR-0029 rule 9: the sample's engine link must be missed without its line, and must be
    # extra when C-2026-002's Plan no longer names the package, or when a second case planned it
    # in the same window (no link then, and a Log line in each)
    def drop_engine_link(ledger):
        ledger[:] = [(w, e) for w, e in ledger if not e.get("detail", "").startswith("planned by ")]

    def unplan(cases):
        return [(f, fm, body.replace("`io.github.example.display-profiles`", "`display-profiles`") if fm["id"] == "C-2026-002" else body)
                for f, fm, body in cases]

    def second_window(ledger):
        ledger.append(("<self-check>:start", {
            "id": "7" + "Z" * 23 + "SW", "ts": "2026-09-13T10:00:00+02:00", "source": "seldon",
            "kind": "case-started", "subject": "C-2026-007", "actor": "human", "case": "C-2026-007"}))

    def plan_it(cases):
        return [(f, fm, re.sub(r"^## Plan\n", "## Plan\n- [ ] `io.github.example.display-profiles`\n", body, flags=re.M)
                 if fm["id"] == "C-2026-007" else body) for f, fm, body in cases]

    rule9 = [
        ("without its line the link is missed", drop_engine_link, None, "!= what rule 9 writes"),
        ("an unplanned package is not linked", None, unplan, "!= what rule 9 writes"),
        ("two cases that planned it link nothing", second_window, plan_it, "is planned here and in C-2026-002"),
    ]
    for label, m, mc, want in rule9:
        ledger, case_files = load_logbook(LOGBOOK, [], m, mc)
        errs = check_planned_links(ledger, case_files)
        if not any(want in e for e in errs):
            out.append(f"self-check 'rule 9: {label}': {errs}")

    # round 2: the harm guard takes persistence paths, an unreadable case blocks, a Plan comment
    # is no plan (planned_links on the sample without its engine line)
    UNIT = "~/.config/systemd/user/display.service"

    def add_unit(ledger):
        drop_engine_link(ledger)
        ledger.append(("<self-check>:unit", {
            "id": "01M2CZW4J034FDMVAAEWT2G7X9", "ts": "2026-09-13T10:59:30+02:00", "source": "config",
            "kind": "config-add", "subject": UNIT, "detail": "sha256 — → 1234abcd", "actor": "system",
            "zone": "yellow", "meta": {"hashTo": "1234abcd" * 8}}))

    def plan_unit(cases):
        return [(f, fm, body.replace("## Plan\n", f"## Plan\n- `{UNIT}`\n", 1)
                 if fm["id"] == "C-2026-002" else body) for f, fm, body in cases]

    def without_002(cases):
        return [c for c in cases if c[1]["id"] != "C-2026-002"]

    def commented(cases):
        return [(f, fm, body.replace("`io.github.example.display-profiles`",
                                     "<!-- io.github.example.display-profiles -->")
                 if fm["id"] == "C-2026-002" else body) for f, fm, body in cases]

    round2 = [
        ("a persistence path below R3 is not linked", add_unit, plan_unit,
         lambda lines, logs: (["01M2CZW4J034FDMVAAEWT2G7X8"] == [e["id"] for e, _, _ in lines]
                              and any("alwaysRedPaths" in t for t in logs.get("C-2026-002", [])))),
        ("an unreadable case blocks its window (another case planned it too)",
         lambda ledger: (drop_engine_link(ledger), second_window(ledger)),
         lambda cases: plan_it(without_002(cases)),
         lambda lines, logs: lines == []),
        ("a persistence path links to a case raised to R3 before it", add_unit,
         lambda cases: [(f, fm, body.replace("· started (snapshot 108) · human\n",
                                             "· started (snapshot 108) · human\n"
                                             "- 2026-09-12 09:40 · set risk R1 → R3 · human\n", 1)
                         if fm["id"] == "C-2026-002" else body) for f, fm, body in plan_unit(cases)],
         lambda lines, logs: sorted(e["id"] for e, _, _ in lines)
         == ["01M2CZW4J034FDMVAAEWT2G7X8", "01M2CZW4J034FDMVAAEWT2G7X9"]),
        ("a Plan comment is no plan", drop_engine_link, commented,
         lambda lines, logs: lines == []),
    ]
    for label, m, mc, ok in round2:
        ledger, case_files = load_logbook(LOGBOOK, [], m, mc)
        lines, logs = planned_links([e for _, e in ledger], case_files)
        if not ok(lines, logs):
            out.append(f"self-check 'rule 9: {label}': lines {[(e['id'], c) for e, c, _ in lines]}, logs {logs}")
    return out, len(cases) + len(proposals) + 2 + len(rule9) + len(round2)


# --------------------------------------------------------------------------- snapshot info files

def check_snapshot_info_files():
    import xml.etree.ElementTree as ET
    out = []
    with open(os.path.join(FIX, "logs", "snapper.json"), encoding="utf-8") as f:
        listed = {s["number"]: s for s in json.load(f)["root"] if s["number"] != 0}
    base = os.path.join(FIX, "logs", "snapshots")
    dirs = sorted(int(d) for d in os.listdir(base) if d.isdigit())
    if dirs != sorted(listed):
        out.append(f"{rel(base)}: numbers {dirs} != logs/snapper.json {sorted(listed)}")
    for n in dirs:
        path = os.path.join(base, str(n), "info.xml")
        if n not in listed:
            continue
        s, root = listed[n], ET.parse(path).getroot()
        text = lambda tag: (root.findtext(tag) or "")
        local = dt.datetime.strptime(s["date"], "%Y-%m-%d %H:%M:%S").replace(tzinfo=dt.timezone(dt.timedelta(hours=2)))
        want = {"num": str(n), "type": s["type"], "pre_num": str(s["pre-number"]) if s["type"] == "post" else "",
                "description": s["description"], "cleanup": s["cleanup"],
                "date": local.astimezone(dt.timezone.utc).strftime("%Y-%m-%d %H:%M:%S")}
        have = {k: text(k) for k in want}
        if have != want:
            out.append(f"{rel(path)}: {have} != logs/snapper.json {want}")
        userdata = {u.findtext("key"): u.findtext("value") for u in root.findall("userdata")}
        if userdata != (s["userdata"] or {}):
            out.append(f"{rel(path)}: userdata {userdata} != logs/snapper.json {s['userdata']}")
    return out


# --------------------------------------------------------------------------- main

def collect_instances():
    """(label, instance, schema id, must_fail) for every JSON fixture. Unmapped files are an error."""
    items, unmapped = [], []
    files = sorted(glob.glob(os.path.join(FIX, "**", "*.json"), recursive=True)
                   + glob.glob(os.path.join(FIX, "**", "*.jsonl"), recursive=True))
    for f in files:
        r = os.path.relpath(f, FIX).replace(os.sep, "/")
        if r.endswith(".jsonl"):
            if not r.startswith("logbook/ledger/"):
                unmapped.append(r)
                continue
            with open(f, encoding="utf-8") as fh:
                for n, line in enumerate(fh, 1):
                    if line.strip():
                        items.append((f"fixtures/{r}:{n}", json.loads(line), EVENT, False))
            continue
        with open(f, encoding="utf-8") as fh:
            inst = json.load(fh)
        if r in ("index.sample.json", "index.attention-all.json") or re.fullmatch(r"index-variants/[a-z0-9-]+\.json", r):
            sid, bad = INDEX, False
        elif re.fullmatch(r"logs/snapper(-[a-z0-9-]+)?\.json", r):
            sid, bad = EXT["snapper"], False
        elif re.fullmatch(r"logs/plugin-list-[a-z0-9-]+\.json", r):
            sid, bad = EXT["plugin-list"], False
        elif r == "logs/plugin-catalog.json":
            sid, bad = EXT["plugin-catalog"], False
        elif re.fullmatch(r"hooks/claude-code-[a-z0-9-]+\.json", r):
            sid, bad = EXT["hook"], False
        elif re.fullmatch(r"invalid/(index|event|case)\.[a-z0-9-]+\.json", r):
            sid, bad = ID + r.split("/")[1].split(".")[0] + ".schema.json", True
        else:
            unmapped.append(r)
            continue
        items.append((f"fixtures/{r}", inst, sid, bad))
    for f in sorted(glob.glob(os.path.join(LOGBOOK, "work", "*", "C-*.md"))):
        fm, _ = frontmatter(f)
        items.append((rel(f) + " (frontmatter)", fm, CASE, False))
    return items, unmapped


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--validator", choices=["auto", "jsonschema", "check-jsonschema", "builtin"])
    ap.add_argument("--write-index", action="store_true",
                    help="rewrite the logbook-derived parts of fixtures/index.sample.json, then validate")
    ap.add_argument("-q", "--quiet", action="store_true")
    ap.add_argument("--derive", metavar="LOGBOOK",
                    help="only print the index parts derived from LOGBOOK as JSON, problems on stderr "
                         "(the engine's parity tests, WP-077)")
    ap.add_argument("--today", metavar="DATE", help="with --derive: the index's date, YYYY-MM-DD")
    a = ap.parse_args()
    problems = []

    if a.derive:
        today = dt.date.fromisoformat(a.today) if a.today else dt.date.today()
        derived, _, _ = derive(a.derive, today, problems)
        json.dump(derived, sys.stdout, ensure_ascii=False)
        for p in problems:
            print(f"FAIL {p}", file=sys.stderr)
        return 1 if problems else 0

    schemas = load_schemas()
    backend = pick_backend(schemas, a.validator)

    with open(SAMPLE, encoding="utf-8") as f:
        sample = json.load(f)
    today = dt.date.fromisoformat(sample["generatedAt"][:10])
    derived, ledger, case_files = derive(LOGBOOK, today, problems)

    derived_all, _, _ = derive(LOGBOOK, today, problems, legacy=True)

    def as_sample(d):
        out = {k: sample[k] for k in ("contractVersion", "generatedAt", "engineVersion")}
        out["logbook"] = {"path": sample["logbook"]["path"], **d["logbook"]}
        if "git" in sample["logbook"]:
            out["logbook"]["git"] = sample["logbook"]["git"]
        out["state"] = sample["state"]
        for k in ("summary", "today", "events", "drift", "cases", "decisions", "system", "memory", "series"):
            out[k] = d[k]
        return out

    if a.write_index:
        out = as_sample(derived)
        dump_json(SAMPLE, out)
        sample = out
        print(f"wrote {rel(SAMPLE)}")
        dump_json(ATTENTION_ALL, as_sample(derived_all))
        print(f"wrote {rel(ATTENTION_ALL)}")
        for name, ops in VARIANTS.items():
            path = os.path.join(FIX, "index-variants", f"{name}.json")
            try:
                dump_json(path, apply_overlay(sample, ops, name))
            except Fail as e:
                problems.append(f"{e} (not written)")
                continue
            print(f"wrote {rel(path)}")

    # 1. schema validation of every JSON fixture
    items, unmapped = collect_instances()
    for r in unmapped:
        problems.append(f"fixtures/{r}: no schema mapping in scripts/validate-fixtures.py")
    ok = 0
    for label, inst, sid, must_fail in items:
        errs = backend.validate(inst, sid)
        if must_fail:
            if errs:
                ok += 1
            else:
                problems.append(f"{label}: expected to FAIL against {sid.rsplit('/', 1)[1]} but passed")
        elif errs:
            problems += [f"{label}: {e}" for e in errs[:10]]
        else:
            ok += 1

    # 2. the sample index derives from the sample logbook
    for k in ("summary", "today", "events", "drift", "cases", "decisions", "system", "memory", "series"):
        problems += [f"index.sample.json /{k}{d}" for d in diff(sample.get(k), derived[k])]
    for k in ("language", "machine"):
        if sample["logbook"].get(k) != derived["logbook"][k]:
            problems.append(f"index.sample.json /logbook/{k}: != PROJECT.md")
    problems += check_case_logs(ledger, case_files)
    problems += check_planned_links(ledger, case_files)
    problems += check_times(sample, "index.sample.json")

    # 2b. the `attention = "all"` index is the sample logbook's legacy derivation
    if not os.path.exists(ATTENTION_ALL):
        problems.append(f"{rel(ATTENTION_ALL)}: missing; run with --write-index")
    else:
        with open(ATTENTION_ALL, encoding="utf-8") as f:
            have = json.load(f)
        problems += [f"{rel(ATTENTION_ALL)} {d} (regenerate with --write-index)"
                     for d in diff(have, as_sample(derived_all))]

    # 3. every index variant is the sample plus its overlay
    variant_files = {os.path.basename(f)[:-5] for f in glob.glob(os.path.join(FIX, "index-variants", "*.json"))}
    for name in sorted(variant_files - set(VARIANTS)):
        problems.append(f"fixtures/index-variants/{name}.json: no overlay in VARIANTS (scripts/validate-fixtures.py)")
    for name, ops in VARIANTS.items():
        path = os.path.join(FIX, "index-variants", f"{name}.json")
        if not os.path.exists(path):
            problems.append(f"{rel(path)}: missing; run with --write-index")
            continue
        with open(path, encoding="utf-8") as f:
            have = json.load(f)
        try:
            want = apply_overlay(sample, ops, name)
        except Fail as e:  # one broken overlay must not hide the other problems
            problems.append(str(e))
        else:
            problems += [f"{rel(path)} {d} (regenerate with --write-index)" for d in diff(have, want)]
        problems += check_times(have, rel(path))
        if name == "index-stale":
            for k, v in (("generatedAt", have["generatedAt"]), ("state.lastCapture", have["state"]["lastCapture"])):
                if instant(STALE_NOW) - instant(v) <= STALE_AFTER:
                    problems.append(f"{rel(path)} {k} {v} is not more than 2 h before STALE_NOW {STALE_NOW}")

    # 3b. a `-pre` hook payload is its PostToolUse sibling as PreToolUse, without tool_response
    for pre in sorted(glob.glob(os.path.join(FIX, "hooks", "*-pre.json"))):
        post = pre[:-len("-pre.json")] + ".json"
        if not os.path.exists(post):
            problems.append(f"{rel(pre)}: no PostToolUse sibling {os.path.basename(post)}")
            continue
        with open(pre, encoding="utf-8") as f:
            have = json.load(f)
        with open(post, encoding="utf-8") as f:
            want = json.load(f)
        want["hook_event_name"] = "PreToolUse"
        want.pop("tool_response", None)
        problems += [f"{rel(pre)} {d} (must equal {os.path.basename(post)} as PreToolUse without tool_response)"
                     for d in diff(have, want)]

    # 3c. logs/snapshots/<n>/info.xml mirror logs/snapper.json (the snapper collector's info-file
    #     path, WP-060): same numbers, type, pre-number, description, cleanup, userdata; `date` in UTC
    #     there, local time (the fixture's +02:00) in the list
    problems += check_snapshot_info_files()

    # 4. ADR-0013 mutation self-checks on the sample logbook
    errs, n_checks = self_checks(today)
    problems += errs

    if problems:
        for p in problems:
            print(f"FAIL {p}")
        print(f"validate-fixtures: {len(problems)} problem(s); backend {backend.name}")
        return 1
    if not a.quiet:
        n_ev = len(ledger)
        print(f"validate-fixtures: ok — {ok} instances ({len(items)} incl. {sum(i[3] for i in items)} expected failures), "
              f"{n_ev} ledger events traced to index.sample.json, {len(VARIANTS)} variants, "
              f"{n_checks} self-checks; backend {backend.name}")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Fail as e:
        print(f"FAIL {e}")
        sys.exit(1)
    except (OSError, ValueError, KeyError) as e:
        print(f"FAIL {type(e).__name__}: {e}")
        sys.exit(1)
