pragma ComponentBehavior: Bound

import QtQuick
import qs.Commons
import qs.Commons as Commons
import qs.Ui
import "../../Model.js" as Model

// The agent's triage proposal in the Changelog's detail (ADR-0034 §6,
// ADR-0036, WP-124b): the sticky bar *Apply proposals* / *Discard* with the
// line "N items proposed by <actor> at <at>, C crises held back — apply
// each below" under its buttons; then the crises, each with its own button
// (`drift apply <id> --item <eventId>`, one per run; by key ↓ walks them
// and Enter twice applies the one under the cursor, WP-201), and the items
// Apply takes, each with the change's subject, the action, and every evidence
// text with the engine's "by <authors> ·" first (wrapped, never clipped).
// An item whose evidence names an agent or an unknown author is marked.
// After a run the items say done, skipped or refused (the engine's words);
// `applied` marks the run, not the items.
//
// Bound to the proposal the user opened (`seenId`, set by the Changelog
// when it shows this pane; WP-124b round 2): Apply and Discard name that
// id, and when the index names another proposal by now the pane says so
// ("Replaced by a newer proposal … — review it") and offers only Review;
// a gone one says it is gone, with the last answer about it.
//
// Every text here comes from the logbook or the agent: plain text only
// (CONTRACT.md rule 6; model.test.js checks every Text of this file).
DetailPane {
  id: root

  property var section: null
  // The proposal the user opened; "" until the Changelog shows the pane.
  property string seenId: ""
  // The pane is on screen: only then are the items built (a 200-item
  // proposal costs nothing while the Changelog shows an event).
  property bool shown: false

  readonly property var service: root.section ? root.section.service : null
  readonly property var indexData: root.service && root.service.indexShown ? root.service.index : null
  readonly property var seen: Model.triageSeen(root.indexData, root.seenId)
  readonly property bool current: root.seen.state === "current"
  readonly property var proposal: root.current && root.service ? root.service.triageView : null
  // The last answer about this proposal only (never another's).
  readonly property var result: root.service && root.service.triageResult && root.seenId !== ""
    && root.service.triageResult.proposalId === root.seenId ? root.service.triageResult : null
  readonly property bool pending: !!root.service && !!root.service.triageResult && root.service.triageResult.pending === true
  readonly property bool canWrite: !!root.service && root.service.canWrite
  readonly property var arm: root.section ? root.section.arm : null
  readonly property string discardKey: "triage:discard:" + root.seenId
  // An arm holds only for the current proposal: a replaced one drops it
  // (review F2), so the bar never asks to confirm a write that would not
  // happen.
  readonly property bool discardArmed: root.current && !!root.arm && root.arm.armedId === root.discardKey
  // Enter arms Apply proposals, the second Enter applies (WP-126c; a click
  // applies at once, as before)
  // the id names what the second Enter would apply (WP-201 round 2, I7)
  readonly property string applyKey: Model.triageApplyKey(root.seenId, root.proposal)
  readonly property bool applyArmed: root.current && !!root.arm && root.arm.armedId === root.applyKey
  readonly property bool applyEnabled: !!root.proposal && root.canWrite && !root.pending && root.proposal.readable
  // A crisis by key (WP-201): the crisis under the cursor (-1: none, the
  // bar has the keys), walked by ↓/↑ from the bar once the items are built
  // (round 2, I1: never a crisis whose item and button are not on screen;
  // until then ↓ goes to the list) (Changelog.move); Enter arms it, the
  // second Enter runs the click's applyCrisis. Any move, a detour (a
  // press, another selection, leaving the proposal) or a replaced
  // proposal drops the cursor's place and the arm.
  property int crisisAt: -1
  readonly property int crisisCount: root.shown && root.proposal && items.status === Loader.Ready ? root.proposal.crises.length : 0
  readonly property var keyedCrisis: root.crisisAt >= 0 && root.crisisAt < root.crisisCount ? root.proposal.crises[root.crisisAt] : null
  // The keyed crisis's button (set by its delegate), for crisisVisible.
  property Item keyedButton: null
  // Why nothing can be applied now ("" a run is under way, or it can).
  readonly property string blocker: root.service ? root.service.writeBlocker : ""
  readonly property string crisisState: Model.crisisState(root.keyedCrisis, root.canWrite && !root.pending && root.current)
  readonly property bool crisisArmed: root.current && !!root.keyedCrisis && !!root.arm
    && root.arm.armedId === Model.crisisKey(root.seenId, root.keyedCrisis)
  readonly property color foregroundColor: Commons.Color.popups.text

  title: "Proposal"
  meta: root.seenId
  hint: root.discardArmed || root.applyArmed || root.crisisArmed ? root.arm.hint : root.proposal ? root.proposal.head : root.seen.text

  // The bar's actions, rebuilt only when what they show changes (not on
  // every index reload).
  readonly property string actionsKey: [root.seen.state, root.seenId, root.proposal ? root.proposal.applyCount : -1,
    root.proposal ? root.proposal.readable : false, root.canWrite, root.pending, root.discardArmed, root.applyArmed].join("|")
  onActionsKeyChanged: root.actions = root.buildActions()
  // Every triage arm, a crisis's too: when this proposal comes back as the
  // current one (A armed, B replaces it, B goes) the arm stays gone.
  onCurrentChanged: {
    if (root.current) return
    root.crisisAt = -1
    if (root.arm && root.arm.armedId.indexOf("triage:") === 0) root.arm.disarm()
  }
  onSeenIdChanged: root.crisisAt = -1
  onShownChanged: if (!root.shown) root.crisisAt = -1
  onCrisisCountChanged: if (root.crisisAt >= root.crisisCount) root.crisisAt = -1
  onCrisisAtChanged: if (root.arm && root.arm.armedId.indexOf("triage:crisis:") === 0) root.arm.disarm()
  Component.onCompleted: root.actions = root.buildActions()

  function buildActions() {
    if (root.seen.state === "replaced")
      return [
        { id: "review", label: "Review the new proposal", primary: true },
        { id: "apply", label: "Apply proposals", enabled: false },
        { id: "discard", label: "Discard", enabled: false }
      ]
    if (!root.proposal) return []
    return [
      { id: "apply", primary: true,
        label: root.proposal.applyCount > 0 ? "Apply proposals (" + root.proposal.applyCount + ")" : "Apply proposals",
        // also with nothing open: a second run says what it skipped
        enabled: root.applyEnabled, armed: root.applyArmed },
      { id: "discard", label: root.discardArmed ? "Confirm discard" : "Discard", armed: root.discardArmed, enabled: root.canWrite && !root.pending }
    ]
  }

  // Enter (WP-126c): on a replaced proposal, Review the new one; on a
  // crisis (WP-201) arm it, the second Enter applies that item alone; else
  // arm Apply proposals, and the second Enter applies it. The crises are
  // never with the rest (ADR-0036).
  function activate() {
    if (root.seen.state === "replaced") {
      if (root.section) root.section.showTriage()
      return true
    }
    if (root.crisisAt >= 0) return root.pressCrisis()
    if (!root.applyEnabled || !root.arm) return false
    var n = root.proposal.applyCount
    if (root.arm.press(root.applyKey, "Apply " + (n > 0 ? n + " " : "") + "proposed " + (n === 1 ? "change" : "changes")
        + " as you? Press Enter again. Crises stay, each with its own button."))
      root.service.applyProposal(root.seenId, "")
    return true
  }
  function back() { return false }

  // ↓/↑ inside the shown proposal (Model.crisisMove): true when the key
  // stayed among the crises (or went back to the bar), false when it
  // leaves them for the list or *Ask agent to sort*.
  function crisisMove(dy, rows) {
    var to = Model.crisisMove({ at: root.crisisAt, count: root.crisisCount, rows: rows }, dy)
    root.crisisAt = to === null ? -1 : to
    return to !== null
  }

  // The arm's id is built from the live item (Model.crisisKey): an item
  // rewritten since the first Enter only arms again, with its new hint.
  function pressCrisis() {
    var c = root.keyedCrisis
    if (!c || !c.open || !root.current || !root.canWrite || root.pending || !root.arm) return false
    if (root.arm.press(Model.crisisKey(root.seenId, c), Model.crisisArmHint(c)))
      root.applyCrisis(c.eventId)
    return true
  }

  // One crisis, one run: the click's and the second Enter's call.
  function applyCrisis(eventId) {
    if (root.service && root.current) root.service.applyProposal(root.seenId, eventId)
  }

  // A crisis delegate's keyed state changed, or it was made (keyed) by a
  // rebuild: remember its button and bring it into view.
  function keyCrisis(item, button, keyed) {
    if (keyed) {
      root.keyedButton = button
      root.reveal(item, button)
    } else if (root.keyedButton === button) {
      root.keyedButton = null
    }
  }

  // The crisis under the cursor inside the scrolling content (I2): the
  // whole item when it fits, else its button's end at the bottom; a
  // scroll only when it is not in view already.
  function reveal(item, button) {
    var f = root.flickable
    var view = f.height
    var top = item.mapToItem(f.contentItem, 0, 0).y
    var from = top
    var to = top + item.height
    if (item.height > view) {
      to = button.mapToItem(f.contentItem, 0, 0).y + button.height
      from = to - view
    }
    var y = f.contentY
    if (from < y) y = from
    else if (to > y + view) y = to - view
    else return
    var max = Math.max(0, f.contentHeight - view)
    f.contentY = Math.max(0, Math.min(y, Math.max(max, to - view)))
  }

  // The keyed crisis's button lies inside the visible part of the pane.
  function crisisVisible() {
    var b = root.keyedButton
    if (root.crisisAt < 0 || !b) return false
    var f = root.flickable
    var y = b.mapToItem(f.contentItem, 0, 0).y
    return y >= f.contentY - 0.5 && y + b.height <= f.contentY + f.height + 0.5
  }

  onActionTriggered: function(id) {
    if (!root.service) return
    if (id === "review") {
      if (root.section) root.section.showTriage()
    } else if (!root.current) {
      return
    } else if (id === "apply") {
      if (root.arm) root.arm.disarm()
      root.service.applyProposal(root.seenId, "")
    } else if (id === "discard") {
      // the agent's unapplied work goes: arm twice
      if (root.arm && root.arm.press(root.discardKey,
          "Discard proposal " + root.seenId + "? Click Confirm discard. The logbook does not change."))
        root.service.discardProposal(root.seenId)
    }
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
      shown: root.seenId !== "",
      id: root.seenId,
      seen: root.seen.state,
      head: v ? v.head : "",
      hint: root.hint,
      state: v ? v.state : "",
      readable: v ? v.readable : false,
      actions: root.actions.map(function(a) { return a.label + (a.enabled === false ? " (off)" : "") }),
      applyArmed: root.applyArmed,
      discardArmed: root.discardArmed,
      crisisAt: root.crisisAt,
      crisisArmed: root.crisisArmed,
      crisisHint: root.crisisCount > 0 ? Model.crisisHint(root.crisisState, root.blocker) : "",
      crisisVisible: root.crisisVisible(),
      scroll: { y: Math.round(root.flickable.contentY), h: Math.round(root.flickable.contentHeight), view: Math.round(root.flickable.height) },
      result: root.result && !root.result.pending ? resultText.text : "",
      resultOk: root.result ? root.result.ok : true,
      built: items.status === Loader.Ready,
      crises: v && root.shown ? v.crises.map(root.itemView) : [],
      regular: v && root.shown ? v.regular.map(root.itemView) : []
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
  // every evidence text, its authors first.
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
      color: root.tone.dim
    }

    Line {
      objectName: "triageFlag"
      visible: card.modelData.flagged
      text: "Read twice: some evidence names an agent or an unknown author."
      color: root.tone.accentText
      font.bold: true
    }

    Line {
      visible: text !== ""
      text: card.modelData.outcome
        ? (card.modelData.outcome.state === "done" ? "Done" : card.modelData.outcome.state === "skipped" ? "Skipped" : "Refused")
          + (card.modelData.outcome.reason !== "" ? ": " + card.modelData.outcome.reason : "")
        : !card.modelData.open ? "No longer open: nothing to apply." : ""
      color: card.modelData.outcome && card.modelData.outcome.state === "refused" ? root.tone.urgentText : root.tone.dim
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
          color: root.tone.dim
          font.pixelSize: Style.font.caption
        }

        // The engine's text: "by <authors> · …" first, wrapped anywhere (a
        // long path breaks too), never clipped (Fable, WP-124 round 3).
        Line {
          objectName: "evidenceText"
          text: ref.modelData.text
          wrapMode: Text.WrapAtWordBoundaryOrAnywhere
          color: ref.modelData.flagged ? root.tone.accentText : root.foregroundColor
        }
      }
    }
  }

  Column {
    width: parent.width
    spacing: Style.spacing.lg

    Line {
      visible: root.seenId === ""
      text: "No proposal. “Ask agent to sort N changes” in the list asks for one."
      color: root.tone.dim
      font.pixelSize: Style.font.body
    }

    // Replaced or gone: said plainly; nothing here applies it.
    Line {
      objectName: "triageSeen"
      visible: root.seen.state === "replaced" || root.seen.state === "gone"
      text: root.seen.text
      color: root.tone.urgentText
      font.bold: true
    }

    Line {
      visible: !!root.proposal
      text: root.proposal ? root.proposal.state : ""
      color: root.tone.dim
    }

    Line {
      id: resultText
      objectName: "triageResult"
      visible: text !== ""
      text: !root.result ? ""
        : root.result.pending ? root.result.text
        : root.result.gone ? root.result.text + ". The proposal is gone; the list shows what is open now."
        : root.result.text
      color: root.result && !root.result.ok ? root.tone.urgentText : root.tone.dim
    }

    Line {
      visible: !!root.proposal && !root.proposal.readable
      text: "The proposal file could not be read as the engine writes it. Discard it, or ask the agent again."
      color: root.tone.urgentText
    }

    Line {
      visible: !!root.proposal && root.proposal.readable
      text: "Apply re-reads the file and every reference. If the file was changed since you opened it, what Apply writes can differ from what is shown here."
      color: root.tone.dim
      font.pixelSize: Style.font.caption
    }

    // The items, built only while the pane is shown, and in the background
    // (incubated in slices): a proposal of 200 items × 10 refs never holds
    // the shell thread for the length of its build (R2).
    Loader {
      id: items
      width: parent.width
      active: root.shown && !!root.proposal
      asynchronous: true

      sourceComponent: Column {
        width: items.width
        spacing: Style.spacing.lg

        // Crises: never inside Apply (ADR-0028 §3); one click or Enter
        // twice each, one per run (WP-201).
        Line {
          visible: !!root.proposal && root.proposal.crises.length > 0
          text: "CRISES — EACH ON ITS OWN"
          color: root.tone.urgentText
          font.pixelSize: Style.font.caption
          font.bold: true
        }

        // the keys that reach them (WP-201)
        Line {
          objectName: "crisisHint"
          visible: !!root.proposal && root.proposal.crises.length > 0
          text: Model.crisisHint(root.crisisState, root.blocker)
          color: root.tone.dim
          font.pixelSize: Style.font.caption
        }

        Repeater {
          model: root.proposal ? root.proposal.crises : []

          Column {
            id: crisis

            required property var modelData
            required property int index
            readonly property bool keyed: root.crisisAt === crisis.index

            width: parent.width
            spacing: Style.spacing.sm
            onKeyedChanged: root.keyCrisis(crisis, crisisButton, crisis.keyed)
            // laid out (or its texts wrapped) after it was keyed: in view again
            onYChanged: if (crisis.keyed) root.reveal(crisis, crisisButton)
            onHeightChanged: if (crisis.keyed) root.reveal(crisis, crisisButton)
            Component.onCompleted: if (crisis.keyed) root.keyCrisis(crisis, crisisButton, true)

            Proposed {
              modelData: crisis.modelData
            }

            Button {
              id: crisisButton
              text: "Apply this crisis"
              tooltipText: "Only this item: drift apply --item " + crisis.modelData.eventId
              bordered: true
              enabled: root.canWrite && !root.pending && crisis.modelData.open && root.current
              foreground: Commons.Color.urgent
              fontFamily: root.fontFamily
              fontSize: Style.font.caption
              verticalPadding: Style.spacing.xs
              hasCursor: crisis.keyed
              onClicked: root.applyCrisis(crisis.modelData.eventId)

              FocusRing {
                shown: crisis.keyed
              }
            }
          }
        }

        Line {
          visible: !!root.proposal && root.proposal.regular.length > 0
          text: "WHAT APPLY TAKES"
          color: root.tone.dim
          font.pixelSize: Style.font.caption
          font.bold: true
        }

        Repeater {
          model: root.proposal ? root.proposal.regular : []

          Proposed {}
        }

        Line {
          text: "Applying writes each resolution as you, with the agent's name and the evidence. Nothing is required: unproven changes stay quietly open."
          color: root.tone.dim
          font.pixelSize: Style.font.caption
        }
      }
    }
  }
}
