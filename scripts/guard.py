#!/usr/bin/env python3
"""Red-zone guard (AGENTS.md §6): the shell parser and the rules.

`scripts/guard.sh` runs this file; the comment block there states the rule.
Input: the Claude Code PreToolUse JSON on stdin. Exit 0 allows the command,
exit 2 blocks it with the reason on stderr. Python 3 standard library only.

The guard parses the command like bash does and decides on the command
position of every simple command, wherever it runs: in a list or pipeline,
in `$(…)`, backticks or `<(…)`, behind wrappers (`env`, `timeout`,
`xargs`, …), in `bash -c`/`sh -c`/`eval`/`trap` strings, in a heredoc fed to
a shell, and in the remote command of `ssh`. Quoted strings and heredoc
bodies are data everywhere else. What it cannot parse or cannot know (an
unterminated quote, a computed command name, a shell reading a pipe) is
blocked: fail closed.
"""

import fnmatch
import itertools
import json
import os
import pwd
import re
import sys

UNK = ""  # text the guard cannot know: an unknown variable, a command's output
STAR, QMARK, LBRACK = "", "", ""  # unquoted glob characters
GLOBS = STAR + QMARK + LBRACK
MAX_DEPTH = 12
MAX_CANDIDATES = 16

RED = "AGENTS.md §6 red zone"
CLOSED = "fail closed, cannot check this command"


class Blocked(Exception):
    """A red-zone command."""


class Unsure(Exception):
    """A command the guard cannot check; blocked as well."""


class ParseError(Exception):
    pass


# --------------------------------------------------------------------------
# Syntax tree

class List:
    def __init__(self, items):
        self.items = items  # [(AndOr, separator)]


class AndOr:
    def __init__(self, pipelines, ops):
        self.pipelines = pipelines
        self.ops = ops  # '&&' / '||' between the pipelines


class Pipeline:
    def __init__(self, cmds):
        self.cmds = cmds


class Simple:
    def __init__(self):
        self.assigns = []
        self.words = []
        self.redirs = []


class Compound:
    def __init__(self, kind, **kw):
        self.kind = kind
        self.redirs = []
        self.__dict__.update(kw)


class Redir:
    def __init__(self, op, fd):
        self.op = op
        self.fd = fd
        self.target = None  # word
        self.delim = None  # heredoc
        self.quoted = False
        self.body = None
        self.parts = None  # unquoted heredoc body: parts with expansions


# A word is a list of parts (kind, payload, quoted):
#   ('lit', text)   literal text
#   ('var', name)   $name, ${name}
#   ('unk', parts)  an expansion whose value is unknown; parts = nested parts
#   ('nest', parts) no value of its own, only nested parts (a ${x:-$(…)} default)
#   ('cmd', List)   $(…) or backticks
#   ('proc', List)  <(…) or >(…)


# --------------------------------------------------------------------------
# Parser

NAME_RE = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")
ASSIGN_RE = re.compile(r"(?P<name>[A-Za-z_][A-Za-z0-9_]*)(?P<idx>\[[^\]]*\])?(?P<op>\+?=)")
RESERVED_RE = re.compile(
    r"(if|then|else|elif|fi|do|done|case|esac|while|until|for|select|function|"
    r"in|time|coproc|\[\[|\{|\}|!)(?=[ \t\n;&|()<>]|$)"
)
REDIR_RE = re.compile(r"(\d+|\{[A-Za-z_][A-Za-z0-9_]*\})?(&>>|&>|<<<|<<-|<<|<>|<&|>>|>&|>\||<|>)")
OPS = ["&>>", ";;&", "&&", "||", ";;", ";&", "|&", "&>", "<<<", "<<-", "<<", "<>", "<&",
       ">>", ">&", ">|", "|", "&", ";", "(", ")", "<", ">", "\n"]
ANSI_C = {"n": "\n", "t": "\t", "r": "\r", "a": "\a", "b": "\b", "e": "\x1b", "E": "\x1b",
          "f": "\f", "v": "\v", "\\": "\\", "'": "'", '"': '"', "?": "?"}


