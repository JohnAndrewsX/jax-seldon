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
import signal
import socket
import sys

UNK = ""  # text the guard cannot know: an unknown variable, a command's output
STAR, QMARK, LBRACK = "", "", ""  # unquoted glob characters
GLOBS = STAR + QMARK + LBRACK
MAX_DEPTH = 12
MAX_CANDIDATES = 16

# The hook has 5 s (.claude/settings.json); a hook that times out does not
# block, so the guard bounds its own work and fails closed beyond it.
MAX_INPUT = 256 * 1024  # bytes of hook input, checked before parsing
MAX_VARS = 256  # distinct variables tracked in one command; keeps every scope copy small
TIME_BUDGET = 3.0  # seconds of checking, then fail closed

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
            cmd = Compound("func", name=m.group(), body=self.parse_command())
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
                    w = cmd.words[0]
                    name = "".join(p for _, p, _ in w) if all(k == "lit" for k, _, _ in w) else None
                    return Compound("func", name=name, body=self.parse_command())
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
            op = re.fullmatch(r"([A-Za-z_][A-Za-z0-9_]*)[%#/:^,+?@].*", text, re.S)
            if op:  # ${HOME%/} and the like: NAME's value or something else
                return [("varop", op.group(1), in_dq)] + ([("nest", nested, in_dq)] if nested else [])
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
    text = re.sub(r"\[(?=.*\])", LBRACK, text)  # a `[` without a later `]` is literal (`[ -f x ]`)
    return text.replace("*", STAR).replace("?", QMARK)


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

PRIVILEGE = {"sudo", "sudoedit", "doas", "su", "pkexec", "run0", "runuser"}
PACKAGE = {"pacman", "yay", "paru", "makepkg", "pacstrap"}
SERVICE = {"systemctl", "loginctl", "reboot", "shutdown", "poweroff", "halt", "mkinitcpio", "systemd-run"}
SHELLS = {"bash", "sh", "zsh", "dash", "ksh", "mksh", "rbash", "ash", "yash", "fish"}

# `omarchy <route>` that changes the system: (first word, second words or None).
# Each word matches as a prefix (`omarchy updates`, `omarchy installed …`), as
# the grep rules before WP-130 did.
# `hook` runs the user's hooks, `branch` and `channel set` switch what Omarchy
# installs, `plugin enable|disable` changes the shell (WP-130 round 2).
OMARCHY_SYSTEM = [("pkg", ("add", "aur", "drop", "install", "remove")), ("update", None), ("install", None),
                  ("theme", ("set",)), ("plugin", ("add", "remove", "update", "clone", "enable", "disable")),
                  ("snapshot", None), ("migrate", None), ("refresh", None), ("hook", None), ("dev", ("link",)),
                  ("branch", None), ("channel", ("set",))]
OMARCHY_BIN_SYSTEM = ("pkg-add", "pkg-aur", "pkg-drop", "pkg-install", "pkg-remove", "update", "install",
                      "theme-set", "plugin-add", "plugin-remove", "plugin-update", "plugin-clone", "plugin-enable",
                      "plugin-disable", "snapshot", "migrate", "refresh", "hook", "dev-link", "branch",
                      "channel-set")
AGENT_VERBS = ("prompt", "launch", "start", "run", "chat", "ask")

LOGINCTL_READ = ("list-", "show-", "session-status", "user-status", "seat-status")
LOCAL_HOSTS = {"localhost", "localhost.localdomain", "ip6-localhost", "ip6-loopback", "::1", "0.0.0.0", "0",
               "0:0:0:0:0:0:0:1"}
TERMINALS = {"foot", "footclient", "alacritty", "ghostty", "kitty", "xterm", "wezterm", "konsole", "gnome-terminal",
             "kgx", "st", "urxvt", "xfce4-terminal", "terminator", "tilix"}
# git settings that make git run a program (`git -c`, `git clone -c`)
# git settings whose value names a program (`git -c`, `git clone -c`); they pass
# when the value is a harmless one (SAFE_PROGRAMS), except the always-unsafe ones
GIT_PROGRAM_KEY = re.compile(r"(core\.(pager|editor|sshcommand|fsmonitor|hookspath|askpass|gitproxy)"
                             r"|credential\..*|sequence\.editor|diff\.external|merge\.tool|uploadpack\..*hook"
                             r"|pager\..*|gpg\..*program"
                             r"|.*\.(cmd|command|driver|textconv|helper|program|clean|smudge|process))", re.I)
GIT_ALWAYS_UNSAFE_KEY = re.compile(r"(alias\..*|include.*)", re.I)  # a shell alias, another config file
GIT_PROTOCOL_KEY = re.compile(r"protocol\.(allow|ext\..*)", re.I)  # ext:: runs a program
GIT_PROGRAM_VARS = {"GIT_PAGER", "PAGER", "GIT_EDITOR", "EDITOR", "VISUAL", "GIT_SEQUENCE_EDITOR",
                    "GIT_SSH_COMMAND", "GIT_SSH", "GIT_ASKPASS", "SSH_ASKPASS", "GIT_EXTERNAL_DIFF",
                    "GIT_PROXY_COMMAND", "GIT_EXEC_PATH", "GIT_TEMPLATE_DIR"}
SAFE_PROGRAMS = {"", "cat", "less", "more", "true", "false", ":", "head", "tail", "/bin/true", "/usr/bin/true",
                 "/bin/cat", "/usr/bin/cat"}
# An unknown program's arguments that start with one of these run as that
# command (`strace -f sed -i … /etc/x`): checked like a wrapped command.
SUFFIX_COMMANDS = {"rm", "rmdir", "unlink", "shred", "tee", "truncate", "touch", "mkdir", "setfacl", "chmod", "chown",
                   "chgrp", "chattr", "cp", "mv", "ln", "install", "sed", "dd", "rsync", "scp", "patch", "tar",
                   "unzip", "curl", "wget", "git", "find", "xargs", "env", "nohup", "timeout", "nice", "exec",
                   "command", "flock", "ssh", "eval", "source", "."}
TMUX_KEY_RE = re.compile(r"[CMS]-.+|BSpace|BTab|DC|End|Home|IC|NPage|PageDown|PgDn|PPage|PageUp|PgUp|Up|Down|Left"
                         r"|Right|Escape|F\d{1,2}|KP.*|Any|Mouse.*|Wheel.*|Double.*|Triple.*")
GIT_DIR_WRITERS = {"fetch", "gc", "config", "tag", "branch", "update-ref", "notes", "prune", "repack", "remote",
                   "update-index", "init"}
GIT_MUTATING = {"checkout", "switch", "restore", "reset", "clean", "stash", "pull", "merge", "rebase", "apply", "am",
                "rm", "mv", "add", "commit", "cherry-pick", "revert", "worktree"}
