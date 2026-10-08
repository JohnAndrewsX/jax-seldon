pragma ComponentBehavior: Bound

import QtQuick
import qs.Commons
import qs.Ui
import "../../Model.js" as Model

// Resolve one open drift item, inline in an event's detail (desk sections
// 1 and 2, SPEC-PLUGIN §5.4; the 0.1 DriftSheet without its card, same
// API). The detail's sticky bar opens it on Link to case…, Explain… or
// Dismiss… (showForm); Enter on an open drift row opens the default one
// (Link when the engine proposes a case, else Explain; ADR-0028 §3: none
// is required). Always shown, the form or not: a group's members (ADR-0013,
// `seldon drift show` for those the index no longer lists), "proposed for
// C-…", and once resolved the folded resolution and the engine's answer.
//
//   Link:    Case [the proposed case first]    Resolve [All 3] [Only libinput]
//   Explain: why it changed; zone (the item's), risk (R1), area (optional)
//   Dismiss: the reason
//   [Link] [Cancel]
//   Press Enter again: Link firefox and 2 more to C-2026-005
//   <the engine's answer>
//
// Every call goes through Service.drift() with a fixed argument list built
// by Model.driftArgs(): the event id and case id checked against their
// schema patterns, the text one argument after `--`, exactly as typed. The
// form names the event it was opened for, so *Only …* (`--only`) resolves
// exactly that row; without it the engine resolves the whole group.
//
// Writing arms twice: Enter in a text field or on the action button arms
// the call and shows "Press Enter again: …", the second Enter runs it; a
// click runs it at once. Any change to the form disarms; a new index with
// the same item does not. The fields keep their text until the engine has
// resolved the item, so a refusal never loses it; Esc or Cancel hides the
// form and keeps the draft (per event). The resolved rows arrive with the
// next index (Service.qml's FileView).
//
// Keyboard: while anything in the form has focus the section is `editing`
// and the desk keeps out. Tab walks the case / scope (Link), text, zone,
// risk, area (Explain), text (Dismiss) → the action button → Cancel; in a
// picker ←/→ (h/l) move and Enter or Space picks.
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
  readonly property string hint: !root.isOpen ? ""
    : !root.canWrite ? root.writeBlocker
    : root.armed ? "Press Enter again: " + root.summary
    : ""
  readonly property color dim: Util.alpha(foreground, 0.65)
  readonly property real labelWidth: Style.space(64)
  // The form under the sticky bar is shown (showForm); the members, the
  // resolution and the answer show without it.
  property bool formShown: false
  // The event detail lists the whole transaction above, every member open
  // (WP-137): the member lines would repeat it, so they hide.
  property bool membersShownAbove: false

  // The keys leave the form (Esc, Cancel, the item resolved).
  signal leaveRequested()

  // Bind to event `id`: its draft if it has one, else the defaults for its
  // item (Link with the proposed case when there is one, else Explain).
  // Another event hides the form; the keys move only through showForm().
  function openFor(id) {
    var next = String(id || "")
    if (next !== root.eventId) {
      root.saveDraft()
      root.formShown = false
      root.eventId = next
      root.lastItem = root.item
      root.loadDraft()
    }
    root.armedSig = ""
    root.notice = ""
    root.fetchMembers()
  }

  // Show the form for `action` (link, explain, dismiss; "" the current
  // one) and give it the keys.
  function showForm(action) {
    if (!root.isOpen) return false
    if (action) root.setAction(action)
    root.formShown = true
    Qt.callLater(root.focusFirst)
    return true
  }

  // Hide the form, keep the draft, hand the keys back.
  function hideForm() {
    if (casePicker.popupOpen) casePicker.close()
    root.saveDraft()
    root.formShown = false
    root.leaveRequested()
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
    if (!root.visible || !root.formShown || !root.isOpen) return
    if (root.action === "explain") intentField.forceActiveFocus()
    else if (root.action === "dismiss") reasonField.forceActiveFocus()
    else submitKey.forceActiveFocus()
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

  onFormKeyChanged: {
    root.armedSig = ""
    root.notice = ""
  }
  onActionChanged: root.armedSig = ""
  // Resolved: the form goes and the keys go back to the desk.
  onItemChanged: {
    if (root.item) {
      root.lastItem = root.item
    } else if (root.formShown) {
      root.formShown = false
      root.leaveRequested()
    }
  }
  // A case that is no longer offered drops out of the form.
  onOptionsChanged: {
    for (var i = 0; i < root.options.length; i++)
      if (root.options[i].value === root.caseId) return
    root.caseId = root.options.length > 0 ? root.options[0].value : ""
  }
  onResultChanged: {
    if (!root.result || root.result.pending || root.sentSig === "") return
    root.sentSig = ""
    if (root.result.ok && !root.result.already) root.forgetDraft()
  }
  onVisibleChanged: if (!visible) root.armedSig = ""

  Keys.onEscapePressed: function(event) {
    root.hideForm()
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

    // A group's members (ADR-0013): what the index lists, the rest from
    // `seldon drift show`.
    Text {
      width: parent.width
      visible: root.memberLines.length > 0 && !root.membersShownAbove
      textFormat: Text.PlainText
      text: root.shown ? Model.plural(root.shown.members, "package", "packages") + " in one transaction:" : ""
      color: root.dim
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
    }

    Repeater {
      model: root.membersShownAbove ? [] : root.memberLines

      Text {
        required property string modelData

        width: column.width
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
      text: root.isOpen || root.eventId === "" ? "" : root.resolution !== "" ? "Resolved: " + root.resolution : root.shown ? "No longer open drift" : ""
      color: root.foreground
      wrapMode: Text.Wrap
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
      font.italic: true
    }

    // The form, while shown and the item is open.
    Column {
      id: formColumn
      objectName: "driftForm"
      width: parent.width
      spacing: Style.spacing.md
      visible: root.formShown && root.isOpen

      Text {
        width: parent.width
        textFormat: Text.PlainText
        text: root.action === "link" ? "LINK TO A CASE" : root.action === "explain" ? "EXPLAIN" : "DISMISS"
        color: Color.muted
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        font.letterSpacing: Style.space(1)
        font.bold: true
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

      Row {
        spacing: Style.spacing.sm

        // The action button: a Tab stop whose Enter arms first (a qs.Ui
        // Button's own Enter would run at once); a click runs.
        Item {
          id: submitKey
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
          text: "Cancel"
          focusable: true
          bordered: true
          foreground: root.foreground
          accent: root.accent
          fontFamily: root.fontFamily
          fontSize: Style.font.caption
          verticalPadding: Style.spacing.xs
          tooltipText: "Esc; the fields keep their text"
          onClicked: root.hideForm()
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
