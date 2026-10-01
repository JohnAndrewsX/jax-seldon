pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import qs.Commons
import qs.Ui
import "../Model.js" as Model

// Today tab (SPEC-PLUGIN §5): the summary counts, the QuickEntry, today's
// journal entries and yesterday's, collapsed behind one row. Data from
// Model.todayView().
//
// Keyboard (forwarded by Panel.qml): ↑/↓ move the cursor, Enter on the
// yesterday row opens or closes it, `e` opens today's journal in the editor.
// `n` (Panel.qml, from any tab) focuses the QuickEntry.
Item {
  id: root

  property var service: null
  property var indexData: null
  property bool cursorActive: false
  property color foreground: Color.foreground
  property color accent: Color.accent
  property color urgent: Color.urgent
  property string fontFamily: Style.font.family

  property bool yesterdayOpen: false
  property int cursor: 0

  signal openJournalRequested()
  signal cursorWanted()
  // QuickEntry hands the keys back (Esc in the field).
  signal leaveRequested()

  readonly property bool editing: quickEntry.editing
  property alias quickEntry: quickEntry

  readonly property var view: Model.todayView(indexData)
  readonly property color dim: Util.alpha(foreground, 0.65)
  // Entries of today, the yesterday toggle, and yesterday's entries when open.
  readonly property var rows: {
    var out = []
    for (var i = 0; i < view.entries.length; i++) out.push({ type: "entry", entry: view.entries[i] })
    if (view.entries.length === 0) out.push({ type: "empty" })
    if (view.yesterday.length > 0) out.push({ type: "toggle" })
    if (yesterdayOpen)
      for (var j = 0; j < view.yesterday.length; j++) out.push({ type: "entry", entry: view.yesterday[j] })
    return out
  }
  readonly property int rowCount: rows.length

  function move(dy) {
    if (root.rows.length === 0) return
    root.cursor = Math.max(0, Math.min(root.rows.length - 1, root.cursor + dy))
  }

  function activate() {
    var row = root.rows[root.cursor]
    if (row && row.type === "toggle") root.yesterdayOpen = !root.yesterdayOpen
  }

  function textKey(t) {
    if (t === "e") {
      root.openJournalRequested()
      return true
    }
    return false
  }

  onRowsChanged: if (root.cursor >= root.rows.length) root.cursor = Math.max(0, root.rows.length - 1)

  // The heading sits outside the list: a ListView header scrolls away when
  // the model is replaced (a new index), which hid it on the test host.
  Column {
    anchors.fill: parent
    spacing: Style.spacing.lg

    Column {
      id: head
      width: parent.width
      spacing: Style.spacing.lg

      Item {
        width: parent.width
        implicitHeight: Math.max(title.implicitHeight, editButton.implicitHeight)

        Text {
          id: title
          anchors.left: parent.left
          anchors.verticalCenter: parent.verticalCenter
          textFormat: Text.PlainText
          text: root.view.title
          color: root.foreground
          font.family: root.fontFamily
          font.pixelSize: Style.font.subtitle
          font.bold: true
        }

        Button {
          id: editButton
          anchors.right: parent.right
          anchors.verticalCenter: parent.verticalCenter
          text: "Open in editor"
          tooltipText: "Key e"
          bordered: true
          foreground: root.foreground
          fontFamily: root.fontFamily
          fontSize: Style.font.caption
          verticalPadding: Style.spacing.xs
          onClicked: root.openJournalRequested()
        }
      }

      Flow {
        width: parent.width
        spacing: Style.spacing.xl
        visible: root.indexData !== null

        Repeater {
          model: root.view.stats

          Row {
            id: stat

            required property var modelData

            spacing: Style.spacing.sm

            Text {
              anchors.baseline: statLabel.baseline
              textFormat: Text.PlainText
              text: String(stat.modelData.value)
              color: root.foreground
              font.family: root.fontFamily
              font.pixelSize: Style.font.title
              font.bold: true
            }

            Text {
              id: statLabel
              textFormat: Text.PlainText
              text: stat.modelData.label
              color: root.dim
              font.family: root.fontFamily
              font.pixelSize: Style.font.caption
            }
          }
        }
      }

      QuickEntry {
        id: quickEntry
        width: parent.width
        service: root.service
        indexData: root.indexData
        foreground: root.foreground
        urgent: root.urgent
        fontFamily: root.fontFamily
        onLeaveRequested: root.leaveRequested()
      }

      PanelSectionHeader {
        text: "JOURNAL"
        foreground: root.foreground
        fontFamily: root.fontFamily
      }
    }

    ListView {
      id: list
      width: parent.width
      height: Math.max(0, parent.height - head.height - parent.spacing)
      clip: true
      spacing: Style.spacing.sm
      boundsBehavior: Flickable.StopAtBounds
      interactive: contentHeight > height
      ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }

      model: root.rows
      currentIndex: root.cursorActive ? root.cursor : -1
      onCurrentIndexChanged: if (currentIndex >= 0) Qt.callLater(keepCurrentVisible)
      // A new model (the yesterday row opened, a new index) starts at the top;
      // keep the cursor row in view.
      onCountChanged: if (currentIndex >= 0) Qt.callLater(keepCurrentVisible)
      function keepCurrentVisible() {
        if (currentIndex >= 0) positionViewAtIndex(currentIndex, ListView.Contain)
      }

      delegate: CursorSurface {
        id: rowItem

        required property var modelData
        required property int index

        width: ListView.view.width
        implicitHeight: rowColumn.implicitHeight + Style.spacing.md * 2
        hasCursor: root.cursorActive && index === root.cursor
        foreground: root.foreground
        accent: root.accent

        Column {
          id: rowColumn
          x: Style.spacing.lg
          y: Style.spacing.md
          width: parent.width - Style.spacing.lg * 2
          spacing: Style.spacing.xxs

          Text {
            width: parent.width
            visible: rowItem.modelData.type === "entry"
            textFormat: Text.PlainText
            text: rowItem.modelData.type === "entry" ? Model.entryMeta(rowItem.modelData.entry) : ""
            color: root.dim
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
          }

          Text {
            width: parent.width
            textFormat: Text.PlainText
            text: rowItem.modelData.type === "entry" ? rowItem.modelData.entry.text
              : rowItem.modelData.type === "toggle"
                ? (root.yesterdayOpen ? "▾ " : "▸ ") + "Yesterday · " + Model.plural(root.view.yesterday.length, "entry", "entries")
                : root.indexData ? "Nothing in today's journal yet." : "No index to show"
            color: rowItem.modelData.type === "entry" ? root.foreground : root.dim
            wrapMode: Text.Wrap
            font.family: root.fontFamily
            font.pixelSize: rowItem.modelData.type === "entry" ? Style.font.body : Style.font.bodySmall
          }
        }

        MouseArea {
          anchors.fill: parent
          hoverEnabled: true
          onEntered: {
            root.cursor = rowItem.index
            root.cursorWanted()
          }
          onClicked: {
            root.cursor = rowItem.index
            root.activate()
          }
        }
      }
    }
  }
}
