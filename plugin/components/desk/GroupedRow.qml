pragma ComponentBehavior: Bound

import QtQuick
import qs.Commons
import qs.Commons as Commons
import ".."

// A desk list delegate with an optional group header above its row (the
// Changelog's day, Today's NEEDS YOU / JOURNAL, Work's ACTIVE · 2): the
// header (GroupHeader, dim small capitals) when `header` is set (the
// first row of its group), then a ListRow. The ListView stays one list,
// so the cursor walks rows only. `gaps` ([{ header, text, command }],
// Work's, WP-206): groups with no listed row that come before this row's
// group (a hand-made index; the cut ladder leaves them after the last
// row, in Work's footer), each a GroupHeader above the header.
Column {
  id: root

  property string header: ""
  property var gaps: []
  property alias title: row.title
  property alias meta: row.meta
  property alias aside: row.aside
  property alias mark: row.mark
  property alias alert: row.alert
  property alias selected: row.selected
  property alias cursor: row.cursor
  property color foreground: Commons.Color.popups.text
  property string fontFamily: Style.font.family

  signal clicked()

  spacing: Style.spacing.sm

  Repeater {
    model: root.gaps

    GroupHeader {
      required property var modelData

      width: root.width
      header: modelData.header
      text: modelData.text
      command: modelData.command
      fontFamily: root.fontFamily
    }
  }

  GroupHeader {
    visible: root.header !== ""
    width: root.width
    header: root.header
    fontFamily: root.fontFamily
  }

  ListRow {
    id: row
    width: root.width
    height: implicitHeight
    foreground: root.foreground
    fontFamily: root.fontFamily
    onClicked: root.clicked()
  }
}
