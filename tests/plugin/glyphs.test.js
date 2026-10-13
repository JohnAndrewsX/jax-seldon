// Glyph coverage (WP-178, SPEC-PLUGIN §7 "Glyphs"): every non-ASCII
// character in the plugin's QML, JS and manifest — written, or as a \u
// escape in a string literal — is in JetBrainsMono Nerd Font, the font
// Omarchy maps `monospace` to; a symbol the font lacks is drawn as an SVG
// with components/MaskIcon.qml instead. A missing glyph falls back to
// another font (or a colour emoji) and breaks the theme's look.
//
// The font's charset comes from the installed faces (`fc-list`, under a
// private HOME, XDG_CACHE_HOME, XDG_CONFIG_HOME and a 0700 XDG_RUNTIME_DIR,
// all removed afterwards) where fontconfig and the font are there; else
// from the pinned file tests/plugin/glyphs/jetbrainsmono-nerd-font.charset
// (CI; its header says how it was made). SELDON_GLYPHS_PINNED=1 reads the
// pinned file on any host.
//
// Run: node tests/plugin/glyphs.test.js
"use strict"

const fs = require("fs")
const os = require("os")
const path = require("path")
const assert = require("assert")
const { spawnSync } = require("child_process")

const root = path.resolve(__dirname, "../..")
const plugin = path.join(root, "plugin")
const pinnedFile = path.join(__dirname, "glyphs/jetbrainsmono-nerd-font.charset")
const FAMILY = "JetBrainsMono Nerd Font"

let passed = 0
function test(name, fn) {
  try {
    fn()
    passed++
  } catch (e) {
    console.error("FAIL " + name + "\n  " + e.message)
    process.exitCode = 1
  }
}
const same = (a, b) => assert.strictEqual(JSON.stringify(a), JSON.stringify(b))

// fontconfig's charset ("20-7e a0-131 …") as sorted [from, to] ranges.
function parseCharset(text) {
  const ranges = []
  for (const part of text.trim().split(/\s+/)) {
    if (part === "") continue
    const m = /^([0-9a-f]+)(?:-([0-9a-f]+))?$/i.exec(part)
    if (!m) throw new Error("not a charset range: " + part)
    ranges.push([parseInt(m[1], 16), parseInt(m[2] || m[1], 16)])
  }
  return ranges.sort((a, b) => a[0] - b[0])
}

function covers(ranges, cp) {
  let lo = 0
  let hi = ranges.length - 1
  while (lo <= hi) {
    const mid = (lo + hi) >> 1
    if (cp < ranges[mid][0]) hi = mid - 1
    else if (cp > ranges[mid][1]) lo = mid + 1
    else return true
  }
  return false
}

function pinnedCharset() {
  const lines = fs.readFileSync(pinnedFile, "utf8").split("\n").filter((l) => l.trim() !== "" && !l.startsWith("#"))
  assert.strictEqual(lines.length, 1, "the pinned file holds one charset line")
  return parseCharset(lines[0])
}

// The installed faces' charsets ([] when fontconfig or the font is
// missing). fc-list gets a private HOME and runtime dir: it never reads
// or writes the user's (AGENTS.md §6).
function installedCharsets() {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "seldon-glyphs-"))
  try {
    const rt = path.join(tmp, "rt")
    fs.mkdirSync(rt, { mode: 0o700 })
    fs.chmodSync(rt, 0o700)
    const env = {
      PATH: process.env.PATH || "/usr/bin:/bin",
      HOME: tmp,
      XDG_CACHE_HOME: path.join(tmp, "cache"),
      XDG_CONFIG_HOME: path.join(tmp, "config"),
      XDG_DATA_HOME: path.join(tmp, "data"),
      XDG_RUNTIME_DIR: rt
    }
    const r = spawnSync("fc-list", ["--format", "%{charset}\\n", ":family=" + FAMILY], { env, encoding: "utf8", timeout: 20000 })
    if (r.error || r.status !== 0) return []
    return r.stdout.split("\n").filter((l) => l.trim() !== "").map(parseCharset)
  } finally {
    fs.rmSync(tmp, { recursive: true, force: true })
  }
}

// What the shell renders from the plugin: every .qml and .js under
// plugin/, at any depth, and the manifest (Omarchy's settings panel).
function sources(dir) {
  let out = []
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name)
    if (e.isDirectory()) out = out.concat(sources(p))
    else if (/\.(qml|js)$/.test(e.name) || p === path.join(plugin, "manifest.json"))
      out.push({ file: path.relative(root, p), src: fs.readFileSync(p, "utf8") })
  }
  return out
}

// Control and format characters are no glyphs (a "\u0000" a check looks
// for); everything else above ASCII is.
const NOT_A_GLYPH = /^[\p{Cc}\p{Cf}\p{Zl}\p{Zp}]$/u

