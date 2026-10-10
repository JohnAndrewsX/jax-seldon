import QtQuick
import qs.Commons
import ".."

// A thin progress bar (prototype `.progress`): a track in the divider tone,
// the done part in the accent. `value` 0–1.
Item {
  id: root

  property real value: 0
  property color foreground: Color.popups.text
  property color accent: Color.accent

  readonly property Tone tone: Tone {}

  implicitHeight: Math.max(2, Style.space(4))

  Rectangle {
    anchors.fill: parent
    radius: height / 2
    color: root.tone.divider
  }

  Rectangle {
    width: parent.width * Math.max(0, Math.min(1, root.value))
    height: parent.height
    radius: height / 2
    color: root.accent
  }
}
