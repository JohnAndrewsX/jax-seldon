pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import qs.Commons
import qs.Ui
import "../Model.js" as Model

// Decisions tab (SPEC-PLUGIN §5, WP-023): the logbook's ADRs from
// index.decisions, newest first (Model.decisionRows), each with id, status,
// title, date and file; a proposed decision carries the accent stripe, a
// superseded one is struck through. *New decision* shows the
// NewDecisionSheet in place of the list.
//
// Opening goes through Service.openInEditor() with the decision id, checked
// against the schema pattern (`seldon open ADR-NNNN --editor --json`); a row
// whose id does not match is shown, never opened. A created decision is
// opened by the service and the cursor follows it into the list once the
// index lists it.
//
// Keyboard (forwarded by Panel.qml):
//   ↑ / ↓, k / j   move through the decisions
//   Enter, Space   open the decision under the cursor in the editor
//   e              the same
//   d              new decision (the sheet; Esc gives the keys back)
Item {
  id: root

  property var service: null
  property var indexData: null
  property bool cursorActive: false
  property color foreground: Color.foreground
  property color accent: Color.accent
  property color urgent: Color.urgent
  property color muted: Color.muted
  property string fontFamily: Style.font.family

  property int cursor: 0
  // The decision the cursor is on, by id, so it stays on it when the index
  // changes (and finds a new decision once the index lists it).
  property string selectedId: ""
  property bool sheetOpen: false

  signal cursorWanted()
  // The sheet gives the keys back (Esc, Cancel, a created decision).
  signal leaveRequested()

  readonly property var rows: Model.decisionRows(indexData)
  readonly property int rowCount: rows.length
  readonly property var current: rows.length > 0 ? rows[Math.max(0, Math.min(cursor, rows.length - 1))] : null
  readonly property bool canWrite: !!service && service.canWrite
  readonly property bool editing: sheetOpen && sheet.editing
  readonly property var result: service ? service.decideResult : null
  readonly property color dim: Util.alpha(foreground, 0.65)
  property alias sheet: sheet

  function toneColor(tone) {
    return tone === "accent" ? root.accent : tone === "muted" ? root.muted : root.dim
  }

  function select(i) {
    if (root.rows.length === 0) return
    root.cursor = Math.max(0, Math.min(root.rows.length - 1, i))
    root.selectedId = root.rows[root.cursor].id
  }

  function move(dy) {
    root.select(root.cursor + dy)
  }

  function findSelected() {
    for (var i = 0; i < root.rows.length; i++) {
      if (root.rows[i].id === root.selectedId) {
        root.cursor = i
        return true
      }
    }
    return false
  }

  // Put the cursor on decision `id` now if the index has it, else once it does.
  function follow(id) {
    root.selectedId = id
    root.findSelected()
  }

  function openRow(row) {
    if (row && row.actionable && root.service) return root.service.openInEditor(row.id)
    return false
  }

  function activate() {
    root.openRow(root.current)
  }

  function openSheet() {
    root.sheetOpen = true
    Qt.callLater(sheet.focusTitle)
  }

  function closeSheet() {
    root.sheetOpen = false
    root.leaveRequested()
  }

  function textKey(t) {
    if (t === "e") {
      root.openRow(root.current)
      return true
    }
    if (t === "d") {
      if (root.canWrite) root.openSheet()
      return true
    }
    return false
  }

  onRowsChanged: {
    if (root.findSelected()) return
    root.cursor = Math.max(0, Math.min(root.cursor, root.rows.length - 1))
    if (root.selectedId === "" && root.rows.length > 0) root.selectedId = root.rows[root.cursor].id
  }
  // A hidden tab does not reliably lose the focus of its sheet (WP-021).
  onVisibleChanged: if (!visible && root.editing) root.leaveRequested()

  Column {
    anchors.fill: parent
    spacing: Style.spacing.md

    Item {
      id: header
      width: parent.width
      implicitHeight: Math.max(summaryText.implicitHeight, newButton.implicitHeight)

      Text {
        id: summaryText
        anchors.left: parent.left
        anchors.right: newButton.left
        anchors.rightMargin: Style.spacing.sm
        anchors.verticalCenter: parent.verticalCenter
        textFormat: Text.PlainText
        text: root.rows.length > 0 ? Model.decisionSummary(root.rows) : ""
        color: root.foreground
        elide: Text.ElideRight
        font.family: root.fontFamily
        font.pixelSize: Style.font.bodySmall
        font.bold: true
      }

      Button {
        id: newButton
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        text: "New decision"
        iconText: "+"
        iconSize: Style.font.caption
        tooltipText: root.canWrite ? "Key d" : (root.service ? root.service.writeBlocker : "")
        enabled: root.canWrite
        selected: root.sheetOpen
        bordered: true
        foreground: root.foreground
        accent: root.accent
        fontFamily: root.fontFamily
        fontSize: Style.font.caption
        verticalPadding: Style.spacing.xs
        onClicked: root.sheetOpen ? root.closeSheet() : root.openSheet()
      }
    }

    // The engine's answer to the last new decision. Progress and a refusal
    // show in the sheet, next to its title.
    Text {
      id: resultLine
      width: parent.width
      visible: text !== ""
      textFormat: Text.PlainText
      text: root.result && !root.sheetOpen && !root.result.pending ? root.result.text : ""
      color: root.result && !root.result.ok ? root.urgent : root.dim
      elide: Text.ElideRight
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
    }

    ListView {
      id: list
      width: parent.width
      height: parent.height - header.height - (resultLine.visible ? resultLine.height + parent.spacing : 0) - parent.spacing
      visible: !root.sheetOpen
      clip: true
      spacing: Style.spacing.sm
      boundsBehavior: Flickable.StopAtBounds
      interactive: contentHeight > height
      ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }

      model: root.rows
      currentIndex: root.rows.length > 0 ? Math.min(root.cursor, root.rows.length - 1) : -1
      onCurrentIndexChanged: if (currentIndex >= 0) Qt.callLater(keepCurrentVisible)
      onCountChanged: if (currentIndex >= 0) Qt.callLater(keepCurrentVisible)
      function keepCurrentVisible() {
        if (currentIndex >= 0) positionViewAtIndex(currentIndex, ListView.Contain)
      }

      delegate: CursorSurface {
        id: tile

        required property var modelData
        required property int index

        readonly property bool here: index === root.cursor

        width: ListView.view.width
        implicitHeight: tileColumn.implicitHeight + Style.spacing.md * 2
        hasCursor: root.cursorActive && here
        current: !root.cursorActive && here
        foreground: root.foreground
        accent: root.accent

        // Under the content, so the Open button keeps its own clicks.
        MouseArea {
          anchors.fill: parent
          cursorShape: Qt.PointingHandCursor
          onClicked: {
            root.select(tile.index)
            root.cursorWanted()
          }
          onDoubleClicked: root.openRow(tile.modelData)
        }

        Rectangle {
          x: 0
          y: Style.spacing.sm
          width: Style.spacing.xs
          height: tile.height - Style.spacing.sm * 2
          radius: width / 2
          visible: tile.modelData.tone !== ""
          color: root.toneColor(tile.modelData.tone)
        }

        Column {
          id: tileColumn
          x: Style.spacing.xs + Style.spacing.md
          y: Style.spacing.md
          width: tile.width - x - Style.spacing.md
          spacing: Style.spacing.xxs

          Item {
            width: parent.width
            implicitHeight: Math.max(tileId.implicitHeight, openButton.visible ? openButton.implicitHeight : tileStatus.implicitHeight)

            Text {
              id: tileId
              anchors.left: parent.left
              anchors.verticalCenter: parent.verticalCenter
              textFormat: Text.PlainText
              text: tile.modelData.id
              color: root.dim
              font.family: root.fontFamily
              font.pixelSize: Style.font.caption
            }

            Text {
              id: tileStatus
              anchors.left: tileId.right
              anchors.leftMargin: Style.spacing.md
              anchors.right: openButton.visible ? openButton.left : parent.right
              anchors.rightMargin: openButton.visible ? Style.spacing.sm : 0
              anchors.verticalCenter: parent.verticalCenter
              textFormat: Text.PlainText
              text: tile.modelData.status
              color: tile.modelData.status === "proposed" ? root.accent : root.dim
              elide: Text.ElideRight
              font.family: root.fontFamily
              font.pixelSize: Style.font.caption
              font.italic: true
            }

            Button {
              id: openButton
              anchors.right: parent.right
              anchors.verticalCenter: parent.verticalCenter
              visible: tile.here && tile.modelData.actionable
              text: "Open"
              tooltipText: "Open " + tile.modelData.id + " in the editor (Enter or e)"
              bordered: true
              foreground: root.foreground
              accent: root.accent
              fontFamily: root.fontFamily
              fontSize: Style.font.caption
              verticalPadding: Style.spacing.xxs
              onClicked: {
                root.select(tile.index)
                root.openRow(tile.modelData)
              }
            }
          }

          Text {
            width: parent.width
            textFormat: Text.PlainText
            text: tile.modelData.title
            color: tile.modelData.status === "superseded" ? root.dim : root.foreground
            wrapMode: Text.Wrap
            maximumLineCount: 2
            elide: Text.ElideRight
            font.family: root.fontFamily
            font.pixelSize: Style.font.bodySmall
            font.strikeout: tile.modelData.status === "superseded"
          }

          Text {
            width: parent.width
            visible: text !== ""
            textFormat: Text.PlainText
            text: Model.decisionMeta(tile.modelData)
            color: root.dim
            elide: Text.ElideMiddle
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
          }
        }
      }
    }

    NewDecisionSheet {
      id: sheet
      width: parent.width
      visible: root.sheetOpen
      service: root.service
      foreground: root.foreground
      accent: root.accent
      urgent: root.urgent
      fontFamily: root.fontFamily
      onLeaveRequested: root.closeSheet()
      onCreated: function(decisionId) {
        if (decisionId !== "") root.follow(decisionId)
        root.closeSheet()
      }
    }
  }

  Text {
    anchors.centerIn: parent
    visible: root.rows.length === 0 && !root.sheetOpen
    textFormat: Text.PlainText
    text: root.indexData ? "No decisions yet. New decision (key d) writes the first." : "No index to show"
    color: root.dim
    font.family: root.fontFamily
    font.pixelSize: Style.font.bodySmall
  }
}
