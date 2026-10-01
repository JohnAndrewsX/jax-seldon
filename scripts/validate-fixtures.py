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
real `seldon index` output with the same fixture. `--write-index` rewrites those
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

# ADR-0013 §3: default of config.toml [drift] alwaysRed (fnmatch globs, case-sensitive).
ALWAYS_RED = ["linux*", "systemd", "glibc", "hyprland", "omarchy", "quickshell"]

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


def plan_section(body):
    m = re.search(r"^## Plan\n(.*?)(?=^## |\Z)", body, re.S | re.M)
    return m.group(1) if m else ""


def instant(ts):
    return dt.datetime.fromisoformat(ts.replace("Z", "+00:00"))


def order_event(e):
    return {k: e[k] for k in EVENT_KEYS if k in e}


# --------------------------------------------------------------------------- derivation (ADR-0012)

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


def derive(lb, today, problems, mutate=None, mutate_cases=None):
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

    # open drift; caseless pacman events of one transaction form one item (ADR-0013 §1-§3)
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
        explicit = [m for m in members if m.get("explicit") is True]
        lead = min(explicit or members, key=lambda m: m["id"])
        d = {"eventId": lead["id"], "ts": lead["ts"], "source": lead["source"], "kind": lead["kind"],
             "subject": lead["subject"], "detail": lead.get("detail"), "actor": lead["actor"]}
        if lead["source"] == "pacman":
            d["zone"] = "yellow" if all(routine(m) for m in members) else "red"
        elif "zone" in lead:
            d["zone"] = lead["zone"]
        d["crisis"] = d.get("zone") == "red"
        d["proposedCase"] = proposed(lead["subject"])
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
    # actor) count as one.
    first = {}
    for e in sorted(events, key=lambda e: (instant(e["ts"]), e["id"])):
        if e["source"] in DRIFT_SOURCES and "case" not in e:
            first.setdefault(("tx", e["txId"]) if e["source"] == "pacman" and e.get("txId") else e["id"], e)
        elif e["kind"] == "resolution":
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
        "events": folded[:500],
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
    # ADR-0011: snapper runs without ALLOW_USERS; everything else as in the sample.
    "snapper-degraded": [
        {"op": "test", "path": "/state/collectors/1/name", "value": "snapper"},
        {"op": "replace", "path": "/state/collectors/1/ok", "value": False},
        {"op": "add", "path": "/state/collectors/1/message",
         "value": "snapper: No permissions. The snapper config does not list this user in ALLOW_USERS; see `seldon doctor`."},
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
        {"op": "test", "path": "/events/56/id", "value": "01M1MB2M1GWZYF485HTGVZ1KS3"},
        {"op": "test", "path": "/events/56/resolution", "value": "explained"},
        {"op": "add", "path": "/events/56/case", "value": "C-2026-002"},
    ],
    # ADR-0020: the index lists at most 200 open drift items, the summary counts all of them. The
    # list stays the sample's four, so the plugin shows "+246 more open drift items not listed here".
    "drift-capped": [
        {"op": "test", "path": "/summary/openDrift", "value": 4},
        {"op": "replace", "path": "/summary/openDrift", "value": 250},
    ],
    # CONTRACT.md rule 4: index.events may omit members of an open group. noto-fonts leaves events,
    # the firefox group keeps `members: 3`, so the drift sheet lists two and asks `seldon drift show`.
    "drift-members-capped": [
        {"op": "test", "path": "/drift/3/members", "value": 3},
        {"op": "test", "path": "/events/31/id", "value": "01M3SXBRV0E702XKBM22HEV1B8"},
        {"op": "test", "path": "/events/31/subject", "value": "noto-fonts"},
        {"op": "remove", "path": "/events/31"},
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

    cases = [
        ("unchanged", None, zone("yellow", False)),
        ("one member explicit", set_on_first(explicit=True), zone("red", True)),
        ("member subject linux", set_on_first(subject="linux"), zone("red", True)),
        ("member subject linux-firmware (glob)", set_on_first(subject="linux-firmware"), zone("red", True)),
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
    out = []
    for label, mutate, expect in cases:
        problems = []
        derived, _, _ = derive(LOGBOOK, today, problems, mutate)
        tx_items = [d for d in derived["drift"] if d.get("txId") == GROUP_TX
                    or (d["source"] == "pacman" and d["ts"].startswith("2026-09-30"))]
        err = problems[:1] or ([f"{len(tx_items)} items for the group"] if len(tx_items) > 1 else [])
        if not err:
            g = tx_items[0] if tx_items else None
            e = expect(g, derived)
            err = [e] if e else []
        out += [f"self-check '{label}': {e}" for e in err]

    # ADR-0015 §4 token rule, end to end: an open caseless `zed` upgrade, every open case's Plan
    # replaced by a neutral one (the sample's Plans name zed in other forms), and one Plan line
    # in C-2026-007.
    def add_zed(ledger):
        ledger.append(("<self-check>:zed", {
            "id": "7" + "Z" * 23 + "ZD", "ts": "2026-10-01T16:55:00+02:00", "source": "pacman", "kind": "upgrade",
            "subject": "zed", "detail": "0.198.4-1 → 0.198.5-1", "actor": "system", "zone": "red", "explicit": False,
            "txId": "tx-20261001T165500", "meta": {"command": "pacman -Syu", "from": "0.198.4-1", "to": "0.198.5-1"}}))

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
    return out, len(cases) + len(proposals) + 2


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
        if r == "index.sample.json" or re.fullmatch(r"index-variants/[a-z0-9-]+\.json", r):
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
    a = ap.parse_args()
    problems = []

    schemas = load_schemas()
    backend = pick_backend(schemas, a.validator)

    with open(SAMPLE, encoding="utf-8") as f:
        sample = json.load(f)
    today = dt.date.fromisoformat(sample["generatedAt"][:10])
    derived, ledger, case_files = derive(LOGBOOK, today, problems)

    if a.write_index:
        out = {k: sample[k] for k in ("contractVersion", "generatedAt", "engineVersion")}
        out["logbook"] = {"path": sample["logbook"]["path"], **derived["logbook"]}
        if "git" in sample["logbook"]:
            out["logbook"]["git"] = sample["logbook"]["git"]
        out["state"] = sample["state"]
        for k in ("summary", "today", "events", "drift", "cases", "decisions", "system", "memory", "series"):
            out[k] = derived[k]
        dump_json(SAMPLE, out)
        sample = out
        print(f"wrote {rel(SAMPLE)}")
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
    problems += check_times(sample, "index.sample.json")

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
