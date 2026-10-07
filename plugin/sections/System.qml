pragma ComponentBehavior: Bound

import QtQuick
import qs.Commons
import "../components/desk"
import "../Model.js" as Model

// Section 5, System (ADR-0034 §2, WP-123; the 0.1 System tab). The list:
// five tiles — Omarchy, Packages, Snapshots, Deviations, Collectors — each
// with its big value (Model.systemTiles; every field of index.system is
// optional, a tile without its data shows "—"). The detail: the big value
// and its unit, the lead line, the key/value rows and where they come
// from. The sticky bar: *Open in editor* opens the logbook's STATUS.md
// (`seldon open status --editor --json`), the full report.
//
// Keys: ↑/↓ j/k move, Enter shows the detail (stacked layout), `e` opens
// STATUS.md.
ReadingSection {
  id: root

  readonly property var openResult: root.service ? root.service.openResult : null

  allRows: Model.systemTiles(root.index, root.service ? root.service.nowMs : Date.now())
  searchFields: ["title", "meta", "lead"]
  emptyText: root.index ? "No tile matches the search." : "No index to show"
  detailTitle: root.current ? root.current.title : ""
  actions: [{ id: "status", label: "Open in editor", primary: false, enabled: !!root.index }]
  actionMeta: "STATUS.md"
  rowTitle: function(r) { return r.title }
  rowMeta: function(r) { return r.meta }
  rowStripe: function(r) { return r.stripe }

  function openStatus() {
    if (root.service && root.index) root.service.openInEditor("status")
    return true
  }

  function textKey(t) {
    if (t === "e") return root.openStatus()
    return false
  }

  function view() {
    var v = root.baseView()
    v.tiles = root.rows.map(function(r) { return r.id + " " + r.meta })
    v.big = root.current ? root.current.big : ""
    v.detailRows = root.current ? root.current.rows.map(function(r) { return r[0] }) : []
    v.openResult = root.openResult ? root.openResult.text : ""
    return v
  }

  onActionTriggered: function(id) { if (id === "status") root.openStatus() }

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

    Flow {
      width: parent.width
      spacing: Style.spacing.lg

      Text {
        id: big
        textFormat: Text.PlainText
        text: root.current ? root.current.big : ""
        color: root.current && root.current.empty ? Color.muted : Color.popups.text
        font.family: Style.font.family
        font.pixelSize: Style.font.display
        font.bold: true
      }

      Text {
        visible: text !== ""
        height: big.height
        verticalAlignment: Text.AlignBottom
        bottomPadding: Style.spacing.sm
        textFormat: Text.PlainText
        text: root.current ? root.current.unit : ""
        color: Color.muted
        font.family: Style.font.family
        font.pixelSize: Style.font.body
      }
    }

    Text {
      width: parent.width
      visible: text !== ""
      textFormat: Text.PlainText
      text: root.current ? root.current.lead : ""
      color: Color.muted
      wrapMode: Text.Wrap
      font.family: Style.font.family
      font.pixelSize: Style.font.body
    }

    KeyValues {
      width: parent.width
      visible: rows.length > 0
      rows: root.current ? root.current.rows : []
    }

    Text {
      width: parent.width
      textFormat: Text.PlainText
      text: "From the dossier; rebuilt on every capture."
      color: Color.muted
      wrapMode: Text.Wrap
      font.family: Style.font.family
      font.pixelSize: Style.font.caption
    }
  }
}