// Every non-ASCII character of the sources: [{ at: "file:line", cp }],
// once per file and character. Written characters anywhere (comments
// too); \uXXXX and \u{X…} escapes inside a string literal ("…" or '…'), so
// a regular expression's escaped range is not taken for a glyph.
function glyphsOf(files) {
  const out = []
  for (const { file, src } of files) {
    const seen = new Set()
    const add = (cp, line) => {
      if (cp < 0x80 || seen.has(cp) || NOT_A_GLYPH.test(String.fromCodePoint(cp))) return
      seen.add(cp)
      out.push({ at: file + ":" + line, cp })
    }
    src.split("\n").forEach((l, i) => {
      for (const ch of l) add(ch.codePointAt(0), i + 1)
      for (const s of l.matchAll(/"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'/g))
        for (const e of s[0].matchAll(/\\u\{([0-9a-fA-F]{1,6})\}|\\u([0-9a-fA-F]{4})/g)) add(parseInt(e[1] || e[2], 16), i + 1)
    })
  }
  return out
}

const hex = (cp) => "U+" + cp.toString(16).toUpperCase().padStart(4, "0")

// The characters no charset of `faces` covers: ["file:line U+XXXX c"].
function uncovered(glyphs, faces) {
  return glyphs.filter((g) => !faces.every((f) => covers(f, g.cp)))
    .map((g) => g.at + " " + hex(g.cp) + " " + String.fromCodePoint(g.cp))
}

const pinned = pinnedCharset()
const installed = process.env.SELDON_GLYPHS_PINNED === "1" ? [] : installedCharsets()
const faces = installed.length > 0 ? installed : [pinned]
const from = installed.length > 0 ? installed.length + " installed faces (fc-list)" : "the pinned charset (" + path.relative(root, pinnedFile) + ")"

test("every non-ASCII glyph in plugin/ is in JetBrainsMono Nerd Font", () => {
  const glyphs = glyphsOf(sources(plugin))
  assert.ok(glyphs.length >= 20, "only " + glyphs.length + " glyphs found")
  // the Nerd Font glyphs the desk uses are among them, as escapes too
  for (const cp of [0xF0996, 0xF0453, 0xF0AE3, 0xF03D7]) assert.ok(glyphs.some((g) => g.cp === cp), hex(cp))
  same(uncovered(glyphs, faces), [])
})

test("the check sees a missing glyph, written or escaped, and skips regexes and controls", () => {
  const probe = [{ file: "plugin/components/desk/Probe.qml", src: [
    "Text { text: \"↻ reinstalled\" }",
    "// ⏸ in a comment counts too",
    "var a = \"\\u21BA\" + '\\u{23F0}'",
    "var ok = \"\\u{F0453} ↑ − …\"",
    "var re = /[\\u27E1\\u0000-\\u001f]/",
    "if (t.indexOf(\"\\u0000\") !== -1) {}"
  ].join("\n") }]
  same(uncovered(glyphsOf(probe), [pinned]), [
    "plugin/components/desk/Probe.qml:1 U+21BB ↻",
    "plugin/components/desk/Probe.qml:2 U+23F8 ⏸",
    "plugin/components/desk/Probe.qml:3 U+21BA ↺",
    "plugin/components/desk/Probe.qml:3 U+23F0 ⏰"])
})

test("the pinned charset is the font Omarchy ships", () => {
  // ASCII, the arrows and dots the desk writes, and Material Design icons
  for (const cp of [0x20, 0x7E, 0xB7, 0x2014, 0x2026, 0x2190, 0x2193, 0x2212, 0x25B8, 0x2713, 0xF0453, 0xF0AE3, 0xF0996])
    assert.ok(covers(pinned, cp), hex(cp))
  // what SPEC-PLUGIN §7 names as missing
  for (const cp of [0x23F8, 0x21BA, 0x21BB, 0x23F0, 0x27E1]) assert.ok(!covers(pinned, cp), hex(cp))
  // the installed faces, where there are any, are the pinned charset
  if (installed.length > 0) {
    const flat = (r) => JSON.stringify(r)
    const differ = installed.filter((f) => flat(f) !== flat(pinned)).length
    if (differ > 0) console.log("glyphs.test.js: NOTICE: " + differ + " of the installed faces differ from the pinned charset; refresh it (its header)")
  }
})

test("parseCharset and covers", () => {
  const r = parseCharset(" 20-7e  a0 f0000-f0002 ")
  same(r, [[0x20, 0x7E], [0xA0, 0xA0], [0xF0000, 0xF0002]])
  for (const [cp, want] of [[0x1F, false], [0x20, true], [0x7E, true], [0x7F, false], [0xA0, true], [0xF0001, true], [0xF0003, false]])
    assert.strictEqual(covers(r, cp), want, hex(cp))
  assert.throws(() => parseCharset("20-7e zz"))
})

console.log("glyphs.test.js: " + passed + " passed, against " + from + (process.exitCode ? ", some FAILED" : ""))
