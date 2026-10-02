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
// Keyboard (forwarded by Panel.qml): ↑/↓ move the cursor, Enter on an open
// drift row opens the DriftSheet (link / explain / dismiss, WP-021), on any
// other row expands it (full text), f / F cycle the source filter, `e` opens
// this month's ledger in the editor. *Capture now* spins while the capture
// and the status after it run; the new rows arrive with the index
// (Service.qml's FileView), not from the capture's output. So do resolved
// drift rows: they show the folded resolution once the engine has rebuilt
// the index. While the sheet is open it takes the tab's place; when the
// index lists fewer drift items than it counts (ADR-0020) a line says how
// many more there are.
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
  property bool capturing: false
  property var captureResult: null

  property string filter: "all"
  property int cursor: 0
  property string expandedId: ""
  property bool sheetOpen: false
  property alias sheet: sheet

  signal captureRequested()
  signal openLedgerRequested()
  signal cursorWanted()
  // The sheet gives the keys back (Esc, Cancel, Close).
  signal leaveRequested()

  readonly property var rows: Model.changelogRows(indexData, filter)
  readonly property var chips: Model.filterChips(indexData)
  readonly property int rowCount: rows.length
  readonly property string moreDrift: Model.moreDriftText(indexData)
  readonly property bool editing: sheetOpen && sheet.editing
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
    if (row.drift) root.openSheet(row.id)
    else root.expandedId = root.expandedId === row.id ? "" : row.id
  }

  // The drift sheet for event `id` (an open drift row, or a group member).
  function openSheet(id) {
    if (!Model.driftItemFor(root.indexData, id)) return false
    root.sheetOpen = true
    sheet.openFor(id)
    return true
  }

  // The crisis strip: the first crisis, with the cursor on its row.
  function openCrisis() {
    var id = Model.firstCrisis(root.indexData)
    if (id === "") return false
    root.setFilter("all")
    for (var i = 0; i < root.rows.length; i++) {
      if (root.rows[i].id === id) {
        root.cursor = i
        break
      }
    }
    return root.openSheet(id)
  }

  function closeSheet() {
    root.sheetOpen = false
    root.leaveRequested()
  }

  function setFilter(id) {
    root.filter = id
  }

  function textKey(t) {
    if (t === "f" || t === "F") {
      root.setFilter(Model.cycleFilter(root.filter, t === "F" ? -1 : 1))
      return true
    }
    if (t === "e") {
      root.openLedgerRequested()
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
  // Another tab shown (a click on the tab strip, IPC): the sheet stays open
  // but gives the keys back.
  onVisibleChanged: if (!visible && root.editing) root.leaveRequested()

  Column {
    anchors.fill: parent
    spacing: Style.spacing.lg
    visible: !root.sheetOpen

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

      // Wraps instead of eliding, so the sort order is never cut off
      // (WP-039).
      Text {
        id: countText
        objectName: "changelogHeader"
        anchors.left: parent.left
        anchors.right: ledgerButton.left
        anchors.rightMargin: Style.spacing.sm
        anchors.verticalCenter: parent.verticalCenter
        textFormat: Text.PlainText
        text: Model.plural(root.rows.length, "event", "events")
          + (root.filter !== "all" ? " from " + root.filter : "") + " · newest first"
        color: root.dim
        wrapMode: Text.Wrap
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
      }

      Button {
        id: ledgerButton
        anchors.right: captureButton.left
        anchors.rightMargin: Style.spacing.sm
        anchors.verticalCenter: parent.verticalCenter
        text: "Ledger"
        tooltipText: "Open this month's ledger in the editor (key e)"
        bordered: true
        foreground: root.foreground
        fontFamily: root.fontFamily
        fontSize: Style.font.caption
        verticalPadding: Style.spacing.xs
        onClicked: root.openLedgerRequested()
      }

      Button {
        id: captureButton
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        text: root.capturing ? "Capturing" : "Capture now"
        iconText: root.capturing ? "󰦖" : ""
        iconSpinning: root.capturing
        iconSize: Style.font.caption
        tooltipText: "Key c"
        bordered: true
        foreground: root.foreground
        fontFamily: root.fontFamily
        fontSize: Style.font.caption
        verticalPadding: Style.spacing.xs
        onClicked: if (!root.capturing) root.captureRequested()
      }
    }

    Text {
      id: moreLine
      width: parent.width
      visible: text !== ""
      textFormat: Text.PlainText
      text: root.moreDrift
      color: root.dim
      elide: Text.ElideRight
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
    }

    Text {
      id: captureLine
      width: parent.width
      visible: text !== ""
      textFormat: Text.PlainText
      text: root.captureResult && !root.captureResult.pending ? "Last capture: " + root.captureResult.text : ""
      color: root.captureResult && !root.captureResult.ok ? root.urgent : root.dim
      elide: Text.ElideRight
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
    }

    ListView {
      id: list
      width: parent.width
      height: Math.max(0, parent.height - chipFlow.height - header.height - parent.spacing * 2
        - (captureLine.visible ? captureLine.height + parent.spacing : 0)
        - (moreLine.visible ? moreLine.height + parent.spacing : 0))
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
        // An expanded row grows after its layout pass; keep all of it in view.
        onHeightChanged: if (delegateRoot.index === list.currentIndex) Qt.callLater(list.keepCurrentVisible)

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
          onResolveRequested: {
            root.cursor = delegateRoot.index
            root.cursorWanted()
            root.openSheet(delegateRoot.modelData.id)
          }
        }
      }
    }
  }

  // The sheet in the tab's place, scrollable if a long group needs it.
  Flickable {
    id: sheetView
    anchors.fill: parent
    visible: root.sheetOpen
    clip: true
    contentWidth: width
    contentHeight: sheet.implicitHeight
    boundsBehavior: Flickable.StopAtBounds
    interactive: contentHeight > height
    ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }

    DriftSheet {
      id: sheet
      width: sheetView.width
      visible: root.sheetOpen
      service: root.service
      indexData: root.indexData
      foreground: root.foreground
      accent: root.accent
      urgent: root.urgent
      muted: root.muted
      fontFamily: root.fontFamily
      onLeaveRequested: root.closeSheet()
    }
  }

  Text {
    anchors.centerIn: parent
    visible: root.rows.length === 0 && !root.sheetOpen
    textFormat: Text.PlainText
    text: root.indexData ? "No events from " + root.filter : "No index to show"
    color: root.dim
    font.family: root.fontFamily
    font.pixelSize: Style.font.bodySmall
  }
}