class Parser:
    def __init__(self, src, depth=0):
        if depth > MAX_DEPTH:
            raise ParseError("nested too deep")
        self.s = src
        self.n = len(src)
        self.i = 0
        self.depth = depth
        self.pending = []  # heredocs whose body starts after the next newline

    # -- helpers
    def at(self, k=0):
        j = self.i + k
        return self.s[j] if j < self.n else ""

    def startswith(self, t):
        return self.s.startswith(t, self.i)

    def describe(self):
        if self.i >= self.n:
            return "end of input"
        return repr(self.s[self.i:self.i + 20])

    def skip_blank(self):
        while self.i < self.n:
            c = self.s[self.i]
            if c in " \t":
                self.i += 1
            elif c == "\\" and self.at(1) == "\n":
                self.i += 2
            elif c == "#":
                while self.i < self.n and self.s[self.i] != "\n":
                    self.i += 1
            else:
                break

    def newline(self):
        self.i += 1
        pending, self.pending = self.pending, []
        for r in pending:
            self.read_heredoc(r)

    def skip_newlines(self):
        while True:
            self.skip_blank()
            if self.at() == "\n":
                self.newline()
            else:
                break

    def peek_op(self):
        for op in OPS:
            if self.s.startswith(op, self.i):
                return op
        return None

    def peek_reserved(self):
        m = RESERVED_RE.match(self.s, self.i)
        return m.group(1) if m else None

    def is_term(self, terms):
        op = self.peek_op()
        if op is not None and op in terms:
            return True
        r = self.peek_reserved()
        return r is not None and r in terms

    def expect(self, ch):
        self.skip_blank()
        if self.at() != ch:
            raise ParseError(f"expected {ch!r} at {self.describe()}")
        self.i += 1

    def expect_reserved(self, word):
        self.skip_newlines()
        if self.peek_reserved() != word:
            raise ParseError(f"expected {word!r} at {self.describe()}")
        self.i += len(word)

    # -- grammar
    def parse_script(self):
        lst = self.parse_list(set())
        if self.pending:
            raise ParseError(f"unterminated heredoc (<<{self.pending[0].delim})")
        if self.i < self.n:
            raise ParseError(f"unexpected {self.describe()}")
        return lst

    def parse_list(self, terms):
        items = []
        while True:
            self.skip_newlines()
            if self.i >= self.n or self.is_term(terms):
                break
            ao = self.parse_andor()
            self.skip_blank()
            op = self.peek_op()
            if op in (";", "&"):
                self.i += 1
                items.append((ao, op))
                continue
            if op == "\n":
                items.append((ao, "\n"))
                continue
            items.append((ao, None))
            if self.i >= self.n or self.is_term(terms):
                break
            raise ParseError(f"unexpected {self.describe()}")
        return List(items)

    def parse_andor(self):
        pipelines = [self.parse_pipeline()]
        ops = []
        while True:
            self.skip_blank()
            op = self.peek_op()
            if op not in ("&&", "||"):
                break
            self.i += 2
            self.skip_newlines()
            ops.append(op)
            pipelines.append(self.parse_pipeline())
        return AndOr(pipelines, ops)

    def parse_pipeline(self):
        while True:
            self.skip_blank()
            r = self.peek_reserved()
            if r == "!":
                self.i += 1
            elif r == "time":
                self.i += 4
                self.skip_blank()
                if self.startswith("-p") and self.at(2) in (" ", "\t", "\n", ""):
                    self.i += 2
            else:
                break
        cmds = [self.parse_command()]
        while True:
            self.skip_blank()
            op = self.peek_op()
            if op not in ("|", "|&"):
                break
            self.i += len(op)
            self.skip_newlines()
            cmds.append(self.parse_command())
        return Pipeline(cmds)

    def parse_command(self):
        self.skip_blank()
        if self.i >= self.n:
            raise ParseError("missing command at end of input")
        r = self.peek_reserved()
        if self.startswith("(("):
            self.i += 2
            cmd = Compound("arith", parts=self.read_arith())
        elif self.at() == "(":
            self.i += 1
            body = self.parse_list({")"})
            self.expect(")")
            cmd = Compound("subshell", body=body)
        elif r == "{":
            self.i += 1
            body = self.parse_list({"}"})
            self.expect_reserved("}")
            cmd = Compound("group", body=body)
        elif r == "if":
            self.i += 2
            lists = [self.parse_list({"then"})]
            self.expect_reserved("then")
            lists.append(self.parse_list({"elif", "else", "fi"}))
            while True:
                r2 = self.peek_reserved()
                if r2 == "elif":
                    self.i += 4
                    lists.append(self.parse_list({"then"}))
                    self.expect_reserved("then")
                    lists.append(self.parse_list({"elif", "else", "fi"}))
                elif r2 == "else":
                    self.i += 4
                    lists.append(self.parse_list({"fi"}))
                elif r2 == "fi":
                    self.i += 2
                    break
                else:
                    raise ParseError("if without fi")
            cmd = Compound("if", lists=lists)
        elif r in ("while", "until"):
            self.i += len(r)
            cond = self.parse_list({"do"})
            self.expect_reserved("do")
            body = self.parse_list({"done"})
            self.expect_reserved("done")
            cmd = Compound("if", lists=[cond, body])
        elif r in ("for", "select"):
            cmd = self.parse_for(r)
        elif r == "case":
            cmd = self.parse_case()
        elif r == "function":
            self.i += 8
            self.skip_blank()
            m = re.compile(r"[^ \t\n;&|()<>]+").match(self.s, self.i)
            if not m:
                raise ParseError("function without a name")
            self.i = m.end()
            self.skip_blank()
            if self.at() == "(":
                self.i += 1
                self.expect(")")
            self.skip_newlines()
            cmd = Compound("func", body=self.parse_command())
        elif r == "[[":
            cmd = self.parse_cond()
        elif r == "coproc":
            raise ParseError("coproc is not supported")
        elif r in ("then", "else", "elif", "fi", "do", "done", "esac", "}"):
            raise ParseError(f"unexpected {r!r}")
        else:
            return self.parse_simple()
        self.read_redirs(cmd)
        return cmd

    def parse_for(self, r):
        self.i += len(r)
        self.skip_blank()
        var, words = None, None
        if self.startswith("(("):
            self.i += 2
            words = [[("unk", self.read_arith(), False)]]
        else:
            m = NAME_RE.match(self.s, self.i)
            if not m:
                raise ParseError(f"{r} without a variable name")
            var = m.group()
            self.i = m.end()
            self.skip_newlines()
            if self.peek_reserved() == "in":
                self.i += 2
                words = []
                while True:
                    self.skip_blank()
                    if self.i >= self.n or self.at() in (";", "\n"):
                        break
                    w = self.read_word()
                    if w is None:
                        raise ParseError(f"bad word in {r} list at {self.describe()}")
                    words.append(w)
        self.skip_blank()
        if self.at() == ";":
            self.i += 1
        self.expect_reserved("do")
        body = self.parse_list({"done"})
        self.expect_reserved("done")
        return Compound("for", var=var, words=words, body=body)

    def parse_case(self):
        self.i += 4
        self.skip_blank()
        word = self.read_word()
        if word is None:
            raise ParseError("case without a word")
        self.expect_reserved("in")
        items = []
        while True:
            self.skip_newlines()
            if self.peek_reserved() == "esac":
                self.i += 4
                break
            if self.i >= self.n:
                raise ParseError("case without esac")
            if self.at() == "(":
                self.i += 1
            pats = []
            while True:
                self.skip_blank()
                w = self.read_word()
                if w is None:
                    raise ParseError(f"bad case pattern at {self.describe()}")
                pats.append(w)
                self.skip_blank()
                if self.at() == "|":
                    self.i += 1
                    continue
                if self.at() == ")":
                    self.i += 1
                    break
                raise ParseError(f"bad case pattern at {self.describe()}")
            body = self.parse_list({";;", ";&", ";;&", "esac"})
            op = self.peek_op()
            if op in (";;", ";&", ";;&"):
                self.i += len(op)
            items.append((pats, body))
        return Compound("case", word=word, items=items)

    def parse_cond(self):
        self.i += 2
        words = []
        while True:
            self.skip_newlines()
            if self.startswith("]]") and (self.i + 2 >= self.n or self.s[self.i + 2] in " \t\n;&|)"):
                self.i += 2
                return Compound("cond", words=words)
            if self.i >= self.n:
                raise ParseError("[[ without ]]")
            w = self.read_word(cond=True)
            if w is None:
                raise ParseError(f"bad [[ ]] expression at {self.describe()}")
            words.append(w)

    def parse_simple(self):
        cmd = Simple()
        while True:
            self.skip_blank()
            if self.i >= self.n:
                break
            if self.try_redir(cmd):
                continue
            c = self.at()
            if c in "\n;&|)":
                break
            if c == "(":
                if len(cmd.words) == 1 and not cmd.assigns and not cmd.redirs:
                    self.i += 1
                    self.expect(")")
                    self.skip_newlines()
                    return Compound("func", body=self.parse_command())
                raise ParseError(f"unexpected '(' at {self.describe()}")
            w = self.read_word()
            if w is None:
                raise ParseError(f"unexpected {self.describe()}")
            if not cmd.words and is_assignment(w):
                cmd.assigns.append(w)
            else:
                cmd.words.append(w)
        if not cmd.words and not cmd.assigns and not cmd.redirs:
            raise ParseError(f"missing command before {self.describe()}")
        return cmd

    def read_redirs(self, cmd):
        while True:
            self.skip_blank()
            if not self.try_redir(cmd):
                return

    def try_redir(self, cmd):
        m = REDIR_RE.match(self.s, self.i)
        if not m:
            return False
        fd, op = m.group(1), m.group(2)
        if op in ("<", ">") and m.end() < self.n and self.s[m.end()] == "(":
            return False  # process substitution, read as a word
        self.i = m.end()
        r = Redir(op, fd)
        self.skip_blank()
        w = self.read_word()
        if w is None:
            raise ParseError(f"redirection {op} without a target")
        if op in ("<<", "<<-"):
            text, quoted = "", False
            for kind, payload, q in w:
                if kind != "lit":
                    raise ParseError("heredoc delimiter with an expansion")
                text += payload
                quoted = quoted or q
            r.delim, r.quoted = text, quoted
            self.pending.append(r)
        else:
            r.target = w
        cmd.redirs.append(r)
        return True

    def read_heredoc(self, r):
        strip = r.op == "<<-"
        lines = []
        while True:
            if self.i >= self.n:
                raise ParseError(f"unterminated heredoc (<<{r.delim})")
            j = self.s.find("\n", self.i)
            end = self.n if j < 0 else j
            line = self.s[self.i:end]
            self.i = self.n if j < 0 else j + 1
            if strip:
                line = line.lstrip("\t")
            if line == r.delim:
                break
            lines.append(line)
        r.body = "".join(line + "\n" for line in lines)
        if not r.quoted:
            r.parts = Parser(r.body, self.depth).read_string("heredoc")

    # -- words
    def read_word(self, cond=False):
        parts, buf, start = [], [], self.i
        stop = " \t\n" if cond else " \t\n;&|()<>"

        def flush():
            if buf:
                parts.append(("lit", "".join(buf), False))
                buf.clear()

        while self.i < self.n:
            c = self.s[self.i]
            if c in "<>" and self.at(1) == "(":
                flush()
                self.i += 2
                body = self.parse_list({")"})
                self.expect(")")
                parts.append(("proc", body, False))
                continue
            if c in stop:
                if c == "(" and not parts and re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*\+?=", "".join(buf)):
                    flush()
                    self.i += 1
                    parts.append(("unk", self.read_array(), False))
                    continue
                break
            if c == "\\":
                if self.at(1) == "\n":
                    self.i += 2
                elif self.i + 1 >= self.n:
                    buf.append("\\")
                    self.i += 1
                else:
                    flush()
                    parts.append(("lit", self.s[self.i + 1], True))
                    self.i += 2
            elif c == "'":
                flush()
                j = self.s.find("'", self.i + 1)
                if j < 0:
                    raise ParseError("unterminated single quote")
                parts.append(("lit", self.s[self.i + 1:j], True))
                self.i = j + 1
            elif c == '"':
                flush()
                self.i += 1
                parts.append(("lit", "", True))
                parts.extend(self.read_string("dq"))
            elif c == "$":
                flush()
                parts.extend(self.read_dollar(False))
            elif c == "`":
                flush()
                parts.append(self.read_backtick(False))
            else:
                buf.append(c)
                self.i += 1
        flush()
        return parts if self.i > start else None

    def read_array(self):
        nested = []
        while True:
            self.skip_newlines()
            if self.at() == ")":
                self.i += 1
                return nested
            if self.i >= self.n:
                raise ParseError("unterminated array")
            w = self.read_word()
            if w is None:
                raise ParseError(f"bad array at {self.describe()}")
            nested.extend(w)

    def read_string(self, mode):
        """Double-quoted text ('dq', up to the closing quote) or an unquoted
        heredoc body ('heredoc', up to the end): literal except `$` and `\\``."""
        parts, buf = [], []
        esc = '$`"\\\n' if mode == "dq" else "$`\\\n"

        def flush():
            if buf:
                parts.append(("lit", "".join(buf), True))
                buf.clear()

        while True:
            if self.i >= self.n:
                if mode == "dq":
                    raise ParseError("unterminated double quote")
                break
            c = self.s[self.i]
            if mode == "dq" and c == '"':
                self.i += 1
                break
            nx = self.at(1)
            if c == "\\" and nx and nx in esc:
                if nx != "\n":
                    buf.append(nx)
                self.i += 2
            elif c == "$":
                flush()
                parts.extend(self.read_dollar(True))
            elif c == "`":
                flush()
                parts.append(self.read_backtick(True))
            else:
                buf.append(c)
                self.i += 1
        flush()
        return parts

    def read_dollar(self, in_dq):
        nx = self.at(1)
        if nx == "(":
            if self.at(2) == "(":
                self.i += 3
                return [("unk", self.read_arith(), in_dq)]
            self.i += 2
            body = self.parse_list({")"})
            self.expect(")")
            return [("cmd", body, in_dq)]
        if nx == "{":
            self.i += 2
            return self.read_param(in_dq)
        if nx == "'" and not in_dq:
            self.i += 2
            return [("lit", self.read_ansi_c(), True)]
        if nx == '"' and not in_dq:
            self.i += 2
            return [("lit", "", True)] + self.read_string("dq")
        if nx == "[":
            j = self.s.find("]", self.i)
            if j < 0:
                raise ParseError("unterminated $[")
            self.i = j + 1
            return [("unk", [], in_dq)]
        m = NAME_RE.match(self.s, self.i + 1)
        if m:
            self.i = m.end()
            return [("var", m.group(), in_dq)]
        if nx and (nx.isdigit() or nx in "@*#?-$!"):
            self.i += 2
            return [("unk", [], in_dq)]
        self.i += 1
        return [("lit", "$", in_dq)]

    def read_param(self, in_dq):
        start, nested = self.i, []
        while True:
            if self.i >= self.n:
                raise ParseError("unterminated ${")
            c = self.s[self.i]
            if c == "}":
                text = self.s[start:self.i]
                self.i += 1
                break
            if c == "$":
                nested.extend(self.read_dollar(True))
            elif c == "`":
                nested.append(self.read_backtick(True))
            elif c == '"':
                self.i += 1
                nested.extend(self.read_string("dq"))
            elif c == "'" and not in_dq:
                j = self.s.find("'", self.i + 1)
                if j < 0:
                    raise ParseError("unterminated single quote")
                self.i = j + 1
            elif c == "\\":
                self.i += 2
            else:
                self.i += 1
        m = re.fullmatch(r"([A-Za-z_][A-Za-z0-9_]*)(:?[-=].*)?", text, re.S)
        if not m:
            return [("unk", nested, in_dq)]
        out = [("var", m.group(1), in_dq)]
        if nested:
            out.append(("nest", nested, in_dq))
        return out

    def read_arith(self):
        nested, depth = [], 0
        while True:
            if self.i >= self.n:
                raise ParseError("unterminated arithmetic")
            c = self.s[self.i]
            if c == ")" and depth == 0:
                if self.at(1) != ")":
                    raise ParseError("bad arithmetic")
                self.i += 2
                return nested
            if c == "(":
                depth += 1
                self.i += 1
            elif c == ")":
                depth -= 1
                self.i += 1
            elif c == "$":
                nested.extend(self.read_dollar(True))
            elif c == "`":
                nested.append(self.read_backtick(True))
            elif c == '"':
                self.i += 1
                nested.extend(self.read_string("dq"))
            elif c == "'":
                j = self.s.find("'", self.i + 1)
                if j < 0:
                    raise ParseError("unterminated single quote")
                self.i = j + 1
            elif c == "\\":
                self.i += 2
            else:
                self.i += 1

    def read_backtick(self, in_dq):
        self.i += 1
        buf = []
        while True:
            if self.i >= self.n:
                raise ParseError("unterminated backquote")
            c = self.s[self.i]
            if c == "`":
                self.i += 1
                break
            nx = self.at(1)
            if c == "\\" and nx and (nx in "$`\\" or (in_dq and nx == '"')):
                buf.append(nx)
                self.i += 2
                continue
            buf.append(c)
            self.i += 1
        return ("cmd", Parser("".join(buf), self.depth + 1).parse_script(), in_dq)

    def read_ansi_c(self):
        out = []
        while True:
            if self.i >= self.n:
                raise ParseError("unterminated $'")
            c = self.s[self.i]
            if c == "'":
                self.i += 1
                return "".join(out)
            if c == "\\" and self.i + 1 < self.n:
                nx = self.s[self.i + 1]
                self.i += 2
                if nx in ANSI_C:
                    out.append(ANSI_C[nx])
                elif nx in "xuU":
                    width = {"x": 2, "u": 4, "U": 8}[nx]
                    m = re.compile("[0-9A-Fa-f]{1,%d}" % width).match(self.s, self.i)
                    if m:
                        out.append(chr(int(m.group(), 16)))
                        self.i = m.end()
                    else:
                        out.append("\\" + nx)
                elif nx in "01234567":
                    m = re.compile("[0-7]{0,2}").match(self.s, self.i)
                    out.append(chr(int(nx + m.group(), 8)))
                    self.i = m.end()
                elif nx == "c" and self.i < self.n:
                    out.append(chr(ord(self.s[self.i]) & 31))
                    self.i += 1
                else:
                    out.append("\\" + nx)
                continue
            out.append(c)
            self.i += 1


