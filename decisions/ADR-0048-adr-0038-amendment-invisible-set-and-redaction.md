# ADR-0048 — ADR-0038 amended: the invisible set and the redaction model

**Status:** proposed (WP-159 round 2; Opus stage 1, Fable stage 2 to follow)
**Date:** 2026-10-08

> Amends [ADR-0038](ADR-0038-index-details.md) §2 (the dropped direction
> and format characters) and §3 (the characters `source` refuses), as
> [ADR-0037](ADR-0037-adr-0028-amendment-toggles-and-links.md) and
> [ADR-0042](ADR-0042-adr-0028-amendment-files-pacman-left.md) amended
> ADR-0028. ADR-0038 stays accepted and unedited; its fields, its order
> "control characters, redaction, clip" and §1 and §4 stand. Its WP-140
> amendment note stands; this ADR widens that set further. Contract 2 is
> unchanged. Work package: WP-159.

## Context

A secret split by an invisible character (`to<U+200B>ken=…`,
`Authorization: Bearer<U+3164> …`, `ghp_0123<U+FE0F>4567…`) passed the
redaction in a `seldon log` note, and with the characters WP-140 left out
of the set, in `import task` and the commit subjects (WP-102b review 1,
N5; WP-143 Fable stage 2). ADR-0038 §2 drops the set from the index's
texts *before* the redaction, and WP-140 noted that a note keeps them. Two
findings of the WP-159 stage-1 review shape the model below:

- dropping the characters glues a token to the word before it, so a rule
  anchored at a word boundary (`sk-…`, `mysql … -p…`, the curl family,
  `sshpass`, `cookie:`, `x-…-token:`, `docker login`, `nmcli`, HTTPie)
  misses `x<U+200B>sk-…`, which the text as given masks (B1);
- a lone control character that is no white space (`to<BS>ken=`,
  `ghp_0123<ESC>4567…`) splits a token the same way.

## Decision

### 1. The set (amends §2 and §3)

The characters §2 drops from the index's texts and §3 refuses in `source`
are `redact::is_invisible`: the WP-140 set plus the fillers U+034F,
U+115F, U+1160, U+17B4, U+17B5, U+3164 and U+FFA0, the variation
selectors U+180B–U+180D, U+180F, U+FE00–U+FE0F and U+E0100–U+E01EF, and
U+2065. It holds every assigned Default_Ignorable_Code_Point of Unicode,
plus the format characters U+0600–U+0605 and U+FFF9–U+FFFB. A task
file's path may not hold them either (`import::bad_path_char`, the
plugin's `BAD_PATH_CHARS`, `fixtures/bad-path-chars.txt`).

### 2. The redaction reads two copies (amends §2's order)

Every redaction (the built-in rules and `[redaction] patterns`) reads
the text twice:

1. **without the characters it reads past**: the set of §1 and every
   control character that is no white space (NUL, BS, BEL, ESC, CSI, …;
   tab, the line ends, VT, FF and NEL stay, because the rules read them as
   white space). A match there masks the original span. The characters it
   left out go back where they stood, except inside a masked match and at
   its edges;
2. **as given**, after the first: there an invisible character is a
   boundary, so a rule anchored at one finds what the first copy glued to
   the word before it.

A text in which neither reading finds anything is written as it was,
invisible and control characters included (a note keeps its joiners,
WP-140). The control characters count for the redaction's reading only.
The set of §1 does not change by them, because paths refuse controls and
the index turns them into spaces.

### 3. Redact, then drop

Where §2 drops the set (the index's texts) and wherever else Seldon drops
it (a hook's command line, the closing summary, plugin commit subjects,
the text `import task` reads), the order is: control characters as
spaces where that rule applies, then the redaction of §2, then the set
dropped. A boundary an invisible character makes is read before the
character goes. `plan show --json` `intent` marks the characters it does
not drop (`‹U+XXXX›`) after the redaction too. Its `hidden` counts every
one the text holds, those inside a masked secret included: the file still
holds them, and an agent reads the file.

## Consequences

- A user pattern that names an invisible or control character matches
  nothing in the first reading; it may match in the second.
- A text with such a character costs two passes of the rules
  (`check-perf` redaction rows; WP-159 round 2's handover has the
  numbers). A text without one costs one, as before.
- An imported case edited by hand with an emoji and U+FE0F shows
  `hidden > 0` and keeps the desk's Start off (fails safe; WP-102b's
  rule).
- The index's texts drop VS16: an emoji in a desk detail may draw in text
  style.
- Not decided here (WP-169): ANSI CSI/OSC sequences as whole units, raw
  ESC in the journal and Markdown, tag characters spelling a hidden text
  in a note, and the Cf characters outside the set (U+06DD, U+070F,
  U+0890–U+0891, U+08E2, U+110BD, U+110CD, U+13430–U+1343F).

## Alternatives considered

- *An amendment note at the end of ADR-0038* (WP-140's way): AGENTS.md §7
  says an accepted ADR is not edited, and this changes the redaction
  model, not only wording.
- *Drop the set before every redaction* (WP-159's option (b)): loses the
  boundary (B1) and changes every note with a joiner.
- *Read only the copy* (WP-159 round 1): the B1 regression.
- *A second reading restricted to the boundary-anchored rules*: cheaper,
  but correct only while no other rule reads the context before a match;
  the full second reading stays correct when rules change.
