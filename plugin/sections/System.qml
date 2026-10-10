pragma ComponentBehavior: Bound

import QtQuick
import qs.Commons
import qs.Ui
import "../components/desk"
import "../Model.js" as Model

// Section 5, System (ADR-0034 §2, WP-123; the 0.1 System tab). The list:
// seven tiles — Omarchy, Packages, Snapshots, Deviations, Collectors,
// Recently edited, Ignored by pacman — each with its big value
// (Model.systemTiles; every field of index.system is optional, a tile
// without its data shows "—"). The detail: the big value and its unit,
// the lead line, the key/value rows and where they come from. The sticky bar: *Open in editor* opens the logbook's STATUS.md
// (`seldon open status --editor --json`), the full report.
//
// The sixth tile, Recently edited (WP-139, ADR-0046): files under
// ~/.config modified in the last 7 days outside the watched paths
// (index.system.recentConfig, paths and times only). Its detail lists them,
// each with its age, "not watched" and *Watch*, which runs `seldon config
// watch --json -- <path>` (Service.watchPath): the path joins watchPaths
// and the row goes with the index the engine rebuilds.
//
// The seventh, Ignored by pacman (WP-165, ADR-0052): the IgnorePkg and
// IgnoreGroup names of pacman.conf (index.system.pacmanIgnore), as plain
// text; no action.
//
// Keys: ↑/↓ j/k move, Enter shows the detail (stacked layout), `e` opens
// STATUS.md.
ReadingSection {
  id: root

  readonly property var openResult: root.service ? root.service.openResult : null
  readonly property var watchResult: root.service ? root.service.watchResult : null
  readonly property bool canWrite: !!root.service && root.service.canWrite
  readonly property bool watching: !!root.watchResult && root.watchResult.pending
  readonly property var files: root.current ? root.current.files : []

  function watch(path) {
    if (!root.service || !root.canWrite || root.watching) return false
    return root.service.watchPath(path)
  }

  allRows: root.index ? Model.systemTiles(root.index, root.service ? root.service.nowMs : Date.now()) : []
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
    v.files = root.files.map(function(f) { return f.path + " " + f.age })
    v.watchResult = root.watchResult ? root.watchResult.text : ""
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
      color: root.openResult && !root.openResult.ok ? root.tone.urgentText : root.tone.dim
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
        color: root.current && root.current.empty ? root.tone.dim : Color.popups.text
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
        color: root.tone.dim
        font.family: Style.font.family
        font.pixelSize: Style.font.body
      }
    }

    Text {
      width: parent.width
      visible: text !== ""
      textFormat: Text.PlainText
      text: root.current ? root.current.lead : ""
      color: root.tone.dim
      wrapMode: Text.Wrap
      font.family: Style.font.family
      font.pixelSize: Style.font.body
    }

    KeyValues {
      width: parent.width
      visible: rows.length > 0
      rows: root.current ? root.current.rows : []
    }

    // Recently edited: the engine's answer to the last Watch, then the files
    Text {
      width: parent.width
      visible: text !== ""
      textFormat: Text.PlainText
      // also after the last row went (N3)
      text: root.current && root.current.id === "recent" && root.watchResult ? root.watchResult.text : ""
      color: root.watchResult && !root.watchResult.ok ? root.tone.urgentText : root.tone.dim
      wrapMode: Text.WrapAnywhere
      font.family: Style.font.family
      font.pixelSize: Style.font.caption
    }

    Column {
      width: parent.width
      visible: root.files.length > 0
      spacing: Style.spacing.md

      Repeater {
        model: root.files

        Item {
          id: fileRow
          required property var modelData
          width: parent ? parent.width : 0
          height: Math.max(fileText.implicitHeight, watchButton.implicitHeight)

          Column {
            id: fileText
            anchors.left: parent.left
            anchors.right: watchButton.left
            anchors.rightMargin: Style.spacing.md
            anchors.verticalCenter: parent.verticalCenter

            Text {
              width: parent.width
              textFormat: Text.PlainText
              text: fileRow.modelData.path
              color: Color.popups.text
              elide: Text.ElideMiddle
              font.family: Style.font.family
              font.pixelSize: Style.font.bodySmall
            }

            Text {
              width: parent.width
              textFormat: Text.PlainText
              text: (fileRow.modelData.age !== "" ? fileRow.modelData.age + " · " : "") + "not watched"
              color: root.tone.dim
              font.family: Style.font.family
              font.pixelSize: Style.font.caption
            }
          }

          Button {
            id: watchButton
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            text: "Watch"
            tooltipText: root.canWrite ? "Add to watchPaths: the next capture records its changes"
              : (root.service ? root.service.writeBlocker : "")
            enabled: root.canWrite && !root.watching
            bordered: true
            foreground: Color.popups.text
            fontFamily: Style.font.family
            fontSize: Style.font.caption
            onClicked: root.watch(fileRow.modelData.path)
          }
        }
      }
    }

    Text {
      width: parent.width
      textFormat: Text.PlainText
      // where the tile's data comes from (Model.systemTiles)
      text: root.current ? root.current.source : ""
      color: root.tone.dim
      wrapMode: Text.Wrap
      font.family: Style.font.family
      font.pixelSize: Style.font.caption
    }
  }
}
