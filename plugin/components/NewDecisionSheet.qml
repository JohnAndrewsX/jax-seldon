import QtQuick
import qs.Commons
import qs.Ui
import "../Model.js" as Model

// The new-decision sheet on the Decisions tab (SPEC-PLUGIN §5, WP-023): one
// title. It sends `seldon decide --no-edit --json -- <title>` through
// Service.decide(); the title is one argument after `--`, exactly as typed.
// Once the engine has created the decision, the service opens it in the
// editor (`seldon open <id> --editor --json`, the id from the engine's
// answer, checked against the schema pattern).
//
// Writing follows the drift sheet (WP-021): Enter in the title field or on
// *Create* arms the call and shows "Press Enter again: …", the second Enter
// runs it; a click on *Create* runs it at once. Any change to the title
// disarms. The title stays until the engine has created the decision, so a
// refusal never loses it; then the sheet empties and reports
// `created(decisionId)`.
//
// Keyboard: while anything in the sheet has focus, Panel.qml blocks its own
// keys (`editing`). Tab walks title → Create → Cancel; Esc closes the sheet,
// gives the keys back and keeps the title.
FocusScope {
  id: root

  property var service: null
  property color foreground: Color.foreground
  property color accent: Color.accent
  property color urgent: Color.urgent
  property string fontFamily: Style.font.family

  // The argument list waiting for its second Enter, as JSON, or "".
  property string armedSig: ""
  // The title sent, until the engine answers.
  property string sentTitle: ""
  // A refusal of the plugin's own (nothing reached the engine).
  property string notice: ""

  property alias title: titleField.text

  readonly property bool editing: root.activeFocus
  readonly property bool canWrite: !!service && service.canWrite
  readonly property string writeBlocker: service ? service.writeBlocker : "The Seldon service is not running"
  readonly property var result: service ? service.decideResult : null
  readonly property bool pending: !!result && result.pending
  readonly property var built: Model.decideArgs(root.title)
  readonly property string sig: built.args ? JSON.stringify(built.args) : ""
  readonly property bool armed: sig !== "" && armedSig === sig
  // A created decision is reported on the tab; the sheet shows progress and refusals.
  readonly property string resultText: root.notice !== "" ? root.notice
    : result && (result.pending || !result.ok) ? result.text : ""
  readonly property bool resultOk: root.notice === "" && !!result && result.ok
  readonly property string hint: !root.canWrite ? root.writeBlocker
    : root.armed ? "Press Enter again: create the decision “" + root.title + "”"
    : ""
  readonly property color dim: Util.alpha(foreground, 0.65)

  signal leaveRequested()
  signal created(string decisionId)

  function focusTitle() {
    titleField.forceActiveFocus()
  }

  // Enter in the title field or on Create: arm, then run.
  function enterKey() {
    if (!root.canWrite || root.pending) return false
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
    if (!root.canWrite || root.pending) return false
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
    var sent = root.service.decide(title)
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

    PanelSectionHeader {
      text: "NEW DECISION"
      foreground: root.foreground
      fontFamily: root.fontFamily
    }

    Text {
      width: parent.width
      textFormat: Text.PlainText
      text: "The engine writes it to decisions/ as proposed and opens it in the editor."
      color: root.dim
      wrapMode: Text.Wrap
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
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
      onAccepted: root.enterKey()
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
        Keys.onReturnPressed: root.enterKey()
        Keys.onEnterPressed: root.enterKey()
        Keys.onSpacePressed: root.enterKey()

        Button {
          id: submitButton
          anchors.fill: parent
          text: root.pending ? "Creating" : "Create"
          iconText: root.pending ? "󰦖" : ""
          iconSpinning: root.pending
          iconSize: Style.font.caption
          enabled: root.canWrite && !root.pending
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