def is_assignment(word):
    return bool(word) and word[0][0] == "lit" and not word[0][2] and ASSIGN_RE.match(word[0][1]) is not None


def first_simple(lst):
    if not lst.items:
        return None
    cmd = lst.items[0][0].pipelines[0].cmds[0]
    return cmd if isinstance(cmd, Simple) else None


def is_single(lst):
    return (len(lst.items) == 1 and len(lst.items[0][0].pipelines) == 1
            and len(lst.items[0][0].pipelines[0].cmds) == 1 and first_simple(lst) is not None)


# --------------------------------------------------------------------------
# Values and paths

def plain(s):
    return s.replace(STAR, "*").replace(QMARK, "?").replace(LBRACK, "[")


BRACE_RE = re.compile(r"\{[^{}\s]*(?:,|\.\.)[^{}\s]*\}")


def globmark(text):
    text = BRACE_RE.sub("*", text)
    return text.replace("*", STAR).replace("?", QMARK).replace("[", LBRACK)


def dedupe(values):
    out = []
    for v in values:
        if v not in out:
            out.append(v)
    return out if len(out) <= MAX_CANDIDATES else [UNK]


def wild(c):
    return UNK in c or any(g in c for g in GLOBS)


def to_fnmatch(c):
    table = {STAR: "*", QMARK: "?", LBRACK: "[", UNK: "*", "*": "[*]", "?": "[?]", "[": "[[]"}
    return "".join(table.get(ch, ch) for ch in c)


