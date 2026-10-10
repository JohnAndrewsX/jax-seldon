import QtQuick
import ".."
import "../../Model.js" as Model

// The base of every desk section (sections/*.qml, SPEC-PLUGIN §5). The desk
// creates a section on its first visit, sets `desk` and `sectionId`, and
// keeps it while it is open; only the current one is visible.
//
// What a section reads comes from the desk: the service, the index (null
// while its contents mean nothing in the service's status), the layout
// (Model.deskLayout: sidebar, stacked, solo, listW, detailW), the sidebar
// search text and the shared arm-twice helper (Arm.qml). What the desk
// calls is below; each call returns true when the section used it, so the
// desk can fall back (a letter it does not take, Esc with nothing to
// leave). A section never runs the engine itself: it asks the service.
Item {
  id: root

  property var desk: null
  property string sectionId: ""

  // The theme's text and UI tones (components/Tone.qml), for every section.
  readonly property Tone tone: Tone {}

  readonly property var info: Model.deskSection(root.sectionId)
  readonly property string title: root.info ? root.info.label : ""
  readonly property bool active: !!root.desk && root.desk.opened && root.desk.sectionId === root.sectionId
  readonly property var service: root.desk ? root.desk.service : null
  readonly property var index: root.desk ? root.desk.indexData : null
  readonly property var layout: root.desk ? root.desk.layout : null
  readonly property bool stacked: !!root.layout && root.layout.stacked
  readonly property bool solo: !!root.info && root.info.solo
  readonly property string searchText: root.desk ? root.desk.searchText : ""
  readonly property var arm: root.desk ? root.desk.arm : null
  // Desk.keyPressed's count: every key the desk gets clears the pointer's
  // row (ListColumn, one cursor, SPEC-PLUGIN §5.3).
  readonly property int keyEvents: root.desk ? root.desk.keyEvents : 0
  // In the stacked layout: the detail is shown instead of the list.
  readonly property bool detailShown: !!root.desk && root.desk.detailShown

  // A text field of this section has the keys: the desk stays out of them
  // until it is false again (Esc in the field is the field's).
  property bool editing: false
  // The selected item ("" none); the desk remembers it between opens.
  property string selectedId: ""

  // ↑/↓ (j/k) in the list.
  function move(dy) { return false }
  // ←/→ (the Prime Radiant's periods).
  function moveAcross(dx) { return false }
  // Enter / Space on the cursor's row.
  function activate() { return false }
  // A typed character the desk does not own (letters, `+`).
  function textKey(text) { return false }
  // Select an item by id (IPC `select`, the shim's `resolve`).
  function select(id) { return false }
  // Esc: leave an inline form or state of the section.
  function back() { return false }
  // The rest of an open() payload: { select, filter, period }.
  function applyPayload(payload) {}
  // What the section shows, for Desk.view().
  function view() { return ({}) }
}
