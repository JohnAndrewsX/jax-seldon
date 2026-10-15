import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Layouts
import "Model.js" as Model

// Changes: the index's events newest first, by day, under the chips open ·
// crisis · attention · routine · in case · all (Model.deskChangelog,
// changelogView, changelogChips, as the plugin's Changelog). The selected
// change shows beside the list; an open one there resolves with link,
// explain or dismiss, each pressed twice.
Item {
    id: root

    property var index: null
    property var tones: ({})

    property string chip: Model.CHANGELOG_CHIP_DEFAULT
    // The selection is an event id, so it stays on its event when a new
    // index adds rows above it; cursorRow keeps ↑/↓'s place once the event
    // has left the chip (it was just linked).
    property string selectedId: ""
    property int cursorRow: 0
    property string captureAnswer: ""
    // Another change while a typed reason is unsent: the first try asks
    // (omarchy-app: ask before discarding work), the next one leaves it.
    property bool discardAsked: false

    readonly property var prepared: root.withStatus(Model.deskChangelog(root.index))
    readonly property var rows: Model.changelogView(root.prepared, root.chip, ({}), "")
    readonly property var chips: Model.changelogChips(root.prepared, ({}))
    readonly property var selected: Model.changelogRow(root.prepared, root.selectedId)
    readonly property bool hasDraft: detail.hasDraft
    readonly property bool formFocused: detail.formFocused

    // Each row's resolution or open note, once per index.
    function withStatus(prepared) {
        for (var i = 0; i < prepared.rows.length; i++) prepared.rows[i].statusLine = Model.rowStatus(prepared.rows[i])
        return prepared
    }

    function rowIndex(id) {
        for (var i = 0; i < root.rows.length; i++) if (root.rows[i].id === id) return i
        return -1
    }

    // True when the selection may move away from the selected change: no
    // typed reason, or this is the second try after the ask.
    function mayLeave(id) {
        if (id === root.selectedId || !detail.hasDraft || root.discardAsked) {
            root.discardAsked = false
            return true
        }
        root.discardAsked = true
        return false
    }

    function selectRow(i) {
        if (root.rows.length === 0) return
        var at = Math.max(0, Math.min(root.rows.length - 1, i))
        if (!root.mayLeave(root.rows[at].id)) return
        root.cursorRow = at
        root.selectedId = root.rows[at].id
        list.positionViewAtIndex(at, ListView.Contain)
    }

    function move(dy) {
        var at = root.rowIndex(root.selectedId)
        root.selectRow(at === -1 ? root.cursorRow + (dy > 0 ? 0 : -1) : at + dy)
    }

    function cycleChip(direction) {
        root.chip = Model.cycleChip(root.chip, direction)
    }

    // Enter: the selected open change's form, at its default action; in
    // the form, its press.
    function activate() {
        if (detail.formFocused) detail.press()
        else detail.openForm()
    }

    function leave() {
        if (!detail.leave()) list.forceActiveFocus()
    }

    // Capture now (Space): the engine records what changed and rewrites the
    // index, which the window reads by itself.
    function capture() {
        if (backend.devMode) {
            root.captureAnswer = qsTr("Dev mode is read-only: capture is off.")
            return
        }
        if (backend.busy) return
        root.captureAnswer = backend.run(["capture", "--all", "--json", "--quiet"]) ? qsTr("Capturing…")
            : qsTr("The engine could not be asked.")
    }

    // A new chip starts at its first row, unless a typed reason would go:
    // then the change stays in the detail.
    onChipChanged: Qt.callLater(function() { if (!detail.hasDraft) root.selectRow(0) })
    onRowsChanged: if (root.selectedId === "" && root.rows.length > 0) root.selectRow(0)
    // Another view shown: what was armed is disarmed; a typed reason stays.
    onVisibleChanged: if (!root.visible) detail.disarm()

    Connections {
        target: backend
        function onFinished(args, exitCode, output, errorOutput) {
            if (args[0] !== "capture") return
            var result = Model.captureResult(exitCode, output, errorOutput)
            root.captureAnswer = result.ok ? qsTr("Captured: %1").arg(result.text) : result.text
        }
    }

    function dayText(day) {
        translator.language
        var today = root.prepared ? root.prepared.today : ""
        if (day === "") return qsTr("Undated")
        if (day === today) return qsTr("Today")
        if (today !== "" && day === Model.addDays(today, -1)) return qsTr("Yesterday")
        var parts = day.split("-")
        var date = new Date(Number(parts[0]), Number(parts[1]) - 1, Number(parts[2]))
        if (!isFinite(date.getTime())) return day
        return date.toLocaleDateString(Qt.locale(), today.slice(0, 4) === parts[0] ? "dddd d MMMM" : "dddd d MMMM yyyy")
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        // The chips with their counts.
        Flow {
            Layout.fillWidth: true
            Layout.margins: 16
            Layout.bottomMargin: 8
            spacing: 8

            Repeater {
                model: root.chips
                delegate: SeldonButton {
                    id: chipButton
                    required property var modelData
                    objectName: "chip-" + modelData.id
                    text: qsTr(modelData.label) + "  " + modelData.count.toLocaleString(Qt.locale(), 'f', 0)
                    checkable: true
                    checked: root.chip === modelData.id
                    flat: !checked
                    highlighted: checked
                    focusPolicy: Qt.NoFocus
                    onClicked: root.chip = modelData.id
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 0

            ListView {
                id: list
                objectName: "changesList"
                Layout.preferredWidth: Math.max(360, root.width * 0.48)
                Layout.fillHeight: true
                clip: true
                focus: true
                model: root.rows
                currentIndex: root.rowIndex(root.selectedId)
                keyNavigationEnabled: false
                boundsBehavior: Flickable.StopAtBounds
                ScrollBar.vertical: ScrollBar {}

                section.property: "day"
                section.criteria: ViewSection.FullString
                section.delegate: Item {
                    required property string section
                    width: ListView.view.width
                    height: dayLabel.implicitHeight + 16

                    Label {
                        id: dayLabel
                        textFormat: Text.PlainText
                        anchors.left: parent.left
                        anchors.leftMargin: 16
                        anchors.bottom: parent.bottom
                        anchors.bottomMargin: 4
                        text: root.dayText(parent.section)
                        font.bold: true
                        color: root.tones.dim
                    }
                }

                delegate: ChangeRow {
                    required property var modelData
                    required property int index
                    width: ListView.view.width
                    row: modelData
                    tones: root.tones
                    selected: modelData.id === root.selectedId
                    onClicked: {
                        if (!root.mayLeave(modelData.id)) return
                        root.cursorRow = index
                        root.selectedId = modelData.id
                    }
                }

                Label {
                    objectName: "changesEmpty"
                    textFormat: Text.PlainText
                    anchors.centerIn: parent
                    width: parent.width - 48
                    horizontalAlignment: Text.AlignHCenter
                    wrapMode: Text.Wrap
                    visible: root.rows.length === 0
                    color: root.tones.dim
                    text: root.chip === "open" ? qsTr("Nothing open: every change has a case or a resolution.")
                        : qsTr("No changes under this chip.")
                }
            }

            Rectangle {
                Layout.fillHeight: true
                implicitWidth: 1
                color: root.tones.divider
            }

            ChangeDetail {
                id: detail
                objectName: "changeDetail"
                Layout.fillWidth: true
                Layout.fillHeight: true
                index: root.index
                row: root.selected
                tones: root.tones
                // Typing on, or sending, ends the ask.
                onReasonChanged: root.discardAsked = false
            }
        }

        Label {
            objectName: "discardAsk"
            Layout.fillWidth: true
            Layout.margins: 8
            Layout.leftMargin: 16
            visible: root.discardAsked
            text: qsTr("Your text is not sent yet. Select another change again to drop it, Esc to keep it.")
            color: root.tones.accentText
            wrapMode: Text.Wrap
            textFormat: Text.PlainText
        }

        Label {
            objectName: "captureAnswer"
            textFormat: Text.PlainText
            Layout.fillWidth: true
            Layout.margins: 8
            Layout.leftMargin: 16
            text: root.captureAnswer
            color: root.tones.dim
            visible: text !== ""
            elide: Text.ElideRight
        }
    }
}