# Programs whose arguments are data (or checked elsewhere): the net below
# does not fire for them.
DATA_SINKS = {"echo", "printf", "grep", "egrep", "fgrep", "rg", "cat", "bat", "less", "more", "head", "tail", "man",
              "info", "whatis", "apropos", "which", "type", "whereis", "command", "hash", "stat", "ls", "file", "diff",
              "cmp", "wc", "sort", "uniq", "cut", "tr", "jq", "yq", "git", "gh", "herdr", "tee", "awk", "sed", "nl",
              "column", "fold", "fmt", "paste", "join", "comm", "tac", "rev", "md5sum", "sha256sum", "sha1sum",
              "b2sum", "base64", "xxd", "od", "hexdump", "strings", "readlink", "realpath", "basename", "dirname",
              "test", "[", "true", "false", ":", "printenv", "pgrep", "ps", "id", "getent", "cargo", "rustc",
              "rustup", "just", "python3", "python", "node", "seldon", "qmllint", "journalctl", "systemd-analyze",
              "du", "df", "find", "locate", "fd", "tldr", "date", "sleep", "kill", "wait", "export", "declare",
              "local", "readonly", "unset", "set", "read", "mapfile", "readarray", "cd", "pushd", "popd", "mkdir",
              "touch", "rm", "rmdir", "cp", "mv", "ln", "chmod", "chown", "notify-send", "wl-copy", "xdg-open",
              "tar", "bsdtar", "zip", "unzip", "gzip", "gunzip", "xz", "zstd", "bzip2", "curl", "wget", "rsync",
              "scp", "patch", "pkill", "killall", "pidof", "truncate", "dd", "install", "shred", "unlink",
              "shellcheck", "shfmt"}

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


def loginctl_read_only(args):
    ops, _ = operands(args, "psnoHM", ("--property", "--signal", "--lines", "--output", "--host", "--machine",
                                      "--kill-whom"))
    return bool(ops) and plain(ops[0]).startswith(LOGINCTL_READ)


def join_path(base, p):
    return p if p.startswith(("/", UNK)) else base.rstrip("/") + "/" + p


def local_names():
    names = set(LOCAL_HOSTS)
    try:
        names.add(socket.gethostname().lower())
    except OSError:
        pass
    try:
        with open("/etc/hostname", encoding="utf-8") as f:
            names.add(f.read().strip().lower())
    except OSError:
        pass
    names.update({n.split(".", 1)[0] for n in list(names) if n and not n[0].isdigit() and ":" not in n})
    names.discard("")
    return names


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


def succeeded(scope):
    if ".cdfail" not in scope:
        return scope
    out = dict(scope)
    del out[".cdfail"]
    return out


