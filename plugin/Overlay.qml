import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.Commons
import qs.Ui

// Prime Radiant, the fullscreen overlay (SPEC-PLUGIN §6).
//
// Stub (WP-001): opens and closes through the shell's overlay loader
// (`omarchy-shell shell toggle jax.seldon`), shows the title, and closes on
// Escape or click. Charts arrive in later work packages.
Item {
  id: root

  // Injected by omarchy-shell's panel loader.
  property var shell: null
  property var manifest: null
  property var service: null

  property bool opened: false

  function open(payloadJson) {
    root.opened = true
    Qt.callLater(function() { keyCatcher.forceActiveFocus() })
  }

  function close() {
    root.opened = false
  }

  function dismiss() {
    root.opened = false
    if (root.shell && typeof root.shell.hide === "function")
      root.shell.hide((root.manifest && root.manifest.id) || "jax.seldon")
  }

  PanelWindow {
    id: window
    visible: root.opened
    anchors { top: true; bottom: true; left: true; right: true }
    color: Color.menu.scrim
    WlrLayershell.namespace: "jax-seldon-prime-radiant"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive
    exclusionMode: ExclusionMode.Ignore

    MouseArea {
      anchors.fill: parent
      onClicked: root.dismiss()
    }

    BorderSurface {
      anchors.fill: parent
      anchors.margins: Style.gapsOut
      radius: Style.cornerRadius
      color: Color.popups.background
      borderSpec: Border.surfaceSpec("popups", "border", Color.popups.border, Math.max(1, Style.space(2)))
      padding: Style.spacing.panelPadding

      Item {
        id: keyCatcher
        anchors.fill: parent
        focus: true
        Keys.onEscapePressed: function(event) {
          root.dismiss()
          event.accepted = true
        }

        Text {
          anchors.centerIn: parent
          textFormat: Text.PlainText
          text: "Prime Radiant"
          color: Color.popups.text
          font.family: Style.font.family
          font.pixelSize: Style.font.title
        }
      }
    }
  }
}
