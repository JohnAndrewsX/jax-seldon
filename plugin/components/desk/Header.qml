import QtQuick
import QtQuick.Window
import qs.Commons
import qs.Ui
import ".."
import "../../Model.js" as Model

// The desk's header row (ADR-0034 §2): the mark and "SELDON" over machine ·
// Omarchy version · captured N ago; the status chip when a notice is up
// (the first notice's title and how many more, or "N notices" where the
// title does not fit; a click folds the notices under the header); the KPI strip; Settings (`,`) and Esc.
Item {
  id: root

  readonly property Tone tone: Tone {}

  property string subline: ""
  property var kpis: []
  property string chipText: ""
  // How many notices the chip stands for; when its title does not fit
  // between the mark and the KPI strip, the chip says "N notices".
  property int chipCount: 0
  property string chipTone: ""
  property bool noticesFolded: false
  property color foreground: Color.popups.text
  property color accent: Color.accent
  property color urgent: Color.urgent
  property string fontFamily: Style.font.family

  readonly property real dpr: Screen.devicePixelRatio > 0 ? Screen.devicePixelRatio : 1
  readonly property var mark: Model.panelMark(capMetrics.tightBoundingRect.height, root.dpr)
  readonly property string markFile: headerMark.file
  readonly property bool markReady: headerMark.ready
  readonly property string chipShown: chip.visible ? chipLabel.text : ""
  readonly property color chipColor: root.chipTone === "urgent" ? root.urgent
    : root.chipTone === "neutral" ? root.foreground : root.accent
  // The chip's label: the role as text, derived to read on its tint (WP-177).
  readonly property color chipInk: root.chipTone === "urgent" ? root.tone.urgentText
    : root.chipTone === "neutral" ? root.foreground : root.tone.accentText

  signal settingsRequested()
  signal closeRequested()
  signal chipClicked()
  signal kpiPicked(string id)

  // From heights known at once (not the Rows', set in their polish a frame
  // late): what sits under the header must not move after the first frame.
  implicitHeight: Math.max(brand.height, kpiStrip.figureHeight, Style.spacing.controlHeight) + Style.spacing.xxl * 2

  TextMetrics {
    id: capMetrics
    font: title.font
    text: "H"
  }

  Item {
    id: brand
    x: Style.spacing.huge
    anchors.verticalCenter: parent.verticalCenter
    width: title.x + Math.max(title.implicitWidth, sub.implicitWidth)
    height: Math.max(root.mark.box, sub.y + sub.implicitHeight)

    MaskIcon {
      id: headerMark
      x: 0
      y: 0
      width: root.mark.box
      height: root.mark.box
      file: root.mark.file
      crisp: root.mark.crisp
      color: root.accent
    }

    Text {
      id: title
      x: root.mark.box + root.mark.gap
      y: Math.round((root.mark.baseline - title.baselineOffset) * root.dpr) / root.dpr
      textFormat: Text.PlainText
      text: "SELDON"
      color: root.foreground
      font.family: root.fontFamily
      font.pixelSize: Style.font.heading
      font.bold: true
      font.letterSpacing: Style.space(2)
    }

    Text {
      id: sub
      x: title.x
      anchors.top: title.bottom
      textFormat: Text.PlainText
      text: root.subline
      color: root.tone.dim
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
    }
  }

  TextMetrics {
    id: chipFull
    font: chipLabel.font
    text: "▾ " + root.chipText
  }

  BorderSurface {
    id: chip
    readonly property real room: Math.max(0, kpiStrip.x - brand.x - brand.width - Style.spacing.huge * 2)
    readonly property bool fits: chipFull.advanceWidth + Style.spacing.xl * 2 <= room
    readonly property string shown: fits ? root.chipText
      : root.chipCount === 1 ? "1 notice" : root.chipCount + " notices"
    objectName: "deskChip"
    visible: root.chipText !== ""
    anchors.left: brand.right
    anchors.leftMargin: Style.spacing.huge
    anchors.verticalCenter: parent.verticalCenter
    width: Math.min(chipLabel.implicitWidth + Style.spacing.xl * 2, chip.room)
    height: chipLabel.implicitHeight + Style.spacing.sm * 2
    radius: height / 2
    color: Style.selectedFillFor(root.chipColor, root.chipColor)
    borderSpec: Border.controlSpec("normal", root.chipColor, root.chipColor)

    Text {
      id: chipLabel
      x: Style.spacing.xl
      width: parent.width - Style.spacing.xl * 2
      anchors.verticalCenter: parent.verticalCenter
      textFormat: Text.PlainText
      text: (root.noticesFolded ? "▸ " : "▾ ") + chip.shown
      color: root.chipInk
      elide: Text.ElideRight
      font.family: root.fontFamily
      font.pixelSize: Style.font.bodySmall
    }

    MouseArea {
      anchors.fill: parent
      cursorShape: Qt.PointingHandCursor
      onClicked: root.chipClicked()
    }
  }

  KpiStrip {
    id: kpiStrip
    anchors.right: tools.left
    anchors.rightMargin: Style.spacing.huge
    anchors.verticalCenter: parent.verticalCenter
    kpis: root.kpis
    foreground: root.foreground
    accent: root.accent
    urgent: root.urgent
    fontFamily: root.fontFamily
    onPicked: function(id) { root.kpiPicked(id) }
  }

  Row {
    id: tools
    anchors.right: parent.right
    anchors.rightMargin: Style.spacing.huge
    anchors.verticalCenter: parent.verticalCenter
    spacing: Style.spacing.md

    Item {
      objectName: "deskGear"
      width: Style.spacing.controlHeight
      height: Style.spacing.controlHeight

      Rectangle {
        anchors.fill: parent
        radius: Style.cornerRadius
        color: gearHover.hovered ? Style.hoverFill : "transparent"
      }

      NavIcon {
        anchors.centerIn: parent
        width: Style.space(16)
        height: Style.space(16)
        path: Model.deskSection("settings").icon
        color: root.tone.dim
      }

      HoverHandler {
        id: gearHover
      }

      MouseArea {
        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor
        onClicked: root.settingsRequested()
      }
    }

    Button {
      text: "Esc"
      bordered: true
      foreground: root.tone.dim
      fontSize: Style.font.bodySmall
      onClicked: root.closeRequested()
    }
  }

  Rectangle {
    anchors.bottom: parent.bottom
    width: parent.width
    height: Style.spacing.hairline
    color: root.tone.divider
  }
}
