pragma ComponentBehavior: Bound

import QtQuick
import qs.Commons
import qs.Ui
import "../components"
import "../components/desk"
import "../Model.js" as Model

// Section 1, Today (ADR-0034 §2, SPEC-PLUGIN §5.4; prototype `today`). The
// list: the date beside the day's state pictogram (A11: crisis, case
// active, all clear; attention alone changes nothing, ADR-0028 §4b) and
// *Open in editor*, the tiles (events today, 7 days), the journal field
// (JournalField.qml: `seldon log`), then NEEDS YOU — the crises, newest
// first, a group by its leader (the 0.1 red strip's successor) — and
// JOURNAL: today's entries, the yesterday row, which opens in place.
//
// The detail: nothing selected (on entry), the overview — the setup card
// while Seldon is not set up (SetupCard.qml, WP-119: engine → logbook →
// snapshots), one sentence ("Seldon is recording. 2 changes need you."),
// or, on a first day with no case and nothing open, the first-run card
// ("Seldon is recording. Nothing to do."; the zero tiles quiet),
// the active cases as tiles with their progress (a click opens the case in
// Work) and **New case**: one sentence → `seldon agent start --new --json
// -- <intent>` (Service.startAgentNew, the call and result line Work's Run
// shares). A selected crisis: the event's detail with its sticky bar, as
// in the Changelog (EventDetail.qml).
//
// Rows come from Service.deskToday (built once per index). Keys: ↑/↓ j/k
// move, Enter opens the yesterday row or a crisis's default form, `n` the
// journal field, `i` the New case field, `e` today's journal in the
// editor, Esc a shown form.
//
// While the setup card is up, its row in the list head ("Set up Seldon ·
// 2 of 3 steps to go") is the row above the first one (WP-202, the
// pattern of the Changelog's proposal row): ↑ on the first row, or the
// first ↑ while nothing is selected, selects it; ←/→ pick a button of the
// card's current step, Enter arms it and the second Enter runs it as its
// click does (Service.setupAction); on a stacked desk the first Enter
// shows the card. ↓ goes back to the first row, ↑ on to the notices. A
// click, another selection, the stacked Back or a changed step drops the
// arm.
//
// Before the logbook exists (status notInitialised, WP-138) the list shows
// what the machine remembers on its own (Service.preview, `seldon preview
// --json`): PACKAGES, one row per pacman transaction, and EDITED CONFIG
// FILES, one row per file under ~/.config; the setup card's step 2 says
// what that is without memory and creates the logbook.
Section {
  id: root

  property bool yesterdayOpen: false
  // The intent sent last, until the engine answers (the journal field's
  // pattern: the text stays until the case exists).
  property string sentIntent: ""

  readonly property var today: root.service ? root.service.deskToday : null
  // The active cases' tiles, "agent working" first on a case an agent works on
  // (FEEDBACK 11, WP-156's sessions).
  readonly property var caseTiles: Model.todayCaseTiles(root.today ? root.today.cases : [],
    root.service ? root.service.agentSessions : null)
  // WP-138: the preview while the logbook is not initialised (null in dev
  // mode, where the engine never runs)
  readonly property var preview: root.service && root.service.status === "notInitialised" ? root.service.preview : null
  // WP-119: the setup card, and the first-run card once it is done.
  readonly property var setup: root.service ? root.service.setup : null
  readonly property bool firstRun: !root.setup && !!root.today && root.today.firstRun === true
  // Seldon records, only the optional snapshot step is left: the
  // sentence stays under the card (WP-119 round 2, S2)
  readonly property bool sentenceShown: (!root.setup || root.setup.optionalOnly === true) && !root.firstRun
  readonly property var rows: root.preview ? Model.previewRows(root.preview, root.searchText)
    : Model.todayRows(root.today, root.yesterdayOpen, root.searchText)
  readonly property int cursor: root.rowIndex(root.selectedId)
  readonly property var current: root.cursor >= 0 ? root.rows[root.cursor] : null
  // A crisis stays shown once resolved here (it leaves NEEDS YOU), with the
  // engine's answer, while the index has the event.
  readonly property bool eventShown: Model.EVENT_ID.test(root.selectedId)
    && !!Model.changelogRow(root.service ? root.service.deskChangelog : null, root.selectedId)
  readonly property var result: root.service && root.service.planResult && root.service.planResult.action === "agent-new"
    ? root.service.planResult : null
  readonly property bool pending: !!root.service && !!root.service.planResult && root.service.planResult.pending
  readonly property bool canWrite: !!root.service && root.service.canWrite
  readonly property color foreground: Color.popups.text
  // WP-202: the setup row as the selection, the button the keys are on,
  // and the arm id it presses (it names the step, its banner and the
  // action, so an arm never runs a button that took its place).
  property bool setupFocused: false
  property string setupPick: ""
  readonly property bool setupRowShown: !!root.setup
  readonly property var setupKeys: Model.setupTargets(root.setup)
  readonly property var setupButton: root.setupFocused ? Model.pickAction(root.setupKeys, root.setupPick) : null
  readonly property string setupKey: root.setupButton ? root.setupButton.key : ""
  readonly property bool setupArmed: root.setupKey !== "" && !!root.arm && root.arm.armedId === root.setupKey

  editing: journal.editing || intentField.activeFocus || eventDetail.editing

  function rowIndex(id) {
    if (id === "") return -1
    var rows = root.rows || []
    for (var i = 0; i < rows.length; i++) if (rows[i].id === id) return i
    return -1
  }

  function move(dy) {
    var to = Model.todayMove({ rows: root.rows.length, cursor: root.cursor, setup: root.setupRowShown,
      setupFocused: root.setupFocused }, dy)
    if (to.out) return false
    if (to.setup) {
      // a key selects, as on a row: on a stacked desk Enter shows the card
      if (!root.setupFocused) root.focusSetup(false)
    } else if (to.row >= 0) {
      root.setupFocused = false
      root.selectedId = root.rows[to.row].id
    }
    return true
  }

  function moveAcross(dx) {
    if (!root.setupFocused) return false
    if (!root.stacked || root.detailShown) root.setupPick = Model.stepAction(root.setupKeys, root.setupPick, dx)
    return true
  }

  // The setup row as the selection: the overview with the card, the
  // cursor on the current step's first button; `openDetail` (a click,
  // IPC; not a key) also shows the card on a stacked desk.
  function focusSetup(openDetail) {
    if (!root.setupRowShown) return false
    root.selectedId = ""
    root.setupFocused = true
    var first = Model.pickAction(root.setupKeys, "")
    root.setupPick = first ? first.id : ""
    if (openDetail === true && root.desk) root.desk.showDetail()
    return true
  }

  function disarmSetup() {
    if (root.arm && root.arm.armedId.indexOf("setup:") === 0) root.arm.disarm()
  }

  function leaveKeys() {
    root.setupFocused = false
  }

  function enterFromTop() {
    if (root.setupRowShown) root.focusSetup(false)
    else if (root.rows.length > 0) root.selectedId = root.rows[0].id
  }

  function activate() {
    if (root.setupFocused) {
      if (root.stacked && !root.detailShown) {
        if (root.desk) root.desk.showDetail()
        return true
      }
      var b = root.setupButton
      if (!b || !b.ready || !root.arm) return true
      if (root.arm.press(b.key, Model.keyArmHint(b.label))) root.setupAction(root.setupKeys.step, b.id)
      return true
    }
    var row = root.current
    if (!row) return false
    if (row.type === "toggle") {
      root.yesterdayOpen = !root.yesterdayOpen
      return true
    }
    if (root.stacked && !root.detailShown) {
      if (root.desk) root.desk.showDetail()
      return true
    }
    return row.type === "crisis" ? eventDetail.activate() : false
  }

  function focusJournal() {
    journal.focusField()
  }

  function focusIntent() {
    if (!root.canWrite) return
    if (root.eventShown) root.selectedId = ""
    if (root.desk && root.stacked) root.desk.showDetail()
    Qt.callLater(function() { intentField.forceActiveFocus() })
  }

  function textKey(t) {
    if (t === "n") {
      root.focusJournal()
      return true
    }
    if (t === "i") {
      root.focusIntent()
      return true
    }
    if (t === "e") {
      if (root.service) root.service.openInEditor("journal")
      return true
    }
    return false
  }

  // A crisis by its event id (IPC `select`); `setup`: the setup row
  // (navigation only, WP-202).
  function select(id) {
    if (String(id) === "setup") return root.focusSetup(true)
    var i = root.rowIndex(String(id))
    if (i === -1) return false
    root.selectedId = String(id)
    if (root.desk) root.desk.showDetail()
    return true
  }

  // One of the setup card's buttons (Service.setupAction).
  function setupAction(stepId, actionId) {
    return !!root.service && root.service.setupAction(stepId, actionId)
  }

  function back() {
    return root.eventShown && eventDetail.back()
  }

  function runIntent() {
    if (!root.service || root.pending) return false
    var sent = root.service.startAgentNew(intentField.text)
    if (sent) root.sentIntent = intentField.text
    return sent
  }

  function view() {
    return {
      title: root.today ? root.today.title : "",
      state: root.today && root.today.state ? root.today.state.id : "",
      tiles: root.today ? root.today.tiles.map(function(t) { return t.label + " " + t.value }) : [],
      needs: root.today ? root.today.needs.map(function(r) { return r.id }) : [],
      entries: root.today ? root.today.entries.length : 0,
      yesterday: root.today ? root.today.yesterday.length : 0,
      rows: root.rows.length,
      cursor: root.cursor,
      selected: root.selectedId,
      shown: root.eventShown ? "event" : "overview",
      // the overview's sentence, also while an event is shown; "" while the
      // setup card or the first-run card takes its place
      headline: root.sentenceShown && root.today ? root.today.headline : "",
      firstRun: firstRunCard.visible,
      dimTiles: root.today ? root.today.tiles.filter(function(t) { return t.dim }).map(function(t) { return t.label }) : [],
      cases: root.today ? root.today.cases.map(function(c) { return c.id }) : [],
      caseTexts: root.caseTiles.map(function(c) { return c.text }),
      setupSlot: setupSlot.visible,
      setup: setupCard.view(),
      // WP-202: the setup row in the list head and the keys on the card
      setupRow: {
        shown: setupRow.visible,
        focused: root.setupFocused,
        pick: root.setupButton ? root.setupButton.label : "",
        armed: root.setupArmed,
        hint: root.setupRowShown ? setupHint.text : ""
      },
      preview: {
        shown: !!root.preview && setupCard.visible,
        summary: root.preview ? Model.previewSummary(root.preview) : "",
        setUp: setupCard.view().current === "logbook" && setupCard.view().ready,
        groups: root.rows.filter(function(r, i) { return i === 0 || root.rows[i - 1].group !== r.group })
          .map(function(r) { return r.groupTitle || "" })
      },
      journal: {
        enabled: journal.enabledHere,
        cases: journal.options.length,
        editing: journal.editing,
        text: journal.text,
        caseId: journal.caseId,
        result: journal.resultText
      },
      intent: intentField.text,
      intentEditing: intentField.activeFocus,
      result: root.result ? root.result.text : "",
      resultOk: !!root.result && root.result.ok,
      detail: root.eventShown ? eventDetail.view() : null
    }
  }

  onRowsChanged: if (root.selectedId !== "" && root.rowIndex(root.selectedId) === -1 && !root.eventShown) root.selectedId = ""
  // The setup row stops being the selection on another one (a click, IPC)
  // or when the card goes; its arm goes with it, with a changed step or
  // button, and with the stacked card hidden by its Back (WP-202).
  onSelectedIdChanged: if (root.selectedId !== "") root.setupFocused = false
  onSetupKeysChanged: if (!root.setupKeys) root.setupFocused = false
  onSetupFocusedChanged: if (!root.setupFocused) root.disarmSetup()
  onSetupKeyChanged: root.disarmSetup()
  onDetailShownChanged: if (root.stacked && !root.detailShown) root.disarmSetup()
  // A Run made the case: the field empties.
  onResultChanged: {
    var r = root.result
    if (!r || r.pending || !r.ok) return
    if (intentField.text === root.sentIntent) intentField.text = ""
    root.sentIntent = ""
  }
  // Another section shown: a field gives the keys back (a hidden field
  // keeps Qt's focus otherwise); drafts stay. The setup row's keys go.
  onActiveChanged: {
    if (root.active) return
    root.leaveKeys()
    if (!root.editing) return
    journal.leave()
    eventDetail.back()
    if (intentField.activeFocus && root.desk) root.desk.takeKeys()
  }

  ListColumn {
    id: list
    keyEvents: root.keyEvents
    visible: !root.stacked || !root.detailShown
    width: root.stacked ? root.width : (root.layout ? root.layout.listW : 0)
    height: root.height
    divider: !root.stacked
    title: "Today"
    model: root.rows
    currentIndex: root.cursor
    emptyText: root.preview ? Model.previewSummary(root.preview) : root.index ? "" : "No index to show"

    // The date beside the day's state; *Open in editor* follows it on the
    // same line when there is room, else under it. A header row for the
    // harness's overlap check (WP-211, WP-215).
    Item {
      objectName: "headerRow:today"
      width: parent.width
      implicitHeight: Math.max(stateIcon.visible ? stateIcon.height : 0, dateFlow.implicitHeight)

      MaskIcon {
        id: stateIcon
        anchors.verticalCenter: parent.verticalCenter
        width: Style.space(48)
        height: Style.space(48)
        visible: !!root.today && root.today.state !== null
        file: root.today && root.today.state ? Model.pictogramFile(root.today.state.id, width) : ""
        color: !root.today || !root.today.state || root.today.state.tone === "default" ? root.foreground
          : root.today.state.tone === "urgent" ? Color.urgent : Color.accent
      }

      Flow {
        id: dateFlow
        anchors.left: stateIcon.visible ? stateIcon.right : parent.left
        anchors.leftMargin: stateIcon.visible ? Style.spacing.xl : 0
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        spacing: Style.spacing.md

        Text {
          id: dateText
          width: Math.min(implicitWidth, dateFlow.width)
          height: editButton.implicitHeight
          verticalAlignment: Text.AlignVCenter
          textFormat: Text.PlainText
          text: root.today ? root.today.title : "Today"
          color: root.foreground
          elide: Text.ElideRight
          font.family: Style.font.family
          font.pixelSize: Style.font.subtitle
          font.bold: true
        }

        Button {
          id: editButton
          text: Model.text("action.openEditor", "pro")
          tooltipText: "Today's journal (key e)"
          bordered: true
          foreground: root.foreground
          fontFamily: Style.font.family
          fontSize: Style.font.caption
          verticalPadding: Style.spacing.xs
          onClicked: if (root.service) root.service.openInEditor("journal")
        }
      }
    }

    Row {
      id: tileRow
      width: parent.width
      spacing: Style.spacing.md
      visible: !!root.today && root.today.tiles.length > 0

      Repeater {
        model: root.today ? root.today.tiles : []

        BorderSurface {
          id: tile

          required property var modelData

          width: (tileRow.width - tileRow.spacing) / 2
          implicitHeight: tileColumn.implicitHeight + Style.spacing.lg * 2
          // a zero is quiet (WP-119): the figure in the muted tone
          radius: Style.cornerRadius
          color: Style.normalFill
          borderSpec: Border.flat(root.tone.divider, Math.max(1, Style.space(1)))

          Column {
            id: tileColumn
            x: Style.spacing.xl
            y: Style.spacing.lg
            width: tile.width - Style.spacing.xl * 2
            spacing: Style.spacing.xxs

            Text {
              width: parent.width
              textFormat: Text.PlainText
              text: tile.modelData.label
              color: root.tone.dim
              elide: Text.ElideRight
              font.family: Style.font.family
              font.pixelSize: Style.font.caption
            }

            Text {
              width: parent.width
              textFormat: Text.PlainText
              text: String(tile.modelData.value)
              color: tile.modelData.dim ? root.tone.dim : root.foreground
              font.family: Style.font.family
              font.pixelSize: Style.font.title
              font.bold: true
            }
          }
        }
      }
    }

    JournalField {
      id: journal
      width: parent.width
      service: root.service
      indexData: root.index
      foreground: root.foreground
      urgent: Color.urgent
      fontFamily: Style.font.family
      onLeaveRequested: if (root.desk) root.desk.takeKeys()
    }

    // The setup card's row (WP-202): the row above the first one, the
    // keys' way to the card's buttons; a click selects it too.
    Rectangle {
      id: setupRow
      objectName: "setupRow"
      width: parent.width
      visible: root.setupRowShown
      implicitHeight: setupRowColumn.implicitHeight + Style.spacing.md * 2
      radius: Style.cornerRadius
      color: root.setupFocused ? Style.selectedFillFor(Color.accent, Color.accent) : "transparent"
      border.width: Math.max(1, Style.space(1))
      // selected: the accent tint plus a border of at least 3:1 (WP-177)
      border.color: root.setupFocused ? root.tone.accentUi : Style.normalBorderFor(Color.accent, Color.accent)

      Column {
        id: setupRowColumn
        x: Style.spacing.md
        y: Style.spacing.md
        width: parent.width - Style.spacing.md * 2
        spacing: Style.spacing.xxs

        Text {
          width: parent.width
          textFormat: Text.PlainText
          text: root.setup ? root.setup.headline : ""
          color: root.foreground
          wrapMode: Text.Wrap
          font.family: Style.font.family
          font.pixelSize: Style.font.caption
        }

        Text {
          id: setupHint
          objectName: "setupHint"
          width: parent.width
          textFormat: Text.PlainText
          text: Model.setupHint(root.setupFocused, root.stacked && !root.detailShown)
          color: root.tone.dim
          wrapMode: Text.Wrap
          font.family: Style.font.family
          font.pixelSize: Style.font.caption
        }
      }

      MouseArea {
        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor
        onClicked: root.focusSetup(true)
      }
    }

    delegate: GroupedRow {
      required property var modelData
      required property int index

      width: ListView.view.width
      header: index === 0 || (root.rows[index - 1] || {}).group !== modelData.group
        ? (modelData.groupTitle !== undefined ? modelData.groupTitle : modelData.group === "needs" ? "Needs you" : "Journal")
        : ""
      title: modelData.title
      meta: modelData.meta
      aside: modelData.aside
      stripe: modelData.stripe
      alert: modelData.alert || ""
      selected: modelData.id === root.selectedId
      cursor: list.hoverIndex === index
      onClicked: {
        root.selectedId = modelData.id
        if (modelData.type === "toggle") root.yesterdayOpen = !root.yesterdayOpen
        else if (root.desk && modelData.type === "crisis") root.desk.showDetail()
      }
    }
  }

  EventDetail {
    id: eventDetail
    x: root.stacked ? 0 : list.width
    width: root.width - x
    height: root.height
    visible: root.eventShown && (!root.stacked || root.detailShown)
    backVisible: root.stacked
    section: root
    eventId: root.eventShown ? root.selectedId : ""
    onBackRequested: if (root.desk) root.desk.back()
    onLeaveRequested: if (root.desk) root.desk.takeKeys()
  }

  DetailPane {
    id: overview
    x: root.stacked ? 0 : list.width
    width: root.width - x
    height: root.height
    visible: !root.eventShown && (!root.stacked || root.detailShown)
    backVisible: root.stacked
    title: "Overview"
    onBackRequested: if (root.desk) root.desk.back()

    // The setup card (WP-119): engine → logbook → snapshots, while a step
    // is left; step 2 carries what the machine remembers without Seldon
    // (WP-138's preview, ADR-0047).
    Item {
      id: setupSlot
      objectName: "todaySetupSlot"
      width: parent.width
      visible: !!root.setup
      implicitHeight: visible ? setupCard.implicitHeight : 0

      SetupCard {
        id: setupCard
        objectName: "todaySetupCard"
        width: parent.width
        setup: root.setup
        previewText: root.preview ? Model.previewSummary(root.preview) + " " + Model.PREVIEW_LEAD : ""
        launched: root.service && root.service.setupWatch ? root.service.setupWatch.step : ""
        resultText: root.service ? root.service.setupResult : ""
        cursorAction: root.setupButton ? root.setupButton.id : ""
        armText: root.setupArmed ? root.arm.hint : ""
        foreground: root.foreground
        onActionRequested: function(stepId, actionId) { root.setupAction(stepId, actionId) }
      }
    }

    // The first-run card (WP-119): the first day, no case, nothing open.
    BorderSurface {
      id: firstRunCard
      objectName: "todayFirstRun"
      width: parent.width
      visible: root.firstRun
      implicitHeight: visible ? firstRunColumn.implicitHeight + contentTopInset + contentBottomInset : 0
      radius: Style.cornerRadius
      // a notice's frame on the normal fill, never the cursor's selected fill
      color: Style.normalFillFor(root.foreground, Color.accent)
      borderSpec: Border.controlSpec("normal", root.tone.accentUi, root.tone.accentUi)
      padding: Style.spacing.xl

      Column {
        id: firstRunColumn
        x: firstRunCard.contentLeftInset
        y: firstRunCard.contentTopInset
        width: firstRunCard.width - firstRunCard.contentLeftInset - firstRunCard.contentRightInset
        spacing: Style.spacing.md

        Text {
          width: parent.width
          textFormat: Text.PlainText
          text: Model.FIRST_RUN_TITLE
          color: root.foreground
          wrapMode: Text.Wrap
          font.family: Style.font.family
          font.pixelSize: Style.font.title
          font.bold: true
        }

        Text {
          width: parent.width
          textFormat: Text.PlainText
          text: Model.FIRST_RUN_LEAD
          color: root.foreground
          wrapMode: Text.Wrap
          font.family: Style.font.family
          font.pixelSize: Style.font.bodySmall
        }
      }
    }

    Text {
      id: headline
      width: parent.width
      visible: root.sentenceShown
      textFormat: Text.PlainText
      text: root.today ? root.today.headline : ""
      color: root.foreground
      wrapMode: Text.Wrap
      font.family: Style.font.family
      font.pixelSize: Style.font.title
      font.bold: true
    }

    Text {
      width: parent.width
      visible: !!root.index && !root.firstRun
      textFormat: Text.PlainText
      text: root.today ? root.today.lead : ""
      color: root.tone.dim
      wrapMode: Text.Wrap
      font.family: Style.font.family
      font.pixelSize: Style.font.bodySmall
    }

    Column {
      width: parent.width
      visible: !!root.index
      spacing: Style.spacing.lg

      Text {
        width: parent.width
        textFormat: Text.PlainText
        text: "ACTIVE CASES"
        color: root.tone.dim
        font.family: Style.font.family
        font.pixelSize: Style.font.caption
        font.letterSpacing: Style.space(1)
        font.bold: true
      }

      Text {
        width: parent.width
        visible: !!root.today && root.today.cases.length === 0
        textFormat: Text.PlainText
        text: "No case is active."
        color: root.tone.dim
        font.family: Style.font.family
        font.pixelSize: Style.font.bodySmall
      }

      Flow {
        id: tiles
        width: parent.width
        spacing: Style.spacing.md

        Repeater {
          model: root.caseTiles

          CaseTile {
            required property var modelData

            width: Math.max(Style.space(220), (tiles.width - tiles.spacing) / 2)
            caseData: modelData
            foreground: root.foreground
            onClicked: if (root.desk) {
              root.desk.section("work")
              root.desk.select(modelData.id)
            }
          }
        }
      }
    }

    Column {
      width: parent.width
      spacing: Style.spacing.md

      Text {
        width: parent.width
        textFormat: Text.PlainText
        text: "NEW CASE"
        color: root.tone.dim
        font.family: Style.font.family
        font.pixelSize: Style.font.caption
        font.letterSpacing: Style.space(1)
        font.bold: true
      }

      Item {
        width: parent.width
        implicitHeight: Math.max(intentField.implicitHeight, runButton.implicitHeight)

        TextField {
          id: intentField
          anchors.left: parent.left
          anchors.right: runButton.left
          anchors.rightMargin: Style.spacing.sm
          anchors.verticalCenter: parent.verticalCenter
          enabled: root.canWrite
          placeholderText: root.canWrite ? "One sentence: what should happen? Enter starts an agent"
            : root.service ? root.service.writeBlocker : "The Seldon service is not running"
          foreground: root.foreground
          font.family: Style.font.family
          font.pixelSize: Style.font.bodySmall
          // Return or Enter (WP-173): once per press, a held key's repeat
          // does nothing; `keyEvents` for the harness's key guard read-out.
          readonly property string keyGuard: "intent"
          property int keyEvents: 0
          function keyPressed(event) {
            intentField.keyEvents++
            event.accepted = true
            if (!event.isAutoRepeat) root.runIntent()
          }
          Keys.onReturnPressed: function(event) { intentField.keyPressed(event) }
          Keys.onEnterPressed: function(event) { intentField.keyPressed(event) }
          Keys.onEscapePressed: function(event) {
            if (root.desk) root.desk.takeKeys()
            event.accepted = true
          }
        }

        Button {
          id: runButton
          anchors.right: parent.right
          anchors.verticalCenter: parent.verticalCenter
          readonly property bool running: root.pending && !!root.result
          text: running ? "Starting" : Model.text("action.startAgent", "pro")
          iconText: running ? "󰦖" : ""
          iconSpinning: running
          iconSize: Style.font.caption
          enabled: root.canWrite && !root.pending && intentField.text.trim() !== ""
          selected: true
          bordered: true
          foreground: root.foreground
          fontFamily: Style.font.family
          fontSize: Style.font.caption
          verticalPadding: Style.spacing.xs
          tooltipText: "Creates and starts the case, launches your default agent (Enter; key i)"
          onClicked: root.runIntent()
        }
      }

      Text {
        width: parent.width
        textFormat: Text.PlainText
        text: root.result ? root.result.text : "Starts your default agent on a new case, as Omarchy starts it."
        color: root.result && !root.result.ok ? root.tone.urgentText : root.tone.dim
        wrapMode: Text.Wrap
        font.family: Style.font.family
        font.pixelSize: Style.font.caption
      }
    }
  }
}
