import QtQuick
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
// Tabs: Today, Changelog, System (Work, Decisions and Memory arrive later).
//
// Actions (WP-012) go through the service's queue with fixed argument lists:
// the QuickEntry note (`seldon log`), *Capture now* (capture, then status;
// the index refresh arrives through the FileView), and *Open in editor* per
// tab (`seldon open journal|ledger|status|<caseId> --editor`).
//
// Keyboard (SPEC-PLUGIN §5):
//   Tab / Shift-Tab  the bar's next / previous panel, as every Omarchy panel
//   ← / →, h / l     previous / next tab
//   1–6              a tab by its fixed number (Model.TAB_KEYS: Today 1,
//                    Changelog 2, Work 3, Decisions 4, System 5, Memory 6);
//                    the digit of a tab this version lacks does nothing
//   ↑ / ↓, k / j     move in the tab's list
//   Enter, Space     open the row under the cursor (a group, the yesterday row)
//   f / F            Changelog: next / previous source filter
//   c                capture now
//   n                write a note (Today's QuickEntry; Esc gives the keys back)
//   e                open this tab's file in the editor (journal, ledger, status)
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

  readonly property var tabNames: ["Today", "Changelog", "System"]
  readonly property var tabIds: ["today", "changelog", "system"]
  property int tabIndex: 0
  property bool cursorActive: false

  // The index only when its contents mean something in this status.
  readonly property var indexData: service && service.indexShown ? service.index : null
  readonly property var tabItems: [todayTab, changelogTab, systemTab]
  readonly property var currentTab: tabItems[tabIndex]

  function selectTab(i) {
    var n = root.tabNames.length
    root.tabIndex = ((i % n) + n) % n
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
    } else {
      root.currentTab.textKey(t)
    }
  }

  function setFilter(source) {
    changelogTab.setFilter(source)
  }

  function captureNow() {
    if (root.service) root.service.captureNow()
  }

  // journal | ledger | status | <caseId>; Service.openInEditor validates it.
  function openInEditor(what) {
    if (root.service) root.service.openInEditor(what)
  }

  // The QuickEntry gives the keys back, or its field was hidden with the tab.
  function restoreKeys() {
    if (root.opened && !todayTab.editing && !keyCatcher.activeFocus) keyCatcher.forceActiveFocus()
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
      status: root.service ? root.service.status : "",
      banner: statusBanner.visible && root.service.banner ? root.service.banner.title : "",
      snapper: snapperBanner.visible && root.service.snapperBanner ? root.service.snapperBanner.title : "",
      crisis: crisisStrip.visible ? crisisLabel.text : "",
      today: {
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
        expanded: changelogTab.expandedId
      },
      system: systemTab.sections.map(function(s) { return s.title })
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
    contentWidth: panel.fittedContentWidth(Style.space(380))
    contentHeight: panel.fittedContentHeight(column.implicitHeight)

    PanelKeyCatcher {
      id: keyCatcher
      objectName: "seldonKeys"
      anchors.fill: parent
      // The QuickEntry field and case picker take every key while focused.
      blocked: todayTab.editing
      onCloseRequested: root.close()
      onTabRequested: function(direction) { root.switchPanel(direction) }
      onMoveRequested: function(dx, dy) {
        if (dx !== 0) root.selectTab(root.tabIndex + dx)
        else if (dy !== 0) root.moveCursor(dy)
      }
      onActivateRequested: if (root.cursorActive) root.currentTab.activate()
      onTextKey: function(t) { root.textKey(t) }

      Column {
        id: column
        width: parent.width
        spacing: Style.spacing.lg

        Item {
          width: parent.width
          implicitHeight: Math.max(title.implicitHeight, machine.implicitHeight)

          Text {
            id: title
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            textFormat: Text.PlainText
            text: Model.GLYPH + " Seldon"
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
            anchors.verticalCenter: parent.verticalCenter
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
          width: parent.width
          tabs: root.tabNames
          keys: root.tabIds.map(Model.tabKeyFor)
          currentIndex: root.tabIndex
          foreground: root.foreground
          fontFamily: root.fontFamily
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

        // The red strip (SPEC-PLUGIN §5); a click shows the changelog.
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
            onClicked: {
              changelogTab.setFilter("all")
              root.selectTab(1)
            }
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
            visible: root.tabIndex === 0
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
            visible: root.tabIndex === 1
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
          }

          SystemTab {
            id: systemTab
            anchors.fill: parent
            visible: root.tabIndex === 2
            indexData: root.indexData
            nowMs: root.service ? root.service.nowMs : Date.now()
            cursorActive: root.cursorActive && visible
            foreground: root.foreground
            fontFamily: root.fontFamily
            onCursorWanted: root.cursorActive = true
            onOpenStatusRequested: root.openInEditor("status")
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
