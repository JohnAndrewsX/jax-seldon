pragma ComponentBehavior: Bound

import QtQuick
import qs.Commons
import "../components/desk"
import "../Model.js" as Model

// The frame of the three reading sections (Decisions, System, Memory;
// ADR-0034 §2, WP-123): a list of rows from a Model.js row function,
// narrowed by the sidebar search, and the detail of the selected row with
// its sticky action bar. The selection is the cursor (as the prototype):
// ↑/↓ (j/k) move it and the detail follows; Enter shows the detail in the
// stacked layout. The selection stays on its row by id across index
// updates; a row that is gone (or filtered out) leaves the first row shown.
//
// A section built on this sets `allRows` (each with a unique `id`),
// `searchFields`, the list `title`, `emptyText` and `rowTitle`/`rowMeta`/
// `rowAside`/`rowStripe` (functions of a row), and fills `head` (above the
// list) and the detail (the default property) and `actions` / `actionMeta`.
Section {
  id: root

  property var allRows: []
  property var searchFields: ["id"]
  property string listTitle: root.title
  property string emptyText: ""
  property string detailTitle: ""
  property var actions: []
  property string actionMeta: ""
  property string actionHint: ""
  // Show the detail (true) or what the section puts there instead (a form).
  property bool showRow: true
  property var rowTitle: function(r) { return String(r.id) }
  property var rowMeta: function(r) { return "" }
  property var rowAside: function(r) { return "" }
  property var rowStripe: function(r) { return "" }

  property alias head: list.head
  default property alias detail: pane.content
  readonly property alias listView: list.view
  readonly property alias detailPane: pane

  readonly property var rows: Model.deskFilter(root.allRows, root.searchText, root.searchFields)
  readonly property int cursor: root.rowIndex(root.selectedId)
  // The row the detail shows: the selected one, else the first.
  readonly property var current: root.rows.length === 0 ? null : root.rows[Math.max(0, root.cursor)]

  signal actionTriggered(string id)
  // A click on a row of the list (after it was selected).
  signal rowClicked(string id)

  function rowIndex(id) {
    for (var i = 0; i < root.rows.length; i++) if (root.rows[i].id === id) return i
    return -1
  }

  function move(dy) {
    if (root.rows.length === 0) return true
    // From the row the detail shows: the first when the selection is gone
    // or filtered out (`current`), so the first ↓ moves on from it.
    var from = Math.max(0, root.cursor)
    var i = Math.max(0, Math.min(root.rows.length - 1, from + dy))
    root.selectedId = root.rows[i].id
    return true
  }

  function activate() {
    if (root.desk && root.current) root.desk.showDetail()
    return true
  }

  function select(id) {
    var target = String(id)
    for (var i = 0; i < root.allRows.length; i++) {
      if (root.allRows[i].id !== target) continue
      root.selectedId = target
      if (root.desk) root.desk.showDetail()
      return true
    }
    return false
  }

  function applyPayload(payload) {
    if (payload && payload.select) root.select(payload.select)
  }

  // The read-out every reading section shares; a section adds its own keys.
  function baseView() {
    return {
      rows: root.rows.map(function(r) { return r.id }),
      cursor: root.current ? root.current.id : "",
      filtered: root.rows.length !== root.allRows.length,
      actions: root.actions.filter(function(a) { return a.enabled !== false }).map(function(a) { return a.label }),
      actionMeta: root.actionMeta
    }
  }

  function view() {
    return root.baseView()
  }

  onRowsChanged: if (root.selectedId === "" && root.rows.length > 0) root.selectedId = root.rows[0].id

  ListColumn {
    id: list
    visible: !root.stacked || !root.detailShown
    width: root.stacked ? root.width : (root.layout ? root.layout.listW : 0)
    height: root.height
    divider: !root.stacked
    title: root.listTitle
    emptyText: root.emptyText
    model: root.rows
    currentIndex: root.cursor

    delegate: ListRow {
      required property var modelData
      required property int index
      width: ListView.view.width
      title: root.rowTitle(modelData)
      meta: root.rowMeta(modelData)
      aside: root.rowAside(modelData)
      stripe: root.rowStripe(modelData)
      selected: !!root.current && root.current.id === modelData.id
      cursor: list.hoverIndex === index
      onClicked: {
        root.selectedId = modelData.id
        root.rowClicked(modelData.id)
        if (root.desk) {
          root.desk.showDetail()
          root.desk.giveKeys()
        }
      }
    }
  }

  DetailPane {
    id: pane
    x: root.stacked ? 0 : list.width
    width: root.width - x
    height: root.height
    visible: !root.stacked || root.detailShown
    backVisible: root.stacked
    title: root.detailTitle
    actions: root.showRow && root.current ? root.actions : []
    meta: root.showRow && root.current ? root.actionMeta : ""
    hint: root.actionHint
    onBackRequested: if (root.desk) root.desk.back()
    onActionTriggered: function(id) { root.actionTriggered(id) }
  }
}
