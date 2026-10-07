import QtQuick
import qs.Commons
import "../components/desk"

// A section the desk shell (WP-121) only frames: its list column and detail
// pane, empty, with the section's title and the work package that fills
// it. Replaced section by section in WP-122, WP-123 and WP-125.
Section {
  id: root

  readonly property string comingText: root.info && root.info.wp !== "" ? "Coming in " + root.info.wp + "." : ""

  function view() {
    return { stub: true, wp: root.info ? root.info.wp : "" }
  }

  ListColumn {
    id: list
    visible: !root.solo && (!root.stacked || !root.detailShown)
    width: root.solo ? 0 : root.stacked ? root.width : (root.layout ? root.layout.listW : 0)
    height: root.height
    divider: !root.stacked
    title: root.title
    emptyText: root.comingText
  }

  DetailPane {
    x: root.solo || root.stacked ? 0 : list.width
    width: root.width - x
    height: root.height
    visible: root.solo || !root.stacked || root.detailShown
    backVisible: root.stacked && !root.solo
    title: root.title
    onBackRequested: if (root.desk) root.desk.back()

    Text {
      width: parent.width
      textFormat: Text.PlainText
      text: root.title
      color: Color.popups.text
      wrapMode: Text.Wrap
      font.family: Style.font.family
      font.pixelSize: Style.font.title
      font.bold: true
    }

    Text {
      width: parent.width
      textFormat: Text.PlainText
      text: root.comingText
      color: Color.muted
      wrapMode: Text.Wrap
      font.family: Style.font.family
      font.pixelSize: Style.font.body
    }
  }
}
