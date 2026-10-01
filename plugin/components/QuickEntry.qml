import QtQuick
import qs.Commons
import qs.Ui
import "../Model.js" as Model

// QuickEntry on the Today tab (SPEC-PLUGIN §5): a note for today's journal,
// optionally for one open case. Enter sends `seldon log [--case <id>] --json
// -- <text>` through Service.log(); the text is one argument after `--`,
// exactly as typed. Blank text is refused here. The line under the field
// shows the event id or the engine's error.
//
// Keyboard: `n` on the Today tab focuses the field (Panel.qml). While the
// field or the case picker has focus, Panel.qml blocks its own keys
// (`editing`); Tab moves from the field to the picker, Esc leaves. A
// FocusScope, so `activeFocus` covers the picker's inner trigger too.
FocusScope {
  id: root

  property var service: null
  property var indexData: null
  property color foreground: Color.foreground
  property color urgent: Color.urgent
  property string fontFamily: Style.font.family

  property string caseId: ""

  // Keys belong to the field or the picker, not to the panel.
  readonly property bool editing: root.activeFocus || picker.popupOpen
  readonly property bool enabledHere: !!service && service.canWrite
  readonly property var options: Model.caseOptions(indexData)
  readonly property var result: service ? service.logResult : null
  readonly property bool pending: !!result && result.pending
  readonly property string resultText: result ? result.text : ""
  readonly property color dim: Util.alpha(foreground, 0.65)
  property alias text: field.text

  // Asked to give the keys back to the panel (Esc).
  signal leaveRequested()

  function focusField() {
    if (root.enabledHere) field.forceActiveFocus()
  }

  function setCase(id) {
    root.caseId = id
    picker.value = id
  }

  function submit() {
    if (!root.service) return false
    var sent = root.service.log(field.text, root.caseId)
    if (sent) field.text = ""
    return sent
  }

  function leave() {
    if (picker.popupOpen) picker.close()
    root.leaveRequested()
  }

  // A case that is no longer open drops out of the picker; so does its id.
  onOptionsChanged: {
    for (var i = 0; i < root.options.length; i++)
      if (root.options[i].value === root.caseId) return
    root.setCase("")
  }

  implicitHeight: column.implicitHeight

  Column {
    id: column
    width: parent.width
    spacing: Style.spacing.sm

    TextField {
      id: field
      width: parent.width
      enabled: root.enabledHere
      placeholderText: root.enabledHere ? "Note for today's journal, Enter saves"
        : root.service ? root.service.writeBlocker : "The Seldon service is not running"
      foreground: root.foreground
      font.family: root.fontFamily
      font.pixelSize: Style.font.bodySmall
      onAccepted: root.submit()
      Keys.onEscapePressed: function(event) {
        root.leave()
        event.accepted = true
      }
    }

    Item {
      width: parent.width
      implicitHeight: Math.max(picker.implicitHeight, openCase.implicitHeight)
      visible: root.options.length > 1

      Dropdown {
        id: picker
        anchors.left: parent.left
        anchors.right: openCase.visible ? openCase.left : parent.right
        anchors.rightMargin: openCase.visible ? Style.spacing.sm : 0
        anchors.verticalCenter: parent.verticalCenter
        showLabel: false
        fontFamily: root.fontFamily
        options: root.options
        value: root.caseId
        enabled: root.enabledHere
        onChanged: function(v) { root.caseId = v }
        Keys.onEscapePressed: function(event) {
          root.leave()
          event.accepted = true
        }
      }

      Button {
        id: openCase
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        visible: root.caseId !== ""
        text: "Open case"
        tooltipText: "Open " + root.caseId + " in the editor"
        bordered: true
        foreground: root.foreground
        fontFamily: root.fontFamily
        fontSize: Style.font.caption
        verticalPadding: Style.spacing.xs
        onClicked: if (root.service) root.service.openInEditor(root.caseId)
      }
    }

    Text {
      width: parent.width
      visible: text !== ""
      textFormat: Text.PlainText
      text: root.resultText
      color: root.result && !root.result.ok ? root.urgent : root.dim
      elide: Text.ElideRight
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
    }
  }
}
