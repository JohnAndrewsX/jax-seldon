pragma ComponentBehavior: Bound

import QtQuick
import qs.Commons
import qs.Ui
import "../components/desk"
import "../Model.js" as Model

// Section 4, Decisions (ADR-0034 §2, WP-123; the 0.1 Decisions tab,
// WP-023). The list: the logbook's decisions from index.decisions, newest
// first (Model.decisionRows), a proposed one with the accent stripe; above
// it the count and *New decision*. The detail of the selected decision:
// its title, status, date and file, and — when the index is contract v2 —
// the cases it names (`decisions[].cases`; a click goes to the case in
// Work); the index has no body, so the text is one click away in the
// editor. The sticky bar: *Accept* while it is proposed, *Open in editor*.
//
// Accept is the existing path (WP-123 Decisions 2): the engine accepts no
// decision itself; the user sets `status: accepted` in the frontmatter, so
// Accept opens the file as Open in editor does (`seldon open ADR-NNNN
// --editor --json`, id checked against the schema pattern) and the detail
// says what to change. Neither writes, so neither arms.
//
// New decision (`d` or the button) shows NewDecisionForm in the detail
// pane: Enter twice (or a click on Create) runs `seldon decide --no-edit
// --json -- <title>`; the service then opens the new decision, and the
// selection follows it once the index lists it. Esc or a click on a
// decision leaves the form and keeps the title; `d` brings it back.
//
// Keys: ↑/↓ j/k move, Enter shows the detail (stacked layout), `e` opens
// the selected decision in the editor (Accept's path too), `d` new
// decision.
ReadingSection {
  id: root

  property bool formOpen: false

  readonly property bool canWrite: !!root.service && root.service.canWrite
  readonly property var decision: Model.decisionDetail(root.current)
  readonly property var cases: root.current ? Model.decisionCases(root.index, root.current.id) : null
  readonly property var result: root.service ? root.service.decideResult : null
  readonly property var openResult: root.service ? root.service.openResult : null

  allRows: Model.decisionRows(root.index)
  searchFields: ["id", "title", "status", "date"]
  emptyText: !root.index ? "No index to show"
    : root.allRows.length > 0 ? "No decision matches the search."
    : "No decisions yet. New decision (key d) writes the first."
  detailTitle: root.formOpen ? "New decision" : root.decision ? root.decision.heading : ""
  showRow: !root.formOpen
  actions: root.decision ? root.decision.actions : []
  actionMeta: root.current ? root.current.id : ""
  editing: root.formOpen && form.editing
  rowTitle: function(r) { return r.title !== "" ? r.title : r.id }
  rowMeta: function(r) { return [r.id, r.status].filter(function(p) { return p !== "" }).join(" · ") }
  rowAside: function(r) { return r.date }
  rowStripe: function(r) { return r.status === "proposed" ? "attention" : "" }

  function openCurrent() {
    var row = root.current
    if (row && row.actionable && root.service) root.service.openInEditor(row.id)
    return true
  }

  function openForm() {
    if (!root.canWrite) return
    root.formOpen = true
    if (root.desk) root.desk.showDetail()
    Qt.callLater(form.focusTitle)
  }

  function closeForm() {
    root.formOpen = false
    if (root.desk) root.desk.takeKeys()
  }

  function textKey(t) {
    if (t === "e") return root.openCurrent()
    if (t === "d") {
      root.openForm()
      return true
    }
    return false
  }

  // Esc with the form open but not focused (a click elsewhere) closes it.
  function back() {
    if (!root.formOpen) return false
    root.closeForm()
    return true
  }

  function view() {
    var v = root.baseView()
    v.summary = summary.text
    v.result = resultLine.visible ? resultLine.text : ""
    v.openResult = root.openResult ? root.openResult.text : ""
    v.cases = root.cases === null ? null : root.cases.map(function(c) { return c.id })
    v.form = { open: root.formOpen, editing: form.editing, title: form.title, armed: form.armed, hint: form.hint, result: form.resultText }
    return v
  }

  // A click on a decision shows it: the form gives way, its title kept.
  onRowClicked: if (root.formOpen) root.closeForm()
  onActionTriggered: function(id) {
    if (id === "accept" || id === "open") root.openCurrent()
  }
  // A section change hides the form: give the keys back with it.
  onActiveChanged: if (!root.active && root.formOpen && form.editing && root.desk) root.desk.giveKeys()

  head: [
    Item {
      width: parent ? parent.width : 0
      height: Math.max(summary.implicitHeight, newButton.implicitHeight)

      Text {
        id: summary
        anchors.left: parent.left
        anchors.right: newButton.left
        anchors.rightMargin: Style.spacing.md
        anchors.verticalCenter: parent.verticalCenter
        textFormat: Text.PlainText
        text: root.index ? Model.decisionSummary(root.allRows) : ""
        color: Color.popups.text
        wrapMode: Text.Wrap
        font.family: Style.font.family
        font.pixelSize: Style.font.bodySmall
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
        selected: root.formOpen
        bordered: true
        foreground: Color.popups.text
        fontFamily: Style.font.family
        fontSize: Style.font.caption
        onClicked: root.formOpen ? root.closeForm() : root.openForm()
      }
    },
    // The engine's answer to the last new decision; progress and a refusal
    // show in the form.
    Text {
      id: resultLine
      width: parent ? parent.width : 0
      visible: text !== ""
      textFormat: Text.PlainText
      text: root.result && !root.formOpen && !root.result.pending ? root.result.text : ""
      color: root.result && !root.result.ok ? Color.urgent : Color.muted
      wrapMode: Text.Wrap
      font.family: Style.font.family
      font.pixelSize: Style.font.caption
    }
  ]

  NewDecisionForm {
    id: form
    width: parent.width
    visible: root.formOpen
    service: root.service
    onLeaveRequested: root.closeForm()
    onCreated: function(decisionId) {
      if (decisionId !== "") root.selectedId = decisionId
      root.closeForm()
    }
  }

  Column {
    visible: !root.formOpen && !!root.current
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
      text: root.current ? root.rowTitle(root.current) : ""
      color: Color.popups.text
      wrapMode: Text.Wrap
      font.family: Style.font.family
      font.pixelSize: Style.font.title
      font.bold: true
      font.strikeout: !!root.current && root.current.status === "superseded"
    }

    Text {
      width: parent.width
      visible: text !== ""
      textFormat: Text.PlainText
      text: root.decision ? root.decision.note : ""
      color: root.current && root.current.status === "proposed" ? Color.accent : Color.muted
      wrapMode: Text.Wrap
      font.family: Style.font.family
      font.pixelSize: Style.font.bodySmall
    }

    KeyValues {
      width: parent.width
      rows: root.decision ? root.decision.rows : []
    }

    // Contract v2: the cases this decision names; hidden on contract 1.
    Column {
      visible: root.cases !== null
      width: parent.width
      spacing: Style.spacing.md

      Text {
        width: parent.width
        textFormat: Text.PlainText
        text: "CASES · " + (root.cases ? root.cases.length : 0)
        color: Color.muted
        font.family: Style.font.family
        font.pixelSize: Style.font.caption
        font.letterSpacing: Style.space(1)
        font.bold: true
      }

      Text {
        visible: !!root.cases && root.cases.length === 0
        width: parent.width
        textFormat: Text.PlainText
        text: "This decision names no case."
        color: Color.muted
        font.family: Style.font.family
        font.pixelSize: Style.font.bodySmall
      }

      Repeater {
        model: root.cases || []

        ListRow {
          required property var modelData
          width: parent.width
          title: modelData.title !== "" ? modelData.title : modelData.id
          meta: modelData.title !== "" ? modelData.id + (modelData.status !== "" ? " · " + modelData.status : "") : "not in the index"
          onClicked: if (root.desk && modelData.actionable) {
            root.desk.section("work")
            root.desk.select(modelData.id)
          }
        }
      }
    }

    Text {
      width: parent.width
      textFormat: Text.PlainText
      text: root.decision ? root.decision.lead : ""
      color: Color.muted
      wrapMode: Text.Wrap
      font.family: Style.font.family
      font.pixelSize: Style.font.bodySmall
    }
  }
}
