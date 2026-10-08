pragma ComponentBehavior: Bound

import QtQuick
import qs.Commons
import qs.Ui
import "../components/desk"
import "../Model.js" as Model

// Section 3, Work (ADR-0034 §2, SPEC-PLUGIN §5.4; prototype `work`). The
// list: the one-sentence start (WP-101, ADR-0027 §6) — an intent field and
// *Run*, which sends `seldon agent start --new --json -- <intent>` (one
// argument after `--`, exactly as typed), keeps the text until the engine
// has made the case and then selects it; the WIP line ("2 / 3 active")
// against the bar setting `wipLimit` (warns, never blocks); *By agent*,
// the Completed group's spot-check filter (ADR-0027 §5); *New case*, the
// hand-made case (NewCaseSheet.qml in the detail); the engine's answer to
// the last case action; then the cases by group — Active · Verification ·
// Queued · Completed (completed and dropped, the index's last 50) — with
// id · risk · area, "closed by agent", "reopens …" and the plan's steps.
//
// The detail: the selected case with its sticky bar by status
// (Model.caseDeskActions: Start / Hand to agent / To verification /
// Complete / Drop / Reopen / Open in editor; id · risk at the right), then
// what the index carries (Model.caseDetail): key/values (an imported
// case's source among them), the first paragraph of Intent and Result as
// plain text (ADR-0038), the plan's progress, the log, the linked changes;
// the rest is in the case file, which *Open in editor* shows (the plugin
// never reads Markdown, AGENTS.md §3).
//
// Writing actions arm on the first press or click and run on the second
// (the desk's Arm.qml; the bar reads "Confirm …" and shows the hint);
// Reopen runs at once (it creates a case and destroys nothing). Every call
// goes through Service.plan(), startAgent(), startAgentNew() or
// openInEditor() with a fixed argument list (Model.planArgs, agentArgs,
// agentNewArgs, openArgs). The moved case arrives with the next index and
// the selection follows it by id, or goes to the case a Run, a reopen or
// the sheet made.
//
// Keys: ↑/↓ j/k move, Enter the first action that launches nothing (twice;
// on an active case To verification — Enter never starts an agent), `a`
// Hand to agent (twice), `x` Drop (twice), `r` Reopen, `e` Open in editor, `i` the intent
// field, `+` the new-case sheet; Esc leaves a field or closes the sheet
// (its draft kept). Any other key, a new selection or a new index disarms.
Section {
  id: root

  property bool sheetOpen: false
  // "" or Model.COMPLETED_FILTER_AGENT.
  property string completedFilter: ""
  property string sentIntent: ""

  readonly property var prepared: root.service ? root.service.deskWork : null
  readonly property var work: Model.workView(root.prepared, root.completedFilter, root.searchText)
  readonly property var rows: root.work.rows
  readonly property int cursor: root.rowIndex(root.selectedId)
  readonly property var detailData: Model.caseDetail(root.index, root.prepared, root.selectedId)
  readonly property var current: root.detailData ? root.detailData.row : null
  readonly property var caseActions: Model.caseDeskActions(root.current)
  readonly property var wip: Model.wipStatus(root.index, root.desk ? root.desk.entry.wipLimit : undefined)
  readonly property var result: root.service ? root.service.planResult : null
  readonly property bool pending: !!root.result && root.result.pending
  readonly property bool canWrite: !!root.service && root.service.canWrite
  readonly property bool running: root.pending && !!root.result && root.result.action === "agent-new"
  readonly property string armPrefix: root.current ? "case:" + root.current.id + ":" : "case:"
  readonly property string armed: root.arm && root.arm.armedId.indexOf(root.armPrefix) === 0
    ? root.arm.armedId.slice(root.armPrefix.length) : ""
  readonly property color foreground: Color.popups.text
  readonly property color dim: Util.alpha(root.foreground, 0.65)

  editing: intentField.activeFocus || (root.sheetOpen && sheet.editing)

  function rowIndex(id) {
    var rows = root.rows || []
    for (var i = 0; i < rows.length; i++) if (rows[i].id === id) return i
    return -1
  }

  function disarm() {
    if (root.arm && root.arm.armedId.indexOf("case:") === 0) root.arm.disarm()
  }

  function move(dy) {
    if (root.rows.length === 0) return true
    var from = root.cursor >= 0 ? root.cursor : dy > 0 ? -1 : root.rows.length
    root.selectedId = root.rows[Math.max(0, Math.min(root.rows.length - 1, from + dy))].id
    return true
  }

  // Press an action of the selected case (a key or a click): open runs at
  // once, Reopen too; the other writing actions arm, then run.
  function press(actionId) {
    var c = root.current
    if (actionId === "ask") return !!c && !!root.service && root.service.askAgent("case", c.id)
    var action = Model.caseDeskAction(c, actionId)
    if (!action) return false
    if (!action.write) {
      root.disarm()
      return root.service ? root.service.openInEditor(c.id) : false
    }
    if (!root.canWrite || root.pending) return false
    if (!action.arm) return root.runAction(actionId)
    if (!root.arm || !root.arm.press(root.armPrefix + actionId, Model.caseArmHint(action, c.id))) return false
    return root.runAction(actionId)
  }

  function runAction(actionId) {
    var c = root.current
    root.disarm()
    if (!c || !root.service) return false
    if (actionId === "agent") return root.service.startAgent(c.id)
    var verb = Model.caseActionVerb(actionId)
    return verb !== "" ? root.service.plan(verb, c.id) : false
  }

  function activate() {
    if (root.stacked && !root.detailShown) {
      if (root.desk) root.desk.showDetail()
      return true
    }
    var enter = Model.caseEnterAction(root.current)
    if (root.sheetOpen || !enter) return false
    return root.press(enter.id) || true
  }

  function runIntent() {
    if (!root.service || root.pending) return false
    var sent = root.service.startAgentNew(intentField.text)
    if (sent) root.sentIntent = intentField.text
    return sent
  }

  function focusIntent() {
    root.disarm()
    if (root.sheetOpen) root.closeSheet()
    if (root.stacked && root.desk) root.desk.back()
    Qt.callLater(function() { intentField.forceActiveFocus() })
  }

  function openSheet() {
    root.disarm()
    if (!root.canWrite) return
    root.sheetOpen = true
    if (root.stacked && root.desk) root.desk.showDetail()
    Qt.callLater(sheet.focusTitle)
  }

  function closeSheet() {
    root.sheetOpen = false
    if (root.desk) root.desk.takeKeys()
  }

  function toggleCompletedFilter() {
    root.completedFilter = root.completedFilter === "" ? Model.COMPLETED_FILTER_AGENT : ""
  }

  function textKey(t) {
    if (t === "e") {
      root.press("open")
      return true
    }
    if (t === "a") {
      if (Model.caseDeskAction(root.current, "agent")) root.press("agent")
      return true
    }
    if (t === "x") {
      if (Model.caseDeskAction(root.current, "drop")) root.press("drop")
      return true
    }
    if (t === "r") {
      if (Model.caseDeskAction(root.current, "reopen")) root.press("reopen")
      return true
    }
    if (t === "i") {
      if (root.canWrite) root.focusIntent()
      return true
    }
    if (t === "+") {
      root.openSheet()
      return true
    }
    return false
  }

  function select(id) {
    if (!Model.findWorkRow(root.prepared, String(id))) return false
    if (root.rowIndex(String(id)) === -1) root.completedFilter = ""
    root.sheetOpen = false
    root.selectedId = String(id)
    if (root.desk) root.desk.showDetail()
    return true
  }

  function back() {
    if (!root.sheetOpen) return false
    root.closeSheet()
    return true
  }

  // The bar: the status actions, "Confirm …" on the armed one.
  function barActions() {
    if (root.sheetOpen) return []
    var out = root.caseActions.map(function(a) {
      return {
        id: a.id,
        label: root.armed === a.id ? "Confirm " + a.label.toLowerCase() : a.label,
        primary: a.primary,
        enabled: a.write ? root.canWrite && !root.pending : true
      }
    })
    // Ask agent (WP-124b, ADR-0036 §1): about this case, any status; the
    // agent gets no case to work (`agent ask case <id> --json`).
    if (root.current && root.service && root.service.askAgentAvailable)
      out.push({ id: "ask", label: "Ask agent", primary: false,
        enabled: !(root.service.askResult && root.service.askResult.pending) })
    return out
  }

  // The last ask about this case, or null.
  readonly property var askResult: root.service && root.current && root.service.askResult
    && root.service.askResult.what === "case" && root.service.askResult.target === root.current.id
    ? root.service.askResult : null

  function view() {
    return {
      groups: Model.WORK_GROUPS.map(function(g) { return g.id + " " + root.work.counts[g.id] }),
      labels: Model.WORK_GROUPS.filter(function(g) { return root.work.counts[g.id] > 0 })
        .map(function(g) { return root.work.labels[g.id] }),
      ids: root.rows.map(function(r) { return r.id }),
      filter: root.completedFilter,
      wip: root.wip.text,
      cursor: root.cursor,
      selected: root.selectedId,
      intent: intentField.text,
      intentEditing: intentField.activeFocus,
      pending: root.pending,
      running: root.running,
      result: root.result ? root.result.text : "",
      resultOk: !!root.result && root.result.ok,
      case: root.detailData ? {
        id: root.detailData.id,
        status: root.detailData.status,
        heading: root.detailData.heading,
        actions: detail.actions.map(function(a) { return a.label }),
        ask: root.askResult ? root.askResult.text : "",
        askOk: root.askResult ? root.askResult.ok : true,
        armed: root.armed,
        hint: detail.hint,
        kv: root.detailData.kv.map(function(r) { return r[0] + ": " + r[1] }),
        plan: root.detailData.plan.text,
        log: root.detailData.log.length,
        linked: root.detailData.linked.length,
        linkedMore: root.detailData.linkedMore,
        intent: root.detailData.intent,
        result: root.detailData.result
      } : null,
      sheet: {
        open: root.sheetOpen,
        editing: sheet.editing,
        title: sheet.title,
        zone: sheet.zone,
        risk: sheet.risk,
        priority: sheet.priority,
        area: sheet.area,
        result: sheet.resultText
      },
      bar: { y: Math.round(detail.actionBar.mapToItem(detail, 0, 0).y), sceneY: Math.round(detail.actionBar.mapToItem(null, 0, 0).y),
        h: Math.round(detail.actionBar.height), visible: detail.actionBar.visible },
      scroll: { y: Math.round(detail.flickable.contentY), h: Math.round(detail.flickable.contentHeight),
        view: Math.round(detail.flickable.height) }
    }
  }

  onSelectedIdChanged: root.disarm()
  onRowsChanged: {
    root.disarm()
    if ((root.selectedId === "" || !Model.findWorkRow(root.prepared, root.selectedId)) && root.rows.length > 0)
      root.selectedId = root.rows[0].id
  }
  Component.onCompleted: if (root.selectedId === "" && root.rows.length > 0) root.selectedId = root.rows[0].id
  // A reopen or a Run made a case: select it; a Run's field empties.
  onResultChanged: {
    var r = root.result
    if (!r || r.pending || !r.ok || (r.action !== "reopen" && r.action !== "agent-new")) return
    if (r.action === "agent-new" && intentField.text === root.sentIntent) intentField.text = ""
    root.sentIntent = ""
    if (r.caseId) {
      root.completedFilter = ""
      root.selectedId = r.caseId
    }
  }
  // Another section shown: disarm; a field gives the keys back, the sheet
  // stays open with its draft.
  onActiveChanged: if (!root.active) {
    root.disarm()
    if (root.editing && root.desk) root.desk.takeKeys()
  }

  ListColumn {
    id: list
    visible: !root.stacked || !root.detailShown
    width: root.stacked ? root.width : (root.layout ? root.layout.listW : 0)
    height: root.height
    divider: !root.stacked
    title: "Work"
    model: root.rows
    currentIndex: root.cursor
    emptyText: !root.index ? "No index to show"
      : root.searchText !== "" ? "Nothing here matches “" + root.searchText + "”."
      : "No cases yet. Say what should happen above, or make one by hand with New case."

    // The one-sentence start (ADR-0027 §6).
    Item {
      width: parent.width
      implicitHeight: Math.max(intentField.implicitHeight, runButton.implicitHeight)

      TextField {
        id: intentField
        anchors.left: parent.left
        anchors.right: runButton.left
        anchors.rightMargin: Style.spacing.sm
        anchors.verticalCenter: parent.verticalCenter
        enabled: root.canWrite
        placeholderText: root.canWrite ? "New case: say what to do, Enter runs an agent"
          : root.service ? root.service.writeBlocker : "The Seldon service is not running"
        foreground: root.foreground
        font.family: Style.font.family
        font.pixelSize: Style.font.bodySmall
        onAccepted: root.runIntent()
        Keys.onEscapePressed: function(event) {
          if (root.desk) root.desk.takeKeys()
          event.accepted = true
        }
      }

      Button {
        id: runButton
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        text: root.running ? "Running" : "Run"
        iconText: root.running ? "󰦖" : ""
        iconSpinning: root.running
        iconSize: Style.font.caption
        enabled: root.canWrite && !root.pending && intentField.text.trim() !== ""
        selected: true
        bordered: true
        foreground: root.foreground
        fontFamily: Style.font.family
        fontSize: Style.font.caption
        verticalPadding: Style.spacing.xs
        tooltipText: root.canWrite ? "Creates and starts the case, launches the agent (Enter; key i)"
          : (root.service ? root.service.writeBlocker : "")
        onClicked: root.runIntent()
      }
    }

    Item {
      width: parent.width
      implicitHeight: Math.max(wipText.implicitHeight, newButton.implicitHeight)

      Text {
        id: wipText
        anchors.left: parent.left
        anchors.right: agentFilter.left
        anchors.rightMargin: Style.spacing.sm
        anchors.verticalCenter: parent.verticalCenter
        textFormat: Text.PlainText
        text: root.index ? root.wip.text + (root.wip.tone === "urgent" ? " · over the limit" : root.wip.tone === "accent" ? " · at the limit" : "") : ""
        color: root.wip.tone === "urgent" ? Color.urgent : root.wip.tone === "accent" ? Color.accent : root.foreground
        wrapMode: Text.Wrap
        font.family: Style.font.family
        font.pixelSize: Style.font.bodySmall
        font.bold: true
      }

      Button {
        id: agentFilter
        anchors.right: newButton.left
        anchors.rightMargin: Style.spacing.sm
        anchors.verticalCenter: parent.verticalCenter
        text: "By agent"
        selected: root.completedFilter === Model.COMPLETED_FILTER_AGENT
        bordered: true
        foreground: root.foreground
        fontFamily: Style.font.family
        fontSize: Style.font.caption
        verticalPadding: Style.spacing.xs
        tooltipText: "Completed: only the cases an agent closed (a spot check)"
        onClicked: root.toggleCompletedFilter()
      }

      Button {
        id: newButton
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        text: "New case"
        iconText: "+"
        iconSize: Style.font.caption
        tooltipText: root.canWrite ? "Key +" : (root.service ? root.service.writeBlocker : "")
        enabled: root.canWrite
        selected: root.sheetOpen
        bordered: true
        foreground: root.foreground
        fontFamily: Style.font.family
        fontSize: Style.font.caption
        verticalPadding: Style.spacing.xs
        onClicked: root.sheetOpen ? root.closeSheet() : root.openSheet()
      }
    }

    // The engine's answer to the last case action; a refused new case
    // shows in the sheet, next to its fields.
    Text {
      width: parent.width
      visible: text !== ""
      textFormat: Text.PlainText
      text: root.result && !(root.sheetOpen && root.result.action === "new") ? root.result.text : ""
      wrapMode: Text.Wrap
      maximumLineCount: 3
      elide: Text.ElideRight
      color: root.result && !root.result.ok ? Color.urgent : root.dim
      font.family: Style.font.family
      font.pixelSize: Style.font.caption
    }

    delegate: GroupedRow {
      required property var modelData
      required property int index

      width: ListView.view.width
      header: index === 0 || (root.rows[index - 1] || {}).group !== modelData.group ? root.work.labels[modelData.group] : ""
      title: modelData.title
      meta: modelData.listMeta
      aside: modelData.stepsText
      stripe: modelData.stripe
      selected: modelData.id === root.selectedId && !root.sheetOpen
      cursor: false
      onClicked: {
        root.sheetOpen = false
        root.selectedId = modelData.id
        if (root.desk) root.desk.showDetail()
      }
    }
  }

  DetailPane {
    id: detail
    x: root.stacked ? 0 : list.width
    width: root.width - x
    height: root.height
    visible: !root.stacked || root.detailShown
    backVisible: root.stacked
    title: root.sheetOpen ? "New case" : root.detailData ? root.detailData.heading : "Work"
    meta: root.sheetOpen || !root.detailData ? "" : root.detailData.meta
    actions: root.barActions()
    hint: root.armed !== "" && root.arm ? root.arm.hint
      : !root.sheetOpen && !root.canWrite && root.caseActions.length > 0 && root.service ? root.service.writeBlocker
      : ""
    onBackRequested: if (root.desk) root.desk.back()
    onActionTriggered: function(id) { root.press(id) }

    NewCaseSheet {
      id: sheet
      width: parent.width
      visible: root.sheetOpen
      service: root.service
      foreground: root.foreground
      fontFamily: Style.font.family
      onLeaveRequested: root.closeSheet()
      onCreated: function(caseId) {
        root.sheetOpen = false
        root.completedFilter = ""
        if (caseId !== "") root.selectedId = caseId
        if (root.desk) root.desk.takeKeys()
      }
    }

    Text {
      width: parent.width
      visible: !root.sheetOpen && !root.detailData
      textFormat: Text.PlainText
      text: root.index ? "Nothing selected." : "No index to show"
      color: Color.muted
      wrapMode: Text.Wrap
      font.family: Style.font.family
      font.pixelSize: Style.font.body
    }

    Column {
      width: parent.width
      visible: !root.sheetOpen && !!root.detailData
      spacing: Style.spacing.xxl

      Column {
        width: parent.width
        spacing: Style.spacing.sm

        Text {
          width: parent.width
          textFormat: Text.PlainText
          text: root.detailData ? root.detailData.title : ""
          color: root.foreground
          wrapMode: Text.Wrap
          font.family: Style.font.family
          font.pixelSize: Style.font.title
          font.bold: true
        }

        // The last Ask agent about this case: the engine's answer or refusal.
        Text {
          objectName: "caseAskResult"
          width: parent.width
          visible: !!root.askResult
          textFormat: Text.PlainText
          text: root.askResult ? root.askResult.text : ""
          color: root.askResult && !root.askResult.ok ? Color.urgent : root.dim
          wrapMode: Text.Wrap
          font.family: Style.font.family
          font.pixelSize: Style.font.caption
        }

        // An agent closed it (ADR-0027 §5); a reopen names its case.
        Text {
          width: parent.width
          visible: text !== ""
          textFormat: Text.PlainText
          text: !root.detailData ? ""
            : [root.detailData.closedByAgent ? "completed by agent" : "",
               root.detailData.reopens !== "" ? "reopens " + root.detailData.reopens : ""]
              .filter(function(p) { return p !== "" }).join(" · ")
          color: Color.accent
          font.family: Style.font.family
          font.pixelSize: Style.font.bodySmall
          font.italic: true
        }
      }

      KeyValues {
        width: parent.width
        rows: root.detailData ? root.detailData.kv : []
        foreground: root.foreground
      }

      // The first paragraph of Intent and Result (ADR-0038 §2), plain
      // text; each hidden when the index has none.
      Repeater {
        model: root.detailData ? [["INTENT", root.detailData.intent], ["RESULT", root.detailData.result]]
          .filter(function(p) { return p[1] !== "" }) : []

        Column {
          id: block
          required property var modelData
          width: parent.width
          spacing: Style.spacing.sm

          Text {
            textFormat: Text.PlainText
            text: block.modelData[0]
            color: Color.muted
            font.family: Style.font.family
            font.pixelSize: Style.font.caption
            font.letterSpacing: Style.space(1)
            font.bold: true
          }

          Text {
            width: parent.width
            textFormat: Text.PlainText
            text: block.modelData[1]
            color: root.foreground
            wrapMode: Text.Wrap
            font.family: Style.font.family
            font.pixelSize: Style.font.body
          }
        }
      }

      Column {
        width: parent.width
        spacing: Style.spacing.md

        Text {
          textFormat: Text.PlainText
          text: "PLAN"
          color: Color.muted
          font.family: Style.font.family
          font.pixelSize: Style.font.caption
          font.letterSpacing: Style.space(1)
          font.bold: true
        }

        Progress {
          width: Math.min(parent.width, Style.space(260))
          visible: !!root.detailData && root.detailData.plan.total > 0
          value: root.detailData ? root.detailData.plan.progress : 0
          foreground: root.foreground
        }

        Text {
          width: parent.width
          textFormat: Text.PlainText
          text: !root.detailData ? ""
            : root.detailData.plan.text + (root.detailData.intent !== "" || root.detailData.result !== ""
              ? ". The steps and the full Intent and Result are in the case file."
              : ". The steps, the Intent and the Result are in the case file.")
          color: root.dim
          wrapMode: Text.Wrap
          font.family: Style.font.family
          font.pixelSize: Style.font.bodySmall
        }

        Button {
          visible: !!root.detailData && root.detailData.actionable
          text: "Open in editor"
          tooltipText: "The case file (key e)"
          bordered: true
          foreground: root.foreground
          fontFamily: Style.font.family
          fontSize: Style.font.caption
          verticalPadding: Style.spacing.xs
          onClicked: root.press("open")
        }
      }

      Column {
        width: parent.width
        spacing: Style.spacing.md

        Text {
          textFormat: Text.PlainText
          text: "LOG"
          color: Color.muted
          font.family: Style.font.family
          font.pixelSize: Style.font.caption
          font.letterSpacing: Style.space(1)
          font.bold: true
        }

        Text {
          visible: !!root.detailData && root.detailData.log.length === 0
          textFormat: Text.PlainText
          text: "Nothing in the index yet."
          color: root.dim
          font.family: Style.font.family
          font.pixelSize: Style.font.bodySmall
        }

        KeyValues {
          width: parent.width
          rows: root.detailData ? root.detailData.log : []
          keyWidth: Style.space(130)
          foreground: root.foreground
        }
      }

      Column {
        width: parent.width
        spacing: Style.spacing.md

        Text {
          textFormat: Text.PlainText
          text: "LINKED CHANGES · " + (root.detailData ? root.detailData.linked.length : 0)
          color: Color.muted
          font.family: Style.font.family
          font.pixelSize: Style.font.caption
          font.letterSpacing: Style.space(1)
          font.bold: true
        }

        Text {
          visible: !!root.detailData && root.detailData.linked.length === 0 && root.detailData.linkedMore === ""
          textFormat: Text.PlainText
          text: "No change is linked to this case."
          color: root.dim
          font.family: Style.font.family
          font.pixelSize: Style.font.bodySmall
        }

        KeyValues {
          width: parent.width
          rows: root.detailData ? root.detailData.linked : []
          keyWidth: Style.space(130)
          foreground: root.foreground
        }

        Text {
          width: parent.width
          visible: text !== ""
          textFormat: Text.PlainText
          text: root.detailData ? root.detailData.linkedMore : ""
          color: root.dim
          wrapMode: Text.Wrap
          font.family: Style.font.family
          font.pixelSize: Style.font.caption
        }
      }
    }
  }
}
