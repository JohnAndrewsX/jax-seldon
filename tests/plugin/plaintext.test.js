// Plain text over the whole plugin (E72): every `Text {` block in
// plugin/**/*.qml declares `textFormat: Text.PlainText` itself, so no
// logbook or index text is ever read as rich text (a `<a href>` or an
// `<img>` in an event's detail stays characters). The default, AutoText,
// turns a string that looks like HTML into markup. A block may only differ
// with a line in tests/plugin/plaintext-allow.txt that names its file, its
// `id` and the reason. Every `textFormat:` in the plugin is PlainText, so a
// component built on Text (`component Line: Text`) cannot be switched back
// at a use site either.
//
// Known limit (review 2, N1): a contrived double fault still passes, a
// regex literal with a quote plus a "/*" … "*/" string pair around a bare
// `Text { text: "x" }`. The scan loses the block to the quote, the plain
// count loses it to the comment pair, so the counts agree. Either fault
// alone fails.
//
// The mutants at the end prove the check: each real block with its
// textFormat line removed must fail, and so must the hand-made shapes below
// (fixture-style, no Qt). Run: node tests/plugin/plaintext.test.js
"use strict"

const fs = require("fs")
const path = require("path")

const root = path.resolve(__dirname, "../..")
const allowFile = path.join(root, "tests/plugin/plaintext-allow.txt")

let failures = 0
function fail(msg) {
  console.error("FAIL " + msg)
  failures++
}

// The QML text with comments and string literals blanked to spaces (newlines
// kept), so braces and words inside them neither open blocks nor count as a
// declaration, and offsets still map to lines.
function blank(src) {
  let out = ""
  let i = 0
  while (i < src.length) {
    const c = src[i]
    const n = src[i + 1]
    if (c === "/" && n === "/") {
      while (i < src.length && src[i] !== "\n") { out += " "; i++ }
    } else if (c === "/" && n === "*") {
      const end = src.indexOf("*/", i + 2)
      const stop = end < 0 ? src.length : end + 2
      for (; i < stop; i++) out += src[i] === "\n" ? "\n" : " "
    } else if (c === '"' || c === "'" || c === "`") {
      out += " "
      i++
      while (i < src.length && src[i] !== c) {
        if (src[i] === "\\") { out += " "; i++ }
        if (i < src.length) { out += src[i] === "\n" ? "\n" : " "; i++ }
      }
      if (i < src.length) { out += " "; i++ }
    } else {
      out += c
      i++
    }
  }
  return out
}

// The offset of the `}` that closes the `{` at `open`, or -1.
function closing(text, open) {
  let depth = 0
  for (let i = open; i < text.length; i++) {
    if (text[i] === "{") depth++
    else if (text[i] === "}" && --depth === 0) return i
  }
  return -1
}

// A block's own level: its body with every nested `{…}` removed.
function ownLevel(body) {
  let out = ""
  let depth = 0
  for (const c of body) {
    if (c === "{") depth++
    else if (c === "}") depth--
    else if (depth === 0) out += c
  }
  return out
}

