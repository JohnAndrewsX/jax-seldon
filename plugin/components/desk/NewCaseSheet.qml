import QtQuick
import qs.Commons
import qs.Ui
import "../../Model.js" as Model

// The new-case sheet in the Work section's detail (desk section 3,
// SPEC-PLUGIN §5.4; WP-020), the hand-made case: title, zone,
// risk, priority and an optional area. Enter in a text field (or *Create*)
// sends `seldon plan new --zone <z> --risk <r> [--area <a>] [--priority <p>]
// --json -- <title>` through Service.plan(); the title is one argument after
// `--`, exactly as typed. Zone, risk and priority start at the engine's
// defaults (yellow, R1, normal). The area must be a lowercase slug; a wrong
// one is refused here, before the engine is asked. The fields keep their
// text until the engine has created the case, so a refused case is never
// lost (the QuickEntry pattern); then the sheet empties and reports
// `created(caseId)`.
//
// Keyboard: while anything in the sheet has focus the section is `editing`
// and the desk keeps out of the keys. Tab walks title → zone → risk → priority → area →
// Create → Cancel; in a picker h/l or ←/→ move and Enter or Space picks;
// Esc closes the sheet and gives the keys back.
FocusScope {
  id: root

  property var service: null
  property color foreground: Color.foreground
  property color accent: Color.accent
  property color urgent: Color.urgent
  property string fontFamily: Style.font.family

  property string zone: Model.NEW_CASE_DEFAULTS.zone
  property string risk: Model.NEW_CASE_DEFAULTS.risk
  property string priority: Model.NEW_CASE_DEFAULTS.priority
  // The title of the case being created, until the engine answers.
  property string sentTitle: ""
  // Why the last Create did not go out although the form was fine: another
  // case action was pending (Service.busyRefusal). Cleared by the next Create.
  property string notice: ""

  property alias title: titleField.text
  property alias area: areaField.text

  readonly property bool editing: root.activeFocus
  readonly property bool enabledHere: !!service && service.canWrite
  readonly property var result: service && service.planResult && service.planResult.action === "new" ? service.planResult : null
  readonly property bool pending: !!result && result.pending
  // A created case is reported in the Work list; the sheet shows progress and
  // refusals, and the busy notice in place of either.
  readonly property string resultText: root.notice !== "" ? root.notice
    : result && (result.pending || !result.ok) ? result.text : ""
  readonly property bool areaValid: root.area === "" || Model.AREA.test(root.area)
  readonly property color dim: Util.alpha(foreground, 0.65)
  readonly property real labelWidth: Style.space(64)

  signal leaveRequested()
  signal created(string caseId)

  function focusTitle() {
    titleField.forceActiveFocus()
  }

  // Return or Enter in a field (WP-173): once per press; a held key's
  // auto-repeat (Omarchy's Hyprland, after 250 ms) does nothing.
  // `keyEvents` counts the calls (the harness's key guard read-out).
  readonly property string keyGuard: "sheet"
  property int keyEvents: 0

  function keyPressed(event) {
    root.keyEvents++
    event.accepted = true
    if (!event.isAutoRepeat) root.submit()
  }

  function submit() {
    if (!root.service || root.pending) return false
    var refusals = root.service.busyRefusals
    var sent = root.service.plan("new", {
      title: root.title, zone: root.zone, risk: root.risk, area: root.area, priority: root.priority
    })
    root.notice = !sent && root.service.busyRefusals !== refusals ? root.service.busyRefusal.text : ""
    if (sent) root.sentTitle = root.title
    return sent
  }

  function reset() {
    root.title = ""
    root.area = ""
    root.zone = Model.NEW_CASE_DEFAULTS.zone
    root.risk = Model.NEW_CASE_DEFAULTS.risk
    root.priority = Model.NEW_CASE_DEFAULTS.priority
  }

  onResultChanged: {
    if (!root.result || root.result.pending || root.sentTitle === "") return
    var sent = root.sentTitle
    root.sentTitle = ""
    if (!root.result.ok) return
    if (root.title === sent) root.reset()
    root.created(root.result.caseId)
  }

  Keys.onEscapePressed: function(event) {
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

    TextField {
      id: titleField
      width: parent.width
      enabled: root.enabledHere
      placeholderText: root.enabledHere ? "Title, Enter creates the case"
        : root.service ? root.service.writeBlocker : "The Seldon service is not running"
      foreground: root.foreground
      accent: root.accent
      font.family: root.fontFamily
      font.pixelSize: Style.font.bodySmall
      Keys.onReturnPressed: function(event) { root.keyPressed(event) }
      Keys.onEnterPressed: function(event) { root.keyPressed(event) }
    }

    FormRow {
      width: parent.width
      label: "Zone"
      labelWidth: root.labelWidth
      labelColor: root.dim
      fontFamily: root.fontFamily

      ButtonGroup {
        options: Model.ZONES
        value: root.zone
        enabled: root.enabledHere
        foreground: root.foreground
        accent: root.accent
        fontFamily: root.fontFamily
        fontSize: Style.font.caption
        onChanged: function(v) { root.zone = v }
      }
    }

    FormRow {
      width: parent.width
      label: "Risk"
      labelWidth: root.labelWidth
      labelColor: root.dim
      fontFamily: root.fontFamily

      ButtonGroup {
        options: Model.RISKS
        value: root.risk
        enabled: root.enabledHere
        foreground: root.foreground
        accent: root.accent
        fontFamily: root.fontFamily
        fontSize: Style.font.caption
        onChanged: function(v) { root.risk = v }
      }
    }

    FormRow {
      width: parent.width
      label: "Priority"
      labelWidth: root.labelWidth
      labelColor: root.dim
      fontFamily: root.fontFamily

      ButtonGroup {
        options: Model.PRIORITIES
        value: root.priority
        enabled: root.enabledHere
        foreground: root.foreground
        accent: root.accent
        fontFamily: root.fontFamily
        fontSize: Style.font.caption
        onChanged: function(v) { root.priority = v }
      }
    }

    FormRow {
      width: parent.width
      label: "Area"
      labelWidth: root.labelWidth
      labelColor: root.dim
      fontFamily: root.fontFamily

      TextField {
        id: areaField
        width: parent.width
        enabled: root.enabledHere
        placeholderText: "optional, e.g. dev-env"
        foreground: root.areaValid ? root.foreground : root.urgent
        accent: root.accent
        font.family: root.fontFamily
        font.pixelSize: Style.font.bodySmall
        Keys.onReturnPressed: function(event) { root.keyPressed(event) }
        Keys.onEnterPressed: function(event) { root.keyPressed(event) }
      }
    }

    Text {
      width: parent.width
      visible: !root.areaValid
      textFormat: Text.PlainText
      text: "Area: lowercase letters, digits and -, starting with a letter or digit"
      color: root.urgent
      wrapMode: Text.Wrap
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
    }

    Row {
      spacing: Style.spacing.sm

      KeyButton {
        objectName: "caseCreate"
        text: root.pending ? "Creating" : "Create"
        iconText: root.pending ? "󰦖" : ""
        iconSpinning: root.pending
        iconSize: Style.font.caption
        enabled: root.enabledHere && !root.pending
        selected: true
        bordered: true
        foreground: root.foreground
        accent: root.accent
        fontFamily: root.fontFamily
        fontSize: Style.font.caption
        verticalPadding: Style.spacing.xs
        tooltipText: "Enter in a text field"
        onClicked: root.submit()
      }

      Button {
        text: "Cancel"
        focusable: true
        bordered: true
        foreground: root.foreground
        accent: root.accent
        fontFamily: root.fontFamily
        fontSize: Style.font.caption
        verticalPadding: Style.spacing.xs
        tooltipText: "Esc; the fields keep their text"
        onClicked: root.leaveRequested()
      }
    }

    Text {
      width: parent.width
      visible: text !== ""
      textFormat: Text.PlainText
      text: root.resultText
      color: root.notice === "" && root.result && !root.result.ok ? root.urgent : root.dim
      wrapMode: Text.Wrap
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
    }
  }
}
