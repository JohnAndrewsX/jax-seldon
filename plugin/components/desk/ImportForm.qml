pragma ComponentBehavior: Bound

import QtQuick
import qs.Commons
import qs.Ui
import "../../Model.js" as Model

// *Import tasks…* in the Work section's detail (WP-102b, ADR-0034 §2): the
// user's own Markdown task file becomes cases. A path (from the home, `~/…`,
// or absolute) and an optional area; *Dry run* (Enter in a field) sends
// `seldon import task --json --dry-run [--area <a>] -- <path>` and lists
// what would be created and what is skipped; then one click, *Import N
// cases*, sends the same without `--dry-run`. The path is one argument
// after `--`, never interpolated (CONTRACT.md); Model.importPathError
// spares the call for what cannot be a task file, the engine checks the
// rest (under the home, outside the logbook, a regular `.md` file) and its
// refusal is shown here. Import is offered only for the path and area the
// listed dry run was for: change either and the dry run runs again first.
// After an import the form reports `imported(firstCaseId)`; the fields keep
// their text until then.
//
// Every text the engine returns is user content (CONTRACT.md rule 6):
// Text.PlainText, never evaluated.
//
// Keyboard: while anything in the form has focus the section is `editing`;
// Tab walks path → area → buttons; Esc closes the form (its fields kept).
FocusScope {
  id: root

  property var service: null
  property color foreground: Color.foreground
  property color accent: Color.accent
  property color urgent: Color.urgent
  property string fontFamily: Style.font.family
  // Why the last call did not go out although the form was fine: another
  // import was pending (Service.busyRefusal).
  property string notice: ""
  // The import this form sent, until the engine answers.
  property bool sentImport: false

  property alias path: pathField.text
  property alias area: areaField.text

  readonly property bool editing: root.activeFocus
  readonly property bool enabledHere: !!service && service.canWrite
  readonly property string sendPath: root.path.trim()
  readonly property string pathError: root.sendPath === "" ? "" : Model.importPathError(root.sendPath)
  readonly property bool areaValid: root.area === "" || Model.AREA.test(root.area)
  readonly property var result: service ? service.importResult : null
  readonly property bool pending: !!result && result.pending
  // The result is about what the fields hold now.
  readonly property bool current: !!result && result.path === root.sendPath && result.area === root.area
  readonly property bool dryRunShown: root.current && !root.pending && result.ok && result.dryRun
  readonly property int toCreate: root.dryRunShown ? result.created.length : 0
  readonly property bool canDryRun: root.enabledHere && !root.pending && root.sendPath !== ""
    && root.pathError === "" && root.areaValid
  readonly property bool canImport: root.canDryRun && root.dryRunShown && root.toCreate > 0
  readonly property string resultText: root.notice !== "" ? root.notice
    : root.result && root.current ? root.result.text : ""
  readonly property color dim: Util.alpha(foreground, 0.65)
  readonly property real labelWidth: Style.space(64)

  signal leaveRequested()
  signal imported(string caseId)

  function focusPath() {
    pathField.forceActiveFocus()
  }

  function send(dryRun) {
    if (!root.service || (dryRun ? !root.canDryRun : !root.canImport)) return false
    var refusals = root.service.busyRefusals
    var sent = root.service.importTasks(root.sendPath, root.area, dryRun)
    root.notice = !sent && root.service.busyRefusals !== refusals ? root.service.busyRefusal.text : ""
    root.sentImport = sent && !dryRun
    return sent
  }

  // Return or Enter in a field runs the dry run, once per press (WP-173):
  // a held key's auto-repeat (Omarchy's Hyprland, after 250 ms) does nothing.
  // `keyEvents` counts the calls (the harness's key guard read-out).
  readonly property string keyGuard: "import"
  property int keyEvents: 0

  function keyPressed(event) {
    root.keyEvents++
    event.accepted = true
    if (!event.isAutoRepeat) root.dryRun()
  }

  function dryRun() {
    return root.send(true)
  }

  function importNow() {
    return root.send(false)
  }

  function reset() {
    root.path = ""
    root.area = ""
  }

  onResultChanged: {
    if (!root.result || root.result.pending || !root.sentImport) return
    root.sentImport = false
    if (!root.result.ok || root.result.dryRun) return
    var first = root.result.caseIds.length > 0 ? root.result.caseIds[0] : ""
    root.reset()
    root.imported(first)
  }

  Keys.onEscapePressed: function(event) {
    root.leaveRequested()
    event.accepted = true
  }

  implicitHeight: column.implicitHeight

  Column {
    id: column
    width: parent.width
    spacing: Style.spacing.md

    Text {
      width: parent.width
      textFormat: Text.PlainText
      text: "A Markdown task file under your home. Each open - [ ] item becomes a queued case; a file without checkboxes is one case. The file is only read. An imported case is started by you, after you have read its whole Intent."
      color: root.dim
      wrapMode: Text.Wrap
      font.family: root.fontFamily
      font.pixelSize: Style.font.bodySmall
    }

    TextField {
      id: pathField
      width: parent.width
      enabled: root.enabledHere
      placeholderText: root.enabledHere ? "~/projects/TODO.md, Enter shows a dry run"
        : root.service ? root.service.writeBlocker : "The Seldon service is not running"
      foreground: root.pathError === "" ? root.foreground : root.urgent
      accent: root.accent
      font.family: root.fontFamily
      font.pixelSize: Style.font.bodySmall
      Keys.onReturnPressed: function(event) { root.keyPressed(event) }
      Keys.onEnterPressed: function(event) { root.keyPressed(event) }
    }

    Text {
      width: parent.width
      visible: root.pathError !== ""
      textFormat: Text.PlainText
      text: root.pathError
      color: root.urgent
      wrapMode: Text.Wrap
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
    }

    Item {
      width: parent.width
      implicitHeight: Math.max(areaLabel.implicitHeight, areaField.implicitHeight)

      Text {
        id: areaLabel
        width: root.labelWidth
        anchors.verticalCenter: parent.verticalCenter
        textFormat: Text.PlainText
        text: "Area"
        color: root.dim
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
      }

      TextField {
        id: areaField
        x: root.labelWidth
        width: parent.width - root.labelWidth
        anchors.verticalCenter: parent.verticalCenter
        enabled: root.enabledHere
        placeholderText: "optional, e.g. dev-env"
        foreground: root.areaValid ? root.foreground : root.urgent
        accent: root.accent
        font.family: root.fontFamily
        font.pixelSize: Style.font.bodySmall
        Keys.onReturnPressed: function(event) { root.keyPressed(event) }
        Keys.onEnterPressed: function(event) { root.keyPressed(event) }
      }
    }

    Text {
      width: parent.width
      visible: !root.areaValid
      textFormat: Text.PlainText
      text: "Area: lowercase letters, digits and -, starting with a letter or digit"
      color: root.urgent
      wrapMode: Text.Wrap
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
    }

    Row {
      spacing: Style.spacing.sm

      KeyButton {
        objectName: "importDryRun"
        text: root.pending && root.result.dryRun ? "Reading" : "Dry run"
        iconText: root.pending && root.result.dryRun ? "󰦖" : ""
        iconSpinning: root.pending && root.result.dryRun
        iconSize: Style.font.caption
        enabled: root.canDryRun
        selected: !root.canImport
        bordered: true
        foreground: root.foreground
        accent: root.accent
        fontFamily: root.fontFamily
        fontSize: Style.font.caption
        verticalPadding: Style.spacing.xs
        tooltipText: "Lists what would be created; writes nothing (Enter in a field)"
        onClicked: root.dryRun()
      }

      KeyButton {
        objectName: "importNow"
        text: root.pending && !root.result.dryRun ? "Importing"
          : root.toCreate > 0 ? "Import " + Model.plural(root.toCreate, "case", "cases") : "Import"
        iconText: root.pending && !root.result.dryRun ? "󰦖" : ""
        iconSpinning: root.pending && !root.result.dryRun
        iconSize: Style.font.caption
        enabled: root.canImport
        selected: root.canImport
        bordered: true
        foreground: root.foreground
        accent: root.accent
        fontFamily: root.fontFamily
        fontSize: Style.font.caption
        verticalPadding: Style.spacing.xs
        tooltipText: root.canImport ? "Creates the cases the dry run lists, queued"
          : "Run the dry run for this path and area first"
        onClicked: root.importNow()
      }

      Button {
        text: "Cancel"
        focusable: true
        bordered: true
        foreground: root.foreground
        accent: root.accent
        fontFamily: root.fontFamily
        fontSize: Style.font.caption
        verticalPadding: Style.spacing.xs
        tooltipText: "Esc; the fields keep their text"
        onClicked: root.leaveRequested()
      }
    }

    Text {
      objectName: "importResult"
      width: parent.width
      visible: text !== ""
      textFormat: Text.PlainText
      text: root.resultText
      color: root.notice === "" && root.result && !root.result.ok ? root.urgent : root.foreground
      wrapMode: Text.Wrap
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
      font.bold: root.notice === "" && !!root.result && root.result.ok
    }

    // What the dry run (or the import) lists: the cases, then the skipped
    // tasks with the engine's reason.
    Repeater {
      model: root.result && root.current && root.result.ok ? root.result.created : []

      Column {
        id: createdRow
        required property var modelData
        width: column.width
        spacing: Style.spacing.xs

        Text {
          width: parent.width
          textFormat: Text.PlainText
          text: (createdRow.modelData.id !== "" ? createdRow.modelData.id + " · " : "") + createdRow.modelData.title
          color: root.foreground
          wrapMode: Text.Wrap
          font.family: root.fontFamily
          font.pixelSize: Style.font.bodySmall
        }

        Text {
          width: parent.width
          textFormat: Text.PlainText
          text: [createdRow.modelData.status, createdRow.modelData.source,
            createdRow.modelData.replaces !== "" ? "changed since " + createdRow.modelData.replaces : ""]
            .filter(function(p) { return p !== "" }).join(" · ")
          color: root.dim
          wrapMode: Text.Wrap
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
        }
      }
    }

    Repeater {
      model: root.result && root.current && root.result.ok ? root.result.skipped : []

      Text {
        id: skippedRow
        required property var modelData
        width: column.width
        textFormat: Text.PlainText
        text: "Skipped " + skippedRow.modelData.source + ": " + skippedRow.modelData.reason
          + (skippedRow.modelData.caseId !== "" ? " (" + skippedRow.modelData.caseId + ")" : "")
        color: root.dim
        wrapMode: Text.Wrap
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
      }
    }
  }
}
