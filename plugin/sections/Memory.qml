pragma ComponentBehavior: Bound

import QtQuick
import qs.Commons
import "../components/desk"
import "../Model.js" as Model

// Section 6, Memory (ADR-0034 §2, WP-123; the 0.1 Memory tab, WP-023): what
// every agent reads at the start of a session. The list: the `## `
// headings of memory/lessons.md (index.memory.lessons), then the other
// memory files (index.memory.topics) with their `updated` date
// (Model.memoryRows). The detail: the lesson or topic, its file and date;
// the index carries headings and names only, the text is in the logbook.
// The sticky bar: *Open in editor* — the engine's `seldon open` has no
// memory target yet, so it opens the logbook folder (`seldon open logbook
// --editor --json`, Model.MEMORY_OPEN_TARGET); nothing from the index
// reaches the argument list.
//
// Keys: ↑/↓ j/k move, Enter shows the detail (stacked layout), `e` opens
// the logbook.
ReadingSection {
  id: root

  readonly property var memory: Model.memoryDetail(root.current)
  readonly property var openResult: root.service ? root.service.openResult : null

  // Ids for the selection: a lesson is its heading, a topic its name (the
  // two lists may share a text, so the kind goes first).
  allRows: Model.memoryRows(root.index).map(function(r) {
    var out = { id: r.kind + ":" + r.title }
    for (var k in r) out[k] = r[k]
    return out
  })
  searchFields: ["title", "meta", "kind"]
  emptyText: !root.index ? "No index to show"
    : root.allRows.length > 0 ? "Nothing matches the search."
    : "No lessons or memory files yet"
  detailTitle: root.memory ? root.memory.heading : ""
  actions: [{ id: "open", label: "Open in editor", primary: false, enabled: !!root.index }]
  actionMeta: "memory/"
  rowTitle: function(r) { return r.title }
  rowMeta: function(r) { return r.kind === "lesson" ? "lesson" : r.meta !== "" ? r.meta : "topic" }

  function openLogbook() {
    if (root.service && root.index) root.service.openInEditor(Model.MEMORY_OPEN_TARGET)
    return true
  }

  function textKey(t) {
    if (t === "e") return root.openLogbook()
    return false
  }

  function view() {
    var v = root.baseView()
    v.summary = summary.text
    v.openResult = root.openResult ? root.openResult.text : ""
    return v
  }

  onActionTriggered: function(id) { if (id === "open") root.openLogbook() }

  head: [
    Text {
      id: summary
      width: parent ? parent.width : 0
      visible: text !== ""
      textFormat: Text.PlainText
      text: Model.memorySummary(root.allRows)
      color: Color.popups.text
      wrapMode: Text.Wrap
      font.family: Style.font.family
      font.pixelSize: Style.font.bodySmall
    }
  ]

  Column {
    visible: !!root.current
    width: parent.width
    spacing: Style.spacing.xxl

    Text {
      width: parent.width
      visible: text !== ""
      textFormat: Text.PlainText
      text: root.openResult ? root.openResult.text : ""
      color: root.openResult && !root.openResult.ok ? Color.urgent : Color.muted
      wrapMode: Text.WrapAnywhere
      font.family: Style.font.family
      font.pixelSize: Style.font.caption
    }

    Text {
      width: parent.width
      textFormat: Text.PlainText
      text: root.current ? root.current.title : ""
      color: Color.popups.text
      wrapMode: Text.Wrap
      font.family: Style.font.family
      font.pixelSize: Style.font.title
      font.bold: true
    }

    KeyValues {
      width: parent.width
      visible: rows.length > 0
      rows: root.memory ? root.memory.rows : []
    }

    Text {
      width: parent.width
      textFormat: Text.PlainText
      text: "Every agent reads this at the start of a session. The index carries the headings; the text is in the logbook's memory/ folder."
      color: Color.muted
      wrapMode: Text.Wrap
      font.family: Style.font.family
      font.pixelSize: Style.font.bodySmall
    }
  }
}
