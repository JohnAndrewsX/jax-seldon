import QtQuick
import qs.Commons
import ".."

// A desk list's group header (Today's NEEDS YOU / JOURNAL, the Changelog's
// day, Work's ACTIVE · 2) in dim small capitals: GroupedRow draws its
// header with it. Work's group the summary counts but the index lists no
// row of (a cut index, ADR-0053 §2, WP-206) adds a dim line (`text`) and
// the command that lists them (`command`) on a line of its own, never
// wrapped, so it reads and copies whole. A gap holds no row, so the cursor
// never stops on it.
Column {
  id: root

  property string header: ""
  property string text: ""
  property string command: ""
  property string fontFamily: Style.font.family

  readonly property Tone tone: Tone {}

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

  Text {
    visible: root.text !== ""
    width: root.width
    textFormat: Text.PlainText
    text: root.text
    color: root.tone.dim
    wrapMode: Text.Wrap
    font.family: root.fontFamily
    font.pixelSize: Style.font.bodySmall
  }

  Text {
    visible: root.command !== ""
    width: root.width
    textFormat: Text.PlainText
    text: root.command
    color: root.tone.dim
    elide: Text.ElideRight
    font.family: root.fontFamily
    font.pixelSize: Style.font.bodySmall
  }
}
