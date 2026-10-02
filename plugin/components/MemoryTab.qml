pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import qs.Commons
import qs.Ui
import "../Model.js" as Model

// Memory tab (SPEC-PLUGIN §5, WP-023): what agents read at session start.
// LESSONS lists the `## ` headings of memory/lessons.md (index.memory.lessons),
// TOPICS the other memory files with their path and `updated` date
// (index.memory.topics), both from Model.memoryRows().
//
// *Open* goes through Service.openInEditor() with a fixed target: the
// engine's `seldon open` has no memory target yet, so every row opens the
// logbook folder (`seldon open logbook --editor --json`,
// Model.MEMORY_OPEN_TARGET); nothing from the index reaches the argument
// list.
//
// Keyboard (forwarded by Panel.qml):
//   ↑ / ↓, k / j   move through the rows
//   Enter, Space   open the row under the cursor
//   e              the same
Item {
  id: root

  property var service: null
  property var indexData: null
  property bool cursorActive: false
  property color foreground: Color.foreground
  property color accent: Color.accent
  property string fontFamily: Style.font.family

  property int cursor: 0

  signal cursorWanted()

  readonly property var rows: Model.memoryRows(indexData)
  readonly property int rowCount: rows.length
  readonly property var current: rows.length > 0 ? rows[Math.max(0, Math.min(cursor, rows.length - 1))] : null
  readonly property color dim: Util.alpha(foreground, 0.65)

  function move(dy) {
    if (root.rows.length === 0) return
    root.cursor = Math.max(0, Math.min(root.rows.length - 1, root.cursor + dy))
  }

  function openRow(row) {
    if (row && root.service) return root.service.openInEditor(row.target)
    return false
  }

  function activate() {
    root.openRow(root.current)
  }

  function textKey(t) {
    if (t === "e") {
      root.openRow(root.current)
      return true
    }
    return false
  }

  onRowsChanged: if (root.cursor >= root.rows.length) root.cursor = Math.max(0, root.rows.length - 1)

  Item {
    id: header
    width: parent.width
    implicitHeight: Math.max(headerColumn.implicitHeight, openButton.implicitHeight)

    Column {
      id: headerColumn
      anchors.left: parent.left
      anchors.right: openButton.left
      anchors.rightMargin: Style.spacing.sm
      anchors.verticalCenter: parent.verticalCenter
      spacing: Style.spacing.xxs

      Text {
        width: parent.width
        visible: text !== ""
        textFormat: Text.PlainText
        text: Model.memorySummary(root.rows)
        color: root.foreground
        elide: Text.ElideRight
        font.family: root.fontFamily
        font.pixelSize: Style.font.bodySmall
        font.bold: true
      }

      Text {
        width: parent.width
        textFormat: Text.PlainText
        text: "Opens the logbook folder; the files are in memory/"
        color: root.dim
        wrapMode: Text.Wrap
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
      }
    }

    Button {
      id: openButton
      anchors.right: parent.right
      anchors.verticalCenter: parent.verticalCenter
      text: "Open"
      tooltipText: "Open the logbook in the editor (Enter or e)"
      bordered: true
      foreground: root.foreground
      accent: root.accent
      fontFamily: root.fontFamily
      fontSize: Style.font.caption
      verticalPadding: Style.spacing.xs
      onClicked: root.openRow(root.current || { target: Model.MEMORY_OPEN_TARGET })
    }
  }

  ListView {
    id: list
    anchors.top: header.bottom
    anchors.topMargin: Style.spacing.lg
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.bottom: parent.bottom
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    interactive: contentHeight > height
    ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }

    model: root.rows
    currentIndex: root.cursorActive ? root.cursor : -1
    onCurrentIndexChanged: if (currentIndex >= 0) Qt.callLater(keepCurrentVisible)
    function keepCurrentVisible() {
      if (currentIndex >= 0) positionViewAtIndex(currentIndex, ListView.Contain)
    }

    delegate: Column {
      id: rowItem

      required property var modelData
      required property int index

      width: ListView.view.width
      topPadding: modelData.section !== "" && index > 0 ? Style.spacing.lg : 0

      PanelSectionHeader {
        visible: rowItem.modelData.section !== ""
        bottomPadding: Style.spacing.sm
        text: rowItem.modelData.section
        foreground: root.foreground
        fontFamily: root.fontFamily
      }

      CursorSurface {
        width: parent.width
        implicitHeight: rowColumn.implicitHeight + Style.spacing.xs * 2
        hasCursor: root.cursorActive && rowItem.index === root.cursor
        foreground: root.foreground
        accent: root.accent

        MouseArea {
          anchors.fill: parent
          cursorShape: Qt.PointingHandCursor
          onClicked: {
            root.cursor = rowItem.index
            root.cursorWanted()
          }
          onDoubleClicked: root.openRow(rowItem.modelData)
        }

        Column {
          id: rowColumn
          x: Style.spacing.lg
          y: Style.spacing.xs
          width: parent.width - Style.spacing.lg * 2
          spacing: Style.spacing.xxs

          Text {
            width: parent.width
            textFormat: Text.PlainText
            text: rowItem.modelData.title
            color: root.foreground
            wrapMode: Text.Wrap
            font.family: root.fontFamily
            font.pixelSize: Style.font.bodySmall
          }

          Text {
            width: parent.width
            visible: text !== ""
            textFormat: Text.PlainText
            text: rowItem.modelData.meta
            color: root.dim
            elide: Text.ElideMiddle
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
          }
        }
      }
    }
  }

  Text {
    anchors.centerIn: list
    visible: root.rows.length === 0
    textFormat: Text.PlainText
    text: root.indexData ? "No lessons or memory files yet" : "No index to show"
    color: root.dim
    font.family: root.fontFamily
    font.pixelSize: Style.font.bodySmall
  }
}
