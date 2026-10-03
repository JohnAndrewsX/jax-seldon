pragma ComponentBehavior: Bound

import QtQuick
import qs.Commons
import qs.Ui
import "../Model.js" as Model

// The drift sheet on the Changelog tab (SPEC-PLUGIN §5, WP-021): resolve one
// open drift item. Opened by Enter on an open drift row, the row's
// *Resolve…* button, or the crisis strip (the first crisis).
//
//   RESOLVE DRIFT
//   ▌ glyph kind subject [+N]                         red · crisis
//   ▌ detail · actor · day time
//   ▌ · upgrade libinput 1.29.1-1 → 1.29.2-1     (a group's members)
//   ▌ proposed for C-2026-005
//   [Link] [Explain] [Dismiss]
//   Link:    Case [the proposed case first]    Resolve [All 3] [Only libinput]
//   Explain: why it changed; zone (the item's), risk (R1), area (optional)
//   Dismiss: the reason
//   [Link] [Cancel] [Open case]
//   Press Enter again: Link firefox and 2 more to C-2026-005
//   <the engine's answer>
//
// Every call goes through Service.drift() with a fixed argument list built
// by Model.driftArgs(): the event id and case id checked against their
// schema patterns, the text one argument after `--`, exactly as typed. The
// sheet names the event it was opened from, so *Only …* (`--only`)
// resolves exactly that row; without it the engine resolves the whole
// group. A group's members come from index.events; when the index no
// longer lists all of them, `seldon drift show` fills in the rest.
//
// Writing follows the Work tab (WP-020): Enter in a text field or on the
// action button arms the call and shows "Press Enter again: …", the second
// Enter runs it; a click runs it at once. Any change to the form disarms;
// a new index with the same item does not. The fields keep their text
// until the engine has resolved the item, so a refusal never loses it;
// Esc closes the sheet and keeps the draft (per event). The resolved rows arrive with the next index (Service.qml's
// FileView); then the sheet shows the folded resolution and, for a linked
// or explained item, *Open case*.
//
// Keyboard: while anything in the sheet has focus, Panel.qml blocks its own
// keys (`editing`). Tab walks action → case / scope (Link), text, zone,
// risk, area (Explain), text (Dismiss) → the action button → Cancel → Open
// case; in a picker ←/→ (h/l) move and Enter or Space picks.
FocusScope {
  id: root

  property var service: null
  property var indexData: null
  property color foreground: Color.foreground
  property color accent: Color.accent
  property color urgent: Color.urgent
  property color muted: Color.muted
  property string fontFamily: Style.font.family

  // The event the sheet resolves (a row's id); set through openFor().
  property string eventId: ""
  property string action: "link"
  property string caseId: ""
  property bool only: false
  property string zone: ""
  property string risk: Model.EXPLAIN_RISK_DEFAULT
  // The argument list waiting for its second Enter, as JSON, or "".
  property string armedSig: ""
  // The argument list sent, as JSON, until the engine answers.
  property string sentSig: ""
  // A refusal of the plugin's own (nothing reached the engine): a form the
  // engine would refuse, or another drift action still pending (neutral).
  property string notice: ""
  // The item as it was while still open, for the summary after it resolves.
  property var lastItem: null
  // Drafts of other events, by event id: { action, caseId, only, intent,
  // reason, zone, risk, area }.
  property var drafts: ({})

  property alias intent: intentField.text
  property alias reason: reasonField.text
  property alias area: areaField.text

  readonly property bool editing: root.activeFocus || casePicker.popupOpen
  readonly property bool canWrite: !!service && service.canWrite
  readonly property string writeBlocker: service ? service.writeBlocker : "The Seldon service is not running"
  readonly property var item: Model.driftItemFor(indexData, eventId)
  readonly property var shown: item || lastItem
  readonly property bool isOpen: item !== null
  readonly property var options: Model.caseOptionsFor(indexData, shown)
  readonly property var result: service && service.driftResult && service.driftResult.eventId === eventId ? service.driftResult : null
  readonly property bool pending: !!result && result.pending
  readonly property var form: ({
    eventId: root.eventId,
    caseId: root.caseId,
    only: root.only,
    text: root.action === "explain" ? root.intent : root.reason,
    zone: root.zone,
    risk: root.risk,
    area: root.area,
    itemZone: root.shown ? root.shown.zone : ""
  })
  // The form's content: a new index rebuilds `form` (a new object every
  // time), so only a change of this text is a change to the form (F-555).
  readonly property string formKey: JSON.stringify(form)
  readonly property var built: Model.driftArgs(action, form)
  readonly property string sig: built.args ? JSON.stringify(built.args) : ""
  readonly property bool armed: sig !== "" && armedSig === sig
  readonly property string summary: Model.driftSummary(action, shown, form)
  // `drift show` is asked for the group's leader (fetchMembers), whichever
  // member row the sheet was opened from.
  readonly property var showResult: service && service.driftShown && shown && shown.grouped
    && service.driftShown.eventId === shown.leaderId ? service.driftShown : null
  readonly property var members: showResult && showResult.ok && !showResult.pending && showResult.members.length > 0
    ? showResult.members : shown ? shown.memberList : []
  readonly property var memberLines: shown && shown.grouped ? Model.memberLines(members, shown.members) : []
  readonly property string resolution: Model.eventResolution(indexData, eventId)
  readonly property string resultText: root.notice !== "" ? root.notice : result ? result.text : ""
  readonly property bool resultOk: root.notice !== "" ? root.notice === Model.BUSY_TEXT : !!result && result.ok
  readonly property string caseToOpen: result && result.ok && !result.pending && result.caseId !== "" ? result.caseId : ""
  readonly property string hint: !root.isOpen ? ""
    : !root.canWrite ? root.writeBlocker
    : root.armed ? "Press Enter again: " + root.summary
    : ""
  readonly property color dim: Util.alpha(foreground, 0.65)
  readonly property color toneColor: shown && shown.tone === "urgent" ? urgent : shown && shown.tone === "accent" ? accent : muted
  readonly property real labelWidth: Style.space(64)

  signal leaveRequested()

  // Show event `id`: its draft if it has one, else the defaults for its
  // item (Link with the proposed case when there is one, else Explain).
  function openFor(id) {
    var next = String(id || "")
    if (next !== root.eventId) {
      root.saveDraft()
      root.eventId = next
      root.lastItem = root.item
      root.loadDraft()
    }
    root.armedSig = ""
    root.notice = ""
    root.fetchMembers()
    Qt.callLater(root.focusFirst)
  }

  function saveDraft() {
    if (root.eventId === "") return
    // A resolved event has no draft left to keep (forgotten in onResultChanged).
    if (root.result && root.result.ok && !root.result.pending && !root.result.already) return
    var d = {}
    for (var k in root.drafts) d[k] = root.drafts[k]
    d[root.eventId] = {
      action: root.action, caseId: root.caseId, only: root.only, intent: root.intent,
      reason: root.reason, zone: root.zone, risk: root.risk, area: root.area
    }
    root.drafts = d
  }

  function loadDraft() {
    var d = root.drafts[root.eventId]
    var it = root.item
    root.action = d ? d.action : Model.driftDefaultAction(it)
    root.caseId = d ? d.caseId : root.options.length > 0 ? root.options[0].value : ""
    root.only = d ? d.only : false
    root.intent = d ? d.intent : ""
    root.reason = d ? d.reason : ""
    root.zone = d ? d.zone : it ? it.zone : ""
    root.risk = d ? d.risk : Model.EXPLAIN_RISK_DEFAULT
    root.area = d ? d.area : ""
  }

  function forgetDraft() {
    var d = {}
    for (var k in root.drafts) if (k !== root.eventId) d[k] = root.drafts[k]
    root.drafts = d
  }

  // Ask the engine for the members index.events no longer lists.
  function fetchMembers() {
    var it = root.item
    if (it && it.grouped && it.memberList.length < it.members && root.service) root.service.driftShow(it.leaderId)
  }

  function focusFirst() {
    if (!root.visible) return
    if (!root.isOpen) root.focusDone()
    else if (root.action === "explain") intentField.forceActiveFocus()
    else if (root.action === "dismiss") reasonField.forceActiveFocus()
    else submitKey.forceActiveFocus()
  }

  // Once the item is resolved, the keys sit on Open case or Close.
  function focusDone() {
    if (openCaseButton.visible) openCaseButton.forceActiveFocus()
    else cancelButton.forceActiveFocus()
  }

  function setAction(a) {
    root.action = a
    root.notice = ""
  }

  // Enter in a text field or on the action button: arm, then run.
  function enterKey() {
    if (!root.isOpen || root.pending || !root.canWrite) return false
    if (root.built.error) {
      root.notice = root.built.error
      return false
    }
    if (!root.armed) {
      root.armedSig = root.sig
      return false
    }
    return root.run()
  }

  // A click on the action button runs at once.
  function clickSubmit() {
    if (!root.isOpen || root.pending || !root.canWrite) return false
    if (root.built.error) {
      root.notice = root.built.error
      return false
    }
    return root.run()
  }

  function run() {
    root.armedSig = ""
    root.notice = ""
    if (!root.service) return false
    var sig = root.sig
    var refusals = root.service.busyRefusals
    var sent = root.service.drift(root.action, root.form)
    if (!sent && root.service.busyRefusals !== refusals) root.notice = root.service.busyRefusal.text
    if (sent) root.sentSig = sig
    return sent
  }

  function openCase() {
    if (root.caseToOpen !== "" && root.service) root.service.openInEditor(root.caseToOpen)
  }

  onFormKeyChanged: {
    root.armedSig = ""
    root.notice = ""
  }
  onActionChanged: root.armedSig = ""
  onItemChanged: {
    if (root.item) root.lastItem = root.item
    else if (root.activeFocus) Qt.callLater(root.focusDone)
  }
  // A case that is no longer offered drops out of the form.
  onOptionsChanged: {
    for (var i = 0; i < root.options.length; i++)
      if (root.options[i].value === root.caseId) return
    root.caseId = root.options.length > 0 ? root.options[0].value : ""
  }
  // The index can arrive before the engine's answer: Open case appears last.
  onCaseToOpenChanged: if (root.caseToOpen !== "" && !root.isOpen && root.activeFocus) Qt.callLater(root.focusDone)
  onResultChanged: {
    if (!root.result || root.result.pending || root.sentSig === "") return
    root.sentSig = ""
    if (root.result.ok && !root.result.already) root.forgetDraft()
  }
  onVisibleChanged: if (!visible) root.armedSig = ""

  Keys.onEscapePressed: function(event) {
    if (casePicker.popupOpen) casePicker.close()
    root.saveDraft()
    root.leaveRequested()
    event.accepted = true
  }

  implicitHeight: column.implicitHeight

  // One labelled row: the label left, the control right of it. An inline
  // component has its own id scope, so everything comes in as a property.
  component FormRow: Item {
    id: formRow

    property string label: ""
    property real labelWidth: 0
    property color labelColor: Color.foreground
    property string fontFamily: Style.font.family
    default property alias control: holder.data

    implicitHeight: Math.max(rowLabel.implicitHeight, holder.childrenRect.height)

    Text {
      id: rowLabel
      width: formRow.labelWidth
      anchors.verticalCenter: parent.verticalCenter
      textFormat: Text.PlainText
      text: formRow.label
      color: formRow.labelColor
      font.family: formRow.fontFamily
      font.pixelSize: Style.font.caption
    }

    Item {
      id: holder
      x: formRow.labelWidth
      width: formRow.width - formRow.labelWidth
      height: childrenRect.height
      anchors.verticalCenter: parent.verticalCenter
    }
  }

  Column {
    id: column
    width: parent.width
    spacing: Style.spacing.md

    PanelSectionHeader {
      text: root.shown && root.shown.crisis ? "RESOLVE A RED-ZONE CHANGE" : "RESOLVE DRIFT"
      foreground: root.foreground
      fontFamily: root.fontFamily
    }

    // What the item is.
    BorderSurface {
      id: card
      width: parent.width
      visible: !!root.shown
      implicitHeight: cardColumn.implicitHeight + Style.spacing.lg * 2
      radius: Style.cornerRadius
      color: Style.selectedFillFor(root.foreground, root.accent)
      borderSpec: Border.controlSpec("normal", root.foreground, root.accent)

      Rectangle {
        x: Style.spacing.sm
        y: Style.spacing.lg
        width: Style.spacing.xs
        height: card.height - Style.spacing.lg * 2
        radius: width / 2
        visible: !!root.shown && root.shown.tone !== ""
        color: root.toneColor
      }

      Column {
        id: cardColumn
        x: Style.spacing.sm + Style.spacing.xs + Style.spacing.lg
        y: Style.spacing.lg
        width: card.width - x - Style.spacing.lg
        spacing: Style.spacing.xxs

        Item {
          width: parent.width
          implicitHeight: Math.max(itemSubject.implicitHeight, itemZone.implicitHeight)

          Text {
            id: itemSubject
            anchors.left: parent.left
            anchors.right: itemZone.left
            anchors.rightMargin: Style.spacing.md
            anchors.verticalCenter: parent.verticalCenter
            textFormat: Text.PlainText
            text: root.shown ? root.shown.glyph + "  " + root.shown.kind + "  " + root.shown.subject
              + (root.shown.badge !== "" ? "  " + root.shown.badge : "") : ""
            color: root.foreground
            elide: Text.ElideMiddle
            font.family: root.fontFamily
            font.pixelSize: Style.font.body
            font.bold: true
          }

          Text {
            id: itemZone
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            textFormat: Text.PlainText
            text: root.shown ? root.shown.zone + (root.shown.crisis ? " · crisis" : "") : ""
            color: root.toneColor
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
            font.bold: true
          }
        }

        Text {
          width: parent.width
          visible: text !== ""
          textFormat: Text.PlainText
          text: root.shown ? [root.shown.detail, root.shown.actor, root.shown.day + " " + root.shown.time]
            .filter(function(p) { return p.trim() !== "" }).join(" · ") : ""
          color: root.dim
          elide: Text.ElideRight
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
        }

        Text {
          width: parent.width
          visible: root.memberLines.length > 0
          textFormat: Text.PlainText
          text: root.shown ? Model.plural(root.shown.members, "package", "packages") + " in one transaction:" : ""
          color: root.dim
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
        }

        Repeater {
          model: root.memberLines

          Text {
            required property string modelData

            width: cardColumn.width
            leftPadding: Style.spacing.lg
            textFormat: Text.PlainText
            text: modelData
            color: root.dim
            elide: Text.ElideRight
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
          }
        }

        Text {
          width: parent.width
          visible: text !== ""
          textFormat: Text.PlainText
          text: root.isOpen && root.shown && root.shown.proposedCase !== "" ? "proposed for " + root.shown.proposedCase : ""
          color: root.accent
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
        }

        Text {
          width: parent.width
          visible: text !== ""
          textFormat: Text.PlainText
          text: root.isOpen ? "" : root.resolution !== "" ? "Resolved: " + root.resolution : root.shown ? "No longer open drift" : ""
          color: root.foreground
          wrapMode: Text.Wrap
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
          font.italic: true
        }
      }
    }

    Text {
      width: parent.width
      visible: !root.shown
      textFormat: Text.PlainText
      text: "This event is not open drift in the index."
      color: root.dim
      wrapMode: Text.Wrap
      font.family: root.fontFamily
      font.pixelSize: Style.font.bodySmall
    }

    // The form, while the item is open.
    Column {
      id: formColumn
      width: parent.width
      spacing: Style.spacing.md
      visible: root.isOpen

      ButtonGroup {
        id: actionGroup
        options: Model.DRIFT_ACTIONS.map(function(a) { return { value: a, label: Model.DRIFT_ACTION_LABELS[a] } })
        value: root.action
        enabled: root.canWrite
        foreground: root.foreground
        accent: root.accent
        fontFamily: root.fontFamily
        fontSize: Style.font.caption
        onChanged: function(v) { root.setAction(v) }
      }

      FormRow {
        width: parent.width
        visible: root.action === "link"
        label: "Case"
        labelWidth: root.labelWidth
        labelColor: root.dim
        fontFamily: root.fontFamily

        Dropdown {
          id: casePicker
          width: parent.width
          showLabel: false
          fontFamily: root.fontFamily
          options: root.options
          value: root.caseId
          enabled: root.canWrite
          onChanged: function(v) { root.caseId = v }
        }
      }

      FormRow {
        width: parent.width
        visible: !!root.shown && root.shown.grouped
        label: "Resolve"
        labelWidth: root.labelWidth
        labelColor: root.dim
        fontFamily: root.fontFamily

        ButtonGroup {
          options: root.shown ? [
            { value: "all", label: "All " + root.shown.members },
            { value: "only", label: "Only " + root.shown.namedSubject }
          ] : []
          value: root.only ? "only" : "all"
          enabled: root.canWrite
          foreground: root.foreground
          accent: root.accent
          fontFamily: root.fontFamily
          fontSize: Style.font.caption
          onChanged: function(v) { root.only = v === "only" }
        }
      }

      TextField {
        id: intentField
        width: parent.width
        visible: root.action === "explain"
        enabled: root.canWrite
        placeholderText: root.canWrite ? "Why did it change? Enter twice explains" : root.writeBlocker
        foreground: root.foreground
        accent: root.accent
        font.family: root.fontFamily
        font.pixelSize: Style.font.bodySmall
        onAccepted: root.enterKey()
      }

      FormRow {
        width: parent.width
        visible: root.action === "explain"
        label: "Zone"
        labelWidth: root.labelWidth
        labelColor: root.dim
        fontFamily: root.fontFamily

        ButtonGroup {
          options: Model.ZONES
          value: root.zone
          enabled: root.canWrite
          foreground: root.foreground
          accent: root.accent
          fontFamily: root.fontFamily
          fontSize: Style.font.caption
          onChanged: function(v) { root.zone = v }
        }
      }

      FormRow {
        width: parent.width
        visible: root.action === "explain"
        label: "Risk"
        labelWidth: root.labelWidth
        labelColor: root.dim
        fontFamily: root.fontFamily

        ButtonGroup {
          options: Model.RISKS
          value: root.risk
          enabled: root.canWrite
          foreground: root.foreground
          accent: root.accent
          fontFamily: root.fontFamily
          fontSize: Style.font.caption
          onChanged: function(v) { root.risk = v }
        }
      }

      FormRow {
        width: parent.width
        visible: root.action === "explain"
        label: "Area"
        labelWidth: root.labelWidth
        labelColor: root.dim
        fontFamily: root.fontFamily

        TextField {
          id: areaField
          width: parent.width
          enabled: root.canWrite
          placeholderText: "optional, e.g. dev-env"
          foreground: root.area === "" || Model.AREA.test(root.area) ? root.foreground : root.urgent
          accent: root.accent
          font.family: root.fontFamily
          font.pixelSize: Style.font.bodySmall
          onAccepted: root.enterKey()
        }
      }

      TextField {
        id: reasonField
        width: parent.width
        visible: root.action === "dismiss"
        enabled: root.canWrite
        placeholderText: root.canWrite ? "Why it needs no case. Enter twice dismisses" : root.writeBlocker
        foreground: root.foreground
        accent: root.accent
        font.family: root.fontFamily
        font.pixelSize: Style.font.bodySmall
        onAccepted: root.enterKey()
      }
    }

    Row {
      spacing: Style.spacing.sm

      // The action button: a Tab stop whose Enter arms first (a qs.Ui
      // Button's own Enter would run at once); a click runs.
      Item {
        id: submitKey
        visible: root.isOpen
        activeFocusOnTab: true
        implicitWidth: submitButton.implicitWidth
        implicitHeight: submitButton.implicitHeight
        Keys.onReturnPressed: root.enterKey()
        Keys.onEnterPressed: root.enterKey()
        Keys.onSpacePressed: root.enterKey()

        Button {
          id: submitButton
          anchors.fill: parent
          text: root.pending ? Model.DRIFT_ACTION_LABELS[root.action] + "ing" : Model.DRIFT_ACTION_LABELS[root.action]
          iconText: root.pending ? "󰦖" : ""
          iconSpinning: root.pending
          iconSize: Style.font.caption
          enabled: root.canWrite && !root.pending
          hasCursor: submitKey.activeFocus || root.armed
          selected: true
          bordered: true
          foreground: root.action === "dismiss" ? root.urgent : root.foreground
          accent: root.accent
          fontFamily: root.fontFamily
          fontSize: Style.font.caption
          verticalPadding: Style.spacing.xs
          tooltipText: "Enter twice, or click"
          onClicked: root.clickSubmit()
        }
      }

      Button {
        id: cancelButton
        text: root.isOpen ? "Cancel" : "Close"
        focusable: true
        bordered: true
        foreground: root.foreground
        accent: root.accent
        fontFamily: root.fontFamily
        fontSize: Style.font.caption
        verticalPadding: Style.spacing.xs
        tooltipText: "Esc; the fields keep their text"
        onClicked: {
          root.saveDraft()
          root.leaveRequested()
        }
      }

      Button {
        id: openCaseButton
        visible: root.caseToOpen !== ""
        text: "Open " + root.caseToOpen
        focusable: true
        bordered: true
        foreground: root.foreground
        accent: root.accent
        fontFamily: root.fontFamily
        fontSize: Style.font.caption
        verticalPadding: Style.spacing.xs
        tooltipText: "Open the case file in the editor"
        onClicked: root.openCase()
      }
    }

    Text {
      width: parent.width
      visible: text !== ""
      textFormat: Text.PlainText
      text: root.hint
      color: root.armed ? root.accent : root.dim
      wrapMode: Text.Wrap
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
    }

    Text {
      width: parent.width
      visible: text !== ""
      textFormat: Text.PlainText
      text: root.resultText
      color: root.resultOk ? root.dim : root.urgent
      wrapMode: Text.Wrap
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
    }
  }
}
