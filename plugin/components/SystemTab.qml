pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import qs.Commons
import qs.Ui
import "../Model.js" as Model

// System tab (SPEC-PLUGIN §5): Omarchy version and theme, package counts,
// deviations, plugins, snapshots, areas, collectors. Every field of
// index.system is optional; Model.systemSections() drops what is missing.
//
// Keyboard (forwarded by Panel.qml): ↑/↓ move the cursor.
Item {
  id: root

  property var indexData: null
  property double nowMs: Date.now()
  property bool cursorActive: false
  property color foreground: Color.foreground
  property color accent: Color.accent
  property string fontFamily: Style.font.family

  property int cursor: 0

  signal cursorWanted()

  readonly property var sections: Model.systemSections(indexData, nowMs)
  readonly property color dim: Util.alpha(foreground, 0.65)
  // One row per label/value pair; the first of a section carries its title.
  readonly property var rows: {
    var out = []
    for (var i = 0; i < sections.length; i++)
      for (var j = 0; j < sections[i].rows.length; j++)
        out.push({ section: j === 0 ? sections[i].title : "", label: sections[i].rows[j].label, value: sections[i].rows[j].value })
    return out
  }
  readonly property int rowCount: rows.length

  function move(dy) {
    if (root.rows.length === 0) return
    root.cursor = Math.max(0, Math.min(root.rows.length - 1, root.cursor + dy))
  }

  function activate() {
  }

  function textKey(t) {
    return false
  }

  onRowsChanged: if (root.cursor >= root.rows.length) root.cursor = Math.max(0, root.rows.length - 1)

  ListView {
    id: list
    anchors.fill: parent
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    interactive: contentHeight > height
    ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }

    model: root.rows
    currentIndex: root.cursorActive ? root.cursor : -1
    onCurrentIndexChanged: if (currentIndex >= 0) Qt.callLater(keepCurrentVisible)
    function keepCurrentVisible() {
      if (currentIndex >= 0) positionViewAtIndex(currentIndex, ListView.Contain)
    }

    delegate: Column {
      id: rowItem

      required property var modelData
      required property int index

      width: ListView.view.width
      topPadding: modelData.section !== "" && index > 0 ? Style.spacing.lg : 0

      PanelSectionHeader {
        visible: rowItem.modelData.section !== ""
        bottomPadding: Style.spacing.sm
        text: rowItem.modelData.section
        foreground: root.foreground
        fontFamily: root.fontFamily
      }

      CursorSurface {
        width: parent.width
        implicitHeight: Math.max(label.implicitHeight, value.implicitHeight) + Style.spacing.xs * 2
        hasCursor: root.cursorActive && rowItem.index === root.cursor
        foreground: root.foreground
        accent: root.accent

        Text {
          id: label
          x: Style.spacing.lg
          y: Style.spacing.xs
          width: parent.width * 0.42 - x
          textFormat: Text.PlainText
          text: rowItem.modelData.label
          color: root.dim
          elide: Text.ElideRight
          font.family: root.fontFamily
          font.pixelSize: Style.font.bodySmall
        }

        Text {
          id: value
          x: parent.width * 0.42
          y: Style.spacing.xs
          width: parent.width - x - Style.spacing.lg
          textFormat: Text.PlainText
          text: rowItem.modelData.value
          color: root.foreground
          wrapMode: Text.Wrap
          font.family: root.fontFamily
          font.pixelSize: Style.font.bodySmall
        }

        MouseArea {
          anchors.fill: parent
          hoverEnabled: true
          onEntered: {
            root.cursor = rowItem.index
            root.cursorWanted()
          }
        }
      }
    }
  }

  Text {
    anchors.centerIn: parent
    visible: root.rows.length === 0
    textFormat: Text.PlainText
    text: root.indexData ? "The index has no system section" : "No index to show"
    color: root.dim
    font.family: root.fontFamily
    font.pixelSize: Style.font.bodySmall
  }
}
