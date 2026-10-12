import QtQuick
import qs.Commons
import ".."

// A desk list group the summary counts but the index lists no row of
// (Work's "ACTIVE · 0 / 40" on a cut index, ADR-0053 rung 21, WP-206):
// the header as GroupedRow draws it, then one dim line on where the rows
// are. It holds no row, so the cursor never stops on it.
Column {
  id: root

  property string header: ""
  property string text: ""
  property string fontFamily: Style.font.family

  readonly property Tone tone: Tone {}

  spacing: Style.spacing.sm

  Text {
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
}
