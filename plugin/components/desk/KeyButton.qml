import QtQuick
import qs.Ui

// A form's writing button as a Tab stop (WP-173). A focusable qs.Ui Button
// clicks on every Return, Enter or Space press, a held key's auto-repeats
// too (Omarchy's Hyprland repeats after 250 ms, 40 a second): a held key
// on *Import* would import again and again. Here the key presses it once
// per press and a repeat does nothing; a click is a click. Disabled
// (`enabled: false`), it is no Tab stop and neither key nor click acts.
Item {
  id: root

  property alias text: button.text
  property alias iconText: button.iconText
  property alias iconSpinning: button.iconSpinning
  property alias iconSize: button.iconSize
  property alias selected: button.selected
  property alias bordered: button.bordered
  property alias foreground: button.foreground
  property alias accent: button.accent
  property alias fontFamily: button.fontFamily
  property alias fontSize: button.fontSize
  property alias verticalPadding: button.verticalPadding
  property alias tooltipText: button.tooltipText

  // keyPressed's calls (the harness's key guard read-out).
  readonly property string keyGuard: "button"
  property int keyEvents: 0

  signal clicked()

  activeFocusOnTab: true
  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  function keyPressed(event) {
    root.keyEvents++
    event.accepted = true
    if (!event.isAutoRepeat && root.enabled) root.clicked()
  }

  Keys.onReturnPressed: function(event) { root.keyPressed(event) }
  Keys.onEnterPressed: function(event) { root.keyPressed(event) }
  Keys.onSpacePressed: function(event) { root.keyPressed(event) }

  Button {
    id: button
    anchors.fill: parent
    hasCursor: root.activeFocus
    onClicked: root.clicked()
  }
}
