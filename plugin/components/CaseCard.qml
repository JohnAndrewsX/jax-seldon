pragma ComponentBehavior: Bound

import QtQuick
import qs.Commons
import qs.Ui
import "../Model.js" as Model

// The card of one case on the Work tab (SPEC-PLUGIN §5), a case from
// Model.workColumns():
//
//   ▌ C-2026-004 · active                                red · R2
//   ▌ Zed als zweiten Editor installieren
//   ▌ dev-env · priority normal · 2/4 steps
//   ▌ created 2026-09-28 · started 2026-10-01
//   ▌ agent: claude-code
//   ▌ [1 proposed event]
//   ▌ [Verify] [Start agent] [Drop] [Open]
//   ▌ Enter again: Verify C-2026-004
//
// The stripe is the zone's theme colour (red = urgent, yellow = accent,
// green = muted). A completed case an agent closed reads "completed by
// agent" (ADR-0027 §5); a reopen names the case it reopens in its meta
// line. The buttons are Model.caseActions(); a click asks the tab
// to run one (`actionRequested`); the tab owns arming and the engine call.
// An armed action shows the cursor on its button and the hint line; Start
// agent (WP-022) is armed by key a or a click and asks for the same again.
// Every string is plain text.
BorderSurface {
  id: root

  property var caseData: null
  property string armed: ""
  property bool pending: false
  property bool canWrite: false
  property string writeBlocker: ""
  property color foreground: Color.foreground
  property color accent: Color.accent
  property color urgent: Color.urgent
  property color muted: Color.muted
  property string fontFamily: Style.font.family

  signal actionRequested(string actionId)

  readonly property var actions: Model.caseActions(caseData)
  readonly property color dim: Util.alpha(foreground, 0.65)
  readonly property string tone: caseData ? caseData.tone : ""
  readonly property color toneColor: tone === "urgent" ? urgent : tone === "accent" ? accent : muted
  readonly property var armedAction: Model.caseAction(caseData, armed)
  readonly property string hint: !caseData ? ""
    : armedAction && armedAction.confirm
      ? armedAction.label + " " + caseData.id + "? Press x again or click Confirm " + armedAction.label.toLowerCase() + ". This is final."
    : armedAction && armedAction.twice
      ? armedAction.label + " on " + caseData.id + "? Press a again or click Confirm " + armedAction.label.toLowerCase() + "."
    : armedAction ? "Press Enter again: " + armedAction.label + " " + caseData.id
    : !canWrite && actions.length > 0 ? writeBlocker
    : ""

  implicitHeight: content.implicitHeight + Style.spacing.lg * 2
  radius: Style.cornerRadius
  color: Style.selectedFillFor(root.foreground, root.accent)
  borderSpec: Border.controlSpec("normal", root.foreground, root.accent)

  Rectangle {
    id: stripe
    x: Style.spacing.sm
    y: Style.spacing.lg
    width: Style.spacing.xs
    height: root.height - Style.spacing.lg * 2
    radius: width / 2
    visible: root.tone !== ""
    color: root.toneColor
  }

  Column {
    id: content
    x: stripe.x + stripe.width + Style.spacing.lg
    y: Style.spacing.lg
    width: root.width - x - Style.spacing.lg
    spacing: Style.spacing.xs

    Text {
      width: parent.width
      visible: !root.caseData
      textFormat: Text.PlainText
      text: "No case selected"
      color: root.dim
      font.family: root.fontFamily
      font.pixelSize: Style.font.bodySmall
    }

    Item {
      width: parent.width
      visible: !!root.caseData
      implicitHeight: Math.max(idText.implicitHeight, zoneText.implicitHeight)

      Text {
        id: idText
        anchors.left: parent.left
        anchors.right: zoneText.left
        anchors.rightMargin: Style.spacing.md
        anchors.verticalCenter: parent.verticalCenter
        textFormat: Text.PlainText
        text: root.caseData ? root.caseData.id + " · " + root.caseData.status + (root.caseData.closedByAgent ? " by agent" : "") : ""
        color: root.dim
        elide: Text.ElideRight
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
      }

      Text {
        id: zoneText
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        textFormat: Text.PlainText
        text: root.caseData ? [root.caseData.zone, root.caseData.risk].filter(function(p) { return p !== "" }).join(" · ") : ""
        color: root.tone !== "" ? root.toneColor : root.dim
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        font.bold: true
      }
    }

    Text {
      width: parent.width
      visible: !!root.caseData
      textFormat: Text.PlainText
      text: root.caseData ? root.caseData.title : ""
      color: root.foreground
      wrapMode: Text.Wrap
      maximumLineCount: 2
      elide: Text.ElideRight
      font.family: root.fontFamily
      font.pixelSize: Style.font.body
      font.bold: true
    }

    Text {
      width: parent.width
      visible: text !== ""
      textFormat: Text.PlainText
      text: root.caseData ? Model.caseMeta(root.caseData) : ""
      color: root.dim
      elide: Text.ElideRight
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
    }

    Text {
      width: parent.width
      visible: text !== ""
      textFormat: Text.PlainText
      text: root.caseData ? Model.caseDates(root.caseData) : ""
      color: root.dim
      elide: Text.ElideRight
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
    }

    // The agents the case lists (schema `agents`), e.g. "agent: claude-code".
    Text {
      width: parent.width
      visible: text !== ""
      textFormat: Text.PlainText
      text: root.caseData ? Model.caseAgents(root.caseData) : ""
      color: root.dim
      elide: Text.ElideRight
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
    }

    // Open drift the engine proposes for this case (schema `proposedEvents`).
    BorderSurface {
      visible: !!root.caseData && root.caseData.proposed > 0
      implicitWidth: proposedText.implicitWidth + Style.spacing.md * 2
      implicitHeight: proposedText.implicitHeight + Style.spacing.xxs * 2
      radius: Style.cornerRadius
      color: Style.selectedFillFor(root.accent, root.accent)
      borderSpec: Border.controlSpec("normal", root.accent, root.accent)

      Text {
        id: proposedText
        anchors.centerIn: parent
        textFormat: Text.PlainText
        text: root.caseData ? Model.plural(root.caseData.proposed, "proposed event", "proposed events") : ""
        color: root.foreground
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        font.bold: true
      }
    }

    Flow {
      width: parent.width
      spacing: Style.spacing.sm
      visible: root.actions.length > 0

      Repeater {
        model: root.actions

        Button {
          required property var modelData

          text: root.armed === modelData.id && (modelData.confirm || modelData.twice) ? "Confirm " + modelData.label.toLowerCase() : modelData.label
          enabled: root.canWrite && !root.pending
          hasCursor: root.armed === modelData.id
          selected: modelData.primary && modelData.write
          bordered: true
          foreground: modelData.id === "drop" ? root.urgent : root.foreground
          accent: root.accent
          fontFamily: root.fontFamily
          fontSize: Style.font.caption
          verticalPadding: Style.spacing.xs
          tooltipText: modelData.id === "open" ? "Open the case file in the editor (key e)"
            : modelData.id === "agent" ? "Launch the configured agent on this case (key a, twice)"
            : modelData.id === "reopen" ? "A new active case with the same Intent; this one stays completed (key r)"
            : modelData.primary ? "Key Enter, twice"
            : modelData.id === "drop" ? "Key x, twice"
            : ""
          onClicked: root.actionRequested(modelData.id)
        }
      }
    }

    Text {
      width: parent.width
      visible: text !== ""
      textFormat: Text.PlainText
      text: root.hint
      color: root.armedAction && root.armedAction.confirm ? root.urgent : root.dim
      wrapMode: Text.Wrap
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
    }
  }
}