def comp_match(c, b):
    """Can path component c (maybe a pattern) name the directory b ('*' = any)?"""
    if b == "*":
        return True
    if wild(c):
        return fnmatch.fnmatchcase(b, to_fnmatch(c))
    return c == b


def could_be_inside(comps, base):
    """The path may be base itself or below it."""
    for k, b in enumerate(base):
        if k >= len(comps):
            return False
        if UNK in comps[k]:
            return comp_match(comps[k], b)  # an unknown part may span components
        if not comp_match(comps[k], b):
            return False
    return True


def could_be_ancestor(comps, base):
    """The path may be base itself or a directory above it."""
    for k, c in enumerate(comps):
        if k >= len(base):
            return False
        if UNK in c:
            return comp_match(c, base[k])
        if not comp_match(c, base[k]):
            return False
    return True


def definitely_inside(comps, base):
    if len(comps) < len(base):
        return False
    for k, b in enumerate(base):
        if wild(comps[k]) or (b != "*" and comps[k] != b):
            return False
    return True


def normalize(path):
    """Absolute path → components, with `.` and `..` resolved. A `..` after an
    unknown component ends the known part."""
    out = []
    for c in path.split("/"):
        if c in ("", "."):
            continue
        if c == "..":
            if out and wild(out[-1]):
                out.append(UNK)
                break
            if out:
                out.pop()
            continue
        out.append(c)
    return out


def first_operand(args, short_arg="", long_arg=()):
    """Index of the first non-option argument."""
    i = 0
    while i < len(args):
        a = plain(args[i])
        if a == "--":
            return i + 1
        if not a.startswith("-") or a == "-":
            return i
        if a.startswith("--"):
            i += 2 if ("=" not in a and a in long_arg) else 1
            continue
        for j in range(1, len(a)):
            if a[j] in short_arg:
                if j == len(a) - 1:
                    i += 1
                break
        i += 1
    return i


def operands(args, short_arg="", long_arg=()):
    """(operands, options) of a GNU-style argument list; options as (name, value)."""
    ops, opts, i, dd = [], [], 0, False
    while i < len(args):
        a = args[i]
        p = plain(a)
        if dd or not p.startswith("-") or p == "-":
            ops.append(a)
        elif p == "--":
            dd = True
        elif p.startswith("--"):
            name, eq, val = p.partition("=")
            if not eq and name in long_arg:
                val = args[i + 1] if i + 1 < len(args) else ""
                i += 1
            opts.append((name, val))
        else:
            for j in range(1, len(p)):
                if p[j] in short_arg:
                    if j == len(p) - 1:
                        val = args[i + 1] if i + 1 < len(args) else ""
                        i += 1
                    else:
                        val = a[j + 1:]
                    opts.append(("-" + p[j], val))
                    break
                opts.append(("-" + p[j], None))
        i += 1
    return ops, opts


# --------------------------------------------------------------------------
# Rules

PRIVILEGE = {"sudo", "doas", "su", "pkexec", "run0", "runuser"}
PACKAGE = {"pacman", "yay", "paru", "makepkg", "pacstrap"}
SERVICE = {"systemctl", "loginctl", "reboot", "shutdown", "poweroff", "halt", "mkinitcpio", "systemd-run"}
SHELLS = {"bash", "sh", "zsh", "dash", "ksh", "mksh", "rbash", "ash", "yash", "fish"}

# `omarchy <route>` that changes the system: (first word, second words or None).
# Each word matches as a prefix (`omarchy updates`, `omarchy installed …`), as
# the grep rules before WP-130 did.
OMARCHY_SYSTEM = [("pkg", ("add", "aur", "drop", "install", "remove")), ("update", None), ("install", None),
                  ("theme", ("set",)), ("plugin", ("add", "remove", "update", "clone")), ("snapshot", None),
                  ("migrate", None), ("refresh", None), ("hook", ("install",)), ("dev", ("link",))]
OMARCHY_BIN_SYSTEM = ("pkg-add", "pkg-aur", "pkg-drop", "pkg-install", "pkg-remove", "update", "install",
                      "theme-set", "plugin-add", "plugin-remove", "plugin-update", "plugin-clone", "snapshot",
                      "migrate", "refresh", "hook-install", "dev-link")
AGENT_VERBS = ("prompt", "launch", "start", "run", "chat", "ask")

SYSTEM_DIRS = [["etc"], ["usr"], ["boot"], ["var"]]
PLUGIN_DIR = ["omarchy", "plugins", "jax.seldon"]

SYSTEMCTL_LONG_ARG = {"--type", "--property", "--signal", "--host", "--machine", "--lines", "--output", "--state",
                      "--kill-whom", "--kill-value", "--root", "--image", "--what", "--job-mode", "--preset-mode",
                      "--boot-loader-entry", "--boot-loader-menu", "--timestamp", "--message", "--drop-in",
                      "--when", "--check-inhibitors", "--image-policy", "--reboot-argument"}
PACMAN_OPS = {"--query": "Q", "--sync": "S", "--remove": "R", "--upgrade": "U", "--database": "D",
              "--files": "F", "--deptest": "T", "--version": "V"}


def pacman_read_only(args):
    """`pacman -Q…` (any query) and `pacman -S` with `-p`/`--print` and
    without `-y`/`-u`/`-c`/`-w`: they read the databases and change nothing."""
    ops, flags, longs = set(), set(), set()
    for raw in args:
        a = plain(raw)
        if a == "--":
            break
        if a.startswith("--"):
            name = a.split("=", 1)[0]
            if name in PACMAN_OPS:
                ops.add(PACMAN_OPS[name])
            else:
                longs.add(name)
        elif a.startswith("-") and len(a) > 1:
            for ch in a[1:]:
                (ops if ch.isupper() else flags).add(ch)
    if ops == {"Q"}:
        return True
    return (ops == {"S"} and ("p" in flags or "--print" in longs) and not flags & set("ycuw")
            and not longs & {"--refresh", "--clean", "--sysupgrade", "--downloadonly"})


def systemctl_read_only(args):
    ops, _ = operands(args, "tpsHMno", SYSTEMCTL_LONG_ARG)
    return bool(ops) and plain(ops[0]).startswith(("status", "list-", "show", "is-", "cat"))


def omarchy_route(words, table):
    if not words:
        return False
    for first, seconds in table:
        if seconds is None:
            if words[0].startswith(first):
                return True
        elif words[0] == first and len(words) > 1 and words[1].startswith(seconds):
            return True
    return False


class Ctx:
    def __init__(self, **kw):
        self.remote = False  # inside the remote command of ssh
        self.depth = 0
        self.stdin = None  # stdin of the script: None, ('pipe',), ('file',), ('text', [str])
        self.top_first = None  # the first simple command of the whole command line
        self.top_single = False  # the whole command line is that one simple command
        self.ssh_first = False  # (remote) the ssh is top_first, unwrapped
        self.ssh_single = False  # (remote) … and top_single
        self.r_first = None  # (remote) the first simple command of the remote script
        self.r_single = False  # (remote) the remote script is that one simple command
        self.__dict__.update(kw)

    def but(self, **kw):
        c = Ctx(**self.__dict__)
        c.__dict__.update(kw)
        return c


def merge(scopes):
    keys = set()
    for s in scopes:
        keys.update(s)
    out = {}
    for k in keys:
        out[k] = dedupe([v for s in scopes for v in s.get(k, [None])])
    return out


def host_regex(host):
    # The test-host rule as before WP-130 (operator decision 2026-10-05),
    # matched against the whole command, which must be one line.
    return re.compile(r"[ \t]*(?:timeout[ \t]+[0-9]+[ \t]+)?ssh(?:[ \t]+-[A-Za-z]+(?:[ \t]+[^ \t\n]+)?)*[ \t]+"
                      + re.escape(host) + r"""(?:[ \t]+(?:'[^'\n]*'|"[^"\n]*"|[^;&|'"\n]*))?[ \t]*""")


