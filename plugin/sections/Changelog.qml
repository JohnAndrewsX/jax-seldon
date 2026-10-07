pragma ComponentBehavior: Bound

import QtQuick
import qs.Commons
import qs.Ui
import "../components/desk"
import "../Model.js" as Model

// Section 2, Changelog (ADR-0034 §2, SPEC-PLUGIN §5.4; prototype
// `changelog`). The list: the slot of "Agent sorts N open changes"
// (WP-124b), the chips open · crisis · attention · routine · in case · all
// with their counts, the count line with *Ledger* and *Capture now*, the
// quiet "N changes without a case" (ADR-0028 §4b: dim, no colour), "+N
// more … not listed here" (ADR-0020), "N hidden this session · Show", the
// last capture's answer; then the index's events newest first, by day,
// filtered by the chip and the sidebar search, a stripe on open drift
// (crisis urgent, attention accent). The detail: the selected event
// (EventDetail.qml) with its sticky bar and the inline link / explain /
// dismiss form.
//
// The rows come from Service.deskChangelog (built once per index); the
// selection is an event id, so it stays on its event when a new index adds
// rows above it, and stays shown when its event leaves the chip (it was
// just linked) — the list then has no highlighted row and ↑/↓ continue
// from where it was. A new chip starts at its first row.
//
// Keys: ↑/↓ j/k move, Enter opens the selected open item's default form
// (Link when the engine proposes a case, else Explain), `f` / `F` the next
// / previous chip, `e` this month's ledger in the editor, Esc hides a
// shown form (its draft kept).
Section {
  id: root

  property string chip: Model.CHANGELOG_CHIP_DEFAULT
  // The row number of the selection, for ↑/↓ once it has left the list.
  property int cursorRow: 0
  // A chip set by select(): it keeps the selection instead of the top row.
  property bool chipKeepsSelection: false
  // Created: the chip's first value is no chip change.
  property bool made: false

  readonly property var prepared: root.service ? root.service.deskChangelog : null
  readonly property var hidden: root.service ? root.service.deskHidden : ({})
  readonly property var rows: Model.changelogView(root.prepared, root.chip, root.hidden, root.searchText)
  readonly property var chips: Model.changelogChips(root.prepared, root.hidden)
  readonly property int cursor: root.rowIndex(root.selectedId)
  readonly property int hiddenCount: Model.hiddenCount(root.prepared, root.hidden)
  readonly property string attentionText: Model.attentionText(root.index)
  readonly property string moreText: Model.moreDriftText(root.index)
  readonly property bool capturing: !!root.service && root.service.capturing
  readonly property var captureResult: root.service ? root.service.captureResult : null
  readonly property color foreground: Color.popups.text
  readonly property color dim: Util.alpha(root.foreground, 0.65)

  editing: detail.editing

  function rowIndex(id) {
    var rows = root.rows || []
    for (var i = 0; i < rows.length; i++) if (rows[i].id === id) return i
    return -1
  }

  function selectRow(i) {
    var rows = root.rows || []
    if (rows.length === 0) return
    var at = Math.max(0, Math.min(rows.length - 1, i))
    root.cursorRow = at
    root.selectedId = rows[at].id
  }

  function move(dy) {
    if (root.rows.length === 0) return true
    var from = root.cursor >= 0 ? root.cursor : dy > 0 ? root.cursorRow - 1 : root.cursorRow
    root.selectRow(from + dy)
    return true
  }

  function activate() {
    if (root.stacked && !root.detailShown) {
      if (root.desk) root.desk.showDetail()
      return true
    }
    return detail.activate()
  }

  function setChip(id) {
    if (!Model.isChangelogChip(id)) return
    root.chip = id
  }

  function textKey(t) {
    if (t === "f" || t === "F") {
      root.setChip(Model.cycleChip(root.chip, t === "F" ? -1 : 1))
      return true
    }
    if (t === "e") {
      if (root.service) root.service.openInEditor("ledger")
      return true
    }
    return false
  }

  // Select an event by id (IPC `select`, the shim's `resolve`): in this
  // chip when it has it, else under "all".
  function select(id) {
    var target = String(id)
    if (!Model.changelogRow(root.prepared, target)) return false
    if (root.rowIndex(target) === -1 && root.chip !== "all") {
      root.chipKeepsSelection = true
      root.chip = "all"
    }
    root.selectedId = target
    var i = root.rowIndex(target)
    if (i >= 0) root.cursorRow = i
    if (root.desk) root.desk.showDetail()
    return true
  }

  function back() {
    return detail.back()
  }

  // { filter }: a chip id, or (the 0.1 source filter) a source name, which
  // shows "all" with the source in the sidebar search.
  function applyPayload(p) {
    var f = p && typeof p.filter === "string" ? p.filter : ""
    if (f === "") return
    if (Model.isChangelogChip(f)) {
      root.setChip(f)
    } else if (Model.SOURCES.indexOf(f) !== -1) {
      root.setChip("all")
      if (root.desk) root.desk.searchText = f
    }
  }

  function view() {
    return {
      chip: root.chip,
      chips: root.chips.map(function(c) { return c.id + " " + c.count }),
      rows: root.rows.length,
      cursor: root.cursor,
      selected: root.selectedId,
      first: root.rows.slice(0, 12).map(function(r) { return r.title }),
      stripes: root.rows.filter(function(r) { return r.stripe !== "" }).map(function(r) { return r.subject + " " + r.stripe }),
      badges: root.rows.filter(function(r) { return r.badge !== "" }).map(function(r) { return r.subject + " " + r.badge }),
      resolved: root.rows.filter(function(r) { return r.resolution !== "" }).map(function(r) { return r.subject + ": " + Model.rowStatus(r) }),
      attention: root.attentionText,
      attentionDim: String(attentionLine.color) === String(root.dim),
      more: root.moreText,
      hidden: root.hiddenCount,
      capturing: root.capturing,
      captureResult: root.captureResult ? root.captureResult.text : "",
      triageSlot: triageSlot.visible,
      detail: detail.view()
    }
  }

  // A new chip starts at its first row (once its rows are there); a new
  // index or search keeps the selection by id; an empty selection takes
  // the first row.
  onChipChanged: if (root.made) Qt.callLater(function() {
    if (root.chipKeepsSelection) {
      root.chipKeepsSelection = false
      var i = root.rowIndex(root.selectedId)
      if (i >= 0) root.cursorRow = i
      return
    }
    root.selectRow(0)
    list.view.positionViewAtBeginning()
  })
  onRowsChanged: {
    var i = root.rowIndex(root.selectedId)
    if (i >= 0) root.cursorRow = i
    else if (root.selectedId === "" || !Model.changelogRow(root.prepared, root.selectedId)) root.selectRow(root.cursorRow)
  }
  Component.onCompleted: {
    root.made = true
    if (root.selectedId === "") root.selectRow(0)
  }
  // Another section shown: a shown form gives the keys back (a hidden
  // field keeps Qt's focus otherwise); its draft stays.
  onActiveChanged: if (!root.active && detail.editing) detail.back()

  ListColumn {
    id: list
    visible: !root.stacked || !root.detailShown
    width: root.stacked ? root.width : (root.layout ? root.layout.listW : 0)
    height: root.height
    divider: !root.stacked
    title: "Changelog"
    model: root.rows
    currentIndex: root.cursor
    emptyText: !root.index ? "No index to show"
      : root.searchText !== "" ? "Nothing here matches “" + root.searchText + "”."
      : root.chip === "open" ? "Nothing open: every change is in a case or routine."
      : "No events in " + Model.EVENT_CLASS_LABELS[root.chip === "case" ? "case" : root.chip] + "."

    // "Agent sorts N open changes" (ADR-0034 §6): WP-124b fills the slot.
    Item {
      id: triageSlot
      objectName: "triageSlot"
      width: parent.width
      visible: children.length > 0
      implicitHeight: childrenRect.height
    }

    Flow {
      width: parent.width
      spacing: Style.spacing.sm

      Repeater {
        model: root.chips

        Button {
          required property var modelData

          text: modelData.label + " " + modelData.count
          selected: modelData.id === root.chip
          bordered: true
          foreground: modelData.count > 0 || selected ? root.foreground : root.dim
          fontFamily: Style.font.family
          fontSize: Style.font.caption
          horizontalPadding: Style.spacing.md
          verticalPadding: Style.spacing.xs
          onClicked: root.setChip(modelData.id)
        }
      }
    }

    Item {
      width: parent.width
      implicitHeight: Math.max(countText.implicitHeight, captureButton.implicitHeight)

      Text {
        id: countText
        anchors.left: parent.left
        anchors.right: ledgerButton.left
        anchors.rightMargin: Style.spacing.sm
        anchors.verticalCenter: parent.verticalCenter
        textFormat: Text.PlainText
        text: Model.plural(root.rows.length, "event", "events") + " · newest first"
        color: root.dim
        wrapMode: Text.Wrap
        font.family: Style.font.family
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
        fontFamily: Style.font.family
        fontSize: Style.font.caption
        verticalPadding: Style.spacing.xs
        onClicked: if (root.service) root.service.openInEditor("ledger")
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
        fontFamily: Style.font.family
        fontSize: Style.font.caption
        verticalPadding: Style.spacing.xs
        // Also while it reads Capturing: during a lock-retry wait a click
        // captures at once (Service.captureNow ignores a click while a
        // capture is queued), as the bar and the `c` key do (WP-078).
        onClicked: if (root.service) root.service.captureNow()
      }
    }

    Text {
      id: attentionLine
      width: parent.width
      visible: text !== ""
      textFormat: Text.PlainText
      text: root.attentionText
      color: root.dim
      wrapMode: Text.Wrap
      font.family: Style.font.family
      font.pixelSize: Style.font.caption
    }

    Text {
      width: parent.width
      visible: text !== ""
      textFormat: Text.PlainText
      text: root.moreText
      color: root.dim
      wrapMode: Text.Wrap
      font.family: Style.font.family
      font.pixelSize: Style.font.caption
    }

    Item {
      width: parent.width
      visible: root.hiddenCount > 0
      implicitHeight: Math.max(hiddenText.implicitHeight, showButton.implicitHeight)

      Text {
        id: hiddenText
        anchors.left: parent.left
        anchors.right: showButton.left
        anchors.rightMargin: Style.spacing.sm
        anchors.verticalCenter: parent.verticalCenter
        textFormat: Text.PlainText
        text: Model.plural(root.hiddenCount, "change", "changes") + " hidden this session"
        color: root.dim
        wrapMode: Text.Wrap
        font.family: Style.font.family
        font.pixelSize: Style.font.caption
      }

      Button {
        id: showButton
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        text: "Show"
        bordered: true
        foreground: root.foreground
        fontFamily: Style.font.family
        fontSize: Style.font.caption
        verticalPadding: Style.spacing.xs
        onClicked: if (root.service) root.service.deskHidden = ({})
      }
    }

    Text {
      width: parent.width
      visible: text !== ""
      textFormat: Text.PlainText
      text: root.captureResult && !root.captureResult.pending ? "Last capture: " + root.captureResult.text : ""
      color: root.captureResult && !root.captureResult.ok ? Color.urgent : root.dim
      elide: Text.ElideRight
      font.family: Style.font.family
      font.pixelSize: Style.font.caption
    }

    delegate: GroupedRow {
      required property var modelData
      required property int index

      width: ListView.view.width
      header: index === 0 || (root.rows[index - 1] || {}).day !== modelData.day ? modelData.dayLabel : ""
      title: modelData.title
      meta: modelData.listMeta
      aside: modelData.age
      stripe: modelData.stripe
      selected: modelData.id === root.selectedId
      cursor: false
      onClicked: {
        root.selectedId = modelData.id
        root.cursorRow = index
        if (root.desk) root.desk.showDetail()
      }
    }
  }

  EventDetail {
    id: detail
    x: root.stacked ? 0 : list.width
    width: root.width - x
    height: root.height
    visible: !root.stacked || root.detailShown
    backVisible: root.stacked
    section: root
    eventId: root.selectedId
    onBackRequested: if (root.desk) root.desk.back()
    onLeaveRequested: if (root.desk) root.desk.takeKeys()
  }
}
