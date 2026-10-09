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
// The detail: nothing selected (on entry), the overview — WP-119's setup
// card slot, one sentence ("Seldon is recording. 2 changes need you."),
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
// Before the logbook exists (status notInitialised, WP-138) the list shows
// what the machine remembers on its own (Service.preview, `seldon preview
// --json`): PACKAGES, one row per pacman transaction, and EDITED CONFIG
// FILES, one row per file under ~/.config; the overview's setup slot holds the card
// that says what that is without memory and **Set up Seldon**, the
// notInitialised banner's terminal fix (WP-119's setup card replaces it).
Section {
  id: root

  property bool yesterdayOpen: false
  // The intent sent last, until the engine answers (the journal field's
  // pattern: the text stays until the case exists).
  property string sentIntent: ""

  readonly property var today: root.service ? root.service.deskToday : null
  // WP-138: the preview while the logbook is not initialised (null in dev
  // mode, where the engine never runs)
  readonly property var preview: root.service && root.service.status === "notInitialised" ? root.service.preview : null
  readonly property bool setupFixable: !!root.service && !!root.service.banner
    && root.service.banner.status === "notInitialised"
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

  editing: journal.editing || intentField.activeFocus || eventDetail.editing

  function rowIndex(id) {
    if (id === "") return -1
    var rows = root.rows || []
    for (var i = 0; i < rows.length; i++) if (rows[i].id === id) return i
    return -1
  }

  function move(dy) {
    if (root.rows.length === 0) return true
    var from = root.cursor >= 0 ? root.cursor : dy > 0 ? -1 : root.rows.length
    var i = Math.max(0, Math.min(root.rows.length - 1, from + dy))
    root.selectedId = root.rows[i].id
    return true
  }

  function activate() {
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

  // A crisis by its event id (IPC `select`).
  function select(id) {
    var i = root.rowIndex(String(id))
    if (i === -1) return false
    root.selectedId = String(id)
    if (root.desk) root.desk.showDetail()
    return true
  }

  // Set up Seldon (WP-138): the notInitialised banner's terminal fix.
  function setUp() {
    return root.setupFixable && root.service.fix("terminal", "status")
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
      headline: root.today ? root.today.headline : "",
      cases: root.today ? root.today.cases.map(function(c) { return c.id }) : [],
      setupSlot: setupSlot.visible,
      preview: {
        shown: previewCard.visible,
        summary: root.preview ? Model.previewSummary(root.preview) : "",
        setUp: setupButton.enabled,
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
  // A Run made the case: the field empties.
  onResultChanged: {
    var r = root.result
    if (!r || r.pending || !r.ok) return
    if (intentField.text === root.sentIntent) intentField.text = ""
    root.sentIntent = ""
  }
  // Another section shown: a field gives the keys back (a hidden field
  // keeps Qt's focus otherwise); drafts stay.
  onActiveChanged: if (!root.active && root.editing) {
    journal.leave()
    eventDetail.back()
    if (intentField.activeFocus && root.desk) root.desk.takeKeys()
  }

  ListColumn {
    id: list
    visible: !root.stacked || !root.detailShown
    width: root.stacked ? root.width : (root.layout ? root.layout.listW : 0)
    height: root.height
    divider: !root.stacked
    title: "Today"
    model: root.rows
    currentIndex: root.cursor
    emptyText: root.preview ? Model.previewSummary(root.preview) : root.index ? "" : "No index to show"

    // The date beside the day's state; *Open in editor* follows it on the
    // same line when there is room, else under it.
    Item {
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
          text: "Open in editor"
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
          radius: Style.cornerRadius
          color: Style.normalFill
          borderSpec: Border.flat(Util.alpha(root.foreground, 0.12), Math.max(1, Style.space(1)))

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
              color: Color.muted
              elide: Text.ElideRight
              font.family: Style.font.family
              font.pixelSize: Style.font.caption
            }

            Text {
              width: parent.width
              textFormat: Text.PlainText
              text: String(tile.modelData.value)
              color: root.foreground
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
      cursor: false
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

    // WP-119's setup card (engine → logbook → snapshots) takes this slot
    // while Seldon is not recording; until then it holds the preview card
    // before the logbook exists (WP-138).
    Item {
      id: setupSlot
      objectName: "todaySetupSlot"
      width: parent.width
      visible: !!root.preview
      implicitHeight: root.preview ? previewCard.implicitHeight : 0

      Column {
        id: previewCard
        objectName: "todayPreview"
        width: parent.width
        spacing: Style.spacing.md

        Text {
          width: parent.width
          textFormat: Text.PlainText
          text: Model.PREVIEW_TITLE
          color: root.foreground
          wrapMode: Text.Wrap
          font.family: Style.font.family
          font.pixelSize: Style.font.title
          font.bold: true
        }

        Text {
          width: parent.width
          textFormat: Text.PlainText
          text: Model.PREVIEW_LEAD
          color: root.foreground
          wrapMode: Text.Wrap
          font.family: Style.font.family
          font.pixelSize: Style.font.bodySmall
        }

        Text {
          width: parent.width
          textFormat: Text.PlainText
          text: root.preview ? Model.previewSummary(root.preview) : ""
          color: root.preview && root.preview.ok === false && root.preview.pending !== true ? Color.urgent : Color.muted
          wrapMode: Text.Wrap
          font.family: Style.font.family
          font.pixelSize: Style.font.caption
        }

        Button {
          id: setupButton
          objectName: "todaySetUp"
          text: Model.PREVIEW_SETUP
          tooltipText: "Opens a terminal that creates your logbook and starts recording"
          enabled: root.setupFixable
          selected: true
          bordered: true
          foreground: root.foreground
          fontFamily: Style.font.family
          fontSize: Style.font.caption
          verticalPadding: Style.spacing.xs
          onClicked: root.setUp()
        }
      }
    }

    Text {
      width: parent.width
      visible: !root.preview
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
      visible: !!root.index
      textFormat: Text.PlainText
      text: root.today ? root.today.lead : ""
      color: Color.muted
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
        color: Color.muted
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
        color: Color.muted
        font.family: Style.font.family
        font.pixelSize: Style.font.bodySmall
      }

      Flow {
        id: tiles
        width: parent.width
        spacing: Style.spacing.md

        Repeater {
          model: root.today ? root.today.cases : []

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
        color: Color.muted
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
          text: running ? "Starting" : "Start agent"
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
        color: root.result && !root.result.ok ? Color.urgent : Color.muted
        wrapMode: Text.Wrap
        font.family: Style.font.family
        font.pixelSize: Style.font.caption
      }
    }
  }
}