# The two makepkg forms packaging/README.md uses on the test host over ssh
# (ORCHESTRATION.md §11, WP-040 review); fully literal, the whole command.
MAKEPKG_RE = re.compile(r"[ \t]*ssh[ \t]+[A-Za-z0-9._@-]+[ \t]+'cd /tmp/[A-Za-z0-9._-]+ && "
                        r"makepkg (?:-f|--printsrcinfo > SRCINFO\.new)'[ \t]*")


def read_hosts():
    path = os.environ.get("GUARD_HOSTS_FILE") or os.path.join(os.path.dirname(os.path.abspath(__file__)),
                                                              "guard-hosts.local")
    hosts = set()
    try:
        with open(path, encoding="utf-8") as f:
            for line in f:
                host = re.sub(r"\s", "", line.split("#", 1)[0])
                if host:
                    hosts.add(host)
    except OSError:
        pass
    return hosts


class Guard:
    def __init__(self, command, cwd):
        self.command = command
        self.cwd = cwd
        self.home = os.environ.get("HOME") or pwd.getpwuid(os.getuid()).pw_dir
        self.home_comps = normalize(self.home)
        self.hosts = read_hosts()
        self.host_gate = False

    def run(self):
        whole = self.command.strip(" \t\n")
        if MAKEPKG_RE.fullmatch(whole):
            return
        self.host_gate = any(host_regex(h).fullmatch(whole) for h in self.hosts)
        try:
            tree = Parser(self.command).parse_script()
        except ParseError as e:
            raise Unsure(f"cannot parse: {e}")
        ctx = Ctx(top_first=first_simple(tree), top_single=is_single(tree))
        self.walk_list(tree, {".cwd": [self.cwd]}, ctx)

    # -- walking the tree
    def walk_list(self, lst, scope, ctx):
        for ao, sep in lst.items:
            if sep == "&":
                self.walk_andor(ao, dict(scope), ctx)
            else:
                scope = self.walk_andor(ao, scope, ctx)
        return scope

    def walk_andor(self, ao, scope, ctx):
        # The first pipeline always runs; each later one may not, so what it
        # assigns is one candidate among the values before it.
        scopes, cur, all_and = [], scope, True
        for k, pl in enumerate(ao.pipelines):
            if k > 0 and ao.ops[k - 1] == "||":
                all_and = False
            entry = scope if k == 0 else (cur if all_and else merge(scopes))
            cur = self.walk_pipeline(pl, entry, ctx)
            scopes.append(cur)
        return merge(scopes)

    def walk_pipeline(self, pl, scope, ctx):
        if len(pl.cmds) == 1:
            return self.walk_command(pl.cmds[0], scope, ctx, False)
        for k, cmd in enumerate(pl.cmds):
            self.walk_command(cmd, dict(scope), ctx, k > 0)
        return scope

    def walk_command(self, cmd, scope, ctx, piped):
        if isinstance(cmd, Simple):
            return self.walk_simple(cmd, scope, ctx, piped)
        stdin = self.redirs(cmd.redirs, scope, ctx, cmd) or (("pipe",) if piped else ctx.stdin)
        inner = ctx.but(stdin=stdin)
        kind = cmd.kind
        if kind == "subshell":
            self.walk_list(cmd.body, dict(scope), inner)
            return scope
        if kind == "group":
            return self.walk_list(cmd.body, scope, inner)
        if kind == "if":
            outs, cur = [scope], scope
            for lst in cmd.lists:
                cur = self.walk_list(lst, cur, inner)
                outs.append(cur)
            return merge(outs)
        if kind == "for":
            body_scope = dict(scope)
            values = []
            for w in cmd.words or []:
                self.walk_parts(w, scope, ctx)
                for fields in self.expand(w, scope, ctx):
                    values.extend(fields)
            if cmd.var:
                body_scope[cmd.var] = dedupe(values) if cmd.words else [UNK]
            return merge([scope, self.walk_list(cmd.body, body_scope, inner)])
        if kind == "case":
            self.walk_parts(cmd.word, scope, ctx)
            outs = [scope]
            for pats, body in cmd.items:
                for p in pats:
                    self.walk_parts(p, scope, ctx)
                outs.append(self.walk_list(body, scope, inner))
            return merge(outs)
        if kind == "arith":
            self.walk_parts(cmd.parts, scope, ctx)
            return scope
        if kind == "cond":
            for w in cmd.words:
                self.walk_parts(w, scope, ctx)
            return scope
        if kind == "func":
            self.walk_command(cmd.body, dict(scope), ctx.but(stdin=None), False)
            return scope
        raise Unsure(f"unknown construct {kind}")

    def walk_parts(self, parts, scope, ctx):
        """Check every command nested in a word: $(…), backticks, <(…)."""
        for kind, payload, _ in parts:
            if kind in ("cmd", "proc"):
                self.walk_list(payload, dict(scope), ctx.but(stdin=None))
            elif kind in ("unk", "nest"):
                self.walk_parts(payload, scope, ctx)

    def walk_simple(self, cmd, scope, ctx, piped):
        for w in cmd.assigns + cmd.words:
            self.walk_parts(w, scope, ctx)
        own = self.redirs(cmd.redirs, scope, ctx, cmd)
        stdin = own or (("pipe",) if piped else ctx.stdin)
        env = dict(scope)
        for w in cmd.assigns:
            name, values = self.assignment(w, env, ctx)
            env[name] = values
        if not cmd.words:
            return env
        after = None
        for argv in self.expand_words(cmd.words, scope, ctx):
            res = self.check_argv(argv, env, ctx, stdin, True, cmd)
            if res is not None:
                after = res
        return self.effects(cmd, scope, ctx, after)

    def redirs(self, redirs, scope, ctx, node):
        """Check redirection targets; return what fd 0 reads, if set here."""
        stdin = None
        for r in redirs:
            to_stdin = r.fd in (None, "0")
            if r.op in ("<<", "<<-"):
                if r.parts is not None:
                    self.walk_parts(r.parts, scope, ctx)
                    texts = self.expand_text(r.parts, scope, ctx, tilde=False)
                else:
                    texts = [r.body]
                if to_stdin:
                    stdin = ("text", texts)
                continue
            self.walk_parts(r.target, scope, ctx)
            if r.op == "<<<":
                if to_stdin:
                    stdin = ("text", [t + "\n" for t in self.expand_text(r.target, scope, ctx)])
                continue
            if r.op in ("<", "<&"):
                if to_stdin and r.op == "<":
                    stdin = ("file",)
                continue
            for fields in self.expand(r.target, scope, ctx):
                for f in fields:
                    if r.op == ">&" and (f.isdigit() or f == "-"):
                        continue
                    self.check_write(f, scope, ctx, False, node, True)
        return stdin

    # -- expansion
    def lookup(self, name, scope, ctx):
        if name == "PWD" and name not in scope:
            return list(scope.get(".cwd", [UNK]))
        out = []
        for v in scope.get(name, [None]):
            if v is None:
                if ctx.remote:
                    v = self.home if name == "HOME" else UNK
                elif name in os.environ:
                    v = os.environ[name]
                elif name == "OMARCHY_PATH":
                    v = "/usr/share/omarchy"
                else:
                    v = UNK
            if name == "HOME" and (v == "" or v.startswith(UNK)):
                v = self.home  # an unknown HOME may be the real one
            out.append(v)
        return dedupe(out)

    def cmd_value(self, lst, scope, ctx):
        """The output of $(…) when the guard can know it: pwd, mktemp."""
        c = first_simple(lst) if is_single(lst) else None
        if c is None or c.assigns or not c.words:
            return [UNK]
        out = []
        for argv in self.expand_words(c.words, scope, ctx):
            name = plain(argv[0]).rsplit("/", 1)[-1] if argv else ""
            if name == "pwd":
                out.extend(scope.get(".cwd", [UNK]))
            elif name == "mktemp":
                out.extend(self.mktemp_value(argv[1:], scope, ctx))
            else:
                out.append(UNK)
        return dedupe(out) or [UNK]

    def mktemp_value(self, args, scope, ctx):
        tmp = "/tmp" if ctx.remote else (os.environ.get("TMPDIR") or "/tmp")
        tmpdir, template = None, None
        ops, opts = operands(args, "p", ("--suffix",))
        for name, val in opts:
            if name == "-p":
                tmpdir = val or tmp
            elif name == "--tmpdir" or name == "-t":
                tmpdir = tmpdir or val or tmp
        if ops:
            template = ops[0]
        if template is None:
            return [(tmpdir or tmp).rstrip("/") + "/" + UNK]
        if tmpdir:
            return [tmpdir.rstrip("/") + "/" + UNK]
        if "/" in template:
            d = template.rsplit("/", 1)[0] or "/"
            if d.startswith("/"):
                return [d.rstrip("/") + "/" + UNK]
            return [c.rstrip("/") + "/" + d + "/" + UNK for c in scope.get(".cwd", [UNK])]
        return [c.rstrip("/") + "/" + UNK for c in scope.get(".cwd", [UNK])]

    TILDE_RE = re.compile(r"~([A-Za-z0-9._-]*|[+-])(?=/|$)")

    def options(self, parts, scope, ctx, tilde):
        """Each part with its possible values."""
        parts = list(parts)
        if tilde and parts and parts[0][0] == "lit" and not parts[0][2]:
            m = self.TILDE_RE.match(parts[0][1])
            if m:
                parts = [("tilde", m.group(1), False), ("lit", parts[0][1][m.end():], False)] + parts[1:]
        opts = []
        for kind, payload, _ in parts:
            if kind == "lit":
                opts.append([payload])
            elif kind == "tilde":
                if payload == "":
                    opts.append(self.lookup("HOME", scope, ctx))
                elif payload == "+":
                    opts.append(list(scope.get(".cwd", [UNK])))
                elif payload == "-":
                    opts.append([UNK])
                else:
                    opts.append(["/root" if payload == "root" else "/home/" + payload])
            elif kind == "var":
                opts.append(self.lookup(payload, scope, ctx))
            elif kind == "cmd":
                opts.append(self.cmd_value(payload, scope, ctx))
            elif kind == "proc":
                opts.append(["/dev/fd/" + UNK])
            elif kind == "nest":
                opts.append([""])
            else:
                opts.append([UNK])
        total = 1
        for o in opts:
            total *= len(o)
        if total > MAX_CANDIDATES:
            opts = [o if len(o) == 1 else [UNK] for o in opts]
        return parts, opts

    def expand(self, word, scope, ctx):
        """A word → its possible values, each a list of fields (word splitting
        and glob marks applied where bash applies them)."""
        parts, opts = self.options(word, scope, ctx, True)
        result = []
        for combo in itertools.product(*opts):
            fields, cur, have = [], [], False
            for (kind, _, quoted), text in zip(parts, combo):
                if quoted or kind == "tilde":
                    cur.append(text)
                    have = True
                elif kind == "lit":
                    cur.append(globmark(text))
                    have = True
                elif text:
                    for k, piece in enumerate(re.split(r"[ \t\n]+", text)):
                        if k > 0 and (have or cur):
                            fields.append("".join(cur))
                            cur, have = [], False
                        if piece:
                            cur.append(globmark(piece))
                            have = True
            if have or cur:
                fields.append("".join(cur))
            result.append(fields)
        return result

    def expand_text(self, parts, scope, ctx, tilde=True):
        """Parts → possible strings, without splitting or globbing."""
        parts, opts = self.options(parts, scope, ctx, tilde)
        return dedupe(["".join(combo) for combo in itertools.product(*opts)])

    def expand_words(self, words, scope, ctx):
        per_word = [self.expand(w, scope, ctx) for w in words]
        total = 1
        for p in per_word:
            total *= len(p)
        if total > MAX_CANDIDATES:
            per_word = [p if len(p) == 1 else [[UNK]] for p in per_word]
        return [[f for fields in combo for f in fields] for combo in itertools.product(*per_word)]

    def assignment(self, word, scope, ctx):
        kind, text, _ = word[0]
        m = ASSIGN_RE.match(text)
        name = m.group("name")
        if m.group("idx") or m.group("op") == "+=":
            return name, [UNK]
        rest = text[m.end():]
        value = ([("lit", rest, False)] if rest else []) + list(word[1:])
        return name, self.expand_text(value, scope, ctx)

    def effects(self, cmd, scope, ctx, after):
        """Scope changes of builtins: cd, export, read, unset, eval."""
        new = dict(after if after is not None else scope)
        w0 = cmd.words[0]
        if not all(k == "lit" for k, _, _ in w0):
            return new
        name = "".join(p for _, p, _ in w0)
        argvs = self.expand_words(cmd.words, scope, ctx)
        if name in ("cd", "pushd"):
            values = []
            for argv in argvs:
                args = [a for a in argv[1:] if a == "-" or not a.startswith("-")]
                targets = self.lookup("HOME", scope, ctx) if not args else ([UNK] if args[0] == "-" else [args[0]])
                for t in targets:
                    for base in scope.get(".cwd", [UNK]):
                        values.append(t if t.startswith("/") or t.startswith(UNK) else base.rstrip("/") + "/" + t)
            new[".cwd"] = dedupe(values) or [UNK]
        elif name == "popd":
            new[".cwd"] = [UNK]
        elif name in ("export", "declare", "typeset", "local", "readonly"):
            for w in cmd.words[1:]:
                if is_assignment(w):
                    n, v = self.assignment(w, new, ctx)
                    new[n] = v
        elif name in ("read", "mapfile", "readarray"):
            for argv in argvs:
                for a in argv[1:]:
                    if NAME_RE.fullmatch(a):
                        new[a] = [UNK]
        elif name == "unset":
            for argv in argvs:
                for a in argv[1:]:
                    new.pop(a, None)
        elif name == "printf":
            for argv in argvs:
                if "-v" in argv[1:-1]:
                    new[argv[argv.index("-v") + 1]] = [UNK]
        return new

    # -- the command position
    def cmd_name(self, field):
        if wild(field):
            base = field.rsplit("/", 1)[-1]
            if "/" in field and base and not wild(base):
                return base
            raise Unsure("the command name is computed (a variable, $(…) or a glob); write it literally")
        return field.rsplit("/", 1)[-1]

    def check_argv(self, argv, scope, ctx, stdin, direct, node):
        """Check one command with its expanded arguments. Returns a scope when
        the command changes the shell's variables (eval)."""
        if not argv:
            return None
        name = self.cmd_name(argv[0])
        args = argv[1:]
        wrapped = lambda rest, sc=scope: self.check_argv(rest, sc, ctx, stdin, False, node)  # noqa: E731

        if name in PRIVILEGE:
            raise Blocked(f"privileged or package command: {name}")
        # wrappers: the real command follows
        if name == "env":
            return self.w_env(args, scope, ctx, stdin, node)
        if name == "command":
            i = first_operand(args)
            if any(a != "--" and "v" in a.lower() for a in args[:i]):
                return None  # a lookup, not a run
            return wrapped(args[i:])
        if name in ("builtin", "nohup", "setsid"):
            return wrapped(args[first_operand(args):])
        if name == "exec":
            return wrapped(args[first_operand(args, "a"):])
        if name == "nice":
            return wrapped(args[first_operand(args, "n", ("--adjustment",)):])
        if name == "stdbuf":
            return wrapped(args[first_operand(args, "ioe", ("--input", "--output", "--error")):])
        if name == "ionice":
            return wrapped(args[first_operand(args, "cnpPu", ("--class", "--classdata", "--pid", "--pgid", "--uid")):])
        if name == "time":
            i = first_operand(args, "of", ("--output", "--format"))
            for opt, val in operands(args[:i], "of", ("--output", "--format"))[1]:
                if opt in ("-o", "--output"):
                    self.check_write(val, scope, ctx, False, node, False)
            return wrapped(args[i:])
        if name == "timeout":
            i = first_operand(args, "sk", ("--signal", "--kill-after"))
            return wrapped(args[i + 1:])
        if name == "flock":
            return self.w_flock(args, scope, ctx, stdin, node)
        if name == "xargs":
            i = first_operand(args, "adEILnPs", ("--arg-file", "--delimiter", "--max-args", "--max-procs",
                                                 "--max-chars", "--process-slot-var"))
            return wrapped(args[i:] or ["echo"])
        if name == "watch":
            i = first_operand(args, "nq", ("--interval", "--equexit"))
            if any(plain(a) in ("-x", "--exec") for a in args[:i]):
                return wrapped(args[i:])
            return self.run_script(" ".join(args[i:]), dict(scope), ctx.but(stdin=stdin), "watch") \
                if args[i:] else None
        if name == "busybox":
            if args and plain(args[0]) in SHELLS:
                return self.w_shell(args[1:], scope, ctx, stdin, plain(args[0]))
            return wrapped(args)
        if name == "find":
            return self.r_find(args, scope, ctx, node)
        # code in arguments
        if name in SHELLS:
            return self.w_shell(args, scope, ctx, stdin, name)
        if name == "eval":
            return self.run_script(" ".join(args), scope, ctx.but(stdin=stdin), "eval")
        if name == "trap":
            if len(args) >= 2 and not plain(args[0]).startswith("-"):
                self.run_script(args[0], dict(scope), ctx.but(stdin=None), "trap")
            return None
        if name in ("source", "."):
            if args:
                self.sourced(plain(args[0]), scope, ctx, stdin, name)
            return None
        if name == "ssh":
            return self.w_ssh(args, scope, ctx, stdin, direct, node)
        # rules
        if name in PACKAGE:
            if name == "pacman" and pacman_read_only(args):
                return None
            raise Blocked(f"privileged or package command: {name}")
        if name in SERVICE or name.startswith("grub-"):
            if name == "systemctl" and systemctl_read_only(args):
                return None
            raise Blocked(f"service or boot command: {name}")
        if name == "omarchy":
            self.r_omarchy([plain(a) for a in args], ctx)
            return None
        if name.startswith("omarchy-"):
            self.r_omarchy_bin(name[len("omarchy-"):], ctx)
            return None
        for path, recursive in self.write_targets(name, args):
            self.check_write(path, scope, ctx, recursive, node, direct)
        return None

    def w_env(self, args, scope, ctx, stdin, node):
        env = dict(scope)
        i = 0
        while i < len(args):
            a = plain(args[i])
            if a == "--":
                i += 1
                break
            if a in ("-i", "--ignore-environment", "-", "-0", "--null", "-v", "--debug"):
                i += 1
            elif a in ("-u", "--unset"):
                i += 2
            elif a.startswith(("--unset=", "-u")):
                i += 1
            elif a in ("-C", "--chdir") or a.startswith(("--chdir=", "-C")):
                if a in ("-C", "--chdir"):
                    d = args[i + 1] if i + 1 < len(args) else ""
                    i += 2
                else:
                    d = a.split("=", 1)[1] if a.startswith("--") else a[2:]
                    i += 1
                env[".cwd"] = [d if d.startswith(("/", UNK)) else c.rstrip("/") + "/" + d
                               for c in scope.get(".cwd", [UNK])]
            elif a.startswith(("-S", "--split-string")):
                raise Unsure("env -S is not supported by the guard; write the command directly")
            elif a.startswith("-"):
                raise Unsure(f"env option {a} is not known to the guard")
            else:
                break
        while i < len(args) and ASSIGN_RE.match(args[i]):
            n, _, v = args[i].partition("=")
            env[n.rstrip("+")] = [v]
            i += 1
        if i >= len(args):
            return None
        return self.check_argv(args[i:], env, ctx, stdin, False, node)

    def w_flock(self, args, scope, ctx, stdin, node):
        i = first_operand(args, "wE", ("--timeout", "--conflict-exit-code"))
        if i >= len(args):
            return None
        if not plain(args[i]).isdigit():
            self.check_write(args[i], scope, ctx, False, node, False)  # the lock file
        rest = args[i + 1:]
        if not rest:
            return None
        if plain(rest[0]) in ("-c", "--command"):
            if len(rest) < 2:
                raise Unsure("flock -c without a command")
            return self.run_script(rest[1], dict(scope), ctx.but(stdin=stdin), "flock -c")
        return self.check_argv(rest, scope, ctx, stdin, False, node)

    def w_shell(self, args, scope, ctx, stdin, name):
        i, cmode, smode = 0, False, False
        while i < len(args):
            a = plain(args[i])
            if a in ("--", "-"):
                i += 1
                break
            if a.startswith("--"):
                i += 2 if a in ("--rcfile", "--init-file") else 1
                continue
            if a[:1] in "-+" and len(a) > 1:
                take = 0
                for ch in a[1:]:
                    if ch == "c":
                        cmode = True
                    elif ch == "s":
                        smode = True
                    elif ch in "oO":
                        take += 1
                i += 1 + take
                continue
            break
        rest = args[i:]
        if cmode:
            if not rest:
                raise Unsure(f"{name} -c without a command string")
            return self.run_script(rest[0], dict(scope), ctx.but(stdin=stdin), f"{name} -c")
        if rest and not smode:
            self.sourced(plain(rest[0]), scope, ctx, stdin, name)
            return None
        self.stdin_script(stdin, dict(scope), ctx, name)
        return None

    def sourced(self, path, scope, ctx, stdin, name):
        """A script file run by a shell or `source`: not inspected, unless it is
        stdin or generated."""
        if path in ("/dev/stdin", "/proc/self/fd/0", "-"):
            self.stdin_script(stdin, dict(scope), ctx, name)
        elif path.startswith("/dev/fd/") or UNK in path.rsplit("/", 1)[-1]:
            raise Unsure(f"{name} runs generated code (a process substitution or a computed file name)")

    def stdin_script(self, stdin, scope, ctx, name):
        if stdin is None or stdin[0] == "file":
            return
        if stdin[0] == "pipe":
            raise Unsure(f"{name} reads commands from a pipe; use a heredoc or {name} -c '…'")
        for text in stdin[1]:
            self.run_script(text, scope, ctx.but(stdin=None), f"{name} stdin")

    def run_script(self, text, scope, ctx, what, remote_top=False):
        if ctx.depth >= MAX_DEPTH:
            raise Unsure("commands nested too deep")
        try:
            tree = Parser(plain(text), ctx.depth + 1).parse_script()
        except ParseError as e:
            raise Unsure(f"cannot parse the {what} string: {e}")
        inner = ctx.but(depth=ctx.depth + 1)
        if remote_top:
            inner = inner.but(r_first=first_simple(tree), r_single=is_single(tree))
        return self.walk_list(tree, scope, inner)

    def w_ssh(self, args, scope, ctx, stdin, direct, node):
        i = 0
        while i < len(args):
            a = plain(args[i])
            if a == "--":
                i += 1
                break
            if not a.startswith("-") or len(a) < 2:
                break
            for j in range(1, len(a)):
                if a[j] in "BbcDEeFIiJLlmOoPpQRSWw":
                    if j == len(a) - 1:
                        val = plain(args[i + 1]) if i + 1 < len(args) else ""
                        i += 1
                    else:
                        val = a[j + 1:]
                    if a[j] == "o":
                        key = re.split(r"[\s=]", val.strip(), maxsplit=1)[0].lower()
                        if key in ("proxycommand", "localcommand", "knownhostscommand"):
                            raise Blocked(f"ssh option {key} runs a local command")
                    break
            i += 1
        if i >= len(args):
            return None
        host, rest = plain(args[i]), args[i + 1:]
        if self.host_gate and not ctx.remote and host in self.hosts:
            return None  # a test host the operator released (scripts/guard-hosts.local)
        first = direct and node is ctx.top_first and ctx.depth == 0 and not ctx.remote
        rctx = ctx.but(remote=True, stdin=stdin, ssh_first=first, ssh_single=first and ctx.top_single)
        rscope = {"HOME": [self.home], ".cwd": [self.home]}
        if rest:
            self.run_script(" ".join(rest), rscope, rctx, "ssh remote command", remote_top=True)
        else:
            self.stdin_script(stdin, rscope, rctx.but(depth=ctx.depth), "ssh")
        return None

    def r_find(self, args, scope, ctx, node):
        i = 0
        while i < len(args) and (plain(args[i]) in ("-H", "-L", "-P") or plain(args[i]).startswith(("-D", "-O"))):
            i += 2 if plain(args[i]) == "-D" else 1
        starts = []
        while i < len(args) and not (plain(args[i]).startswith("-") or plain(args[i]) in ("(", "!", ")", ",")):
            starts.append(args[i])
            i += 1
        starts = starts or ["."]
        while i < len(args):
            a = plain(args[i])
            if a == "-delete":
                for s in starts:
                    self.check_write(s, scope, ctx, True, node, False)
            elif a in ("-exec", "-execdir", "-ok", "-okdir"):
                j = i + 1
                while j < len(args) and plain(args[j]) not in (";", "+"):
                    j += 1
                for s in starts:
                    sub = [x.replace("{}", s.rstrip("/") + "/" + UNK) for x in args[i + 1:j]]
                    self.check_argv(sub, scope, ctx, None, False, node)
                i = j
            elif a in ("-fprint", "-fprint0", "-fls", "-fprintf") and i + 1 < len(args):
                self.check_write(args[i + 1], scope, ctx, False, node, False)
                i += 1
            i += 1
        return None

    def r_omarchy(self, args, ctx):
        before = args[:args.index("--")] if "--" in args else args
        if "--help" in before or "-h" in before:
            return  # the dispatcher prints help and runs nothing (/usr/bin/omarchy)
        if omarchy_route(args, [("agent", AGENT_VERBS)]) or (args[:1] == ["launch"] and len(args) > 1):
            if not ctx.remote:
                raise Blocked("agent or app launcher on the dev host")
            return
        if omarchy_route(args, OMARCHY_SYSTEM):
            theme = args[:1] == ["theme"] and len(args) > 1 and args[1].startswith("set")
            if theme and ctx.remote and ctx.ssh_single and ctx.r_single:
                return  # the theme sweep over ssh (ORCHESTRATION.md §11)
            raise Blocked("omarchy command that changes the system")

    def r_omarchy_bin(self, rest, ctx):
        if rest == "agent" or rest.startswith(("agent-", "launch-")):
            if not ctx.remote:
                raise Blocked("agent or app launcher on the dev host")
            return
        if rest.startswith(OMARCHY_BIN_SYSTEM):
            raise Blocked("omarchy command that changes the system")

    # -- writes
    def write_targets(self, name, args):
        """(path, recursive) pairs a file command writes, removes or changes."""
        if name in ("rm", "rmdir", "unlink", "shred", "tee"):
            ops, opts = operands(args, "nsu" if name == "shred" else "")
            rec = name == "rm" and any(o in ("-r", "-R", "--recursive") for o, _ in opts)
            return [(p, rec) for p in ops]
        if name in ("truncate", "touch", "mkdir", "setfacl"):
            short = {"truncate": "sr", "touch": "dtr", "mkdir": "m", "setfacl": "mMxX"}[name]
            longs = ("--size", "--reference", "--date", "--time", "--mode", "--context", "--modify", "--remove",
                     "--set", "--modify-file", "--remove-file", "--set-file")
            ops, opts = operands(args, short, longs)
            rec = name == "setfacl" and any(o in ("-R", "--recursive") for o, _ in opts)
            return [(p, rec) for p in ops]
        if name in ("chmod", "chown", "chgrp", "chattr"):
            ops, opts = operands(args, "vp" if name == "chattr" else "", ("--from", "--reference"))
            rec = any(o in ("-R", "--recursive") for o, _ in opts)
            if not any(o == "--reference" for o, _ in opts):
                ops = ops[1:]
            return [(p, rec) for p in ops]
        if name in ("cp", "mv", "ln", "install"):
            short = "tSmogZ" if name == "install" else "tS"
            ops, opts = operands(args, short, ("--target-directory", "--suffix", "--mode", "--owner", "--group"))
            tdir = [v for o, v in opts if o in ("-t", "--target-directory")]
            if name == "install" and any(o in ("-d", "--directory") for o, _ in opts):
                return [(p, False) for p in ops]
            dests = tdir or ops[-1:]
            sources = ops if tdir else ops[:-1]
            out = [(d, False) for d in dests]
            if name == "mv":
                out += [(s, True) for s in sources]
            return out
        if name == "sed":
            return [(p, False) for p in self.sed_files(args)]
        if name == "dd":
            return [(a[3:], False) for a in args if a.startswith("of=")]
        if name in ("rsync", "scp"):
            ops = [a for a in args if not a.startswith("-")]
            dest = ops[-1] if len(ops) >= 2 else None
            if dest and not re.match(r"[^/]*:", dest) and not dest.startswith("rsync://"):
                return [(dest, False)]
        return []

    @staticmethod
    def sed_files(args):
        in_place, script, files, i = False, False, [], 0
        while i < len(args):
            a = args[i]
            if a.startswith("--"):
                if a.startswith("--in-place"):
                    in_place = True
                elif a.split("=", 1)[0] in ("--expression", "--file"):
                    script = True
                    i += 0 if "=" in a else 1
                elif a == "--line-length":
                    i += 1
            elif a.startswith("-") and len(a) > 1:
                for j in range(1, len(a)):
                    if a[j] == "i":
                        in_place = True
                        break
                    if a[j] in "efl":
                        script = script or a[j] in "ef"
                        if j == len(a) - 1:
                            i += 1
                        break
            else:
                files.append(a)
            i += 1
        if not in_place:
            return []
        return files if script else files[1:]

    def check_write(self, path, scope, ctx, recursive, node, direct):
        if not path or path.startswith(UNK):
            return  # an unknown path: nothing to decide
        if path.startswith("/"):
            fulls = [path]
        else:
            fulls = [c.rstrip("/") + "/" + path for c in scope.get(".cwd", [UNK]) if not c.startswith(UNK)]
        for full in fulls:
            comps = normalize(full)
            for base in SYSTEM_DIRS:
                if could_be_inside(comps, base) or (recursive and could_be_ancestor(comps, base)):
                    raise Blocked("write under /etc, /usr, /boot or /var")
            seldon_ok = ctx.remote and ctx.ssh_first and direct and node is ctx.r_first
            for root in (self.home_comps + [".config"], ["home", "*", ".config"], ["root", ".config"]):
                if could_be_inside(comps, root):
                    if definitely_inside(comps, root + PLUGIN_DIR):
                        continue
                    if seldon_ok and definitely_inside(comps, root + ["seldon"]):
                        continue  # Seldon's config on the test host (docs/TESTING.md, smoke test)
                    raise Blocked("write under ~/.config outside the jax.seldon plugin dir")
                if recursive and could_be_ancestor(comps, root):
                    raise Blocked("write under ~/.config outside the jax.seldon plugin dir")
            for d in (self.home_comps + ["Seldon"], self.home_comps + [".local", "state", "seldon"]):
                if could_be_inside(comps, d) or (recursive and could_be_ancestor(comps, d)):
                    raise Blocked("write under the real ~/Seldon or ~/.local/state/seldon")


def main():
    try:
        data = json.loads(sys.stdin.read())
    except ValueError:
        print(f"guard: blocked ({CLOSED}): the hook input is not JSON", file=sys.stderr)
        return 2
    tool_input = data.get("tool_input") if isinstance(data, dict) else None
    command = tool_input.get("command") if isinstance(tool_input, dict) else None
    if command is None or command == "":
        return 0
    if not isinstance(command, str):
        print(f"guard: blocked ({CLOSED}): the command is not a string", file=sys.stderr)
        return 2
    cwd = data.get("cwd") if isinstance(data.get("cwd"), str) and data.get("cwd").startswith("/") else os.getcwd()
    try:
        Guard(command, cwd).run()
    except Blocked as e:
        print(f"guard: blocked ({RED}): {e}", file=sys.stderr)
        return 2
    except Unsure as e:
        print(f"guard: blocked ({CLOSED}): {e}", file=sys.stderr)
        return 2
    except RecursionError:
        print(f"guard: blocked ({CLOSED}): commands nested too deep", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except SystemExit:
        raise
    except BaseException as e:  # fail closed on any bug
        print(f"guard: blocked ({CLOSED}): internal error {type(e).__name__}: {e}", file=sys.stderr)
        sys.exit(2)
