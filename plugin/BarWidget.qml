import QtQuick
import qs.Commons
import qs.Ui

// Seldon pill for the bar (SPEC-PLUGIN §4).
//
// Stub (WP-001): renders the glyph with the theme's bar colours. Counts,
// tooltip detail and the Panel arrive in later work packages.
BarWidget {
  id: root
  moduleName: "jax.seldon"

  // Shape contract for shell.summon/hide/toggle routing: Bar.findPanelWidget
  // requires open/close/opened on the bar-widget root. No panel yet.
  readonly property bool opened: false

  function open() {}
  function close() {}

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  WidgetButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: "⟡"
    fontSize: Style.font.body
    tooltipText: "Seldon"
  }
}
