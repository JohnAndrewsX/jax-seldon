pragma ComponentBehavior: Bound

import QtQuick
import qs.Commons
import qs.Ui
import "../Model.js" as Model

// The banner for a non-ok service status (SPEC-PLUGIN §5, AGENTS.md §7).
//
// Renders one Model.bannerFor() object: title, the detail (one sentence,
// wrapped), the fix command when there is one (wrapped anywhere: the
// install one-liner is a long URL), one button per fix action, and under
// them the banner's `hint` when it has one (the rules banner). It only reports
// clicks; Service.fix() carries them out. Every string is set as plain text.
// Left of the text, the status's state pictogram (A11, Model.statusPictogram)
// in the banner's tone, `pictogramSize` square: 48 in the panel, 96 in the
// Prime Radiant; none for a status without one (contract mismatch).
// Tones: "urgent", "neutral" (the foreground: the capture-warning notice,
// WP-085), else the accent. A banner with `full` shows it in a tooltip
// while the pointer is over the banner.
BorderSurface {
  id: root

  readonly property Tone tone: Tone {}

  property var banner: null
  property color foreground: Color.foreground
  property color accent: Color.accent
  property color urgent: Color.urgent
  property string fontFamily: Style.font.family
  property real pictogramSize: Style.space(48)

  signal actionRequested(string actionId)

  readonly property color toneColor: banner && banner.tone === "urgent" ? urgent
    : banner && banner.tone === "neutral" ? foreground
    : accent
  // The title in the tone's colour as text: derived to read on the tint
  // (WP-177); the tint, border, pictogram and button keep the raw role.
  readonly property color toneText: banner && banner.tone === "urgent" ? root.tone.urgentText
    : banner && banner.tone === "neutral" ? foreground
    : root.tone.accentText
  readonly property string command: banner && banner.command ? banner.command : ""
  readonly property string hint: banner && banner.hint ? banner.hint : ""
  readonly property string pictogram: banner ? Model.statusPictogram(banner.status) : ""
  readonly property string tooltipText: banner && banner.full ? banner.full : ""
  readonly property bool hovered: hover.hovered
  readonly property bool tooltipShown: tooltip.visible
  readonly property real tooltipWidth: tooltip.width
  // The tooltip shows its own wrapping label, and the text fits it (not
  // cut off at the tooltip's edge).
  readonly property bool tooltipFits: tooltip.contentItem === tooltipLabel
    && tooltipLabel.contentWidth <= tooltipLabel.width - tooltipLabel.leftPadding - tooltipLabel.rightPadding + 0.5

  visible: banner !== null
  implicitWidth: Style.space(320)
  implicitHeight: visible ? Math.max(content.implicitHeight, pictogramIcon.visible ? pictogramIcon.height : 0) + contentTopInset + contentBottomInset : 0
  radius: Style.spacing.labelGap
  color: Style.selectedFillFor(toneColor, toneColor)
  borderSpec: Border.controlSpec("normal", toneColor, toneColor)
  padding: Style.spacing.xl

  HoverHandler {
    id: hover
  }

  // The shell's tooltip, but never wider than the banner: its own text
  // item neither wraps nor limits its width, so a long warning ran off the
  // window. Same tokens, plain text, wrapped (WP-085).
  PanelToolTip {
    id: tooltip
    width: root.width
    visible: root.tooltipText !== "" && hover.hovered
    text: root.tooltipText
    fontFamily: root.fontFamily

    contentItem: Text {
      id: tooltipLabel
      textFormat: Text.PlainText
      text: tooltip.text
      wrapMode: Text.Wrap
      color: tooltip.panelForeground
      font.family: tooltip.fontFamily
      font.pixelSize: tooltip.fontSize
      leftPadding: Border.left(tooltip.panelBorderSpec) + Style.spacing.controlPaddingX
      rightPadding: Border.right(tooltip.panelBorderSpec) + Style.spacing.controlPaddingX
      topPadding: Border.top(tooltip.panelBorderSpec) + Style.spacing.controlPaddingY
      bottomPadding: Border.bottom(tooltip.panelBorderSpec) + Style.spacing.controlPaddingY
    }
  }

  MaskIcon {
    id: pictogramIcon
    x: root.contentLeftInset
    y: root.contentTopInset
    width: root.pictogramSize
    height: root.pictogramSize
    visible: root.pictogram !== ""
    file: Model.pictogramFile(root.pictogram, root.pictogramSize)
    color: root.toneColor
  }

  Column {
    id: content
    x: root.contentLeftInset + (pictogramIcon.visible ? pictogramIcon.width + Style.spacing.xl : 0)
    y: root.contentTopInset
    width: root.width - x - root.contentRightInset
    spacing: Style.spacing.md

    Text {
      width: parent.width
      textFormat: Text.PlainText
      text: root.banner ? root.banner.title : ""
      color: root.toneText
      font.family: root.fontFamily
      font.pixelSize: Style.font.subtitle
      font.bold: true
      wrapMode: Text.Wrap
    }

    Text {
      width: parent.width
      textFormat: Text.PlainText
      text: root.banner ? root.banner.detail : ""
      color: root.foreground
      font.family: root.fontFamily
      font.pixelSize: Style.font.bodySmall
      wrapMode: Text.Wrap
    }

    Text {
      width: parent.width
      visible: root.command !== ""
      textFormat: Text.PlainText
      text: root.command
      color: root.tone.dim
      font.family: root.fontFamily
      font.pixelSize: Style.font.bodySmall
      wrapMode: Text.WrapAnywhere
    }

    Flow {
      width: parent.width
      spacing: Style.spacing.controlGap

      Repeater {
        model: root.banner ? root.banner.actions : []

        Button {
          required property var modelData
          required property int index

          text: modelData.label
          foreground: root.foreground
          accent: root.toneColor
          fontFamily: root.fontFamily
          bordered: true
          selected: index === 0
          onClicked: root.actionRequested(modelData.id)
        }
      }
    }

    Text {
      width: parent.width
      visible: root.hint !== ""
      textFormat: Text.PlainText
      text: root.hint
      color: root.foreground
      font.family: root.fontFamily
      font.pixelSize: Style.font.bodySmall
      wrapMode: Text.Wrap
    }
  }
}