const TEXT_BLOCK = /(^|[^A-Za-z0-9_.])((?:[A-Za-z_][A-Za-z0-9_]*\.)?Text)\s*\{/g
const PLAIN = /(^|[\s;])textFormat\s*:\s*Text\.PlainText\s*($|[\s;])/
const ANY_FORMAT = /(^|[^A-Za-z0-9_.])textFormat\s*:\s*([A-Za-z0-9_.]*)/g

// Every Text block of one file: { line, id, plain }.
function textBlocks(src) {
  const text = blank(src)
  const blocks = []
  let m
  TEXT_BLOCK.lastIndex = 0
  while ((m = TEXT_BLOCK.exec(text)) !== null) {
    const open = m.index + m[0].length - 1
    const close = closing(text, open)
    const own = ownLevel(close < 0 ? text.slice(open + 1) : text.slice(open + 1, close))
    const id = /(^|[\s;])id\s*:\s*([A-Za-z_][A-Za-z0-9_]*)/.exec(own)
    blocks.push({
      line: text.slice(0, open).split("\n").length,
      id: id ? id[2] : "",
      plain: PLAIN.test(own)
    })
  }
  return blocks
}

// Every `textFormat:` value of one file that is not Text.PlainText.
function otherFormats(src) {
  const text = blank(src)
  const found = []
  let m
  ANY_FORMAT.lastIndex = 0
  while ((m = ANY_FORMAT.exec(text)) !== null) {
    if (m[2] !== "Text.PlainText")
      found.push({ line: text.slice(0, m.index + m[1].length).split("\n").length, value: m[2] || "(an expression)" })
  }
  return found
}

// The allow-list: `<file>#<id> <reason>` per line; `#` at the start of a
// line is a comment. A reason is required, an id too (an exception names
// its block).
function readAllow(text, report = fail) {
  const entries = []
  text.split("\n").forEach((raw, i) => {
    const line = raw.trim()
    if (line === "" || line.startsWith("#")) return
    const m = /^(\S+\.qml)#([A-Za-z_][A-Za-z0-9_]*)\s+(\S.*)$/.exec(line)
    if (!m) {
      report("plaintext-allow.txt:" + (i + 1) + ": not '<file>#<id> <reason>': " + line)
      return
    }
    entries.push({ file: m[1], id: m[2], reason: m[3], used: false })
  })
  return entries
}

// The source without comments, by a plain scan that knows no strings
// (a `//` counts only at the line start or after a blank, so a URL in a
// string stays): an independent second reading of the file.
function withoutComments(src) {
  return src.replace(/\/\*[\s\S]*?\*\//g, " ").replace(/(^|\s)\/\/.*$/gm, "$1")
}

// `Text {` counted the plain way. blank() also skips strings; a quote it
// misreads (a JS regex literal such as /"/) would hide every Text block
// after it, so the two counts must agree.
function naiveTextCount(src) {
  return (withoutComments(src).match(/\bText\s*\{/g) || []).length
}

// Ways to set textFormat that the declarative check cannot follow: an
// assignment (`t.textFormat = …`, `textFormat = …` in a handler), a
// `Binding { property: "textFormat" }` and `property alias f:
// t.textFormat`. None is supported; each fails with its line.
var IMPERATIVE = [
  { re: /(^|[^=!<>])\btextFormat\s*=(?!=)/, what: "an assignment to textFormat" },
  { re: /["']textFormat["']/, what: "textFormat named as a string (a Binding's property)" },
  { re: /\.textFormat\b/, what: "textFormat reached through an object (an alias or a script)" }
]

// The failures of one file's source against the allow-list.
function check(file, src, allow) {
  const problems = []
  const naive = naiveTextCount(src)
  const scanned = textBlocks(src).length
  if (naive !== scanned)
    problems.push(file + ": " + naive + " `Text {` in the source but " + scanned
      + " found by the scan; something (a quote in a regex literal?) hides blocks from it")
  withoutComments(src).split("\n").forEach((line, i) => {
    for (const imp of IMPERATIVE)
      if (imp.re.test(line)) problems.push(file + ":" + (i + 1) + ": " + imp.what + " is not supported (E72)")
  })
  for (const b of textBlocks(src)) {
    if (b.plain) continue
    const entry = b.id !== "" && allow.find((a) => a.file === file && a.id === b.id)
    if (entry) {
      entry.used = true
      continue
    }
    problems.push(file + ":" + b.line + ": a Text block" + (b.id ? " (id " + b.id + ")" : "")
      + " without `textFormat: Text.PlainText` (E72; an exception needs a line in tests/plugin/plaintext-allow.txt)")
  }
  for (const f of otherFormats(src)) {
    const listed = allow.some((a) => a.file === file)
    if (!listed) problems.push(file + ":" + f.line + ": textFormat is " + f.value + ", only Text.PlainText (E72)")
  }
  return problems
}

function qmlFiles(dir) {
  const out = []
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name)
    if (e.isDirectory()) out.push(...qmlFiles(p))
    else if (e.isFile() && e.name.endsWith(".qml")) out.push(p)
  }
  return out.sort()
}

// 1. The plugin tree.
const allow = readAllow(fs.existsSync(allowFile) ? fs.readFileSync(allowFile, "utf8") : "")
const files = qmlFiles(path.join(root, "plugin")).map((p) => ({
  file: path.relative(root, p),
  src: fs.readFileSync(p, "utf8")
}))
let blocks = 0
for (const f of files) {
  blocks += textBlocks(f.src).length
  for (const p of check(f.file, f.src, allow)) fail(p)
}
for (const a of allow) if (!a.used) fail("plaintext-allow.txt: " + a.file + "#" + a.id + " matches no Text block that needs it; remove the line")
if (blocks === 0) fail("no Text block found under plugin/; the pattern is broken")

// 2. Mutants of the real tree: each Text block with its own textFormat
// declaration removed must fail, at its line.
const DECL = /textFormat\s*:\s*Text\.PlainText/
let mutants = 0
for (const f of files) {
  const lines = f.src.split("\n")
  for (const b of textBlocks(f.src)) {
    if (!b.plain) continue
    // the block's own declaration: the first line from the block's opening
    // whose removal leaves this block without one
    let copy = null
    for (let i = b.line - 1; i < lines.length && copy === null; i++) {
      if (!DECL.test(lines[i])) continue
      const tried = lines.slice()
      tried[i] = tried[i].replace(DECL, "")
      const target = textBlocks(tried.join("\n")).find((x) => x.line === b.line)
      if (target && !target.plain) copy = tried
    }
    if (copy === null) {
      fail(f.file + ":" + b.line + ": mutant: no textFormat line of this block could be removed")
      continue
    }
    const problems = check(f.file, copy.join("\n"), [])
    if (!problems.some((p) => p.startsWith(f.file + ":" + b.line + ": a Text block")))
      fail(f.file + ":" + b.line + ": mutant survived: the block without its textFormat passed")
    mutants++
  }
}
if (mutants !== blocks - allow.filter((a) => a.used).length) fail("mutants: " + mutants + " of " + blocks + " blocks mutated")

// The regex-quote mutant (review 1, F5): a function with /"/ before the
// first Text block of every real file that has one, and a Text block
// without textFormat after it. The scan misreads the quote and loses the
// blocks after it; the count check must say so.
let regexMutants = 0
for (const f of files) {
  const first = textBlocks(f.src)[0]
  if (!first) continue
  const lines = f.src.split("\n")
  lines.splice(first.line - 1, 0, "  function seldonMutant(s) { return s.replace(/\"/g, \"\") }",
    "  Text { text: \"mutant\" }")
  const problems = check(f.file, lines.join("\n"), [])
  if (!problems.some((p) => p.includes("found by the scan")))
    fail(f.file + ": the regex-quote mutant survived: " + problems.join("; "))
  regexMutants++
}
if (regexMutants === 0) fail("no file for the regex-quote mutant")

// 3. Hand-made shapes: each `bad` must fail, each `good` must pass.
const shapes = {
  bad: {
    "no textFormat": "Item {\n  Text {\n    text: \"a\"\n  }\n}\n",
    "rich text": "Text {\n  textFormat: Text.RichText\n}\n",
    "auto text": "Text { textFormat: Text.AutoText }\n",
    "only in a child": "Text {\n  text: \"a\"\n  Item {\n    property int textFormat: Text.PlainText\n  }\n  MouseArea { textFormat: Text.PlainText }\n}\n",
    "only in a comment": "Text {\n  // textFormat: Text.PlainText\n  text: \"a\"\n}\n",
    "only in a block comment": "Text {\n  /* textFormat: Text.PlainText */\n  text: \"a\"\n}\n",
    "only in a string": "Text {\n  text: \"textFormat: Text.PlainText\"\n}\n",
    "only in a binding block": "Text {\n  text: {\n    textFormat: Text.PlainText\n    return \"a\"\n  }\n}\n",
    "an inline component": "Item {\n  component Line: Text {\n    wrapMode: Text.Wrap\n  }\n}\n",
    "a contentItem": "PanelToolTip {\n  contentItem: Text {\n    text: tooltip.text\n  }\n}\n",
    "a qualified type": "QQ.Text {\n  text: \"a\"\n}\n",
    "a one-liner": "Text { text: \"a\" }\n",
    "after a regex with a quote": "Item {\n  function f(s) { return s.replace(/\"/g, \"\") }\n  Text { text: \"a\" }\n}\n",
    "after a regex with a quote, declared": "Item {\n  function f(s) { return s.replace(/\"/g, \"\") }\n  Text { textFormat: Text.PlainText }\n}\n",
    "an assignment": "Text {\n  id: t\n  textFormat: Text.PlainText\n  Component.onCompleted: t.textFormat = Text.RichText\n}\n",
    "a bare assignment": "Text {\n  textFormat: Text.PlainText\n  onTextChanged: textFormat = Text.RichText\n}\n",
    "a Binding": "Item {\n  Text { id: t; textFormat: Text.PlainText }\n  Binding { target: t; property: \"textFormat\"; value: Text.RichText }\n}\n",
    "an alias": "Item {\n  property alias format: t.textFormat\n  Text { id: t; textFormat: Text.PlainText }\n}\n",
    "rich text at a use site": "Item {\n  component Line: Text { textFormat: Text.PlainText }\n  Line { textFormat: Text.StyledText }\n}\n",
    "an expression": "Text {\n  textFormat: Text.PlainText\n}\nLine { textFormat: root.format }\n",
    "the second of two": "Column {\n  Text { textFormat: Text.PlainText }\n  Text { text: \"b\" }\n}\n"
  },
  good: {
    "plain": "Text {\n  textFormat: Text.PlainText\n  text: \"<b>a</b>\"\n}\n",
    "a one-liner": "Text { textFormat: Text.PlainText; text: \"a\" }\n",
    "after a child": "Text {\n  Item { width: 1 }\n  textFormat: Text.PlainText\n}\n",
    "a brace in a string": "Text {\n  text: \"{\"\n  textFormat: Text.PlainText\n}\n",
    "a brace in a comment": "Text {\n  // }\n  textFormat: Text.PlainText\n}\n",
    "a comparison and a comment": "Text {\n  textFormat: Text.PlainText\n  // t.textFormat = Text.RichText\n  visible: textFormat == Text.PlainText\n}\n",
    "a URL in a string": "Text { textFormat: Text.PlainText; text: \"https://x.org\" }\nText { textFormat: Text.PlainText }\n",
    "not a Text": "TextField {\n  text: \"a\"\n}\nTextMetrics { text: \"a\" }\nStyledLabel { }\n",
    "an inline component": "Item {\n  component Line: Text {\n    textFormat: Text.PlainText\n  }\n  Line { text: \"a\" }\n}\n"
  }
}
for (const [name, src] of Object.entries(shapes.bad))
  if (check("shape.qml", src, []).length === 0) fail("shape '" + name + "' passed; it must fail")
for (const [name, src] of Object.entries(shapes.good)) {
  const problems = check("shape.qml", src, [])
  if (problems.length > 0) fail("shape '" + name + "' failed; it must pass: " + problems.join("; "))
}

// 4. The allow-list: a listed block passes, a line without a reason or id
// is refused, a line that matches nothing fails.
{
  const src = "Text {\n  id: legacy\n  text: \"a\"\n}\n"
  const listed = [{ file: "shape.qml", id: "legacy", reason: "a test", used: false }]
  if (check("shape.qml", src, listed).length !== 0 || !listed[0].used) fail("allow-list: a listed block must pass and count as used")
  if (check("other.qml", src, listed).length === 0) fail("allow-list: a line for another file must not cover this one")
  const refused = []
  readAllow("plugin/x.qml#legacy\nplugin/x.qml a reason\n# a comment\n\n", (m) => refused.push(m))
  if (refused.length !== 2) fail("allow-list: a line without a reason or an id must be refused: " + refused.join("; "))
}

if (failures > 0) {
  console.error("plaintext.test.js: " + failures + " failure(s)")
  process.exitCode = 1
} else {
  console.log("plaintext.test.js: " + blocks + " Text blocks in " + files.length + " files, all PlainText; "
    + mutants + " mutants and " + regexMutants + " regex-quote mutants caught; " + (Object.keys(shapes.bad).length + Object.keys(shapes.good).length) + " shapes")
}
