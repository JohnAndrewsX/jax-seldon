pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import qs.Commons
import qs.Ui
import "../Model.js" as Model

// Changelog tab (SPEC-PLUGIN §5): index.events newest first, grouped by day,
// with source filter chips. Rows come from Model.changelogRows(); see
// EventRow.qml for what one row shows.
//
// Keyboard (forwarded by Panel.qml): ↑/↓ move the cursor, Enter expands the
// row (full text, a group's members), f / F cycle the source filter.
Item {
  id: root

  property var indexData: null
  property bool cursorActive: false
  property color foreground: Color.foreground
  property color accent: Color.accent
  property color urgent: Color.urgent
  property color muted: Color.muted
  property string fontFamily: Style.font.family

  property string filter: "all"
  property int cursor: 0
  property string expandedId: ""

  signal captureRequested()
  signal cursorWanted()

  readonly property var rows: Model.changelogRows(indexData, filter)
  readonly property var chips: Model.filterChips(indexData)
  readonly property int rowCount: rows.length
  readonly property color dim: Util.alpha(foreground, 0.65)

  function clampCursor(i) {
    return Math.max(0, Math.min(root.rows.length - 1, i))
  }

  function move(dy) {
    if (root.rows.length === 0) return
    root.cursor = root.clampCursor(root.cursor + dy)
  }

  function activate() {
    var row = root.rows[root.cursor]
    if (!row) return
    root.expandedId = root.expandedId === row.id ? "" : row.id
    // The row grew or shrank; keep all of it in view.
    Qt.callLater(list.keepCurrentVisible)
  }

  function setFilter(id) {
    root.filter = id
  }

  function textKey(t) {
    if (t === "f" || t === "F") {
      root.setFilter(Model.cycleFilter(root.filter, t === "F" ? -1 : 1))
      return true
    }
    return false
  }

  onFilterChanged: {
    root.cursor = 0
    root.expandedId = ""
    list.positionViewAtBeginning()
  }
  onRowsChanged: if (root.cursor >= root.rows.length) root.cursor = Math.max(0, root.rows.length - 1)

  Column {
    anchors.fill: parent
    spacing: Style.spacing.lg

    Flow {
      id: chipFlow
      width: parent.width
      spacing: Style.spacing.sm

      Repeater {
        model: root.chips

        Button {
          required property var modelData

          text: modelData.label + " " + modelData.count
          selected: modelData.id === root.filter
          bordered: true
          foreground: modelData.count > 0 || selected ? root.foreground : root.dim
          fontFamily: root.fontFamily
          fontSize: Style.font.caption
          horizontalPadding: Style.spacing.md
          verticalPadding: Style.spacing.xs
          onClicked: root.setFilter(modelData.id)
        }
      }
    }

    Item {
      id: header
      width: parent.width
      implicitHeight: Math.max(countText.implicitHeight, captureButton.implicitHeight)

      Text {
        id: countText
        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        textFormat: Text.PlainText
        text: Model.plural(root.rows.length, "event", "events")
          + (root.filter !== "all" ? " from " + root.filter : "") + " · newest first"
        color: root.dim
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
      }

      Button {
        id: captureButton
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        text: "Capture now"
        tooltipText: "Key c"
        bordered: true
        foreground: root.foreground
        fontFamily: root.fontFamily
        fontSize: Style.font.caption
        verticalPadding: Style.spacing.xs
        onClicked: root.captureRequested()
      }
    }

    ListView {
      id: list
      width: parent.width
      height: Math.max(0, parent.height - chipFlow.height - header.height - parent.spacing * 2)
      clip: true
      spacing: Style.spacing.xxs
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
        id: delegateRoot

        required property var modelData
        required property int index

        readonly property bool firstOfDay: index === 0 || root.rows[index - 1].day !== modelData.day
        readonly property bool expanded: root.expandedId === modelData.id

        width: ListView.view.width
        spacing: Style.spacing.xxs

        PanelSectionHeader {
          visible: delegateRoot.firstOfDay
          text: delegateRoot.modelData.dayLabel.toUpperCase()
          foreground: root.foreground
          fontFamily: root.fontFamily
        }

        EventRow {
          width: parent.width
          row: delegateRoot.modelData
          expanded: delegateRoot.expanded
          members: delegateRoot.expanded && delegateRoot.modelData.txId !== ""
            ? Model.groupMembers(root.indexData, delegateRoot.modelData.txId) : []
          hasCursor: root.cursorActive && delegateRoot.index === root.cursor
          foreground: root.foreground
          accent: root.accent
          urgent: root.urgent
          muted: root.muted
          fontFamily: root.fontFamily
          onClicked: {
            root.cursor = delegateRoot.index
            root.cursorWanted()
            root.activate()
          }
          onHoveredRow: {
            root.cursor = delegateRoot.index
            root.cursorWanted()
          }
        }
      }
    }
  }

  Text {
    anchors.centerIn: parent
    visible: root.rows.length === 0
    textFormat: Text.PlainText
    text: root.indexData ? "No events from " + root.filter : "No index to show"
    color: root.dim
    font.family: root.fontFamily
    font.pixelSize: Style.font.bodySmall
  }
}
