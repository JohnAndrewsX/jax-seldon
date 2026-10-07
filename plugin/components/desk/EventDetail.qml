import QtQuick
import qs.Commons
import qs.Ui
import "../../Model.js" as Model

// One event in a desk detail (sections 1 and 2; ADR-0034 §2, prototype
// `eventDetail`): the sticky bar, then "source · kind", the full subject,
// its class, the "why loud" callout for a crisis, the key/values (when,
// who, what, case, rule, source, zone, resolution, event id) and the
// DriftForm, which lists a group's members and resolves open drift inline.
//
// Bar (Model.eventActions): open drift → [Ask agent, WP-124b], Link to
// case…, Explain…, Dismiss… (attention also Hide / Show); an event with a
// case → Open case (Work with the case selected); routine → none. Link,
// Explain and Dismiss only open the form: the form's own button writes,
// armed by the first Enter (the bar shows the hint while armed). Hide
// writes nothing (Service.deskHidden, this shell session); a crisis has no
// Hide (ADR-0028 §3).
DetailPane {
  id: root

  // The section showing it (desk, service, index, the prepared rows).
  property var section: null
  property string eventId: ""

  readonly property var service: root.section ? root.section.service : null
  readonly property var indexData: root.section ? root.section.index : null
  readonly property var prepared: root.service ? root.service.deskChangelog : null
  readonly property var detail: Model.eventDetail(root.indexData, root.prepared, root.eventId)
  readonly property bool hidden: !!root.detail && !!root.service && root.service.deskHidden[root.detail.hideKey] === true
  readonly property alias form: form
  readonly property bool editing: form.editing
  readonly property color foregroundColor: Color.popups.text

  // The keys left the form (Esc, Cancel, resolved).
  signal leaveRequested()

  title: root.detail ? root.detail.heading : ""
  meta: root.detail ? root.detail.classLabel : ""
  hint: form.formShown && form.armed ? form.hint : ""
  actions: Model.eventActions(root.detail, {
    askAgent: !!root.service && root.service.askAgentAvailable,
    hidden: root.hidden
  })

  // Enter on the event's row: open drift → its default form.
  function activate() {
    if (!root.detail || !root.detail.open) return false
    return form.showForm("")
  }

  // Esc: a shown form hides (draft kept).
  function back() {
    if (!form.formShown) return false
    form.hideForm()
    return true
  }

  function trigger(id) {
    if (id === "link" || id === "explain" || id === "dismiss") {
      form.showForm(id)
    } else if (id === "hide") {
      if (!root.service || !root.detail) return
      var next = ({})
      for (var k in root.service.deskHidden) next[k] = root.service.deskHidden[k]
      if (root.hidden) delete next[root.detail.hideKey]
      else next[root.detail.hideKey] = true
      root.service.deskHidden = next
    } else if (id === "case") {
      var desk = root.section ? root.section.desk : null
      if (!desk || !root.detail) return
      var caseId = root.detail.caseId
      desk.section("work")
      desk.select(caseId)
    }
  }

  function view() {
    return {
      id: root.eventId,
      found: !!root.detail,
      heading: root.detail ? root.detail.heading : "",
      cls: root.detail ? root.detail.cls : "",
      whyLoud: root.detail ? root.detail.whyLoud : "",
      kv: root.detail ? root.detail.kv.map(function(r) { return r[0] + ": " + r[1] }) : [],
      actions: root.actions.map(function(a) { return a.label }),
      hidden: root.hidden,
      bar: { y: Math.round(root.actionBar.mapToItem(root, 0, 0).y), sceneY: Math.round(root.actionBar.mapToItem(null, 0, 0).y),
        h: Math.round(root.actionBar.height), visible: root.actionBar.visible },
      scroll: { y: Math.round(root.flickable.contentY), h: Math.round(root.flickable.contentHeight), view: Math.round(root.flickable.height) },
      form: {
        shown: form.formShown && form.isOpen,
        editing: form.editing,
        eventId: form.eventId,
        isOpen: form.isOpen,
        action: form.action,
        caseId: form.caseId,
        cases: form.options.map(function(o) { return o.value }),
        only: form.only,
        intent: form.intent,
        reason: form.reason,
        zone: form.shown ? form.shown.zone : "",
        explainZone: form.zone,
        risk: form.risk,
        area: form.area,
        armed: form.armed,
        hint: form.hint,
        result: form.resultText,
        resultOk: form.resultOk,
        already: !!form.result && form.result.already === true,
        resolution: form.resolution,
        members: form.memberLines,
        subject: form.shown ? form.shown.subject : "",
        badge: form.shown ? form.shown.badge : "",
        crisis: !!form.shown && form.shown.crisis
      }
    }
  }

  onEventIdChanged: form.openFor(root.eventId)
  Component.onCompleted: form.openFor(root.eventId)
  onActionTriggered: function(id) { root.trigger(id) }

  Text {
    width: parent.width
    visible: !root.detail
    textFormat: Text.PlainText
    text: !root.indexData ? "No index to show" : root.eventId === "" ? "Nothing selected."
      : "This event is not in the index any more."
    color: Color.muted
    wrapMode: Text.Wrap
    font.family: root.fontFamily
    font.pixelSize: Style.font.body
  }

  Column {
    width: parent.width
    visible: !!root.detail
    spacing: Style.spacing.lg

    Text {
      width: parent.width
      textFormat: Text.PlainText
      text: root.detail ? root.detail.title : ""
      color: root.foregroundColor
      wrapMode: Text.WrapAnywhere
      font.family: root.fontFamily
      font.pixelSize: Style.font.title
      font.bold: true
    }

    Text {
      width: parent.width
      textFormat: Text.PlainText
      text: root.detail ? root.detail.classLabel + (root.hidden ? " · hidden this session" : "") : ""
      color: root.detail && root.detail.cls === "crisis" ? Color.urgent
        : root.detail && root.detail.cls === "attention" ? Color.accent : Color.muted
      font.family: root.fontFamily
      font.pixelSize: Style.font.bodySmall
      font.bold: true
    }

    // Why loud? (a crisis only)
    BorderSurface {
      id: callout
      objectName: "whyLoud"
      width: parent.width
      visible: !!root.detail && root.detail.whyLoud !== ""
      implicitHeight: calloutColumn.implicitHeight + Style.spacing.lg * 2
      radius: Style.cornerRadius
      color: Style.normalFill
      borderSpec: Border.flat(Color.urgent, Math.max(1, Style.space(1)))

      Column {
        id: calloutColumn
        x: Style.spacing.xl
        y: Style.spacing.lg
        width: callout.width - Style.spacing.xl * 2
        spacing: Style.spacing.xs

        Text {
          width: parent.width
          textFormat: Text.PlainText
          text: "Why loud?"
          color: Color.urgent
          font.family: root.fontFamily
          font.pixelSize: Style.font.bodySmall
          font.bold: true
        }

        Text {
          width: parent.width
          textFormat: Text.PlainText
          text: root.detail ? root.detail.whyLoud : ""
          color: root.foregroundColor
          wrapMode: Text.Wrap
          font.family: root.fontFamily
          font.pixelSize: Style.font.bodySmall
        }
      }
    }

    KeyValues {
      width: parent.width
      rows: root.detail ? root.detail.kv : []
      foreground: root.foregroundColor
      fontFamily: root.fontFamily
    }

    DriftForm {
      id: form
      width: parent.width
      service: root.service
      indexData: root.indexData
      foreground: root.foregroundColor
      fontFamily: root.fontFamily
      onLeaveRequested: root.leaveRequested()
    }

    Text {
      width: parent.width
      visible: !!root.detail && root.detail.open
      textFormat: Text.PlainText
      text: "None of this is required. An agent explains only what it can prove."
      color: Color.muted
      wrapMode: Text.Wrap
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
    }
  }
}
