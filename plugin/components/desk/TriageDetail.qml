pragma ComponentBehavior: Bound

import QtQuick
import qs.Commons
import qs.Ui

// The agent's triage proposal in the Changelog's detail (ADR-0034 §6,
// ADR-0036, WP-124b): the sticky bar *Apply proposals* / *Discard* and the
// line "N items proposed by <actor> at <at>, C crises held back — apply
// each below"; then the crises, each with its own button (`drift apply
// <id> --item <eventId>`, one per run), and the items Apply takes, each
// with the change's subject, the action, and every evidence text with the
// engine's "by <author> ·" first (wrapped, never clipped). An item whose
// evidence is an agent's words or has no known author is marked. After a
// run the items say done, skipped or refused (the engine's words);
// `applied` marks the run, not the items.
//
// Every text here comes from the logbook or the agent: plain text only
// (CONTRACT.md rule 6). The calls go through Service (fixed argv,
// Model.validateArgs); Apply names the id this pane shows, and the service
// refuses it when the index names another proposal by then.
DetailPane {
  id: root

  property var section: null

  readonly property var service: root.section ? root.section.service : null
  readonly property var proposal: root.service ? root.service.triageView : null
  readonly property var result: root.service && root.service.triageResult && root.proposal
    && root.service.triageResult.proposalId === root.proposal.id ? root.service.triageResult : null
  readonly property bool pending: !!root.service && !!root.service.triageResult && root.service.triageResult.pending === true
  readonly property bool canWrite: !!root.service && root.service.canWrite
  readonly property color foregroundColor: Color.popups.text

  title: "Proposal"
  meta: root.proposal ? root.proposal.id : ""
  // The bar's actions, rebuilt only when what they show changes: a new
  // index (every engine call reloads it) must not recreate the buttons
  // under a click.
  readonly property string actionsKey: !root.proposal ? ""
    : [root.proposal.id, root.proposal.applyCount, root.proposal.readable, root.canWrite, root.pending].join("|")
  onActionsKeyChanged: root.actions = root.buildActions()
  Component.onCompleted: root.actions = root.buildActions()

  function buildActions() {
    if (!root.proposal) return []
    return [
      { id: "apply", primary: true,
        label: root.proposal.applyCount > 0 ? "Apply proposals (" + root.proposal.applyCount + ")" : "Apply proposals",
        // also with nothing open: a second run says what it skipped
        enabled: root.canWrite && !root.pending && root.proposal.readable },
      { id: "discard", label: "Discard", enabled: root.canWrite && !root.pending }
    ]
  }

  function activate() { return false }
  function back() { return false }

  onActionTriggered: function(id) {
    if (!root.service || !root.proposal) return
    if (id === "apply") root.service.applyProposal(root.proposal.id, "")
    else if (id === "discard") root.service.discardProposal(root.proposal.id)
  }

  function itemView(it) {
    return {
      id: it.eventId,
      subject: it.subject,
      action: it.actionText,
      crisis: it.crisis,
      open: it.open,
      flagged: it.flagged,
      evidence: it.evidence.map(function(e) { return e.label + ": " + e.text }),
      outcome: it.outcome ? it.outcome.state + (it.outcome.reason !== "" ? ": " + it.outcome.reason : "") : ""
    }
  }

  function view() {
    var v = root.proposal
    return {
      shown: !!v,
      id: v ? v.id : "",
      head: v ? v.head : "",
      state: v ? v.state : "",
      readable: v ? v.readable : false,
      actions: root.actions.map(function(a) { return a.label + (a.enabled === false ? " (off)" : "") }),
      result: root.result && !root.result.pending ? root.result.text : "",
      resultOk: root.result ? root.result.ok : true,
      crises: v ? v.crises.map(root.itemView) : [],
      regular: v ? v.regular.map(root.itemView) : []
    }
  }

  component Line: Text {
    width: parent ? parent.width : 0
    textFormat: Text.PlainText
    wrapMode: Text.Wrap
    color: root.foregroundColor
    font.family: root.fontFamily
    font.pixelSize: Style.font.bodySmall
  }

  // One proposed item: subject, action, intent, the mark, the outcome, and
  // every evidence text, author first.
  component Proposed: Column {
    id: card

    required property var modelData

    width: parent ? parent.width : 0
    spacing: Style.spacing.xs

    Text {
      width: parent.width
      textFormat: Text.PlainText
      text: card.modelData.subject
      color: root.foregroundColor
      wrapMode: Text.WrapAnywhere
      font.family: root.fontFamily
      font.pixelSize: Style.font.body
      font.bold: true
    }

    Line {
      text: card.modelData.actionText + (card.modelData.kind !== "" ? "  ·  " + card.modelData.kind : "")
    }

    Line {
      visible: card.modelData.intent !== ""
      text: card.modelData.intent
      color: Color.muted
    }

    Line {
      objectName: "triageFlag"
      visible: card.modelData.flagged
      text: "Read twice: some evidence is an agent's words or has no known author."
      color: Color.accent
      font.bold: true
    }

    Line {
      visible: text !== ""
      text: card.modelData.outcome
        ? (card.modelData.outcome.state === "done" ? "Done" : card.modelData.outcome.state === "skipped" ? "Skipped" : "Refused")
          + (card.modelData.outcome.reason !== "" ? ": " + card.modelData.outcome.reason : "")
        : !card.modelData.open ? "No longer open: nothing to apply." : ""
      color: card.modelData.outcome && card.modelData.outcome.state === "refused" ? Color.urgent : Color.muted
    }

    Repeater {
      model: card.modelData.evidence

      Column {
        id: ref

        required property var modelData

        width: card.width
        spacing: 0

        Line {
          text: ref.modelData.label
          color: Color.muted
          font.pixelSize: Style.font.caption
        }

        // The engine's text: "by <author> · …" first, wrapped, never
        // clipped on the left (Fable, WP-124 round 3).
        Line {
          objectName: "evidenceText"
          text: ref.modelData.text
          color: ref.modelData.flagged ? Color.accent : root.foregroundColor
        }
      }
    }
  }

  Column {
    width: parent.width
    spacing: Style.spacing.lg

    Line {
      visible: !root.proposal
      text: "No proposal. “Agent sorts N open changes” in the list asks for one."
      color: Color.muted
      font.pixelSize: Style.font.body
    }

    Line {
      objectName: "triageHead"
      visible: !!root.proposal
      text: root.proposal ? root.proposal.head : ""
      font.pixelSize: Style.font.body
      font.bold: true
    }

    Line {
      visible: !!root.proposal
      text: root.proposal ? root.proposal.state : ""
      color: Color.muted
    }

    Line {
      objectName: "triageResult"
      visible: text !== ""
      text: !root.result ? ""
        : root.result.pending ? root.result.text
        : root.result.gone ? root.result.text + ". The proposal is gone; the list shows what is open now."
        : root.result.text
      color: root.result && !root.result.ok ? Color.urgent : Color.muted
    }

    Line {
      visible: !!root.proposal && !root.proposal.readable
      text: "The proposal file could not be read as the engine writes it. Discard it, or ask the agent again."
      color: Color.urgent
    }

    // Crises: never inside Apply (ADR-0028 §3); one click each, one per run.
    Line {
      visible: !!root.proposal && root.proposal.crises.length > 0
      text: "CRISES — EACH ON ITS OWN"
      color: Color.urgent
      font.pixelSize: Style.font.caption
      font.bold: true
    }

    Repeater {
      model: root.proposal ? root.proposal.crises : []

      Column {
        id: crisis

        required property var modelData

        width: parent.width
        spacing: Style.spacing.sm

        Proposed {
          modelData: crisis.modelData
        }

        Button {
          text: "Apply this crisis"
          tooltipText: "Only this item: drift apply --item " + crisis.modelData.eventId
          bordered: true
          enabled: root.canWrite && !root.pending && crisis.modelData.open
          foreground: Color.urgent
          fontFamily: root.fontFamily
          fontSize: Style.font.caption
          verticalPadding: Style.spacing.xs
          onClicked: if (root.service && root.proposal) root.service.applyProposal(root.proposal.id, crisis.modelData.eventId)
        }
      }
    }

    Line {
      visible: !!root.proposal && root.proposal.regular.length > 0
      text: "WHAT APPLY TAKES"
      color: Color.muted
      font.pixelSize: Style.font.caption
      font.bold: true
    }

    Repeater {
      model: root.proposal ? root.proposal.regular : []

      Proposed {}
    }

    Line {
      visible: !!root.proposal
      text: "Applying writes each resolution as you, with the agent's name and the evidence. Nothing is required: unproven changes stay quietly open."
      color: Color.muted
      font.pixelSize: Style.font.caption
    }
  }
}
