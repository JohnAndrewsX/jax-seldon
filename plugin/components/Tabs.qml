pragma ComponentBehavior: Bound

import QtQuick
import qs.Commons
import qs.Ui

// The panel's tab strip (SPEC-PLUGIN §5). Same pattern as the provider
// switch of the built-in agents panel: one bordered Button per tab, the
// current one selected. It only reports clicks; Panel.qml owns the current
// index and the keyboard.
//
// Widths (WP-039): a cell is never narrower than its label in bold (the
// selected look) plus the Button's padding, so selecting a tab never
// changes the layout. When every label fits an equal share, the cells share
// the width equally; when they fit only at their own widths, each cell gets
// its width plus an equal part of the rest; when they do not fit at all
// (a large font with `[spacing] scale-with-font = false`), the strip wraps
// onto a second line instead of clipping a label.
Flow {
  id: root

  property var tabs: []
  // The digit that selects each tab (Model.TAB_KEYS), for the tooltip.
  property var keys: []
  property int currentIndex: 0
  property color foreground: Color.foreground
  property string fontFamily: Style.font.family
  property real fontSize: Style.font.bodySmall

  signal activated(int index)

  // Each cell's own width, in tab order (0 until its Button exists).
  readonly property var naturalWidths: {
    var out = []
    for (var i = 0; i < repeater.count; i++) {
      var cell = repeater.itemAt(i) as TabCell
      out.push(cell ? cell.naturalWidth : 0)
    }
    return out
  }
  readonly property real gaps: spacing * Math.max(0, tabs.length - 1)
  readonly property real naturalTotal: naturalWidths.reduce(function(a, w) { return a + w }, 0)
  readonly property bool fitsOneLine: naturalTotal + gaps <= width
  readonly property real equalShare: tabs.length > 0 ? Math.floor((width - gaps) / tabs.length) : 0
  readonly property bool equalFits: naturalWidths.every(function(w) { return w <= root.equalShare })
  // The cells' widths in whole pixels; on one line the last cell takes the
  // rounding rest, so the strip ends flush with the panel.
  readonly property var cellWidths: {
    if (!fitsOneLine) return naturalWidths
    var rest = (width - gaps - naturalTotal) / Math.max(1, tabs.length)
    var out = naturalWidths.map(function(w) { return root.equalFits ? root.equalShare : Math.floor(w + rest) })
    if (out.length > 0) {
      var used = out.slice(0, -1).reduce(function(a, w) { return a + w }, 0)
      out[out.length - 1] = Math.floor(width - gaps - used)
    }
    return out
  }

  spacing: Style.spacing.md

  component TabCell: Button {
    id: cell

    // Bold is wider in most fonts; the Button's label is bold only while
    // selected, so add the difference while it is not.
    readonly property real naturalWidth: Math.ceil(implicitWidth
      + (selected ? 0 : Math.max(0, boldLabel.advanceWidth - plainLabel.advanceWidth)))

    TextMetrics {
      id: plainLabel
      font.family: cell.fontFamily
      font.pixelSize: cell.fontSize
      text: cell.text
    }

    TextMetrics {
      id: boldLabel
      font.family: cell.fontFamily
      font.pixelSize: cell.fontSize
      font.bold: true
      text: cell.text
    }
  }

  Repeater {
    id: repeater
    model: root.tabs

    TabCell {
      required property var modelData
      required property int index

      width: root.cellWidths[index] || 0
      text: modelData
      tooltipText: root.keys[index] ? "Key " + root.keys[index] : ""
      selected: index === root.currentIndex
      bordered: true
      foreground: root.foreground
      fontFamily: root.fontFamily
      fontSize: root.fontSize
      onClicked: root.activated(index)
    }
  }
}
