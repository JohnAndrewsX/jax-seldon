pragma ComponentBehavior: Bound

import QtQuick
import qs.Commons
import ".."

// A desk list delegate with an optional group header above its row (the
// Changelog's day, Today's NEEDS YOU / JOURNAL, Work's ACTIVE · 2): the
// header in dim small capitals when `header` is set (the first row of
// its group), then a ListRow. The ListView stays one list, so the cursor
// walks rows only. `gaps` ([{ header, text }], Work's, WP-206): groups
// with no listed row that come before this row's group, each a GroupGap
// above the header.
Column {
  id: root

  property string header: ""
  property var gaps: []
  property alias title: row.title
  property alias meta: row.meta
  property alias aside: row.aside
  property alias stripe: row.stripe
  property alias alert: row.alert
  property alias selected: row.selected
  property alias cursor: row.cursor
  property color foreground: Color.popups.text
  property string fontFamily: Style.font.family

  readonly property Tone tone: Tone {}

  signal clicked()

  spacing: Style.spacing.sm

  Repeater {
    model: root.gaps

    GroupGap {
      required property var modelData

      width: root.width
      header: modelData.header
      text: modelData.text
      fontFamily: root.fontFamily
    }
  }

  Text {
    visible: root.header !== ""
    width: root.width
    topPadding: Style.spacing.md
    textFormat: Text.PlainText
    text: root.header.toUpperCase()
    color: root.tone.dim
    elide: Text.ElideRight
    font.family: root.fontFamily
    font.pixelSize: Style.font.caption
    font.letterSpacing: Style.space(1)
    font.bold: true
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
