pragma ComponentBehavior: Bound

import QtQuick
import qs.Commons
import qs.Ui
import "components"
import "components/desk"
import "sections"
import "Model.js" as Model

// The desk (ADR-0034, SPEC-PLUGIN §5): Seldon's one surface. The manifest's
// `overlay` entry point, so `omarchy-shell shell summon|hide|toggle
// jax.seldon [payload]`, the pill (left click toggles, middle click opens
// the Prime Radiant) and the `jax.seldon.panel` shim all land here.
//
// The shell's loader creates this item bare and then injects shell (the
// plugin's scoped facade), manifest and service; every binding tolerates
// service === null. `hide` calls close() and the loader drops the item, so
// what the desk remembers between opens (section, selections) lives in the
// service (Service.deskMemory).
//
// Window (§1): components/desk/DeskWindow.qml, a layer-shell surface over
// the focused monitor; the desk is a card centred in it, its width from the
// setting `deskWidth` (Model.deskGeometry), the rest of the surface a
// transparent click-catcher that closes. Inside: the header (mark, machine,
// status chip, KPI strip, Settings, Esc), the notices (today's banners and
// their fixes), the sidebar (nine targets, search, fold) and the current
// section (sections/*.qml on components/desk/Section.qml), laid out by
// Model.deskLayout: the sidebar icons-only under 960 px or when
// `deskSidebar` is collapsed, list and detail stacked under 760 px.
//
// Keys (§2): 1–8 sections, `,` Settings, Alt+↑/↓ the previous / next of
// the nine (wrapping), `/` the search, ↑/↓ j/k, ←/→ and Enter/Space to the
// section, Esc in this order: the section's own state (an inline form),
// the search filter, the stacked detail, then close. Every other character
// goes to the section first (Section.textKey); unused, `c` captures, `n`
// goes to Today and `+` to Work with the key. A focused field keeps every
// key (Esc in it is the field's). Tab does nothing: the desk is not a bar
// popup. Writing actions arm on the first press (Arm.qml): any other key
// disarms.
//
// Settings writes (§1): Settings › Appearance and the sidebar's fold button
// call writeSetting(), which sends the whole shell.json entry with the one
// changed key through the facade's updateEntryInline — only on an explicit
// action (a slider release, a preset, a click), never while dragging, and
// never with a value already stored. A refused write leaves the stored
// value and shows where to set it.
//
// IPC while loaded (`omarchy-shell shell call jax.seldon <method> <arg>`):
// view "" (JSON, see view()), section <id>, select <id>, and for the Prime
// Radiant setPeriod <id> and hover "<slot> <x>,<y>".
Item {
  id: root

  // Injected by omarchy-shell's panel loader.
  property var shell: null
  property var manifest: null
  property var service: null

  property bool opened: false
  property string sectionId: Model.DESK_SECTION_DEFAULT
  // The sections created so far: { <id>: true }. A section is made on its
  // first visit and kept while the desk is loaded.
  property var visited: ({})
  // Stacked layout: the detail is shown instead of the list.
  property bool detailShown: false
  property string searchText: ""
  property bool noticesFolded: false

  // ---- Settings: the live preview while the slider is dragged or
  // scrolled (-1 none), and what was written until the shell's reload
  // brings it back.
  property int previewWidth: -1
  property var pendingEntry: null
  property bool settingsRefused: false
  // A change made while the plugin is not in the bar (no entry to write).
  property bool settingsNoEntry: false
  // Every facade write: { key, value, settings, ok }.
  property var writes: []

  readonly property Arm arm: Arm {}

  readonly property string pluginId: root.manifest && root.manifest.id ? String(root.manifest.id) : "jax.seldon"
  // The index only when its contents mean something in this status.
  readonly property var indexData: root.service && root.service.indexShown ? root.service.index : null
  readonly property bool entryKnown: !!root.service && root.service.entryKnown
  readonly property var entry: root.pendingEntry !== null ? root.pendingEntry
    : !root.service ? ({})
    : !root.service.entryKnown && root.service.localEntry ? root.service.localEntry
    : root.service.entrySettings
  readonly property int storedWidth: Model.clampDeskWidth(root.entry.deskWidth)
  readonly property int widthPct: root.previewWidth >= 0 ? root.previewWidth : root.storedWidth
  readonly property string sidebarPref: Model.deskSidebarMode(root.entry.deskSidebar)
  readonly property real windowWidth: window.width
  readonly property var geometry: Model.deskGeometry(window.width, window.height, root.widthPct, Style.gapsOut)
  readonly property var info: Model.deskSection(root.sectionId)
  // The thresholds are the desk's own width (§1: "desk width < 960 px").
  readonly property var layout: Model.deskLayout(root.geometry.w, root.sidebarPref, root.info.solo, {
      sidebar: Style.space(210), icons: Style.space(56), listMin: Style.space(260), listMax: Style.space(360)
    })
  property Section currentSection: null
  readonly property bool editing: sidebar.searchFocused || (!!root.currentSection && root.currentSection.editing)

  readonly property color foreground: Color.popups.text
  readonly property string fontFamily: Style.font.family

  // ---- Lifecycle

  function open(payloadJson) {
    var p = Model.deskPayload(payloadJson)
    // Every open, also of an open desk (a summon, the pill of another
    // monitor): to the monitor Hyprland has focused.
    window.retarget()
    if (p.section !== "") root.section(p.section)
    root.visit(root.sectionId)
    if (root.currentSection) root.currentSection.applyPayload(p)
    if (p.select !== "") root.select(p.select)
    if (!root.opened) {
      root.opened = true
      if (root.service) {
        root.service.clearRulesResult()
        root.service.checkRules(false)
      }
    }
    root.giveKeys()
  }

  function close() {
    root.arm.disarm()
    root.flushWheel()
    root.previewWidth = -1
    root.remember()
    root.opened = false
  }

  // Esc, the Esc button and a click beside the desk: close through the
  // shell, which then calls close() and unloads.
  function dismiss() {
    root.close()
    if (root.shell && typeof root.shell.hide === "function") root.shell.hide(root.pluginId)
  }

  function giveKeys() {
    Qt.callLater(function() { if (root.opened && !root.editing) keyCatcher.forceActiveFocus() })
  }

  // A section's field hands the keys back (Esc, Cancel, a sent form): the
  // desk takes them from it, whatever has the focus now.
  function takeKeys() {
    if (root.opened) keyCatcher.forceActiveFocus()
  }

  // ---- Sections

  function componentFor(id) {
    return id === "today" ? todayComponent
      : id === "changelog" ? changelogComponent
      : id === "work" ? workComponent
      : id === "decisions" ? decisionsComponent
      : id === "system" ? systemComponent
      : id === "memory" ? memoryComponent
      : id === "radiant" ? radiantComponent
      : id === "graph" ? graphComponent
      : settingsComponent
  }

  function sectionItem(id) {
    var loader = sectionRepeater.itemAt(Model.deskSectionIndex(id)) as Loader
    return loader ? loader.item as Section : null
  }

  function visit(id) {
    if (root.visited[id] === true) return
    var next = ({})
    for (var k in root.visited) next[k] = root.visited[k]
    next[id] = true
    root.visited = next
  }

  // Show a section ("ok"), or "unknown section" for anything else.
  function section(id) {
    var target = String(id)
    if (!Model.deskSection(target)) return "unknown section"
    if (target !== root.sectionId) {
      root.remember()
      root.sectionId = target
      root.detailShown = false
      root.searchText = ""
      root.arm.disarm()
    }
    root.visit(target)
    root.currentSection = root.sectionItem(target)
    root.remember()
    // The keys come back to the desk, from the search field too (a click
    // on a sidebar row leaves the field focused otherwise).
    if (root.opened && !(root.currentSection && root.currentSection.editing)) keyCatcher.forceActiveFocus()
    return "ok"
  }

  // Select an item of the current section ("ok"), else "not found".
  function select(id) {
    var s = root.currentSection
    if (!s || !s.select(String(id))) return "not found"
    return "ok"
  }

  function showDetail() {
    if (root.layout.stacked) root.detailShown = true
  }

  function back() {
    root.detailShown = false
    root.giveKeys()
  }

  // What the desk keeps in the service between opens.
  function remember() {
    if (!root.service) return
    var selected = ({})
    var old = root.service.deskMemory && root.service.deskMemory.selected ? root.service.deskMemory.selected : {}
    for (var k in old) selected[k] = old[k]
    for (var i = 0; i < Model.DESK_SECTIONS.length; i++) {
      var id = Model.DESK_SECTIONS[i].id
      var s = root.sectionItem(id)
      if (s) selected[id] = s.selectedId
    }
    root.service.deskMemory = { section: root.sectionId, selected: selected }
  }

  function restore() {
    var memory = root.service ? root.service.deskMemory : null
    if (memory && Model.deskSection(memory.section) && !root.opened) root.sectionId = memory.section
  }

  function sectionLoaded(id, item) {
    item.desk = root
    item.sectionId = id
    var memory = root.service ? root.service.deskMemory : null
    var kept = memory && memory.selected ? memory.selected[id] : undefined
    if (typeof kept === "string" && kept !== "") item.selectedId = kept
    if (id === root.sectionId) root.currentSection = item
  }

  // ---- Keys

  function escapeKey() {
    if (root.currentSection && root.currentSection.back()) return
    if (root.searchText !== "") {
      root.searchText = ""
      return
    }
    if (root.layout.stacked && root.detailShown) {
      root.back()
      return
    }
    root.dismiss()
  }

  function textKey(t) {
    if (root.currentSection && root.currentSection.textKey(t)) return true
    if (t === "c" || t === "C") {
      if (root.service) root.service.captureNow()
      return true
    }
    if (t === "n" || t === "+") {
      root.section(t === "n" ? "today" : "work")
      if (root.currentSection) root.currentSection.textKey(t)
      return true
    }
    return false
  }

  function key(event) {
    var k = event.key
    var t = event.text
    var alt = (event.modifiers & Qt.AltModifier) !== 0
    if (alt && (k === Qt.Key_Up || k === Qt.Key_Down)) {
      root.section(Model.deskCycle(root.sectionId, k === Qt.Key_Down ? 1 : -1))
      return true
    }
    if (k === Qt.Key_Escape) {
      root.escapeKey()
      return true
    }
    if (k === Qt.Key_Tab || k === Qt.Key_Backtab) return true
    if (k === Qt.Key_Up || t === "k") {
      if (root.currentSection) root.currentSection.move(-1)
      return true
    }
    if (k === Qt.Key_Down || t === "j") {
      if (root.currentSection) root.currentSection.move(1)
      return true
    }
    if (k === Qt.Key_Left || k === Qt.Key_Right) {
      if (root.currentSection) root.currentSection.moveAcross(k === Qt.Key_Left ? -1 : 1)
      return true
    }
    if (k === Qt.Key_Return || k === Qt.Key_Enter || k === Qt.Key_Space) {
      if (root.currentSection) root.currentSection.activate()
      return true
    }
    if (t === "/") {
      root.focusSearch()
      return true
    }
    var target = Model.deskSectionForKey(t)
    if (target !== "") {
      root.section(target)
      return true
    }
    if (t.length === 1) return root.textKey(t)
    return false
  }

  function focusSearch() {
    if (root.layout.sidebar === "icons") return
    sidebar.focusSearch()
  }

  // ---- Settings

  // The mouse wheel or a touchpad over the width slider: a preview only.
  // One write follows 600 ms after the last notch (wheelTimer), or the next
  // release or preset click takes its place, or closing the desk flushes
  // it — never a write per notch (every write is a config event).
  function previewWheel(value) {
    root.previewWidth = value
    wheelTimer.restart()
  }

  function flushWheel() {
    if (!wheelTimer.running) return
    wheelTimer.stop()
    var value = root.previewWidth
    root.previewWidth = -1
    if (value >= 0) root.writeSetting("deskWidth", value)
  }

  // Write one setting through the facade: "written", "unchanged" (the
  // value is already stored: no call, no config event), "refused", or
  // "session" (the plugin is not in the bar: kept in the service, not
  // written).
  function writeSetting(key, value) {
    if (key === "deskWidth" && wheelTimer.running) {
      wheelTimer.stop()
      root.previewWidth = -1
    }
    var stored = key === "deskWidth" ? root.storedWidth : key === "deskSidebar" ? root.sidebarPref : root.entry[key]
    if (stored === value) return "unchanged"
    var next = Model.deskSettingsWrite(root.entry, key, value)
    if (next === null) return "unchanged"
    if (!root.entryKnown) {
      if (root.service) root.service.localEntry = next
      root.settingsNoEntry = true
      root.settingsRefused = false
      return "session"
    }
    var ok = false
    if (root.shell && typeof root.shell.updateEntryInline === "function") {
      try {
        ok = root.shell.updateEntryInline(root.pluginId, next) === true
      } catch (e) {
        ok = false
      }
    }
    root.writes = root.writes.concat([{ key: key, value: value, settings: next, ok: ok }])
    root.settingsRefused = !ok
    if (!ok) return "refused"
    root.pendingEntry = next
    pendingTimer.restart()
    return "written"
  }

  // ---- The Prime Radiant's IPC (the 0.1 overlay's names, SPEC-PLUGIN §8)

  // `setPeriod <id>`: shows section 7 with that period; returns the period
  // now selected. An unknown id changes nothing — not the section either —
  // and returns section 7's period ("" before its first visit).
  function setPeriod(id) {
    var value = String(id)
    if (Model.isPeriod(value)) root.section("radiant")
    var radiant = root.sectionItem("radiant") as Radiant
    return radiant ? radiant.setPeriod(value) : ""
  }

  // `hover "<slot> <x>,<y>"` (fractions of the chart's plot) or "" (clear),
  // while section 7 is shown: JSON { slot, hover } or { error }.
  function hover(arg) {
    var radiant = root.sectionItem("radiant") as Radiant
    if (!radiant || !radiant.active) return JSON.stringify({ error: "the Prime Radiant is not shown" })
    return radiant.hover(arg)
  }

  // ---- Read-out

  // What the desk shows, as JSON, for the harness and the test host.
  function view(arg) {
    var s = root.currentSection
    var last = root.writes.length > 0 ? root.writes[root.writes.length - 1] : null
    return JSON.stringify({
      opened: root.opened,
      section: root.sectionId,
      selected: s ? s.selectedId : "",
      visited: Object.keys(root.visited).sort(),
      screen: window.screenName,
      window: { w: Math.round(window.width), h: Math.round(window.height) },
      desk: { x: Math.round(card.x), y: Math.round(card.y), w: Math.round(card.width), h: Math.round(card.height) },
      widthPct: root.widthPct,
      layout: root.layout,
      detailShown: root.detailShown,
      keys: keyCatcher.activeFocus,
      editing: root.editing,
      search: { focused: sidebar.searchFocused, text: root.searchText },
      status: root.service ? root.service.status : "",
      subline: header.subline,
      mark: { file: header.markFile, ready: header.markReady },
      kpis: Model.deskKpis(root.indexData).map(function(k) { return k.id + " " + k.value }),
      counts: Model.deskCounts(root.indexData),
      notices: notices.items.map(function(n) { return n.banner.title }),
      noticesFolded: root.noticesFolded,
      chip: header.chipText,
      chipShown: header.chipShown,
      settings: {
        stored: root.storedWidth,
        sidebar: root.sidebarPref,
        preview: root.previewWidth,
        pending: root.pendingEntry !== null,
        refused: root.settingsRefused,
        noEntry: root.settingsNoEntry,
        wheelPending: wheelTimer.running,
        writes: root.writes.length,
        last: last
      },
      arm: { armed: root.arm.armedId, hint: root.arm.hint },
      lastError: root.service ? root.service.lastError : "",
      sectionView: s ? s.view() : null
    })
  }

  onServiceChanged: if (root.service) {
    root.service.desk = root
    root.restore()
  }
  Component.onDestruction: {
    if (root.service && root.service.desk === root) {
      root.remember()
      root.service.desk = null
    }
  }

  // The shell's reload brought the written entry back (or another one).
  Connections {
    target: root.service
    function onEntrySettingsChanged() {
      root.pendingEntry = null
    }
    // The rules check once the engine is there, when the desk opened first.
    function onEngineStateChanged() {
      if (root.opened) root.service.checkRules(false)
    }
  }

  // A shell that took the write but never sent it back: the stored value
  // returns after this.
  Timer {
    id: pendingTimer
    interval: 5000
    onTriggered: root.pendingEntry = null
  }

  Timer {
    id: wheelTimer
    interval: 600
    onTriggered: {
      var value = root.previewWidth
      root.previewWidth = -1
      if (value >= 0) root.writeSetting("deskWidth", value)
    }
  }

  Component { id: todayComponent; Today {} }
  Component { id: changelogComponent; Changelog {} }
  Component { id: workComponent; Work {} }
  Component { id: decisionsComponent; Decisions {} }
  Component { id: systemComponent; System {} }
  Component { id: memoryComponent; Memory {} }
  Component { id: radiantComponent; Radiant {} }
  Component { id: graphComponent; Graph {} }
  Component { id: settingsComponent; Settings {} }

  DeskWindow {
    id: window
    visible: root.opened

    Item {
      id: frame
      anchors.fill: parent

      // Beside the desk: a click closes it. Transparent, no dimming: the
      // desk reads as an application, not a modal.
      MouseArea {
        anchors.fill: parent
        onClicked: root.dismiss()
      }

      BorderSurface {
        id: card
        objectName: "desk"
        x: root.geometry.x
        y: root.geometry.y
        width: root.geometry.w
        height: root.geometry.h
        radius: Style.cornerRadius
        color: Color.popups.background
        borderSpec: Border.surfaceSpec("popups", "border", Color.popups.border, Math.max(1, Style.space(2)))

        // Clicks on the desk stay on the desk.
        MouseArea {
          anchors.fill: parent
        }

        Item {
          id: keyCatcher
          objectName: "deskKeys"
          x: card.contentLeftInset
          y: card.contentTopInset
          width: card.width - card.contentLeftInset - card.contentRightInset
          height: card.height - card.contentTopInset - card.contentBottomInset
          focus: true
          Keys.priority: Keys.BeforeItem
          Keys.onPressed: function(event) {
            if (root.editing) return
            root.arm.touched = false
            var used = root.key(event)
            if (!root.arm.touched) root.arm.disarm()
            if (used) event.accepted = true
          }

          Header {
            id: header
            width: parent.width
            height: implicitHeight
            subline: Model.deskSubline(root.indexData, root.service ? root.service.lastCapture : "",
              root.service ? root.service.nowMs : Date.now())
            kpis: Model.deskKpis(root.indexData)
            chipText: notices.items.length === 0 ? ""
              : notices.items[0].banner.title + (notices.items.length > 1 ? " +" + (notices.items.length - 1) : "")
            chipCount: notices.items.length
            chipTone: notices.items.length === 0 ? "" : String(notices.items[0].banner.tone || "")
            noticesFolded: root.noticesFolded
            foreground: root.foreground
            fontFamily: root.fontFamily
            onSettingsRequested: root.section("settings")
            onCloseRequested: root.dismiss()
            onChipClicked: root.noticesFolded = !root.noticesFolded
            onKpiPicked: function(id) {
              root.section(id === "crises" || id === "attention" ? "changelog" : "work")
              if (root.currentSection)
                root.currentSection.applyPayload({ filter: id === "crises" ? "crisis" : id === "attention" ? "open" : "", select: "", period: "", section: root.sectionId })
            }
          }

          Notices {
            id: notices
            anchors.top: header.bottom
            width: parent.width
            service: root.service
            folded: root.noticesFolded
            foreground: root.foreground
            fontFamily: root.fontFamily
          }

          Item {
            id: body
            anchors.top: notices.bottom
            anchors.bottom: footer.top
            width: parent.width
            clip: true

            Sidebar {
              id: sidebar
              width: root.layout.sidebarW
              height: parent.height
              icons: root.layout.sidebar === "icons"
              current: root.sectionId
              counts: Model.deskCounts(root.indexData)
              searchText: root.searchText
              foreground: root.foreground
              fontFamily: root.fontFamily
              onPicked: function(id) { root.section(id) }
              onFoldRequested: root.writeSetting("deskSidebar", root.sidebarPref === "open" ? "collapsed" : "open")
              onSearchEdited: function(text) { root.searchText = text }
              onSearchLeft: keyCatcher.forceActiveFocus()
            }

            Item {
              id: sectionArea
              x: sidebar.width
              width: parent.width - sidebar.width
              height: parent.height

              Text {
                visible: !root.service
                x: Style.spacing.huge
                y: Style.spacing.huge
                width: parent.width - Style.spacing.huge * 2
                textFormat: Text.PlainText
                text: "The Seldon service is not running. Enable the plugin in Setup > Plugins."
                color: root.foreground
                wrapMode: Text.Wrap
                font.family: root.fontFamily
                font.pixelSize: Style.font.body
              }

              Repeater {
                id: sectionRepeater
                model: Model.DESK_SECTIONS

                Loader {
                  id: sectionLoader

                  required property var modelData

                  anchors.fill: parent
                  active: root.visited[sectionLoader.modelData.id] === true
                  visible: !!root.service && root.sectionId === sectionLoader.modelData.id
                  sourceComponent: root.componentFor(sectionLoader.modelData.id)
                  onLoaded: root.sectionLoaded(sectionLoader.modelData.id, sectionLoader.item as Section)
                }
              }
            }
          }

          Item {
            id: footer
            anchors.bottom: parent.bottom
            width: parent.width
            height: Math.max(footLeft.implicitHeight, footRight.implicitHeight) + Style.spacing.md * 2

            Rectangle {
              width: parent.width
              height: Style.spacing.hairline
              color: Util.alpha(root.foreground, 0.12)
            }

            Text {
              id: footLeft
              x: Style.spacing.huge
              anchors.verticalCenter: parent.verticalCenter
              width: Math.max(0, parent.width - footRight.implicitWidth - Style.spacing.huge * 3)
              textFormat: Text.PlainText
              text: !root.service ? ""
                : root.service.lastError !== "" ? root.service.lastError
                : root.service.devMode ? "Dev mode, read-only: " + root.service.indexPath
                : ""
              color: root.service && root.service.lastError !== "" ? Color.urgent : Color.muted
              elide: Text.ElideMiddle
              font.family: root.fontFamily
              font.pixelSize: Style.font.caption
            }

            Text {
              id: footRight
              anchors.right: parent.right
              anchors.rightMargin: Style.spacing.huge
              anchors.verticalCenter: parent.verticalCenter
              textFormat: Text.PlainText
              text: "1–8 sections · , settings · Alt+↑/↓ next · / search · Esc close"
              color: Color.muted
              font.family: root.fontFamily
              font.pixelSize: Style.font.caption
            }
          }
        }
      }
    }
  }
}