def settle(scope):
    if ".cdfail" not in scope:
        return scope
    out = dict(scope)
    old = [v for v in out.pop(".cdfail") if v is not None]
    out[".cwd"] = dedupe(list(out.get(".cwd", [UNK])) + old)
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
    # The fixed git-ignored file. `GUARD_HOSTS_FILE` replaces it only for the
    # test table (`SELDON_TEST_GUARD` set): a settings `env` block must not
    # widen the guard.
    path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "guard-hosts.local")
    if os.environ.get("SELDON_TEST_GUARD") and os.environ.get("GUARD_HOSTS_FILE"):
        path = os.environ["GUARD_HOSTS_FILE"]
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
        self.functions = {}  # name → body, defined in this command
        self.namerefs = {}  # declare -n NAME=TARGET; None = unknown target
        self.links = {}  # tuple(link components) → target components (ln, ln -s in this command)

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
        # A `cd` that fails leaves the old directory: after `&&` it succeeded,
        # anywhere else the old one stays a candidate (`.cdfail`).
        merged, cur, all_and = None, scope, True
        for k, pl in enumerate(ao.pipelines):
            if k > 0 and ao.ops[k - 1] == "||":
                all_and = False
            entry = scope if k == 0 else (succeeded(cur) if all_and else settle(merged))
            cur = self.walk_pipeline(pl, entry, ctx)
            merged = cur if merged is None else merge([merged, cur])  # running merge: linear in the chain
        return settle(merged)

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
                self.set_var(body_scope, cmd.var, dedupe(values) if cmd.words else [UNK])
                self.bounded(body_scope)
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
            if cmd.name:
                self.functions[cmd.name] = cmd.body
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

    def bounded(self, scope):
        if sum(1 for k in scope if not k.startswith(".")) > MAX_VARS:
            raise Unsure(f"more than {MAX_VARS} variables in one command; split it")
        return scope

    def walk_simple(self, cmd, scope, ctx, piped):
        for w in cmd.assigns + cmd.words:
            self.walk_parts(w, scope, ctx)
        own = self.redirs(cmd.redirs, scope, ctx, cmd)
        stdin = own or (("pipe",) if piped else ctx.stdin)
        env = dict(scope)
        w0 = cmd.words[0] if cmd.words else None
        reader = w0 is not None and all(k == "lit" for k, _, _ in w0) and \
            "".join(p for _, p, _ in w0) in ("read", "mapfile", "readarray")
        for w in cmd.assigns:
            name, values = self.assignment(w, env, ctx)
            self.set_var(env, name, values, ifs_ok=reader)
        if not cmd.words:
            return self.bounded(env)
        after = None
        for argv in self.expand_words(cmd.words, scope, ctx):
            res = self.check_argv(argv, env, ctx, stdin, True, cmd)
            if res is not None:
                after = res
        return self.bounded(self.effects(cmd, scope, ctx, after))

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

    def set_var(self, scope, name, values, ifs_ok=False):
        for _ in range(8):
            if name not in self.namerefs:
                break
            name = self.namerefs[name]
            if name is None:
                return  # a nameref with an unknown target: its value stays unknown
        if name == "IFS" and not ifs_ok:
            raise Unsure("IFS changed; word splitting cannot be modelled")
        scope[name] = values

    # -- expansion
    def lookup(self, name, scope, ctx, _hops=0):
        if name in self.namerefs:
            target = self.namerefs[name]
            return [UNK] if target is None or _hops > 8 else self.lookup(target, scope, ctx, _hops + 1)
        if name == "OLDPWD" and name not in scope and not ctx.remote:
            return [os.environ.get("OLDPWD") or UNK]
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

    TILDE_RE = re.compile(r"([A-Za-z_][A-Za-z0-9_]*=)?~([A-Za-z0-9._-]*|[+-])(?=/|$)")

    def options(self, parts, scope, ctx, tilde):
        """Each part with its possible values."""
        parts = list(parts)
        if tilde and parts and parts[0][0] == "lit" and not parts[0][2]:
            m = self.TILDE_RE.match(parts[0][1])
            if m:  # `~/x`, and `name=~/x` as bash expands it in any word
                parts = ([("lit", m.group(1), True)] if m.group(1) else []) + [
                    ("tilde", m.group(2), False), ("lit", parts[0][1][m.end():], False)] + parts[1:]
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
                    opts.append(self.lookup("OLDPWD", scope, ctx))
                else:
                    opts.append(["/root" if payload == "root" else "/home/" + payload])
            elif kind == "var":
                opts.append(self.lookup(payload, scope, ctx))
            elif kind == "varop":  # ${HOME%/}: maybe the value itself
                opts.append(dedupe(self.lookup(payload, scope, ctx) + [UNK]))
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
        # eval may change variables; its values become candidates beside the old ones
        new = merge([scope, after]) if after is not None else dict(scope)
        w0 = cmd.words[0]
        if not all(k == "lit" for k, _, _ in w0):
            return new
        name = "".join(p for _, p, _ in w0)
        argvs = self.expand_words(cmd.words, scope, ctx)
        old = list(scope.get(".cwd", [UNK]))
        if name in ("cd", "pushd"):
            values = []
            for argv in argvs:
                args = [a for a in argv[1:] if a == "-" or not a.startswith("-")]
                if not args:
                    targets = self.lookup("HOME", scope, ctx)
                elif args[0] == "-":
                    targets = self.lookup("OLDPWD", scope, ctx)
                else:
                    targets = [args[0]]
                for t in targets:
                    for base in old:
                        values.append(join_path(base, t))
            new[".cwd"] = dedupe(values) or [UNK]
            new["OLDPWD"] = old
            new[".cdfail"] = old  # if the cd fails, the old directory stays
        elif name == "popd":
            new[".cwd"] = dedupe(old + [UNK])
            new["OLDPWD"] = old
        elif name in ("export", "declare", "typeset", "local", "readonly"):
            nameref = any(all(k == "lit" for k, _, _ in w) and "".join(p for _, p, _ in w)[:1] == "-"
                          and "n" in "".join(p for _, p, _ in w) for w in cmd.words[1:])
            for w in cmd.words[1:]:
                text = "".join(p for _, p, _ in w) if all(k == "lit" for k, _, _ in w) else None
                if is_assignment(w):
                    n, v = self.assignment(w, new, ctx)
                    if nameref:
                        target = v[0] if len(v) == 1 and NAME_RE.fullmatch(v[0]) else None
                        self.namerefs[n] = target
                    else:
                        self.set_var(new, n, v)
                elif nameref and text and NAME_RE.fullmatch(text):
                    self.namerefs[text] = None
        elif name in ("read", "mapfile", "readarray"):
            for argv in argvs:
                for a in argv[1:]:
                    if NAME_RE.fullmatch(a):
                        self.set_var(new, a, [UNK])
        elif name == "unset":
            for argv in argvs:
                for a in argv[1:]:
                    new.pop(a, None)
        elif name == "printf":
            for argv in argvs:
                if "-v" in argv[1:-1]:
                    self.set_var(new, argv[argv.index("-v") + 1], [UNK])
        return new

    # -- the command position
    def cmd_name(self, field):
        if wild(field):
            base = field.rsplit("/", 1)[-1]
            if "/" in field and base and not wild(base):
                return base
            raise Unsure("the command name is computed (a variable, $(…) or a glob); write it literally")
        return field.rsplit("/", 1)[-1]

    def check_argv(self, argv, scope, ctx, stdin, direct, node, links=True):
        """Check one command with its expanded arguments. Returns a scope when
        the command changes the shell's variables (eval, a function call)."""
        if not argv:
            return None
        name = self.cmd_name(argv[0])
        args = argv[1:]
        wrapped = lambda rest, sc=scope: self.check_argv(rest, sc, ctx, stdin, False, node)  # noqa: E731

        first = plain(argv[0])
        if "/" in first:
            if links:  # a link made earlier in this command: check what it points to as well
                for alt in self.paths(first, scope)[1:]:
                    self.check_argv(["/" + "/".join(alt)] + args, scope, ctx, stdin, False, node, links=False)
            if self.is_omarchy_path(first, scope, ctx) and not name.startswith("omarchy"):
                raise Blocked(f"runs an Omarchy script ({first})")
        elif name in self.functions:  # a function defined in this command runs here
            if ctx.depth >= MAX_DEPTH:
                raise Unsure("function calls nested too deep")
            return self.walk_command(self.functions[name], scope, ctx.but(depth=ctx.depth + 1, stdin=stdin), False)

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
            repl = None
            for k in range(i):
                a = plain(args[k])
                if a.startswith("-I"):
                    repl = a[2:] or (plain(args[k + 1]) if k + 1 < len(args) else "")
                elif a.startswith("--replace"):
                    repl = a.split("=", 1)[1] if "=" in a else "{}"
                elif a.startswith("-i") and not a.startswith("--"):
                    repl = a[2:] or "{}"
            rest = args[i:] or ["echo"]
            # the items read from stdin are unknown: in place of the replstr, else appended
            rest = [a.replace(repl, UNK) for a in rest] if repl else rest + [UNK]
            return wrapped(rest)
        if name == "watch":
            i = first_operand(args, "nq", ("--interval", "--equexit"))
            if any(plain(a) in ("-x", "--exec") for a in args[:i]):
                return wrapped(args[i:])
            if args[i:]:
                self.run_script(" ".join(args[i:]), dict(scope), ctx.but(stdin=stdin), "watch")
            return None
        if name == "script":
            ops, opts = operands(args, "cBEIOTm", ("--command", "--log-in", "--log-out", "--log-io",
                                                  "--log-timing", "--echo", "--logging-format", "--output-limit"))
            for opt, val in opts:
                if opt in ("-c", "--command"):
                    self.run_script(val, dict(scope), ctx.but(stdin=stdin), "script -c")
            for p in ops[:1]:
                self.check_write(p, scope, ctx, False, node, False)  # the typescript file
            if not any(opt in ("-c", "--command") for opt, _ in opts):
                self.stdin_script(stdin, dict(scope), ctx, "script")  # its shell reads stdin
            return None
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
            if direct and self.is_ssh_agent_eval(node):
                return None  # eval "$(ssh-agent -s)": the output is variable assignments
            return self.run_script(" ".join(args), scope, ctx.but(stdin=stdin), "eval")
        if name == "trap":
            targs = args[1:] if args and plain(args[0]) == "--" else args
            if len(targs) >= 2 and not plain(targs[0]).startswith("-"):
                self.run_script(targs[0], dict(scope), ctx.but(stdin=None), "trap")
            return None
        if name in ("source", "."):
            if args:
                self.sourced(plain(args[0]), scope, ctx, stdin, name)
            return None
        if name == "ssh":
            return self.w_ssh(args, scope, ctx, stdin, direct, node)
        if name == "hash" and any(plain(a).startswith("-") and "p" in plain(a) for a in args):
            raise Unsure("hash -p maps a command name to another program; the guard does not model it")
        if name in ("alias", "shopt"):
            if (name == "alias" and any("=" in a for a in args)) or \
                    (name == "shopt" and "expand_aliases" in [plain(a) for a in args] and "-u" not in args):
                raise Unsure("aliases change what a command name runs; the guard does not model them")
            return None
        handled, res = self.wrappers(name, args, scope, ctx, stdin, node)
        if handled:
            return res
        # rules
        if name in PACKAGE:
            if name == "pacman" and pacman_read_only(args):
                return None
            raise Blocked(f"privileged or package command: {name}")
        if name in SERVICE or name.startswith("grub-"):
            if name == "systemctl" and systemctl_read_only(args):
                return None
            if name == "loginctl" and loginctl_read_only(args):
                return None
            raise Blocked(f"service or boot command: {name}")
        if name == "omarchy":
            self.r_omarchy([plain(a) for a in args], ctx)
            return None
        if name.startswith("omarchy-"):
            self.r_omarchy_bin(name[len("omarchy-"):], ctx)
            return None
        if name == "git":
            self.r_git(args, scope, ctx, node)
        for path, recursive in self.write_targets(name, args):
            self.check_write(path, scope, ctx, recursive, node, direct)
        if name in ("ln", "cp"):  # after the check: making the link is not a write through it
            self.record_links(name, args, scope, ctx)
        if name not in DATA_SINKS:
            self.net(name, args, scope, ctx, node)
        return None

    def net(self, name, args, scope, ctx, node):
        """An unknown program may run its arguments (an exec wrapper the guard
        does not know). Fail closed when they name a red-zone command, a shell
        or an Omarchy script; check them as a command when they start with a
        known file command or wrapper."""
        for a in args:
            base = plain(a).rsplit("/", 1)[-1]
            red = base in PRIVILEGE or base in PACKAGE or base in SERVICE or base == "omarchy" or \
                base.startswith(("omarchy-", "grub-")) or base in SHELLS
            if red:
                raise Unsure(f"{name} is not known to the guard and its arguments name {base}")
            if self.is_omarchy_path(plain(a), scope, ctx):
                raise Unsure(f"{name} is not known to the guard and its arguments name an Omarchy script ({a})")
        for k, a in enumerate(args):
            if plain(a).rsplit("/", 1)[-1] in SUFFIX_COMMANDS:
                self.check_argv(args[k:], scope, ctx, None, False, node)

    def is_ssh_agent_eval(self, node):
        if node is None or len(node.words) != 2:
            return False
        parts = [p for p in node.words[1] if not (p[0] == "lit" and p[1] == "")]
        if len(parts) != 1 or parts[0][0] != "cmd":
            return False
        c = first_simple(parts[0][1]) if is_single(parts[0][1]) else None
        if c is None or c.assigns or c.redirs or not c.words:
            return False
        texts = []
        for w in c.words:
            if not all(k == "lit" for k, _, _ in w):
                return False
            texts.append("".join(p for _, p, _ in w))
        return texts[0].rsplit("/", 1)[-1] == "ssh-agent" and all(t in ("-s", "-c", "-k", "-D") for t in texts[1:])

    # -- exec wrappers beyond the shell's own (round 3)
    def wrappers(self, name, args, scope, ctx, stdin, node):
        """(handled, result) for programs that run another program."""
        wrapped = lambda rest: self.check_argv(rest, scope, ctx, stdin, False, node)  # noqa: E731
        script = lambda text, what: self.run_script(text, dict(scope), ctx.but(stdin=stdin), what)  # noqa: E731
        p = [plain(a) for a in args]
        if name == "taskset":
            if any(a in ("-p", "--pid") or (a.startswith("-") and not a.startswith("--") and "p" in a) for a in p):
                return True, None
            return True, wrapped(args[first_operand(args) + 1:])
        if name == "chrt":
            if any(a in ("-p", "--pid", "-m", "--max") for a in p):
                return True, None
            return True, wrapped(args[first_operand(args, "TPD", ("--sched-runtime", "--sched-period",
                                                                  "--sched-deadline")) + 1:])
        if name == "systemd-inhibit":
            if "--list" in p:
                return True, None
            return True, wrapped(args[first_operand(args, "", ("--what", "--who", "--why", "--mode")):])
        if name == "systemd-cat":
            return True, wrapped(args[first_operand(args, "tp", ("--identifier", "--priority", "--stderr-priority",
                                                                  "--level-prefix", "--namespace")):])
        if name == "ssh-agent":
            return True, wrapped(args[first_operand(args, "aEtPO"):])
        if name in ("dbus-run-session", "dbus-launch"):
            return True, wrapped(args[first_operand(args, "", ("--config-file", "--dbus-daemon")):])
        if name == "unbuffer":
            return True, wrapped(args[first_operand(args):])
        if name == "uwsm":
            if p[:1] in (["app"], ["start"]):
                rest = args[1:]
                if "--" in [plain(a) for a in rest]:
                    rest = rest[[plain(a) for a in rest].index("--") + 1:]
                else:
                    rest = rest[first_operand(rest, "saudtTSp"):]
                return True, wrapped(rest)
            return True, None
        if name == "gdb":
            if "--args" in p:
                return True, wrapped(args[p.index("--args") + 1:])
            if any(a in ("-ex", "--ex", "-iex", "-x", "-ix", "--command", "--init-command", "-batch", "--batch",
                         "--eval-command", "--init-eval-command") or a.startswith(("--eval-command=", "--command="))
                   for a in p):
                raise Unsure("gdb runs commands the guard cannot check")
            return True, None
        if name in ("bwrap", "parallel", "firejail", "unshare", "nsenter", "chroot"):
            if args:
                raise Unsure(f"{name} runs a command the guard does not model")
            return True, None
        if name == "socat":
            if any(a.upper().startswith(("EXEC:", "SYSTEM:")) or ",EXEC" in a.upper() for a in p):
                raise Unsure("socat runs a program (EXEC:/SYSTEM:)")
            return True, None
        if name == "sg":
            rest = args[1:]
            if rest and plain(rest[0]) == "-c":
                rest = rest[1:]
            if rest:
                script(" ".join(rest), "sg")
            return True, None
        if name == "hyprctl":
            return True, self.w_hyprctl(args, scope, ctx, stdin, node)
        if name == "tmux":
            return True, self.w_tmux(args, scope, ctx, stdin, node)
        if name in TERMINALS:
            for k, a in enumerate(p):
                if a in ("-e", "--command", "-x", "--"):
                    return True, wrapped(args[k + 1:])
            for k, a in enumerate(p):  # a trailing command: check every operand suffix
                if not a.startswith("-"):
                    wrapped(args[k:])
            return True, None
        if name in ("rsync", "scp", "sftp"):
            self.w_copy(name, args)
            return False, None  # the write targets follow
        if name in ("tar", "bsdtar"):
            for a in p:
                if a.startswith(("--use-compress-program", "--to-command", "--checkpoint-action", "--info-script",
                                 "--new-volume-script", "--rsh-command", "--rmt-command")) or \
                        (a.startswith("-") and not a.startswith("--") and ("I" in a or "F" in a)):
                    raise Unsure(f"tar option {a} runs a program")
            if p and not p[0].startswith("-") and ("I" in p[0] or "F" in p[0]):
                raise Unsure(f"tar option {p[0]} runs a program")
            return False, None
        if name == "rg":
            if any(a == "--pre" or a.startswith("--pre=") for a in p):
                raise Unsure("rg --pre runs a program")
            return False, None
        if name in ("fd", "fdfind"):
            for k, a in enumerate(p):
                if a in ("-x", "--exec", "-X", "--exec-batch"):
                    rest = args[k + 1:]
                    if ";" in [plain(r) for r in rest]:
                        rest = rest[:[plain(r) for r in rest].index(";")]
                    holders = ("{}", "{/}", "{//}", "{.}", "{/.}")
                    if any(h in plain(r) for r in rest for h in holders):
                        rest = [r.replace("{//}", UNK).replace("{/.}", UNK).replace("{/}", UNK).replace("{.}", UNK)
                                .replace("{}", UNK) for r in rest]
                    else:
                        rest = rest + [UNK]  # fd appends the path
                    return True, wrapped(rest)
            return False, None
        if name == "rustup":
            if p[:1] == ["run"]:
                rest = args[1:]
                j = first_operand(rest)
                return True, wrapped(rest[j + 1:])  # rustup run [--install] <toolchain> <command>…
            return False, None
        if name == "man":
            for k, a in enumerate(p):
                val = None
                if a == "-P" and k + 1 < len(p):
                    val = p[k + 1]
                elif a.startswith("-P") and len(a) > 2:
                    val = a[2:]
                elif a.startswith("--pager="):
                    val = a.split("=", 1)[1]
                elif a.startswith(("-H", "--html")):
                    raise Unsure("man -H runs a browser")
                if val is not None and val.strip() not in SAFE_PROGRAMS:
                    raise Unsure(f"man -P runs {val!r} as its pager")
            for var in ("MANPAGER", "PAGER", "BROWSER"):
                if var in scope and any(v.strip() not in SAFE_PROGRAMS for v in self.lookup(var, scope, ctx)):
                    raise Unsure(f"man with {var} set in the command runs that program")
            return False, None
        if name == "sort":
            if any(a.startswith("--compress-program") for a in p):
                raise Unsure("sort --compress-program runs a program")
            return False, None
        if name == "wget":
            for k, a in enumerate(p):
                if a.startswith("--use-askpass") or \
                        (a in ("-e", "--execute") and k + 1 < len(p) and "askpass" in p[k + 1].lower()) or \
                        (a.startswith(("--execute=", "-e")) and len(a) > 2 and "askpass" in a.lower()):
                    raise Unsure("wget --use-askpass runs a program")
            return False, None
        return False, None

    def w_hyprctl(self, args, scope, ctx, stdin, node):
        p = [plain(a) for a in args]
        i, batch = 0, False
        while i < len(p) and p[i].startswith("-"):
            if p[i] == "--batch":
                batch = True
                i += 1
                continue
            i += 2 if p[i] in ("-i", "--instance") else 1
        request = " ".join(p[i:])  # hyprctl joins its arguments into one request
        for piece in (request.split(";") if batch else [request]):
            words = piece.split()
            if words[:1] == ["dispatch"] and len(words) > 1 and words[1].startswith("exec"):
                text = piece.split(None, 2)[2] if len(words) > 2 else ""
                text = re.sub(r"^\s*\[[^\]]*\]\s*", "", text)  # window rules: [float] cmd
                if text.strip():
                    self.run_script(text, dict(scope), ctx.but(stdin=None), "hyprctl dispatch exec")
            elif words[:1] == ["keyword"] and len(words) > 1 and ("exec" in words[1] or words[1].startswith("bind")):
                raise Unsure("hyprctl keyword can make Hyprland run a command")
        return None

    TMUX_CMD_OPTS = {"new-session": "cefFnstxy", "new": "cefFnstxy", "new-window": "ceFnt", "neww": "ceFnt",
                     "split-window": "celtF", "splitw": "celtF", "respawn-pane": "cet", "respawnp": "cet",
                     "respawn-window": "cet", "respawnw": "cet", "display-popup": "bcdehsStTwxy",
                     "popup": "bcdehsStTwxy", "run-shell": "cdt", "run": "cdt", "pipe-pane": "t", "pipep": "t",
                     "if-shell": "t", "if": "t"}

    def w_tmux(self, args, scope, ctx, stdin, node):
        p = [plain(a) for a in args]
        i = 0
        while i < len(p) and p[i].startswith("-"):
            if p[i] == "-c" and i + 1 < len(p):
                self.run_script(p[i + 1], dict(scope), ctx.but(stdin=None), "tmux -c")
            i += 2 if p[i] in ("-c", "-f", "-L", "-S", "-T") else 1
        cmds, cur = [], []
        for a in p[i:]:
            if a == ";" or a.endswith("\\;"):
                cmds.append(cur)
                cur = []
            else:
                cur.append(a)
        cmds.append(cur)
        for c in cmds:
            if not c:
                continue
            verb, rest = c[0], c[1:]
            if verb in ("send-keys", "send"):
                j = first_operand(rest, "cNt")
                flags = "".join(o[1:] for o in rest[:j] if o.startswith("-") and not o.startswith("--"))
                if "H" in flags:
                    raise Unsure("tmux send-keys -H sends hex keys the guard cannot read")
                if "X" in flags:
                    continue  # copy-mode commands, nothing is typed
                typed = []  # tmux sends the keys one after another, without spaces
                for k in rest[j:]:
                    if "l" in flags:
                        typed.append(k)
                    elif k in ("Enter", "C-m", "C-j", "KPEnter"):
                        typed.append("\n")
                    elif k == "Space":
                        typed.append(" ")
                    elif k == "Tab":
                        typed.append("\t")
                    elif k in ("C-c", "C-u"):
                        typed = []  # the line is dropped
                    elif TMUX_KEY_RE.fullmatch(k):
                        raise Unsure(f"tmux key {k} changes the typed line in a way the guard cannot follow")
                    else:
                        typed.append(k)
                text = "".join(typed)
                if text.strip():
                    self.run_script(text, dict(scope), ctx.but(stdin=None), "tmux send-keys")
            elif verb in self.TMUX_CMD_OPTS:
                ops = rest[first_operand(rest, self.TMUX_CMD_OPTS[verb]):]
                if verb in ("if-shell", "if"):
                    ops = ops[:1]
                if ops:
                    self.run_script(" ".join(ops), dict(scope), ctx.but(stdin=None), f"tmux {verb}")
        return None

    def w_copy(self, name, args):
        """rsync -e and scp/sftp -S/-o name the transport that runs locally."""
        p = [plain(a) for a in args]
        for k, a in enumerate(p):
            val = None
            if name == "rsync" and a in ("-e", "--rsh") and k + 1 < len(p):
                val = p[k + 1]
            elif name == "rsync" and a.startswith("--rsh="):
                val = a.split("=", 1)[1]
            elif name == "rsync" and a.startswith("-e") and not a.startswith("--") and len(a) > 2:
                val = a[2:]
            elif name in ("scp", "sftp") and a == "-S" and k + 1 < len(p):
                val = p[k + 1]
            elif name == "sftp" and a == "-b":
                raise Unsure("sftp -b runs a batch file the guard cannot check")
            elif name in ("scp", "sftp") and a == "-o" and k + 1 < len(p):
                self.ssh_option(p[k + 1])
            elif name in ("scp", "sftp") and a.startswith("-o") and len(a) > 2:
                self.ssh_option(a[2:])
            if val is not None:
                words = val.split()
                if not words or words[0].rsplit("/", 1)[-1] != "ssh":
                    raise Unsure(f"{name} runs {val!r} as its transport")
                for j, w in enumerate(words):
                    if w == "-o" and j + 1 < len(words):
                        self.ssh_option(words[j + 1])
                    elif w.startswith("-o") and len(w) > 2:
                        self.ssh_option(w[2:])

    @staticmethod
    def ssh_option(val):
        key = re.split(r"[\s=]", val.strip(), maxsplit=1)[0].lower()
        if key in ("proxycommand", "localcommand", "knownhostscommand"):
            raise Blocked(f"ssh option {key} runs a local command")

    def r_git(self, args, scope, ctx, node):
        for k in scope:  # variables set in this command that name a program or config git reads
            if k == "GIT_CONFIG_NOSYSTEM":
                continue
            if k in ("GIT_CONFIG_GLOBAL", "GIT_CONFIG_SYSTEM", "GIT_CONFIG"):
                if any(v != "/dev/null" for v in self.lookup(k, scope, ctx)):
                    raise Unsure(f"git with {k} set in the command reads another config file")
            elif k.startswith("GIT_CONFIG"):
                raise Unsure(f"git with {k} set in the command can run a program")
            elif k in GIT_PROGRAM_VARS and any(v.strip() not in SAFE_PROGRAMS for v in self.lookup(k, scope, ctx)):
                raise Unsure(f"git with {k} set in the command can run a program")
        p = [plain(a) for a in args]
        i, base, work_tree, git_dir = 0, ".", None, None
        while i < len(p):
            a = p[i]
            if a == "-C" and i + 1 < len(p):
                base = join_path(base, args[i + 1])
                i += 2
            elif a == "-c" and i + 1 < len(p):
                self.git_config(p[i + 1])
                i += 2
            elif a.startswith("--config-env"):
                raise Unsure("git --config-env can make git run a program")
            elif a.startswith("--exec-path="):
                raise Unsure("git --exec-path changes which programs git runs")
            elif a == "--work-tree" and i + 1 < len(p):
                work_tree = args[i + 1]
                i += 2
            elif a.startswith("--work-tree="):
                work_tree = args[i].split("=", 1)[1]
                i += 1
            elif a == "--git-dir" and i + 1 < len(p):
                git_dir = args[i + 1]
                i += 2
            elif a.startswith("--git-dir="):
                git_dir = args[i].split("=", 1)[1]
                i += 1
            elif a in ("--namespace", "--super-prefix") and i + 1 < len(p):
                i += 2
            elif a.startswith("-"):
                i += 1
            else:
                break
        if i >= len(p):
            return
        verb, rest, prest = p[i], args[i + 1:], p[i + 1:]
        for k, a in enumerate(prest):
            if a.startswith("ext::") or a in ("--upload-pack", "-u", "--receive-pack", "--exec") or \
                    a.startswith(("--upload-pack=", "--receive-pack=", "--exec=", "--template")):
                if not (verb not in ("clone", "fetch", "pull", "ls-remote", "push", "archive", "init", "submodule")
                        and a == "-u"):
                    raise Unsure(f"git {verb} {a} can run a program")
            if a in ("-c", "--config") and k + 1 < len(prest) and verb in ("clone", "submodule"):
                self.git_config(prest[k + 1])
            if a.startswith("--config=") and verb == "clone":
                self.git_config(a.split("=", 1)[1])
        # the work tree and the repository: options first, then GIT_WORK_TREE / GIT_DIR
        wts = [join_path(base, work_tree)] if work_tree else \
            [join_path(base, v) for v in self.lookup("GIT_WORK_TREE", scope, ctx) if not v.startswith(UNK)] or [base]
        dirs = [join_path(base, git_dir)] if git_dir else \
            [join_path(base, v) for v in self.lookup("GIT_DIR", scope, ctx) if not v.startswith(UNK)]
        if verb in GIT_MUTATING or verb in GIT_DIR_WRITERS:
            for d in dirs:
                self.check_write(d.rstrip("/") + "/" + UNK, scope, ctx, False, node, False)
        if verb == "clone":
            ops, opts = operands(rest, "bocj", ("--branch", "--origin", "--upload-pack", "--reference",
                                                "--reference-if-able", "--separate-git-dir", "--depth",
                                                "--shallow-since", "--shallow-exclude", "--template", "--config",
                                                "--jobs", "--filter", "--server-option", "--bundle-uri"))
            if ops:
                dest = ops[1] if len(ops) > 1 else re.sub(r"\.git$", "", plain(ops[0]).rstrip("/").rsplit("/", 1)[-1]
                                                           .rsplit(":", 1)[-1])
                self.check_write(join_path(base, dest), scope, ctx, False, node, False)
            for o, v in opts:
                if o == "--separate-git-dir":
                    self.check_write(join_path(base, v), scope, ctx, False, node, False)
        elif verb == "init":
            ops, opts = operands(rest, "b", ("--template", "--separate-git-dir", "--initial-branch",
                                             "--object-format", "--ref-format"))
            self.check_write(join_path(base, ops[0]) if ops else base + "/" + UNK, scope, ctx, False, node, False)
            for o, v in opts:
                if o in ("--separate-git-dir",):
                    self.check_write(join_path(base, v), scope, ctx, False, node, False)
        elif verb == "worktree" and prest[:1] == ["add"]:
            ops, _ = operands(rest[1:], "bB", ("--reason",))
            if ops:
                self.check_write(join_path(base, ops[0]), scope, ctx, False, node, False)
        elif verb in GIT_MUTATING:
            for wt in wts:
                self.check_write(wt.rstrip("/") + "/" + UNK, scope, ctx, False, node, False)

    @staticmethod
    def git_config(kv):
        key, _, val = kv.partition("=")
        val = val.strip()
        if GIT_ALWAYS_UNSAFE_KEY.fullmatch(key) or (GIT_PROTOCOL_KEY.fullmatch(key) and val != "never") or \
                (GIT_PROGRAM_KEY.fullmatch(key) and val not in SAFE_PROGRAMS):
            raise Unsure(f"git -c {key} can make git run a program")

    # -- symlinks and hard links made in this command
    def record_links(self, name, args, scope, ctx):
        if name == "cp" and not any(plain(a) in ("-s", "--symbolic-link") or
                                    (plain(a).startswith("-") and not plain(a).startswith("--") and "s" in plain(a))
                                    for a in args):
            return
        ops, opts = operands(args, "tS", ("--target-directory", "--suffix"))
        tdir = [v for o, v in opts if o in ("-t", "--target-directory")]
        rel = any(o in ("-r", "--relative") for o, _ in opts)
        pairs = []
        if tdir:
            pairs = [(t, tdir[0].rstrip("/") + "/" + plain(t).rstrip("/").rsplit("/", 1)[-1]) for t in ops]
        elif len(ops) == 1:
            pairs = [(ops[0], plain(ops[0]).rstrip("/").rsplit("/", 1)[-1])]
        elif len(ops) >= 2:
            dest = ops[-1]
            for t in ops[:-1]:
                pairs.append((t, dest.rstrip("/") + "/" + plain(t).rstrip("/").rsplit("/", 1)[-1]))
            if len(ops) == 2:
                pairs.append((ops[0], dest))
        for target, link in pairs:
            for lc in self.absolute(link, scope):
                if target.startswith(UNK):
                    tcands = [[UNK]]
                elif target.startswith("/"):
                    tcands = [normalize(target)]
                elif rel:
                    tcands = self.absolute(target, scope)
                else:  # a relative target is relative to the link's directory
                    tcands = [normalize("/" + "/".join(lc[:-1]) + "/" + target)]
                for tc in tcands:
                    if any(wild(c) for c in lc):
                        if self.protected_like(tc):
                            raise Unsure("a link to a protected directory at a path the guard cannot know")
                        continue
                    if len(self.links) >= 64:
                        raise Unsure("too many links made in one command")
                    self.links[tuple(lc)] = tc

    def protected_like(self, comps):
        roots = SYSTEM_DIRS + [self.home_comps + [".config"], ["home", "*", ".config"], ["root", ".config"],
                               self.home_comps + ["Seldon"], self.home_comps + [".local", "state", "seldon"]]
        return any(could_be_inside(comps, r) or could_be_ancestor(comps, r) for r in roots)

    def absolute(self, path, scope):
        """A path → normalized components for each working-directory candidate."""
        if not path or path.startswith(UNK):
            return []
        if path.startswith("/"):
            return [normalize(path)]
        return [normalize(c.rstrip("/") + "/" + path) for c in scope.get(".cwd", [UNK]) if not c.startswith(UNK)]

    def via_links(self, comps):
        out, frontier = [comps], [comps]
        for _ in range(8):
            new = []
            for c in frontier:
                for link, target in self.links.items():
                    if len(c) >= len(link) and all(comp_match(c[k], link[k]) for k in range(len(link))):
                        r = list(target) + c[len(link):]
                        if r not in out:
                            out.append(r)
                            new.append(r)
            frontier = new
            if not new or len(out) > 64:
                break
        return out[:64]

    def paths(self, path, scope):
        """All component lists a path may name: itself, then through links."""
        out = []
        for comps in self.absolute(path, scope):
            for c in self.via_links(comps):
                if c not in out:
                    out.append(c)
        return out

    def is_omarchy_path(self, path, scope, ctx):
        roots = [["usr", "share", "omarchy"]]
        for v in self.lookup("OMARCHY_PATH", scope, ctx):
            if v.startswith("/"):
                roots.append(normalize(v))
        return any(could_be_inside(c, r) for c in self.paths(path, scope) for r in roots)

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
            self.set_var(env, n.rstrip("+"), [v])
            i += 1
        if i >= len(args):
            return None
        return self.check_argv(args[i:], env, ctx, stdin, False, node)

    def w_flock(self, args, scope, ctx, stdin, node):
        for k, a in enumerate(args[:-1]):
            if plain(a) in ("-c", "--command"):
                self.run_script(args[k + 1], dict(scope), ctx.but(stdin=stdin), "flock -c")
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
            return None  # checked above
        return self.check_argv(rest, scope, ctx, stdin, False, node)

    def w_shell(self, args, scope, ctx, stdin, name):
        i, cmode, smode, noexec = 0, False, False, False
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
                    elif ch == "n" and a[0] == "-":
                        noexec = True  # bash -n: syntax check, nothing runs
                    elif ch in "oO":
                        take += 1
                i += 1 + take
                continue
            break
        rest = args[i:]
        if noexec and not cmode:
            return None
        if cmode:
            if not rest:
                raise Unsure(f"{name} -c without a command string")
            self.run_script(rest[0], dict(scope), ctx.but(stdin=stdin), f"{name} -c")
            return None
        if rest and not smode:
            self.sourced(plain(rest[0]), scope, ctx, stdin, name)
            return None
        self.stdin_script(stdin, dict(scope), ctx, name)
        return None

    def sourced(self, path, scope, ctx, stdin, name):
        """A script file run by a shell or `source`: not inspected, unless it is
        stdin or generated."""
        if self.is_omarchy_path(path, scope, ctx):
            raise Blocked(f"runs an Omarchy script ({path})")
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
                        self.ssh_option(val)
                    break
            i += 1
        if i >= len(args):
            return None
        host, rest = plain(args[i]), args[i + 1:]
        if self.host_gate and not ctx.remote and host in self.hosts:
            return None  # a test host the operator released (scripts/guard-hosts.local)
        first = direct and node is ctx.top_first and ctx.depth == 0 and not ctx.remote
        rctx = ctx.but(remote=True, stdin=stdin, ssh_first=first, ssh_single=first and ctx.top_single)
        h = host.rsplit("@", 1)[-1].strip("[]").lower()
        if wild(h) or h in local_names() or h.startswith("127."):
            # this machine (or a host the guard cannot know): the local rules, no remote exceptions
            rctx = ctx.but(remote=False, stdin=stdin, ssh_first=False, ssh_single=False, r_first=None,
                           r_single=False)
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
            ops, opts = operands(args, "ns" if name == "shred" else "")
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
        if name == "patch":
            ops, opts = operands(args, "dioprDFBVYzg", ("--directory", "--input", "--output", "--strip",
                                                       "--reject-file", "--ifdef", "--fuzz", "--prefix",
                                                       "--version-control", "--basename-prefix", "--suffix",
                                                       "--get", "--quoting-style"))
            d = next((v for o, v in reversed(opts) if o in ("-d", "--directory")), ".")
            out = [(join_path(d, v), False) for o, v in opts if o in ("-o", "--output", "-r", "--reject-file")
                   and plain(v) != "-"]
            out.append((join_path(d, ops[0]), False) if ops else (d.rstrip("/") + "/" + UNK, False))
            return out
        if name in ("tar", "bsdtar"):
            return self.tar_targets(args)
        if name == "unzip":
            ops, opts = operands(args, "dxP", ())
            if any(o in ("-l", "-t", "-p", "-v", "-Z", "-z") for o, _ in opts):
                return []
            d = next((v for o, v in opts if o == "-d"), ".")
            return [(d.rstrip("/") + "/" + UNK, False)]
        if name == "curl":
            ops, opts = operands(args, "AbcCdDeEFHKmoPrTuUwxXyYzQt",
                                 ("--output", "--output-dir", "--dump-header", "--cookie-jar", "--trace",
                                  "--trace-ascii", "--stderr", "--etag-save", "--data", "--header", "--url",
                                  "--user-agent", "--config", "--user", "--request", "--form", "--cookie"))
            d = next((v for o, v in opts if o == "--output-dir"), ".")
            out = []
            for o, v in opts:
                if o in ("-o", "--output") and plain(v) != "-":
                    out.append((join_path(d, v), False))
                elif o in ("-O", "--remote-name", "--remote-name-all", "-J"):
                    out.append((d.rstrip("/") + "/" + UNK, False))
                elif o in ("-D", "--dump-header", "-c", "--cookie-jar", "--trace", "--trace-ascii", "--stderr",
                           "--etag-save") and v is not None and plain(v) != "-":
                    out.append((v, False))
            return out
        if name == "wget":
            ops, opts = operands(args, "OoaPeiBtTwlQUDARIX", ("--output-document", "--directory-prefix",
                                                               "--output-file", "--append-output"))
            d = next((v for o, v in opts if o in ("-P", "--directory-prefix")), ".")
            out = [(v, False) for o, v in opts if o in ("-o", "--output-file", "-a", "--append-output")]
            docs = [v for o, v in opts if o in ("-O", "--output-document")]
            if docs:
                out += [(join_path(d, v), False) for v in docs if plain(v) != "-"]
            elif ops:
                out.append((d.rstrip("/") + "/" + UNK, False))
            return out
        if name in ("rsync", "scp"):
            ops = [a for a in args if not a.startswith("-")]
            dest = ops[-1] if len(ops) >= 2 else None
            if dest and not re.match(r"[^/]*:", dest) and not dest.startswith("rsync://"):
                return [(dest, False)]
        return []

    @staticmethod
    def tar_targets(args):
        p = [plain(a) for a in args]
        old_style = bool(p) and not p[0].startswith("-")
        letters = p[0] if old_style else ""
        extract = "x" in letters or any(a in ("--extract", "--get") or
                                        (a.startswith("-") and not a.startswith("--") and "x" in a) for a in p)
        create = any(c in letters for c in "cruA") or any(
            a in ("--create", "--append", "--update", "--catenate", "--concatenate") or
            (a.startswith("-") and not a.startswith("--") and any(c in a for c in "cruA")) for a in p)
        dirs, files = [], []
        if old_style:  # `tar xzfC a.tgz dir`: arg-taking letters consume the next words in order
            k = 1
            for c in letters:
                if c in "fbCTXgKLNV" and k < len(args):
                    (files if c == "f" else dirs if c == "C" else []).append(args[k])
                    k += 1
        for k, a in enumerate(p):
            if a in ("-C", "--directory") and k + 1 < len(p):
                dirs.append(args[k + 1])
            elif a.startswith("--directory="):
                dirs.append(args[k].split("=", 1)[1])
            elif a.startswith("--one-top-level="):
                dirs.append(args[k].split("=", 1)[1])
            elif a in ("-f", "--file") and k + 1 < len(p):
                files.append(args[k + 1])
            elif a.startswith("--file="):
                files.append(args[k].split("=", 1)[1])
            elif a.startswith("-") and not a.startswith("--") and len(a) > 2:
                for j, c in enumerate(a[1:], 1):
                    if c in "Cf":
                        val = a[j + 1:] or (args[k + 1] if k + 1 < len(args) else "")
                        (dirs if c == "C" else files).append(val)
                        break
        out = []
        if extract:
            out += [(d.rstrip("/") + "/" + UNK, False) for d in (dirs or ["."])]
        if create:
            out += [(f, False) for f in files if plain(f) != "-"]
        return out

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
        for comps in [c for full in fulls for c in self.via_links(normalize(full))]:
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
    raw = sys.stdin.buffer.read(MAX_INPUT + 1)
    if len(raw) > MAX_INPUT:
        print(f"guard: blocked ({CLOSED}): the hook input is over {MAX_INPUT // 1024} KB, too large to check "
              "within the hook timeout; write big files with the Write/Edit tools or in parts", file=sys.stderr)
        return 2
    try:
        data = json.loads(raw.decode("utf-8"))
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
    budget = TIME_BUDGET
    if os.environ.get("SELDON_TEST_GUARD") and os.environ.get("GUARD_TIME_BUDGET"):
        budget = min(budget, float(os.environ["GUARD_TIME_BUDGET"]))  # the test table, shorter only

    def out_of_time(signum, frame):
        raise Unsure(f"checking took longer than {budget:g} s; split the command or write big files "
                     "with the Write/Edit tools")

    signal.signal(signal.SIGALRM, out_of_time)
    signal.setitimer(signal.ITIMER_REAL, budget)
    try:
        Guard(command, cwd).run()
        signal.setitimer(signal.ITIMER_REAL, 0)
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
