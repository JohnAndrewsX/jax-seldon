pragma ComponentBehavior: Bound

import QtQuick
import qs.Commons
import qs.Ui
import "../../Model.js" as Model

// The setup card (WP-119, SPEC-PLUGIN §5.4 "Setup card"): one card in
// Today's overview while Seldon is not set up — engine → logbook →
// snapshots (optional). The headline counts what is left ("Set up Seldon ·
// 2 of 3 steps to go"); done steps are ticked, later ones wait, the
// current one has its buttons: the terminal fix first (Install, Create
// logbook, Grant), Copy, and on the snapshot step Not now. Under step 2
// the preview's line (ADR-0047). Renders one Model.setupCard() object and
// only reports clicks (actionRequested); Service.setupAction carries them
// out. No mode logic: EASY and PRO show this same card. Every string is
// plain text.
BorderSurface {
  id: root

  property var setup: null
  // The preview's line for step 2 (Model.previewSummary), "" without one.
  property string previewText: ""
  // The step whose terminal opened, while the service looks again.
  property string launched: ""
  // The line under the current step after an action (Not now, a refusal).
  property string resultText: ""
  property color foreground: Color.popups.text
  property color accent: Color.accent
  property string fontFamily: Style.font.family

  signal actionRequested(string stepId, string actionId)

  // What the harness reads: the headline and each step's state.
  function view() {
    var steps = root.setup ? root.setup.steps : []
    var current = steps.filter(function(s) { return s.current })[0] || null
    return {
      shown: root.visible,
      headline: root.setup ? root.setup.headline : "",
      current: root.setup ? root.setup.current : "",
      steps: steps.map(function(s) {
        return s.id + ":" + (s.done ? "done" : s.later ? "later" : s.current ? "current" : "waiting")
      }),
      actions: current ? current.actions.map(function(a) { return a.label }) : [],
      ready: !!current && current.ready === true,
      waiting: current ? current.waiting : "",
      launched: root.launched,
      result: root.resultText
    }
  }

  visible: root.setup !== null
  implicitHeight: visible ? content.implicitHeight + contentTopInset + contentBottomInset : 0
  radius: Style.cornerRadius
  color: Style.selectedFillFor(root.accent, root.accent)
  borderSpec: Border.controlSpec("normal", root.accent, root.accent)
  padding: Style.spacing.xl

  Column {
    id: content
    x: root.contentLeftInset
    y: root.contentTopInset
    width: root.width - root.contentLeftInset - root.contentRightInset
    spacing: Style.spacing.lg

    Text {
      objectName: "setupHeadline"
      width: parent.width
      textFormat: Text.PlainText
      text: root.setup ? root.setup.headline : ""
      color: root.foreground
      wrapMode: Text.Wrap
      font.family: root.fontFamily
      font.pixelSize: Style.font.title
      font.bold: true
    }

    Text {
      width: parent.width
      textFormat: Text.PlainText
      text: root.setup ? root.setup.lead : ""
      color: root.foreground
      wrapMode: Text.Wrap
      font.family: root.fontFamily
      font.pixelSize: Style.font.bodySmall
    }

    Column {
      id: steps
      width: parent.width
      spacing: Style.spacing.lg

      Repeater {
        model: root.setup ? root.setup.steps : []

        Item {
          id: step

          required property var modelData

          readonly property bool done: step.modelData.done === true
          readonly property bool current: step.modelData.current === true
          // the current step in the accent, a done one plain, the rest quiet
          readonly property color tone: step.current ? root.accent : step.done ? root.foreground : Color.muted

          width: steps.width
          implicitHeight: Math.max(mark.height, body.implicitHeight)

          // The number, or a tick once done.
          BorderSurface {
            id: mark
            width: Style.spacing.controlHeight
            height: Style.spacing.controlHeight
            radius: height / 2
            color: step.current ? Style.selectedFillFor(root.accent, root.accent) : "transparent"
            borderSpec: Border.controlSpec("normal", step.tone, step.tone)

            Text {
              anchors.centerIn: parent
              textFormat: Text.PlainText
              text: step.done ? "✓" : String(step.modelData.number)
              color: step.tone
              font.family: root.fontFamily
              font.pixelSize: Style.font.bodySmall
              font.bold: true
            }
          }

          Column {
            id: body
            x: mark.width + Style.spacing.xl
            width: parent.width - x
            spacing: Style.spacing.sm

            Text {
              width: parent.width
              textFormat: Text.PlainText
              text: step.modelData.title + (step.done ? " · done" : step.modelData.later ? " · not now" : "")
              color: step.tone
              wrapMode: Text.Wrap
              font.family: root.fontFamily
              font.pixelSize: Style.font.subtitle
              font.bold: step.current
            }

            Text {
              width: parent.width
              visible: !step.done
              textFormat: Text.PlainText
              text: step.modelData.detail
              color: step.current ? root.foreground : Color.muted
              wrapMode: Text.Wrap
              font.family: root.fontFamily
              font.pixelSize: Style.font.bodySmall
            }

            // Step 2: what the machine remembers on its own (ADR-0047).
            Text {
              width: parent.width
              visible: !step.done && step.modelData.id === "logbook" && root.previewText !== ""
              textFormat: Text.PlainText
              text: root.previewText
              color: Color.muted
              wrapMode: Text.Wrap
              font.family: root.fontFamily
              font.pixelSize: Style.font.caption
            }

            // The command Copy copies, small, as on the notices.
            Text {
              width: parent.width
              visible: step.current
              textFormat: Text.PlainText
              text: step.modelData.command
              color: Color.muted
              wrapMode: Text.WrapAnywhere
              font.family: root.fontFamily
              font.pixelSize: Style.font.caption
            }

            Flow {
              width: parent.width
              visible: step.current
              spacing: Style.spacing.controlGap

              Repeater {
                model: step.current ? step.modelData.actions : []

                Button {
                  required property var modelData
                  required property int index

                  objectName: "setup-" + step.modelData.id + "-" + modelData.id
                  text: modelData.label
                  tooltipText: index === 0 ? step.modelData.hint : ""
                  enabled: step.modelData.ready === true
                  foreground: root.foreground
                  accent: root.accent
                  fontFamily: root.fontFamily
                  fontSize: Style.font.caption
                  verticalPadding: Style.spacing.xs
                  bordered: true
                  selected: index === 0
                  onClicked: root.actionRequested(step.modelData.id, modelData.id)
                }
              }
            }

            Text {
              width: parent.width
              visible: step.current && (step.modelData.waiting !== "" || root.launched === step.modelData.id || root.resultText !== "")
              textFormat: Text.PlainText
              text: step.modelData.waiting !== "" ? step.modelData.waiting
                : root.resultText !== "" ? root.resultText
                : root.launched === step.modelData.id ? Model.SETUP_LAUNCHED_TEXT : ""
              color: Color.muted
              wrapMode: Text.Wrap
              font.family: root.fontFamily
              font.pixelSize: Style.font.caption
            }
          }
        }
      }
    }
  }
}
