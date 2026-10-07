import QtQuick
import qs.Commons
import qs.Ui

// The sidebar search (ADR-0034 §2): filters the current section's list.
// `/` gives it the keys (Desk.qml). Enter leaves the field and keeps the
// filter; Esc clears it and leaves. Either way the desk gets its keys back
// (`done`).
Item {
  id: root

  property string text: ""
  readonly property bool focused: field.activeFocus

  signal edited(string text)
  signal done()

  function focusField() {
    field.forceActiveFocus()
  }

  implicitWidth: Style.space(180)
  implicitHeight: field.implicitHeight

  TextField {
    id: field
    objectName: "deskSearch"
    anchors.fill: parent
    placeholderText: "Search… (/)"
    text: root.text
    font.pixelSize: Style.font.bodySmall
    onTextEdited: root.edited(field.text)
    Keys.onEscapePressed: function(event) {
      root.edited("")
      event.accepted = true
      root.done()
    }
    Keys.onReturnPressed: function(event) {
      event.accepted = true
      root.done()
    }
    Keys.onEnterPressed: function(event) {
      event.accepted = true
      root.done()
    }
  }
}
