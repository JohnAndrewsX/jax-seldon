import QtQuick
import qs.Commons

// A desk list delegate with an optional group header above its row (the
// Changelog's day, Today's NEEDS YOU / JOURNAL, Work's ACTIVE · 2): the
// header in muted small capitals when `header` is set (the first row of
// its group), then a ListRow. The ListView stays one list, so the cursor
// walks rows only.
Column {
  id: root

  property string header: ""
  property alias title: row.title
  property alias meta: row.meta
  property alias aside: row.aside
  property alias stripe: row.stripe
  property alias selected: row.selected
  property alias cursor: row.cursor
  property color foreground: Color.popups.text
  property string fontFamily: Style.font.family

  signal clicked()

  spacing: Style.spacing.sm

  Text {
    visible: root.header !== ""
    width: root.width
    topPadding: Style.spacing.md
    textFormat: Text.PlainText
    text: root.header.toUpperCase()
    color: Color.muted
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
