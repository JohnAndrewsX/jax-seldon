pragma ComponentBehavior: Bound

import QtQuick
import qs.Commons
import qs.Ui
import "../../Model.js" as Model
import ".."

// The desk's sidebar (ADR-0034 §2): the nine targets with their icons and
// counts (Model.deskCounts), the search, and the fold button. Icons only
// when `icons` (the setting `deskSidebar`, or a desk narrower than 960 px).
// It only reports clicks; Desk.qml decides. One cursor (WP-177): the
// pointer's target draws Omarchy's hover fill only after a real pointer
// move (PointerMoveGate); any key and a section change clear it; the
// current section has the accent tint plus an accent bar of at least 3:1.
Item {
  id: root

  readonly property Tone tone: Tone {}

  property bool icons: false
  property string current: ""
  property var counts: ({})
  property string searchText: ""
  property color foreground: Color.popups.text
  property color accent: Color.accent
  property color urgent: Color.urgent
  property string fontFamily: Style.font.family

  readonly property bool searchFocused: search.focused
  readonly property color dim: root.tone.dim
  // The target under a pointer that moved ("" none).
  property string pointerId: ""

  // Desk.keyPressed's count: any key clears the pointer's target.
  property int keyEvents: 0

  function dropPointer() {
    gate.reset()
    root.pointerId = ""
  }

  onCurrentChanged: root.dropPointer()
  onKeyEventsChanged: root.dropPointer()

  PointerMoveGate {
    id: gate
    referenceItem: root
  }

  signal picked(string id)
  signal foldRequested()
  signal searchEdited(string text)
  signal searchLeft()

  function focusSearch() {
    search.focusField()
  }

  // The hairline between the sidebar and the list.
  Rectangle {
    anchors.right: parent.right
    width: Style.spacing.hairline
    height: parent.height
    color: root.tone.divider
  }

  Column {
    id: rows
    x: Style.spacing.lg
    y: Style.spacing.xxl
    width: parent.width - Style.spacing.lg * 2
    spacing: Style.spacing.xxs

    Text {
      visible: !root.icons
      leftPadding: Style.spacing.lg
      bottomPadding: Style.spacing.sm
      textFormat: Text.PlainText
      text: "SECTIONS"
      color: root.tone.dim
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
      font.letterSpacing: Style.space(1)
    }

    Repeater {
      model: Model.DESK_SECTIONS

      Item {
        id: navRow

        required property var modelData
        readonly property bool selected: root.current === navRow.modelData.id
        readonly property var count: root.counts[navRow.modelData.id] || { text: "", tone: "" }

        objectName: "nav:" + navRow.modelData.id
        width: rows.width
        height: Math.max(Style.spacing.controlHeight, label.implicitHeight + Style.spacing.md * 2)

        Rectangle {
          anchors.fill: parent
          radius: Style.cornerRadius
          color: navRow.selected ? Style.selectedFillFor(root.accent, root.accent)
            : root.pointerId === navRow.modelData.id ? Style.hoverFill : "transparent"
        }

        Rectangle {
          visible: navRow.selected
          width: Math.max(2, Style.space(2))
          height: parent.height
          color: root.tone.accentUi
        }

        NavIcon {
          id: icon
          x: root.icons ? Math.round((parent.width - width) / 2) : Style.spacing.lg
          anchors.verticalCenter: parent.verticalCenter
          width: Style.space(16)
          height: Style.space(16)
          path: navRow.modelData.icon
          color: navRow.selected ? root.tone.accentUi : root.foreground
        }

        Text {
          id: label
          visible: !root.icons
          anchors.left: icon.right
          anchors.leftMargin: Style.spacing.lg
          anchors.right: countText.left
          anchors.rightMargin: Style.spacing.md
          anchors.verticalCenter: parent.verticalCenter
          textFormat: Text.PlainText
          text: navRow.modelData.label
          color: navRow.selected ? root.tone.accentText : root.foreground
          elide: Text.ElideRight
          font.family: root.fontFamily
          font.pixelSize: Style.font.body
        }

        Text {
          id: countText
          visible: !root.icons && text !== ""
          anchors.right: parent.right
          anchors.rightMargin: Style.spacing.lg
          anchors.verticalCenter: parent.verticalCenter
          textFormat: Text.PlainText
          text: navRow.count.text
          color: navRow.count.tone === "urgent" ? root.tone.urgentText : root.tone.dim
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
        }

        HoverHandler {
          onPointChanged: if (gate.moved(navRow, point.position)) root.pointerId = navRow.modelData.id
          onHoveredChanged: if (!hovered && root.pointerId === navRow.modelData.id) root.pointerId = ""
        }

        MouseArea {
          anchors.fill: parent
          cursorShape: Qt.PointingHandCursor
          onClicked: root.picked(navRow.modelData.id)
        }
      }
    }
  }

  Column {
    id: foot
    x: Style.spacing.lg
    anchors.bottom: parent.bottom
    anchors.bottomMargin: Style.spacing.xxl
    width: parent.width - Style.spacing.lg * 2
    spacing: Style.spacing.md

    Search {
      id: search
      visible: !root.icons
      width: parent.width
      text: root.searchText
      onEdited: function(text) { root.searchEdited(text) }
      onDone: root.searchLeft()
    }

    Item {
      objectName: "deskFold"
      width: parent.width
      height: Style.spacing.controlHeight

      NavIcon {
        id: foldIcon
        x: root.icons ? Math.round((parent.width - width) / 2) : Style.spacing.lg
        anchors.verticalCenter: parent.verticalCenter
        width: Style.space(16)
        height: Style.space(16)
        path: root.icons ? Model.DESK_EXPAND_ICON : Model.DESK_COLLAPSE_ICON
        color: root.dim
      }

      Text {
        visible: !root.icons
        anchors.left: foldIcon.right
        anchors.leftMargin: Style.spacing.lg
        anchors.verticalCenter: parent.verticalCenter
        textFormat: Text.PlainText
        text: "Collapse"
        color: root.tone.dim
        font.family: root.fontFamily
        font.pixelSize: Style.font.bodySmall
      }

      MouseArea {
        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor
        onClicked: root.foldRequested()
      }
    }
  }
}
