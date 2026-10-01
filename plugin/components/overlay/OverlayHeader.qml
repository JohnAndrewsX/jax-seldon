import QtQuick
import qs.Commons
import qs.Ui

// Row 1 of the Prime Radiant (SPEC-PLUGIN §6): the title, machine, Omarchy
// version and index time on the left; the period selector and the close
// button on the right. It only reports clicks; Overlay.qml owns the period.
Item {
  id: root

  property string meta: ""
  property string caption: ""
  property var periods: []
  property string period: ""
  property color foreground: Color.popups.text
  property color accent: Color.accent
  property string fontFamily: Style.font.family

  signal periodRequested(string period)
  signal closeRequested()

  implicitHeight: Math.max(titles.implicitHeight, controls.implicitHeight)

  Column {
    id: titles
    anchors.left: parent.left
    anchors.right: controls.left
    anchors.rightMargin: Style.spacing.panelGap
    anchors.verticalCenter: parent.verticalCenter
    spacing: Style.spacing.xs

    Text {
      width: parent.width
      textFormat: Text.PlainText
      text: "Prime Radiant"
      color: root.foreground
      font.family: root.fontFamily
      font.pixelSize: Style.font.display
      font.bold: true
      elide: Text.ElideRight
    }

    Text {
      width: parent.width
      visible: root.meta !== ""
      textFormat: Text.PlainText
      text: root.meta
      color: Color.muted
      font.family: root.fontFamily
      font.pixelSize: Style.font.body
      elide: Text.ElideRight
    }
  }

  Column {
    id: controls
    anchors.right: parent.right
    anchors.verticalCenter: parent.verticalCenter
    spacing: Style.spacing.xs

    Row {
      anchors.right: parent.right
      spacing: Style.spacing.panelGap

      ButtonGroup {
        anchors.verticalCenter: parent.verticalCenter
        options: root.periods
        value: root.period
        focusable: false
        foreground: root.foreground
        accent: root.accent
        fontFamily: root.fontFamily
        onChanged: function(value) { root.periodRequested(value) }
      }

      Button {
        anchors.verticalCenter: parent.verticalCenter
        text: "Close"
        tooltipText: "Esc"
        foreground: root.foreground
        accent: root.accent
        fontFamily: root.fontFamily
        bordered: true
        onClicked: root.closeRequested()
      }
    }

    Text {
      anchors.right: parent.right
      textFormat: Text.PlainText
      text: root.caption
      color: Color.muted
      font.family: root.fontFamily
      font.pixelSize: Style.font.bodySmall
    }
  }
}
