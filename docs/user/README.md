# Seldon user guide

The guide for people who use Seldon on their Omarchy machine.

| Language | Start here |
|---|---|
| English | [en/README.md](en/README.md) |
| Deutsch | [de/README.md](de/README.md) |

People who develop Seldon read the specifications in [`docs/`](..)
instead. Agents working inside a logbook read the
[agent guide](../AGENT-GUIDE.md).

## Translation policy

- **English is the source.** Every page is written and changed in
  `en/` first.
- Every other language has the same pages with the same file names,
  headings, code blocks, tables and pictures. A translation is complete;
  it never summarises.
- Commands, options, file names, config keys and the plugin's UI labels
  stay English in every language.
- Each translated page carries, right under its title, the English page
  and commit it matches:
  `<!-- source: en/01-getting-started.md @ 1a2b3c4 -->`. When the English
  page changes, its translation is updated in the same pull request, or
  in a follow-up that sets the new commit.
- `just docs-check` (part of `just check`) fails when the page sets or
  their structure differ, and warns when a translation's source commit
  is older than its English page's last change.
- A new language is a new folder `docs/user/<code>/` with all pages and a
  row in the table above. [STYLE.md](STYLE.md) has the voice, the term
  list and the rules for commands and screenshots.
