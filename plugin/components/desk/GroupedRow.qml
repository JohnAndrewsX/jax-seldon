import QtQuick
import qs.Commons
import ".."

// A desk list delegate with an optional group header above its row (the
// Changelog's day, Today's NEEDS YOU / JOURNAL, Work's ACTIVE · 2): the
// header in dim small capitals when `header` is set (the first row of
// its group), then a ListRow. The ListView stays one list, so the cursor
// walks rows only.
Column {
  id: root

  property string header: ""
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
