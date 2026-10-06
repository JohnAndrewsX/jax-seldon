pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import qs.Commons
import qs.Ui
import "../Model.js" as Model

// Work tab (SPEC-PLUGIN §5): the planning desk. On top the one-sentence
// start (WP-101, ADR-0027 §6): an intent field and *Run*, which sends
// `seldon agent start --new --json -- <intent>` (the text one argument,
// exactly as typed) and follows the new case; the field keeps its text until
// the engine has made the case. Then the WIP text ("2 / 3 active") against
// the `wipLimit` setting, the *Closed by agent* filter of the Completed
// column (the spot check, ADR-0027 §5) and *New case*, the hand-made case
// (the NewCaseSheet, in place of board and card while it is open). Three
// columns, Queued · Active (verification included) · Completed, from
// Model.workColumns(); a completed case an agent closed carries "by agent".
// Under the board the CaseCard of the case under the cursor, with its
// actions; a completed case offers *Reopen* (one click, no arming: it
// creates a case and destroys nothing).
//
// Every action goes through Service.plan(), Service.startAgent(),
// Service.startAgentNew() or Service.openInEditor() with a fixed argument
// list and a case id checked against the schema pattern (Model.planArgs,
// Model.agentArgs, Model.agentNewArgs). The result line shows the engine's
// answer; the moved case arrives with the next index (Service.qml's
// FileView), and the cursor follows it into its new column, or to the case
// a reopen or a Run made.
//
// Keyboard (forwarded by Panel.qml):
//   ↑ / ↓, k / j   move through the cases, column by column
//   Enter, Space   the card's first action: Open runs at once; Start,
//                  Verify and Done are armed by the first press and run by
//                  the second
//   x              Drop, armed by the first press, run by the second
//   a              Start agent on an active case, armed by the first press,
//                  run by the second (WP-022)
//   r              Reopen the completed case under the cursor, at once
//   i              the intent field (Enter runs, Esc gives the keys back)
//   e              open the case under the cursor in the editor
//   +              new case (Panel.qml, from any tab)
// Any other key, a cursor move or a new index disarms. With the mouse, a
// click runs an action; Drop and Start agent ask for a second click.
Item {
  id: root

  property var service: null
  property var indexData: null
  property bool cursorActive: false
  property int wipLimit: Model.WIP_LIMIT_DEFAULT
  property color foreground: Color.foreground
  property color accent: Color.accent
  property color urgent: Color.urgent
  property color muted: Color.muted
  property string fontFamily: Style.font.family

  property int cursor: 0
  // The case the cursor is on, by id, so it follows the case to its new
  // column when the index changes.
  property string selectedId: ""
  // "<caseId> <action>" while an action waits for its second key or click.
  property string armedKey: ""
  property bool sheetOpen: false
  // "" or Model.COMPLETED_FILTER_AGENT: the Completed column's filter.
  property string completedFilter: ""
  // The intent sent last, until the engine answers (the QuickEntry pattern).
  property string sentIntent: ""
  property alias intentField: intentField

  signal cursorWanted()
  // The sheet gives the keys back (Esc, Cancel, a created case).
  signal leaveRequested()

  readonly property var columns: Model.workColumns(indexData, completedFilter)
  readonly property var cases: Model.workCases(columns)
  readonly property int rowCount: cases.length
  readonly property var current: cases.length > 0 ? cases[Math.max(0, Math.min(cursor, cases.length - 1))] : null
  readonly property var wip: Model.wipStatus(indexData, wipLimit)
  readonly property var result: service ? service.planResult : null
  readonly property bool pending: !!result && result.pending
  readonly property bool canWrite: !!service && service.canWrite
  readonly property bool editing: (sheetOpen && sheet.editing) || intentField.activeFocus
  readonly property bool running: pending && !!result && result.action === "agent-new"
  readonly property string armedFor: current && armedKey.indexOf(current.id + " ") === 0 ? armedKey.slice(current.id.length + 1) : ""
  readonly property color dim: Util.alpha(foreground, 0.65)
  property alias sheet: sheet
  property alias card: card

  function toneColor(tone) {
    return tone === "urgent" ? root.urgent : tone === "accent" ? root.accent : tone === "muted" ? root.muted : root.dim
  }

  function disarm() {
    root.armedKey = ""
  }

  function select(i) {
    if (root.cases.length === 0) return
    root.cursor = Math.max(0, Math.min(root.cases.length - 1, i))
    root.selectedId = root.cases[root.cursor].id
    root.disarm()
  }

  function move(dy) {
    root.select(root.cursor + dy)
  }

  // Put the cursor on case `id` now if the index has it, else once it does.
  function follow(id) {
    root.selectedId = id
    root.findSelected()
  }

  function findSelected() {
    for (var i = 0; i < root.cases.length; i++) {
      if (root.cases[i].id === root.selectedId) {
        root.cursor = i
        return true
      }
    }
    return false
  }

  // Arm `actionId` for the current case, or run it when it is armed already.
  function arm(actionId) {
    var c = root.current
    var action = Model.caseAction(c, actionId)
    if (!action || root.pending || !root.canWrite) return false
    if (!action.write) {
      root.disarm()
      return root.service ? root.service.openInEditor(c.id) : false
    }
    if (root.armedFor !== actionId) {
      root.armedKey = c.id + " " + actionId
      return false
    }
    return root.runAction(actionId)
  }

  function runAction(actionId) {
    var c = root.current
    root.disarm()
    if (!c || !root.service) return false
    if (actionId === "agent") return root.service.startAgent(c.id)
    return root.service.plan(actionId, c.id)
  }

  // *Run*: one sentence, a new case, an agent on it (WP-101).
  function runIntent() {
    if (!root.service || root.pending) return false
    var sent = root.service.startAgentNew(intentField.text)
    if (sent) root.sentIntent = intentField.text
    return sent
  }

  function focusIntent() {
    root.disarm()
    if (root.sheetOpen) root.closeSheet()
    intentField.forceActiveFocus()
  }

  function toggleCompletedFilter() {
    root.completedFilter = root.completedFilter === "" ? Model.COMPLETED_FILTER_AGENT : ""
  }

  // A click on a card button: runs it, except Drop and Start agent, which
  // need a second click.
  function clickAction(actionId) {
    var action = Model.caseAction(root.current, actionId)
    if (!action) return
    if (action.confirm || action.twice) root.arm(actionId)
    else if (!action.write) root.arm(actionId)
    else if (!root.pending) root.runAction(actionId)
  }

  function activate() {
    var actions = Model.caseActions(root.current)
    if (actions.length > 0) root.arm(actions[0].id)
  }

  function dropKey() {
    if (Model.caseAction(root.current, "drop")) root.arm("drop")
    else root.disarm()
  }

  function openCurrent() {
    root.disarm()
    if (root.current && root.current.actionable && root.service) root.service.openInEditor(root.current.id)
  }

  function openSheet() {
    root.disarm()
    root.sheetOpen = true
    Qt.callLater(sheet.focusTitle)
  }

  function closeSheet() {
    root.sheetOpen = false
    root.leaveRequested()
  }

  function textKey(t) {
    if (t === "e") {
      root.openCurrent()
      return true
    }
    if (t === "a" && Model.caseAction(root.current, "agent")) {
      root.arm("agent")
      return true
    }
    if (t === "r" && Model.caseAction(root.current, "reopen")) {
      if (root.canWrite && !root.pending) root.runAction("reopen")
      return true
    }
    if (t === "i") {
      if (root.canWrite) root.focusIntent()
      return true
    }
    root.disarm()
    return false
  }

  // Keep the cursor on the selected case wherever the index puts it.
  onCasesChanged: {
    root.disarm()
    if (root.findSelected()) return
    root.cursor = Math.max(0, Math.min(root.cursor, root.cases.length - 1))
    root.selectedId = root.cases.length > 0 ? root.cases[root.cursor].id : ""
  }
  // Another tab shown: disarm; the sheet stays open but gives the keys back
  // (a hidden field keeps Qt's active focus; WP-067).
  onVisibleChanged: if (!visible) {
    root.disarm()
    if (root.editing) root.leaveRequested()
  }
  onCursorActiveChanged: if (!cursorActive) root.disarm()
  // A reopen or a Run made a case: the cursor goes to it; a Run's field
  // empties once the engine has made the case.
  onResultChanged: {
    var r = root.result
    if (!r || r.pending || !r.ok || (r.action !== "reopen" && r.action !== "agent-new")) return
    if (r.action === "agent-new" && intentField.text === root.sentIntent) intentField.text = ""
    root.sentIntent = ""
    if (r.caseId) root.follow(r.caseId)
  }

  Column {
    anchors.fill: parent
    spacing: Style.spacing.md

    // The one-sentence start (ADR-0027 §6).
    Item {
      id: intentRow
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
        accent: root.accent
        font.family: root.fontFamily
        font.pixelSize: Style.font.bodySmall
        onAccepted: root.runIntent()
        Keys.onEscapePressed: function(event) {
          root.leaveRequested()
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
        accent: root.accent
        fontFamily: root.fontFamily
        fontSize: Style.font.caption
        verticalPadding: Style.spacing.xs
        tooltipText: root.canWrite ? "Creates and starts the case, launches the agent (Enter; key i)"
          : (root.service ? root.service.writeBlocker : "")
        onClicked: root.runIntent()
      }
    }

    Item {
      id: header
      width: parent.width
      implicitHeight: Math.max(wipText.implicitHeight, newButton.implicitHeight)

      Text {
        id: wipText
        anchors.left: parent.left
        anchors.right: agentFilter.left
        anchors.rightMargin: Style.spacing.sm
        anchors.verticalCenter: parent.verticalCenter
        textFormat: Text.PlainText
        text: root.indexData ? root.wip.text + (root.wip.tone === "urgent" ? " · over the limit" : root.wip.tone === "accent" ? " · at the limit" : "") : ""
        color: root.wip.tone !== "" ? root.toneColor(root.wip.tone) : root.foreground
        elide: Text.ElideRight
        font.family: root.fontFamily
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
        accent: root.accent
        fontFamily: root.fontFamily
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
        accent: root.accent
        fontFamily: root.fontFamily
        fontSize: Style.font.caption
        verticalPadding: Style.spacing.xs
        onClicked: root.sheetOpen ? root.closeSheet() : root.openSheet()
      }
    }

    // The engine's answer to the last case action. A refused new case shows
    // in the sheet, next to its fields.
    Text {
      id: resultLine
      width: parent.width
      height: implicitHeight
      textFormat: Text.PlainText
      text: root.result && !(root.sheetOpen && root.result.action === "new") ? root.result.text : ""
      wrapMode: Text.Wrap
      maximumLineCount: 2
      color: root.result && !root.result.ok ? root.urgent : root.dim
      elide: Text.ElideRight
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
    }

    Row {
      id: board
      width: parent.width
      height: Style.space(212)
      spacing: Style.spacing.md
      visible: !root.sheetOpen

      Repeater {
        model: root.columns

        Column {
          id: col

          required property var modelData
          required property int index

          // Global index of this column's first case (the cursor walks all).
          readonly property int offset: {
            var n = 0
            for (var k = 0; k < col.index; k++) n += root.columns[k].cases.length
            return n
          }

          width: (board.width - board.spacing * 2) / 3
          height: board.height
          spacing: Style.spacing.sm

          PanelSectionHeader {
            id: colHeader
            text: Model.columnHeader(col.modelData)
            foreground: root.foreground
            fontFamily: root.fontFamily
          }

          ListView {
            id: list
            width: parent.width
            height: Math.max(0, col.height - colHeader.height - col.spacing)
            clip: true
            spacing: Style.spacing.sm
            boundsBehavior: Flickable.StopAtBounds
            interactive: contentHeight > height
            ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }

            model: col.modelData.cases
            readonly property int localCursor: root.cursor - col.offset
            currentIndex: localCursor >= 0 && localCursor < count ? localCursor : -1
            onCurrentIndexChanged: if (currentIndex >= 0) Qt.callLater(keepCurrentVisible)
            onCountChanged: if (currentIndex >= 0) Qt.callLater(keepCurrentVisible)
            function keepCurrentVisible() {
              if (currentIndex >= 0) positionViewAtIndex(currentIndex, ListView.Contain)
            }

            delegate: CursorSurface {
              id: tile

              required property var modelData
              required property int index

              // The outer delegate can go first when a new index replaces the columns.
              readonly property bool here: !!col && col.offset + index === root.cursor
              readonly property bool marked: modelData.status === "verification" || modelData.status === "dropped"

              width: ListView.view.width
              implicitHeight: tileColumn.implicitHeight + Style.spacing.md * 2
              hasCursor: root.cursorActive && here
              current: !root.cursorActive && here
              foreground: root.foreground
              accent: root.accent

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
                  implicitHeight: Math.max(tileId.implicitHeight, tileSteps.implicitHeight)

                  Text {
                    id: tileId
                    anchors.left: parent.left
                    anchors.right: tileSteps.left
                    anchors.rightMargin: Style.spacing.sm
                    textFormat: Text.PlainText
                    text: tile.modelData.id
                    color: root.dim
                    elide: Text.ElideRight
                    font.family: root.fontFamily
                    font.pixelSize: Style.font.caption
                  }

                  Text {
                    id: tileSteps
                    anchors.right: parent.right
                    textFormat: Text.PlainText
                    text: tile.modelData.stepsText
                    color: root.dim
                    font.family: root.fontFamily
                    font.pixelSize: Style.font.caption
                  }
                }

                Text {
                  width: parent.width
                  textFormat: Text.PlainText
                  text: tile.modelData.title
                  color: tile.modelData.status === "dropped" ? root.dim : root.foreground
                  wrapMode: Text.Wrap
                  maximumLineCount: 2
                  elide: Text.ElideRight
                  font.family: root.fontFamily
                  font.pixelSize: Style.font.caption
                  font.strikeout: tile.modelData.status === "dropped"
                }

                Flow {
                  width: parent.width
                  spacing: Style.spacing.sm
                  // Not from the children's `visible`: a hidden Flow hides them too.
                  visible: tile.marked || tile.modelData.proposed > 0 || tile.modelData.closedByAgent

                  Text {
                    id: tileStatus
                    visible: tile.marked
                    textFormat: Text.PlainText
                    text: tile.modelData.status
                    color: tile.modelData.status === "verification" ? root.accent : root.dim
                    font.family: root.fontFamily
                    font.pixelSize: Style.font.caption
                    font.italic: true
                  }

                  // An agent closed it (ADR-0027 §5): the spot check's marker.
                  Text {
                    visible: tile.modelData.closedByAgent
                    textFormat: Text.PlainText
                    text: "by agent"
                    color: root.accent
                    font.family: root.fontFamily
                    font.pixelSize: Style.font.caption
                    font.italic: true
                  }

                  BorderSurface {
                    id: tileBadge
                    visible: tile.modelData.proposed > 0
                    implicitWidth: badgeText.implicitWidth + Style.spacing.sm * 2
                    implicitHeight: badgeText.implicitHeight + Style.spacing.xxs * 2
                    radius: Style.cornerRadius
                    color: Style.selectedFillFor(root.accent, root.accent)
                    borderSpec: Border.controlSpec("normal", root.accent, root.accent)

                    Text {
                      id: badgeText
                      anchors.centerIn: parent
                      textFormat: Text.PlainText
                      text: tile.modelData.proposed + " proposed"
                      color: root.foreground
                      font.family: root.fontFamily
                      font.pixelSize: Style.font.caption
                      font.bold: true
                    }
                  }
                }
              }

              MouseArea {
                anchors.fill: parent
                cursorShape: Qt.PointingHandCursor
                onClicked: {
                  root.select(col.offset + tile.index)
                  root.cursorWanted()
                }
              }
            }
          }

          Text {
            width: parent.width
            visible: col.modelData.cases.length === 0 && !!root.indexData
            textFormat: Text.PlainText
            text: col.modelData.id === "queued" ? "Nothing queued" : col.modelData.id === "active" ? "Nothing active"
              : col.modelData.total > 0 ? "None closed by an agent" : "Nothing completed"
            color: root.dim
            wrapMode: Text.Wrap
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
          }
        }
      }
    }

    CaseCard {
      id: card
      width: parent.width
      visible: !root.sheetOpen && !!root.current
      caseData: root.current
      armed: root.armedFor
      pending: root.pending
      canWrite: root.canWrite
      writeBlocker: root.service ? root.service.writeBlocker : "The Seldon service is not running"
      foreground: root.foreground
      accent: root.accent
      urgent: root.urgent
      muted: root.muted
      fontFamily: root.fontFamily
      onActionRequested: function(actionId) { root.clickAction(actionId) }
    }

    NewCaseSheet {
      id: sheet
      width: parent.width
      visible: root.sheetOpen
      service: root.service
      foreground: root.foreground
      accent: root.accent
      urgent: root.urgent
      fontFamily: root.fontFamily
      onLeaveRequested: root.closeSheet()
      onCreated: function(caseId) {
        if (caseId !== "") root.follow(caseId)
        root.closeSheet()
      }
    }
  }

  Text {
    anchors.centerIn: parent
    visible: !root.indexData && !root.sheetOpen
    textFormat: Text.PlainText
    text: "No index to show"
    color: root.dim
    font.family: root.fontFamily
    font.pixelSize: Style.font.bodySmall
  }
}
