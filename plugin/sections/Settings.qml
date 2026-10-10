pragma ComponentBehavior: Bound

import QtQuick
import qs.Commons
import qs.Ui
import "../components/desk"
import "../Model.js" as Model

// Settings, the ninth target (`,`; ADR-0034 §1, §2). Four groups:
//
// Appearance (live): the desk width — a slider in 10 % steps from 50 to
// 100, the presets 50 / 67 / 75 / Full, the width it gives on this screen
// — and the sidebar, open or collapsed. Dragging previews the width on
// the desk itself; the release (or a preset or sidebar click) writes the
// setting through the shell facade, once (Desk.writeSetting): every
// write is a config event, so nothing writes while dragging. The mouse
// wheel or a touchpad over the slider only previews; one write follows a
// pause (Desk.previewWheel). When the shell refuses, the stored value
// stays and the page says where to set it; when the plugin is not in the
// bar, the change holds for this shell and the page says how to keep it.
//
// Capture, Agents, Quiet (read-only): the values in force and where each
// is set — Omarchy's bar settings for the widget's keys, Seldon's
// config.toml for the engine's. The desk never edits config.toml.
Section {
  id: root

  readonly property var groups: [
    { id: "appearance", label: "Appearance", lead: "Width, sidebar; the theme follows Omarchy" },
    { id: "capture", label: "Capture", lead: "Interval, collectors" },
    { id: "agents", label: "Agents", lead: "Launcher, start folder, active cases" },
    { id: "quiet", label: "Quiet", lead: "What may be loud" }
  ]
  readonly property int cursor: Math.max(0, root.groupIndex(root.selectedId))
  readonly property var current: root.groups[root.cursor]
  readonly property var entry: root.service ? root.service.entrySettings : ({})
  readonly property string configPath: "~/.config/seldon/config.toml"
  readonly property string barSettings: "Omarchy's bar settings (Seldon widget)"
  readonly property real sliderIndex: root.desk ? (root.desk.widthPct - Model.DESK_WIDTH_MIN) / 10 : 5

  selectedId: "appearance"

  function groupIndex(id) {
    for (var i = 0; i < root.groups.length; i++) if (root.groups[i].id === id) return i
    return -1
  }

  function move(dy) {
    var i = Math.max(0, Math.min(root.groups.length - 1, root.cursor + dy))
    root.selectedId = root.groups[i].id
    return true
  }

  function activate() {
    if (root.desk) root.desk.showDetail()
    return true
  }

  function select(id) {
    if (root.groupIndex(String(id)) === -1) return false
    root.selectedId = String(id)
    if (root.desk) root.desk.showDetail()
    return true
  }

  function collectorsText() {
    var state = root.index && root.index.state ? root.index.state : null
    var list = state && Array.isArray(state.collectors) ? state.collectors : []
    if (list.length === 0) return "not known yet (no index)"
    return list.map(function(c) {
      return String(c.name) + (c.enabled === false ? " off" : c.ok === false ? " failing" : " ok")
    }).join(" · ")
  }

  function view() {
    return {
      group: root.selectedId,
      groups: root.groups.map(function(g) { return g.id }),
      slider: root.desk ? root.desk.widthPct : 0,
      preview: preview.text,
      sidebar: root.desk ? root.desk.sidebarPref : "",
      note: refusedNote.visible ? refusedNote.text : ""
    }
  }

  ListColumn {
    id: list
    keyEvents: root.keyEvents
    visible: !root.stacked || !root.detailShown
    width: root.stacked ? root.width : (root.layout ? root.layout.listW : 0)
    height: root.height
    divider: !root.stacked
    title: "Settings"
    model: root.groups
    currentIndex: root.cursor

    delegate: ListRow {
      required property var modelData
      required property int index
      width: ListView.view.width
      title: modelData.label
      meta: modelData.lead
      selected: root.selectedId === modelData.id
      cursor: list.hoverIndex === index
      onClicked: root.select(modelData.id)
    }
  }

  DetailPane {
    id: detail
    x: root.stacked ? 0 : list.width
    width: root.width - x
    height: root.height
    visible: !root.stacked || root.detailShown
    backVisible: root.stacked
    title: root.current.label
    onBackRequested: if (root.desk) root.desk.back()

    // ---- Appearance
    Column {
      visible: root.selectedId === "appearance"
      width: parent.width
      spacing: Style.spacing.xxl

      Text {
        width: parent.width
        textFormat: Text.PlainText
        text: "Desk width"
        color: Color.popups.text
        font.family: Style.font.family
        font.pixelSize: Style.font.title
        font.bold: true
      }

      Text {
        width: parent.width
        textFormat: Text.PlainText
        text: "From 50 % the desk shows sidebar, list and detail side by side; never narrower than 960 px unless the screen is."
        color: root.tone.dim
        wrapMode: Text.Wrap
        font.family: Style.font.family
        font.pixelSize: Style.font.bodySmall
      }

      Row {
        spacing: Style.spacing.xxl

        PanelSlider {
          id: slider
          objectName: "deskWidthSlider"
          width: Math.min(Style.space(360), detail.width / 2)
          anchors.verticalCenter: parent.verticalCenter
          minimum: 0
          maximum: 5
          step: 1
          integer: true
          tickCount: 6
          value: root.sliderIndex
          trackColor: Style.selectedFillFor(Color.popups.text, Color.accent)
          fillColor: Color.accent
          knobColor: Color.accent
          tickColor: Color.popups.background
          // PanelSlider turns every wheel event into moved + released
          // without a press; a drag's moves come while `dragging`.
          property bool fromWheel: false

          onMoved: function(v) {
            if (!root.desk) return
            var pct = Model.DESK_WIDTH_MIN + 10 * Math.round(v)
            slider.fromWheel = !slider.dragging
            if (slider.fromWheel) root.desk.previewWheel(pct)
            else root.desk.previewWidth = pct
          }
          onReleased: function(v) {
            if (!root.desk || slider.fromWheel) return
            root.desk.previewWidth = -1
            root.desk.writeSetting("deskWidth", Model.DESK_WIDTH_MIN + 10 * Math.round(v))
          }
        }

        Text {
          anchors.verticalCenter: parent.verticalCenter
          textFormat: Text.PlainText
          text: (root.desk ? root.desk.widthPct : Model.DESK_WIDTH_DEFAULT) + " %"
          color: Color.popups.text
          font.family: Style.font.family
          font.pixelSize: Style.font.subtitle
          font.bold: true
          font.features: { "tnum": 1 }
        }
      }

      ButtonGroup {
        options: Model.DESK_WIDTH_PRESETS.map(function(p) { return { value: String(p), label: Model.deskPresetLabel(p) } })
        value: root.desk ? String(root.desk.widthPct) : ""
        focusable: false
        fontSize: Style.font.bodySmall
        onChanged: function(v) { if (root.desk) root.desk.writeSetting("deskWidth", Number(v)) }
      }

      Column {
        width: parent.width
        spacing: Style.spacing.md

        Text {
          id: preview
          width: parent.width
          textFormat: Text.PlainText
          text: root.desk ? Model.deskWidthPreview(root.desk.windowWidth, root.desk.widthPct, Style.gapsOut) : ""
          color: root.tone.dim
          font.family: Style.font.family
          font.pixelSize: Style.font.bodySmall
        }

        // The screen, and the desk on it.
        Rectangle {
          width: Math.min(Style.space(420), parent.width)
          height: Style.space(40)
          radius: Style.cornerRadius
          color: "transparent"
          border.width: Math.max(1, Style.space(1))
          border.color: Style.normalBorderFor(Color.popups.text, Color.accent)

          Rectangle {
            readonly property real share: root.desk && root.desk.geometry.avail > 0 ? root.desk.geometry.w / root.desk.geometry.avail : 1
            x: (parent.width - width) / 2
            y: Style.spacing.sm
            width: parent.width * share
            height: parent.height - Style.spacing.sm * 2
            radius: Style.cornerRadius
            color: Style.selectedFillFor(Color.accent, Color.accent)
            border.width: Math.max(1, Style.space(1))
            border.color: Color.accent
          }
        }
      }

      Text {
        width: parent.width
        textFormat: Text.PlainText
        text: "Sidebar"
        color: Color.popups.text
        font.family: Style.font.family
        font.pixelSize: Style.font.title
        font.bold: true
      }

      ButtonGroup {
        options: [{ value: "open", label: "Open" }, { value: "collapsed", label: "Collapsed" }]
        value: root.desk ? root.desk.sidebarPref : Model.DESK_SIDEBAR_DEFAULT
        focusable: false
        fontSize: Style.font.bodySmall
        onChanged: function(v) { if (root.desk) root.desk.writeSetting("deskSidebar", v) }
      }

      Text {
        id: refusedNote
        visible: !!root.desk && (root.desk.settingsRefused || root.desk.settingsNoEntry)
        width: parent.width
        textFormat: Text.PlainText
        text: root.desk && root.desk.settingsNoEntry ? Model.DESK_NO_ENTRY_TEXT
          : "The shell did not take the change. " + Model.DESK_REFUSED_TEXT
        color: Color.popups.text
        wrapMode: Text.Wrap
        font.family: Style.font.family
        font.pixelSize: Style.font.bodySmall
      }

      KeyValues {
        width: parent.width
        rows: [
          ["Stored in", "Omarchy's plugin settings (shell.json), with the other Seldon settings; " + root.barSettings + " show the same keys"],
          ["Under 960 px", "The sidebar shows icons only; below 760 px list and detail take turns"],
          ["Theme", "Follows the active Omarchy theme"]
        ]
      }
    }

    // ---- Capture
    KeyValues {
      visible: root.selectedId === "capture"
      width: parent.width
      rows: [
        ["Capture interval", (root.service ? root.service.captureIntervalMin : Model.CAPTURE_INTERVAL_MIN_DEFAULT) + " min, and at shell start"],
        ["Set in", root.barSettings],
        ["Collectors", root.collectorsText()],
        ["Switched in", root.configPath + ", [collectors]"]
      ]
    }

    // ---- Agents
    KeyValues {
      visible: root.selectedId === "agents"
      width: parent.width
      rows: [
        ["Launcher", "[agent] launcher in " + root.configPath + "; Omarchy's agent by default"],
        ["Start folder", "[agent] workdir in " + root.configPath + "; by default where you start it, as Omarchy does"],
        ["Active cases limit", Model.clampWipLimit(root.entry.wipLimit) + ", set in " + root.barSettings]
      ]
    }

    // ---- Quiet
    KeyValues {
      visible: root.selectedId === "quiet"
      width: parent.width
      rows: [
        ["Changes counted in the bar", (root.service ? root.service.driftInBar : Model.DRIFT_IN_BAR_DEFAULT) + ", set in " + root.barSettings],
        ["What may be loud", "[drift] attention, routine, alwaysRedPaths in " + root.configPath],
        ["Crises", "Always shown: a change that can affect boot, login or the shell and has no case"]
      ]
    }
  }
}
