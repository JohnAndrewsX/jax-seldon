import QtQuick
import QtQuick.Window
import qs.Commons
import qs.Ui
import "components"
import "Model.js" as Model

// The bar panel (SPEC-PLUGIN §5), opened by a left click on the pill or by
// the `jax.seldon.panel` IPC target that BarWidget.qml owns.
//
// Top to bottom: title, tab strip, the status banner (WP-010), the
// snapper-degraded banner (ADR-0011), the red crisis strip, then the current
// tab. The banners and the strip sit above the tabs, so every tab shows them.
// Tabs: Today, Changelog, Work, Decisions, System, Memory, in the order of
// their fixed digits.
//
// Actions (WP-012) go through the service's queue with fixed argument lists:
// the QuickEntry note (`seldon log`), *Capture now* (capture, then status;
// the index refresh arrives through the FileView), and *Open in editor* per
// tab (`seldon open journal|ledger|status|<caseId> --editor`). The Work tab
// (WP-020) adds `seldon plan new|start|verify|done|drop`; the Changelog's
// drift sheet (WP-021) `seldon drift link|explain|dismiss` (and `drift show`
// for a group's members). A click on the red strip opens the sheet for the
// first crisis. The Decisions tab (WP-023) opens `seldon open ADR-NNNN` and
// sends `seldon decide --no-edit` (then opens the new decision); the Memory
// tab opens `seldon open logbook`.
//
// Keyboard (SPEC-PLUGIN §5):
//   Tab / Shift-Tab  the bar's next / previous panel, as every Omarchy panel
//   ← / →, h / l     previous / next tab
//   1–6              a tab by its fixed number (Model.TAB_KEYS: Today 1,
//                    Changelog 2, Work 3, Decisions 4, System 5, Memory 6);
//                    the digit of a tab this version lacks does nothing
//   ↑ / ↓, k / j     move in the tab's list
//   Enter, Space     open the row under the cursor (the yesterday row, a
//                    Changelog row; an open drift row opens its sheet; a
//                    decision or memory row in the editor); on Work, the
//                    card's first action (twice to write)
//   x                Work: drop the case under the cursor (twice)
//   f / F            Changelog: next / previous source filter
//   c                capture now
//   n                write a note (Today's QuickEntry; Esc gives the keys back)
//   +                new case (Work's sheet; Esc gives the keys back)
//   d                Decisions: new decision (its sheet; Enter twice creates)
//   e                open this tab's file in the editor (journal, ledger, the
//                    case under the Work cursor, the decision under the
//                    cursor, status, the logbook for Memory)
//   Esc              close
Panel {
  id: root
  moduleName: "jax.seldon"
  // BarWidget.qml owns the IPC target so it exists before this panel loads.
  manageIpc: false

  // Injected by BarWidget.qml.
  property var anchorItem: null
  property var hostWidget: null
  property var service: null

  // The bar identifies a panel by the widget in its slot (see the clock).
  readonly property var barIdentity: hostWidget || root

  readonly property color foreground: bar ? bar.foreground : Color.foreground
  readonly property color urgent: bar ? bar.urgent : Color.urgent
  readonly property color dim: Util.alpha(foreground, 0.65)
  readonly property string fontFamily: bar ? bar.fontFamily : Style.font.family

  readonly property var tabNames: ["Today", "Changelog", "Work", "Decisions", "System", "Memory"]
  readonly property var tabIds: ["today", "changelog", "work", "decisions", "system", "memory"]
  property int tabIndex: 0
  property bool cursorActive: false

  // The index only when its contents mean something in this status.
  readonly property var indexData: service && service.indexShown ? service.index : null
  readonly property var tabItems: [todayTab, changelogTab, workTab, decisionsTab, systemTab, memoryTab]
  readonly property string tabId: tabIds[tabIndex]
  // A text field or picker of a tab has the keys.
  readonly property bool editing: todayTab.editing || workTab.editing || changelogTab.editing || decisionsTab.editing
  // The bar widget setting `wipLimit` (Work tab).
  readonly property int wipLimit: Model.clampWipLimit(setting("wipLimit", Model.WIP_LIMIT_DEFAULT))
  readonly property var currentTab: tabItems[tabIndex]

  // Every tab change (keys, a click on the strip, IPC `tab`) gives the keys
  // back to the panel: a hidden tab's field keeps Qt's active focus, so
  // typed keys and Enter would reach a field nobody sees (WP-067).
  function selectTab(i) {
    var n = root.tabNames.length
    root.tabIndex = ((i % n) + n) % n
    if (root.opened && !keyCatcher.activeFocus) keyCatcher.forceActiveFocus()
  }

  function selectTabById(id) {
    var i = root.tabIds.indexOf(String(id))
    if (i === -1) return false
    root.selectTab(i)
    return true
  }

  function moveCursor(dy) {
    if (!root.cursorActive) {
      root.cursorActive = true
      return
    }
    root.currentTab.move(dy)
  }

  function textKey(t) {
    if (Model.TAB_KEYS[t] !== undefined) {
      root.selectTabById(Model.TAB_KEYS[t])
    } else if (t === "c" || t === "C") {
      root.captureNow()
    } else if (t === "n") {
      root.selectTabById("today")
      todayTab.quickEntry.focusField()
    } else if (t === "+") {
      root.newCase()
    } else {
      root.currentTab.textKey(t)
    }
  }

  function setFilter(source) {
    changelogTab.setFilter(source)
  }

  // The drift sheet: "crisis" (the strip) or an event id; never runs the
  // engine. False when the event is not open drift.
  function resolve(target) {
    root.selectTabById("changelog")
    root.cursorActive = true
    if (String(target) === "crisis") return changelogTab.openCrisis()
    changelogTab.setFilter("all")
    return changelogTab.openSheet(String(target))
  }

  function captureNow() {
    if (root.service) root.service.captureNow()
  }

  // The Work tab's new-case sheet, from any tab.
  function newCase() {
    root.selectTabById("work")
    if (workTab.canWrite) workTab.openSheet()
  }

  // journal | ledger | status | logbook | <caseId> | <ADR id>;
  // Service.openInEditor validates it.
  function openInEditor(what) {
    if (root.service) root.service.openInEditor(what)
  }

  // The QuickEntry gives the keys back, or its field was hidden with the tab.
  function restoreKeys() {
    if (root.opened && !root.editing && !keyCatcher.activeFocus) keyCatcher.forceActiveFocus()
  }

  // Tab walks the bar's panels from the slot's widget, not from this item.
  function switchPanel(direction) {
    if (root.bar && typeof root.bar.switchPanelFrom === "function")
      return root.bar.switchPanelFrom(root.barIdentity, direction)
    return false
  }

  // What the panel shows right now, for the harness and smoke tests
  // (`omarchy-shell jax.seldon.panel view`).
  function view() {
    var rows = changelogTab.rows
    return {
      opened: root.opened,
      tab: root.tabIds[root.tabIndex],
      cursorActive: root.cursorActive,
      cursor: root.currentTab.cursor,
      // The panel's key catcher has the keys (no text field does).
      keys: keyCatcher.activeFocus,
      tabStrip: { oneLine: tabStrip.fitsOneLine, widths: tabStrip.cellWidths.join(",") },
      status: root.service ? root.service.status : "",
      pill: root.service ? Model.pillText(root.service.counts) : "",
      // The header mark and the pictograms on screen (A5, A11): file names.
      mark: { file: headerMark.file, box: header.mark.box, ready: headerMark.ready },
      bannerPictogram: statusBanner.visible ? statusBanner.pictogram : "",
      banner: statusBanner.visible && root.service.banner ? root.service.banner.title : "",
      snapper: snapperBanner.visible && root.service.snapperBanner ? root.service.snapperBanner.title : "",
      crisis: crisisStrip.visible ? crisisLabel.text : "",
      today: {
        state: todayTab.dayState ? todayTab.dayState.id : "",
        entries: todayTab.view.entries.length,
        yesterday: todayTab.view.yesterday.length,
        rows: todayTab.rowCount,
        quickEntry: {
          enabled: todayTab.quickEntry.enabledHere,
          editing: todayTab.editing,
          text: todayTab.quickEntry.text,
          caseId: todayTab.quickEntry.caseId,
          cases: todayTab.quickEntry.options.length - 1,
          result: todayTab.quickEntry.resultText
        }
      },
      capturing: root.service ? root.service.capturing : false,
      captureResult: root.service && root.service.captureResult ? root.service.captureResult.text : "",
      openResult: root.service && root.service.openResult ? root.service.openResult.text : "",
      lastError: root.service ? root.service.lastError : "",
      changelog: {
        filter: changelogTab.filter,
        rows: rows.length,
        badges: rows.filter(function(r) { return r.badge !== "" }).map(function(r) { return r.subject + " " + r.badge }),
        folded: rows.filter(function(r) { return r.resolutionDetail !== "" }).length,
        snapshots: rows.filter(function(r) { return r.snapshot }).length,
        driftTones: rows.filter(function(r) { return r.drift }).map(function(r) { return r.subject + " " + r.tone }),
        resolved: rows.filter(function(r) { return r.resolution !== "" }).map(function(r) { return r.subject + ": " + Model.rowStatus(r) }),
        more: changelogTab.moreDrift,
        expanded: changelogTab.expandedId,
        selected: changelogTab.selectedId
      },
      drift: root.driftView(),
      work: root.workView(),
      decisions: root.decisionsView(),
      system: systemTab.sections.map(function(s) { return s.title }),
      memory: {
        rows: memoryTab.rows.map(function(r) { return r.kind + " " + r.title }),
        sections: memoryTab.rows.filter(function(r) { return r.section !== "" }).map(function(r) { return r.section }),
        cursor: memoryTab.current ? memoryTab.current.title : ""
      }
    }
  }

  function decisionsView() {
    var d = decisionsTab
    var r = root.service ? root.service.decideResult : null
    return {
      rows: d.rows.map(function(x) { return x.id + " " + x.status }),
      cursor: d.current ? d.current.id : "",
      result: r ? r.text : "",
      resultOk: r ? r.ok : null,
      pending: !!r && r.pending,
      sheet: {
        open: d.sheetOpen,
        editing: d.sheet.editing,
        title: d.sheet.title,
        armed: d.sheet.armed,
        hint: d.sheet.hint,
        result: d.sheet.resultText
      }
    }
  }

  function driftView() {
    var s = changelogTab.sheet
    var it = s.shown
    return {
      open: changelogTab.sheetOpen,
      editing: s.editing,
      eventId: s.eventId,
      isOpen: s.isOpen,
      subject: it ? it.subject : "",
      badge: it ? it.badge : "",
      zone: it ? it.zone : "",
      members: s.memberLines,
      action: s.action,
      caseId: s.caseId,
      cases: s.options.map(function(o) { return o.value }),
      only: s.only,
      intent: s.intent,
      reason: s.reason,
      explainZone: s.zone,
      risk: s.risk,
      area: s.area,
      armed: s.armed,
      hint: s.hint,
      result: s.resultText,
      resultOk: s.resultOk,
      already: !!s.result && s.result.already === true,
      pending: s.pending,
      resolution: s.resolution,
      openCase: s.caseToOpen
    }
  }

  function workView() {
    var c = workTab.current
    var actions = Model.caseActions(c)
    return {
      columns: workTab.columns.map(function(col) { return col.id + " " + col.cases.length }),
      ids: workTab.columns.map(function(col) { return col.cases.map(function(x) { return x.id }).join(",") }),
      wip: workTab.wip.text,
      cursor: c ? c.id : "",
      card: c ? {
        id: c.id,
        status: c.status,
        actions: actions.map(function(a) { return a.label }),
        proposed: c.proposed,
        armed: workTab.armedFor,
        hint: workTab.card.hint
      } : null,
      result: workTab.result ? workTab.result.text : "",
      resultOk: workTab.result ? workTab.result.ok : null,
      pending: workTab.pending,
      sheet: {
        open: workTab.sheetOpen,
        editing: workTab.sheet.editing,
        title: workTab.sheet.title,
        zone: workTab.sheet.zone,
        risk: workTab.sheet.risk,
        priority: workTab.sheet.priority,
        area: workTab.sheet.area,
        result: workTab.sheet.resultText
      }
    }
  }

  onOpenedChanged: if (opened) {
    root.cursorActive = false
    Qt.callLater(function() { keyCatcher.forceActiveFocus() })
  }

  KeyboardPanel {
    id: panel
    anchorItem: root.anchorItem
    owner: root.barIdentity
    bar: root.bar
    open: root.opened
    focusTarget: keyCatcher
    // Wider than the shell's 380 list panels (WP-039): six tabs at their
    // label widths, the Changelog's filter chips in two rows.
    contentWidth: panel.fittedContentWidth(Style.space(460))
    contentHeight: panel.fittedContentHeight(column.implicitHeight)

    PanelKeyCatcher {
      id: keyCatcher
      objectName: "seldonKeys"
      anchors.fill: parent
      // The QuickEntry field and case picker, and the new-case sheet, take
      // every key while focused.
      blocked: root.editing
      onCloseRequested: root.close()
      onTabRequested: function(direction) { root.switchPanel(direction) }
      onMoveRequested: function(dx, dy) {
        if (dx !== 0) root.selectTab(root.tabIndex + dx)
        else if (dy !== 0) root.moveCursor(dy)
      }
      onActivateRequested: if (root.cursorActive) root.currentTab.activate()
      onDeleteRequested: if (root.cursorActive && root.currentTab === workTab) workTab.dropKey()
      onTextKey: function(t) { root.textKey(t) }

      Column {
        id: column
        width: parent.width
        spacing: Style.spacing.lg

        // The header lockup (A5): the mark, then "Seldon" in the heading
        // font, baseline-aligned with the mark's centre on the cap-height
        // centre (Model.panelMark: the delivered 24 / 6 / 18 px at the
        // default font, the same proportions at every other size).
        Item {
          id: header

          readonly property real dpr: Screen.devicePixelRatio > 0 ? Screen.devicePixelRatio : 1
          readonly property var mark: Model.panelMark(capMetrics.tightBoundingRect.height, header.dpr)

          width: parent.width
          implicitHeight: Math.max(header.mark.box, title.y + title.implicitHeight, machine.implicitHeight)

          TextMetrics {
            id: capMetrics
            font: title.font
            text: "H"
          }

          MaskIcon {
            id: headerMark
            x: 0
            y: 0
            width: header.mark.box
            height: header.mark.box
            file: header.mark.file
            crisp: header.mark.crisp
            color: root.foreground
          }

          Text {
            id: title
            x: header.mark.box + header.mark.gap
            y: Math.round((header.mark.baseline - title.baselineOffset) * header.dpr) / header.dpr
            textFormat: Text.PlainText
            text: "Seldon"
            color: root.foreground
            font.family: root.fontFamily
            font.pixelSize: Style.font.heading
            font.bold: true
          }

          Text {
            id: machine
            anchors.right: parent.right
            anchors.left: title.right
            anchors.leftMargin: Style.spacing.lg
            anchors.baseline: title.baseline
            horizontalAlignment: Text.AlignRight
            textFormat: Text.PlainText
            text: !root.service ? ""
              : root.service.busy ? "working…"
              : root.service.lastCapture !== ""
                ? "captured " + Model.relativeAge(Model.timeMs(root.service.lastCapture), root.service.nowMs)
                : ""
            color: root.dim
            elide: Text.ElideLeft
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
          }
        }

        Tabs {
          id: tabStrip
          width: parent.width
          tabs: root.tabNames
          keys: root.tabIds.map(Model.tabKeyFor)
          currentIndex: root.tabIndex
          foreground: root.foreground
          fontFamily: root.fontFamily
          fontSize: Style.font.caption
          onActivated: function(i) { root.selectTab(i) }
        }

        Banner {
          id: statusBanner
          width: parent.width
          banner: root.service ? root.service.banner : null
          foreground: root.foreground
          urgent: root.urgent
          fontFamily: root.fontFamily
          onActionRequested: function(actionId) { if (root.service) root.service.fix(actionId, "status") }
        }

        Banner {
          id: snapperBanner
          width: parent.width
          banner: root.service ? root.service.snapperBanner : null
          foreground: root.foreground
          urgent: root.urgent
          fontFamily: root.fontFamily
          onActionRequested: function(actionId) { if (root.service) root.service.fix(actionId, "snapper") }
        }

        // The red strip (SPEC-PLUGIN §5); a click opens the drift sheet for
        // the first crisis on the Changelog.
        BorderSurface {
          id: crisisStrip
          width: parent.width
          visible: crisisLabel.text !== ""
          implicitHeight: visible ? crisisLabel.implicitHeight + Style.spacing.md * 2 : 0
          radius: Style.cornerRadius
          color: Style.selectedFillFor(root.urgent, root.urgent)
          borderSpec: Border.controlSpec("normal", root.urgent, root.urgent)

          Text {
            id: crisisLabel
            x: Style.spacing.xl
            anchors.verticalCenter: parent.verticalCenter
            width: parent.width - Style.spacing.xl * 2
            textFormat: Text.PlainText
            text: root.service ? root.service.crisisText : ""
            color: root.urgent
            wrapMode: Text.Wrap
            font.family: root.fontFamily
            font.pixelSize: Style.font.bodySmall
            font.bold: true
          }

          MouseArea {
            anchors.fill: parent
            cursorShape: Qt.PointingHandCursor
            onClicked: root.resolve("crisis")
          }
        }

        Text {
          width: parent.width
          visible: !root.service
          textFormat: Text.PlainText
          text: "The Seldon service is not running. Enable the plugin in Setup > Plugins."
          color: root.foreground
          font.family: root.fontFamily
          font.pixelSize: Style.font.body
          wrapMode: Text.Wrap
        }

        Item {
          id: body
          width: parent.width
          height: Style.space(430)

          TodayTab {
            id: todayTab
            anchors.fill: parent
            visible: root.tabId === "today"
            service: root.service
            indexData: root.indexData
            cursorActive: root.cursorActive && visible
            foreground: root.foreground
            urgent: root.urgent
            fontFamily: root.fontFamily
            onOpenJournalRequested: root.openInEditor("journal")
            onCursorWanted: root.cursorActive = true
            onLeaveRequested: keyCatcher.forceActiveFocus()
            onEditingChanged: if (!editing) Qt.callLater(root.restoreKeys)
          }

          ChangelogTab {
            id: changelogTab
            anchors.fill: parent
            visible: root.tabId === "changelog"
            service: root.service
            indexData: root.indexData
            cursorActive: root.cursorActive && visible
            foreground: root.foreground
            urgent: root.urgent
            fontFamily: root.fontFamily
            capturing: !!root.service && root.service.capturing
            captureResult: root.service ? root.service.captureResult : null
            onCaptureRequested: root.captureNow()
            onOpenLedgerRequested: root.openInEditor("ledger")
            onCursorWanted: root.cursorActive = true
            onLeaveRequested: keyCatcher.forceActiveFocus()
            onEditingChanged: if (!editing) Qt.callLater(root.restoreKeys)
          }

          WorkTab {
            id: workTab
            anchors.fill: parent
            visible: root.tabId === "work"
            service: root.service
            indexData: root.indexData
            wipLimit: root.wipLimit
            cursorActive: root.cursorActive && visible
            foreground: root.foreground
            urgent: root.urgent
            fontFamily: root.fontFamily
            onCursorWanted: root.cursorActive = true
            onLeaveRequested: keyCatcher.forceActiveFocus()
            onEditingChanged: if (!editing) Qt.callLater(root.restoreKeys)
          }

          DecisionsTab {
            id: decisionsTab
            anchors.fill: parent
            visible: root.tabId === "decisions"
            service: root.service
            indexData: root.indexData
            cursorActive: root.cursorActive && visible
            foreground: root.foreground
            urgent: root.urgent
            fontFamily: root.fontFamily
            onCursorWanted: root.cursorActive = true
            onLeaveRequested: keyCatcher.forceActiveFocus()
            onEditingChanged: if (!editing) Qt.callLater(root.restoreKeys)
          }

          SystemTab {
            id: systemTab
            anchors.fill: parent
            visible: root.tabId === "system"
            indexData: root.indexData
            nowMs: root.service ? root.service.nowMs : Date.now()
            cursorActive: root.cursorActive && visible
            foreground: root.foreground
            fontFamily: root.fontFamily
            onCursorWanted: root.cursorActive = true
            onOpenStatusRequested: root.openInEditor("status")
          }

          MemoryTab {
            id: memoryTab
            anchors.fill: parent
            visible: root.tabId === "memory"
            service: root.service
            indexData: root.indexData
            cursorActive: root.cursorActive && visible
            foreground: root.foreground
            fontFamily: root.fontFamily
            onCursorWanted: root.cursorActive = true
          }
        }

        Text {
          width: parent.width
          visible: text !== ""
          textFormat: Text.PlainText
          text: root.service ? root.service.lastError : ""
          color: root.urgent
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
          wrapMode: Text.Wrap
        }

        Text {
          width: parent.width
          visible: !!root.service && root.service.devMode
          textFormat: Text.PlainText
          text: root.service ? "Dev mode, read-only: " + root.service.indexPath : ""
          color: root.dim
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
          wrapMode: Text.WrapAnywhere
        }
      }
    }
  }
}
