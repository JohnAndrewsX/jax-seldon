pragma ComponentBehavior: Bound

import QtQuick
import qs.Commons
import qs.Ui
import "../Model.js" as Model

// One Changelog row: one index event, as prepared by Model.changelogRows().
//
//   ▌ glyph  kind  subject  [+N]                         time
//   ▌        detail · actor · case
//   ▌        resolution: why        (folded, ADR-0012 §8 §11)
//
// The stripe on the left is the event's own zone in theme colours (red =
// urgent, yellow = accent, green = muted). Open drift colours the glyph and
// the status line by the drift item instead: urgent for a crisis, else
// accent, so the members of a yellow group (ADR-0013 §3: red in the ledger,
// yellow as an item) do not read as crises. Snapshot rows carry the theme's selected
// fill. A drift group's leader shows "+N"; expanded, it lists the members
// that index.events still holds. Every string is plain text.
CursorSurface {
  id: root

  property var row: null
  property var members: []
  property bool expanded: false
  property color urgent: Color.urgent
  property color muted: Color.muted
  property string fontFamily: Style.font.family

  signal clicked()
  signal hoveredRow()

  readonly property color dim: Util.alpha(foreground, 0.65)
  readonly property string tone: row ? row.tone : ""
  readonly property color toneColor: tone === "urgent" ? urgent : tone === "accent" ? accent : muted
  readonly property string meta: row ? Model.rowMeta(row) : ""
  readonly property string status: row ? Model.rowStatus(row) : ""
  readonly property color statusColor: !row ? dim
    : row.crisis ? urgent
    : row.drift ? accent
    : dim

  current: !!row && row.snapshot
  implicitHeight: content.implicitHeight + Style.spacing.md * 2

  Rectangle {
    id: stripe
    x: 0
    y: Style.spacing.sm
    width: Style.spacing.xs
    height: root.height - Style.spacing.sm * 2
    radius: width / 2
    visible: root.tone !== ""
    color: root.toneColor
  }

  Column {
    id: content
    x: stripe.width + Style.spacing.lg
    y: Style.spacing.md
    width: root.width - x - Style.spacing.lg
    spacing: Style.spacing.xxs

    Item {
      width: parent.width
      implicitHeight: Math.max(glyph.implicitHeight, subject.implicitHeight)

      Text {
        id: glyph
        width: Style.space(18)
        anchors.verticalCenter: parent.verticalCenter
        textFormat: Text.PlainText
        text: root.row ? root.row.glyph : ""
        color: root.row && root.row.drift ? root.statusColor : root.foreground
        font.family: root.fontFamily
        font.pixelSize: Style.font.body
      }

      Text {
        id: kind
        anchors.left: glyph.right
        anchors.verticalCenter: parent.verticalCenter
        textFormat: Text.PlainText
        text: root.row ? root.row.kind : ""
        color: root.dim
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
      }

      Text {
        id: subject
        anchors.left: kind.right
        anchors.leftMargin: Style.spacing.md
        anchors.verticalCenter: parent.verticalCenter
        width: Math.min(implicitWidth, time.x - x - Style.spacing.md - (badge.visible ? badge.width + Style.spacing.md : 0))
        textFormat: Text.PlainText
        text: root.row ? root.row.subject : ""
        color: root.foreground
        elide: Text.ElideMiddle
        font.family: root.fontFamily
        font.pixelSize: Style.font.body
        font.bold: !!root.row && root.row.drift
      }

      BorderSurface {
        id: badge
        visible: !!root.row && root.row.badge !== ""
        anchors.left: subject.right
        anchors.leftMargin: Style.spacing.md
        anchors.verticalCenter: parent.verticalCenter
        implicitWidth: badgeText.implicitWidth + Style.spacing.md * 2
        implicitHeight: badgeText.implicitHeight + Style.spacing.xxs * 2
        radius: Style.cornerRadius
        color: Style.selectedFillFor(root.statusColor, root.statusColor)
        borderSpec: Border.controlSpec("normal", root.statusColor, root.statusColor)

        Text {
          id: badgeText
          anchors.centerIn: parent
          textFormat: Text.PlainText
          text: root.row ? root.row.badge : ""
          color: root.foreground
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
          font.bold: true
        }
      }

      Text {
        id: time
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        textFormat: Text.PlainText
        text: root.row ? root.row.time : ""
        color: root.dim
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
      }
    }

    Text {
      width: parent.width
      leftPadding: glyph.width
      visible: root.meta !== ""
      textFormat: Text.PlainText
      text: root.meta
      color: root.dim
      elide: Text.ElideRight
      wrapMode: root.expanded ? Text.Wrap : Text.NoWrap
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
    }

    Text {
      width: parent.width
      leftPadding: glyph.width
      visible: root.status !== ""
      textFormat: Text.PlainText
      text: root.status
      color: root.statusColor
      wrapMode: Text.Wrap
      maximumLineCount: root.expanded ? 50 : 2
      elide: Text.ElideRight
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
      font.italic: !!root.row && root.row.resolution !== ""
    }

    Repeater {
      model: root.expanded ? root.members : []

      Text {
        required property var modelData

        width: content.width
        leftPadding: glyph.width + Style.spacing.lg
        textFormat: Text.PlainText
        text: "· " + Model.memberLine(modelData)
        color: root.dim
        elide: Text.ElideRight
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
      }
    }
  }

  MouseArea {
    anchors.fill: parent
    hoverEnabled: true
    cursorShape: Qt.PointingHandCursor
    onEntered: root.hoveredRow()
    onClicked: root.clicked()
  }
}
