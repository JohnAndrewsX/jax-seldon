import QtQuick
import qs.Commons
import ".."

// The ring on Seldon's own controls (WP-177; SPEC-PLUGIN §5.3, §7): a
// writing button while it holds the keys or is armed. In the theme's own
// focus border where that reaches 3:1 on the popup surface (Tone.themeFocus:
// the ring yields to the theme), else in the `ui` tone at two pixels. It
// lies over the control's own border, same box and radius, so the control
// keeps one frame and its size. qs.Ui fields keep Omarchy's focus look and
// get no ring.
Rectangle {
  id: root

  property bool shown: false

  objectName: "focusRing"

  readonly property Tone tone: Tone {}

  anchors.fill: parent
  visible: root.shown
  radius: Style.cornerRadius
  color: "transparent"
  border.color: root.tone.focusRing
  border.width: root.tone.focusRingWidth
}
