import QtQuick
import qs.Commons
import qs.Ui
import "../../Model.js" as Model

// New decision, in the Decisions section's detail pane (ADR-0034 §2,
// WP-123; the 0.1 panel's NewDecisionSheet, WP-023): one title. It sends
// `seldon decide --no-edit --json -- <title>` through Service.decide(); the
// title is one argument after `--`, exactly as typed. Once the engine has
// created the decision, the service opens it in the editor (`seldon open
// <id> --editor --json`, the id from the engine's answer, checked against
// the schema pattern).
//
// Writing arms twice (SPEC-PLUGIN §5): Enter in the title field or on
// *Create* arms the call and shows "Press Enter again: …", the second Enter
// runs it; a click on *Create* runs it at once. Any change to the title
// disarms. The title stays until the engine has created the decision, so a
// refusal never loses it; then the form empties and reports
// `created(decisionId)`. While another decision is pending, Create still
// asks the service, which refuses; the form then shows its busy text
// (Service.busyRefusal) in the neutral tone until the next Create or a
// change to the title (WP-078).
//
// Keyboard: while anything in the form has focus, the desk stays out of
// the keys (Section.editing). Tab walks title → Create → Cancel; Esc
// leaves the form, gives the keys back and keeps the title.
FocusScope {
  id: root

  property var service: null
  property color foreground: Color.popups.text
  property color accent: Color.accent
  property color urgent: Color.urgent
  property string fontFamily: Style.font.family

  // The argument list waiting for its second Enter, as JSON, or "".
  property string armedSig: ""
  // The title sent, until the engine answers.
  property string sentTitle: ""
  // A refusal of the plugin's own (nothing reached the engine), or the
  // service's busy text.
  property string notice: ""

  property alias title: titleField.text

  readonly property bool editing: root.activeFocus
  readonly property bool canWrite: !!service && service.canWrite
  readonly property string writeBlocker: service ? service.writeBlocker : "The Seldon service is not running"
  readonly property var result: service ? service.decideResult : null
  readonly property bool pending: !!result && result.pending
  // This form's own decision is pending (the guard of Enter and Create).
  readonly property bool ownPending: root.pending && root.sentTitle !== ""
  // Create takes a click unless this form's own decision is pending, so a
  // click during another call of the service gets the busy text.
  readonly property bool submitEnabled: root.canWrite && !root.ownPending
  readonly property var built: Model.decideArgs(root.title)
  readonly property string sig: built.args ? JSON.stringify(built.args) : ""
  readonly property bool armed: sig !== "" && armedSig === sig
  // A created decision is reported by the section; the form shows progress and refusals.
  readonly property string resultText: root.notice !== "" ? root.notice
    : result && (result.pending || !result.ok) ? result.text : ""
  readonly property bool resultOk: root.notice !== "" ? root.notice === Model.BUSY_TEXT : !!result && result.ok
  readonly property string hint: !root.canWrite ? root.writeBlocker
    : root.armed ? "Press Enter again: create the decision “" + root.title + "”"
    : ""
  readonly property color dim: Util.alpha(foreground, 0.65)

  signal leaveRequested()
  signal created(string decisionId)

  function focusTitle() {
    titleField.forceActiveFocus()
  }

  // Return, Enter (and Space on the button) in the form: a held key
  // repeats its press (Omarchy's Hyprland: repeat_delay 250 ms), and the
  // repeat neither arms nor runs (SPEC-PLUGIN §5.3).
  function keyPressed(event) {
    event.accepted = true
    if (!event.isAutoRepeat) root.enterKey()
  }

  // Enter in the title field or on Create: arm, then run.
  function enterKey() {
    if (!root.canWrite || root.ownPending) return false
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

  // A click on Create runs at once.
  function clickSubmit() {
    if (!root.canWrite || root.ownPending) return false
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
    var title = root.title
    var refusals = root.service.busyRefusals
    var sent = root.service.decide(title)
    if (!sent && root.service.busyRefusals !== refusals) root.notice = root.service.busyRefusal.text
    if (sent) root.sentTitle = title
    return sent
  }

  function close() {
    root.armedSig = ""
    root.leaveRequested()
  }

  onTitleChanged: {
    root.armedSig = ""
    root.notice = ""
  }
  onVisibleChanged: if (!visible) root.armedSig = ""
  onResultChanged: {
    if (!root.result || root.result.pending || root.sentTitle === "") return
    var sent = root.sentTitle
    root.sentTitle = ""
    if (!root.result.ok) return
    if (root.title === sent) root.title = ""
    root.created(root.result.decisionId)
  }

  Keys.onEscapePressed: function(event) {
    root.close()
    event.accepted = true
  }

  implicitHeight: column.implicitHeight

  Column {
    id: column
    width: parent.width
    spacing: Style.spacing.md

    Text {
      width: parent.width
      textFormat: Text.PlainText
      text: "New decision"
      color: root.foreground
      wrapMode: Text.Wrap
      font.family: root.fontFamily
      font.pixelSize: Style.font.title
      font.bold: true
    }

    Text {
      width: parent.width
      textFormat: Text.PlainText
      text: "The engine writes it to decisions/ as proposed and opens it in the editor."
      color: Color.muted
      wrapMode: Text.Wrap
      font.family: root.fontFamily
      font.pixelSize: Style.font.bodySmall
    }

    TextField {
      id: titleField
      width: parent.width
      enabled: root.canWrite
      placeholderText: root.canWrite ? "Title, Enter twice creates the decision" : root.writeBlocker
      foreground: root.foreground
      accent: root.accent
      font.family: root.fontFamily
      font.pixelSize: Style.font.bodySmall
      Keys.onReturnPressed: function(event) { root.keyPressed(event) }
      Keys.onEnterPressed: function(event) { root.keyPressed(event) }
    }

    Row {
      spacing: Style.spacing.sm

      // Create: a Tab stop whose Enter arms first (a qs.Ui Button's own
      // Enter would run at once); a click runs.
      Item {
        id: submitKey
        activeFocusOnTab: true
        implicitWidth: submitButton.implicitWidth
        implicitHeight: submitButton.implicitHeight
        Keys.onReturnPressed: function(event) { root.keyPressed(event) }
        Keys.onEnterPressed: function(event) { root.keyPressed(event) }
        Keys.onSpacePressed: function(event) { root.keyPressed(event) }

        Button {
          id: submitButton
          objectName: "decisionSubmit"
          anchors.fill: parent
          text: root.ownPending ? "Creating" : "Create"
          iconText: root.ownPending ? "󰦖" : ""
          iconSpinning: root.ownPending
          iconSize: Style.font.caption
          enabled: root.submitEnabled
          hasCursor: submitKey.activeFocus || root.armed
          selected: true
          bordered: true
          foreground: root.foreground
          accent: root.accent
          fontFamily: root.fontFamily
          fontSize: Style.font.caption
          verticalPadding: Style.spacing.xs
          tooltipText: "Enter twice, or click"
          onClicked: root.clickSubmit()
        }
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
        tooltipText: "Esc; the title is kept"
        onClicked: root.close()
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
