import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Layouts
import "Model.js" as Model

// The selected change in full, and for open drift the form that resolves it
// (SPEC-ENGINE §3): link to a case, explain (a new completed case) or
// dismiss with a reason. Every resolution is pressed twice: the first press
// arms and says what will happen, the second runs it; any change to the
// form, or another selection, disarms. Neither a case nor a reason is
// required to leave a change open (ADR-0028 §3).
Item {
    id: root

    property var index: null
    property var row: null
    property var tones: ({})

    readonly property var item: root.row && root.row.drift ? Model.driftItemFor(root.index, root.row.id) : null
    readonly property var caseOptions: root.item ? Model.caseOptionsFor(root.index, root.item) : []

    property string action: "link"
    property string reason: ""
    property bool only: false
    // The picked case by id, so a new index that adds or removes cases
    // above it keeps the pick (a position would point at another case).
    property string caseId: ""
    readonly property int caseAt: root.caseIndex(root.caseOptions, root.caseId)
    property bool armed: false
    // The argv the first press armed; the second press runs only this.
    property var armedArgs: []
    property bool pending: false
    // The argv the engine is running for this form; its answer is matched
    // to it, whatever the form shows by then.
    property var pendingArgs: []
    property string answer: ""
    property string error: ""

    readonly property bool hasDraft: root.reason.trim() !== ""
    // The case picker has the keys: ↑/↓ pick a case, Enter presses.
    readonly property bool formFocused: caseBox.activeFocus
    readonly property var form: ({
        eventId: root.row ? root.row.id : "",
        caseId: root.caseAt >= 0 ? root.caseId : "",
        text: root.reason,
        only: root.only,
        itemZone: root.item ? root.item.zone : ""
    })
    readonly property string summary: Model.driftSummary(root.action, root.item, root.form)
    // What the second press would run now; an armed form whose argv this no
    // longer is (another index, another form) disarms.
    readonly property string built: JSON.stringify(Model.driftArgs(root.action, root.form).args || [])
    onBuiltChanged: if (root.armed && root.built !== JSON.stringify(root.armedArgs)) root.disarm()

    function caseIndex(options, id) {
        for (var i = 0; i < options.length; i++) if (options[i].value === id) return i
        return -1
    }

    function sameArgs(a, b) {
        return JSON.stringify(a) === JSON.stringify(b)
    }

    function disarm() {
        root.armed = false
        root.armedArgs = []
        root.error = ""
    }

    // The form starts over on another change, not on a new index: the row
    // object is rebuilt with every index, the answer to a resolution stays.
    // A call still running for the change left is no longer this form's.
    readonly property string rowId: root.row ? root.row.id : ""
    onRowIdChanged: {
        root.disarm()
        root.pending = false
        root.pendingArgs = []
        root.answer = ""
        root.only = false
        root.reason = ""
        var item = Model.driftItemFor(root.index, root.rowId)
        var options = item ? Model.caseOptionsFor(root.index, item) : []
        root.caseId = options.length > 0 ? options[0].value : ""
        root.action = Model.driftDefaultAction(item)
    }
    onActionChanged: root.disarm()
    onReasonChanged: root.disarm()
    onOnlyChanged: root.disarm()
    onCaseIdChanged: root.disarm()

    // Enter on the list: the form at its default action, its field focused.
    function openForm() {
        if (!root.item || root.pending) return
        root.action = Model.driftDefaultAction(root.item)
        if (root.action === "link") caseBox.forceActiveFocus()
        else reasonField.forceActiveFocus()
    }

    // Esc: out of the field, then disarm. False when there was nothing to do.
    function leave() {
        if (reasonField.activeFocus || caseBox.activeFocus) {
            reasonField.focus = false
            caseBox.focus = false
            return false
        }
        if (root.armed) {
            root.disarm()
            return true
        }
        return false
    }

    function press() {
        if (!root.item || root.pending) return
        if (backend.devMode) {
            root.error = qsTr("Dev mode is read-only: nothing is resolved.")
            return
        }
        var built = Model.driftArgs(root.action, root.form)
        if (built.error) {
            root.disarm()
            root.error = built.error
            return
        }
        if (!root.armed) {
            root.armed = true
            root.armedArgs = built.args
            return
        }
        // The second press runs what the first one armed, or nothing.
        if (!root.sameArgs(built.args, root.armedArgs)) {
            root.disarm()
            root.error = qsTr("The form changed after the first press; nothing ran.")
            return
        }
        root.disarm()
        if (!backend.run(built.args)) {
            root.error = qsTr("The engine could not be asked.")
            return
        }
        root.pending = true
        root.pendingArgs = built.args
        root.answer = qsTr("Working…")
    }

    Connections {
        target: backend
        function onFinished(args, exitCode, output, errorOutput) {
            if (!root.pending || !root.sameArgs(args, root.pendingArgs)) return
            root.pending = false
            root.pendingArgs = []
            var result = Model.driftResult(args[1], exitCode, output, errorOutput)
            root.answer = result.text
            if (result.ok) root.reason = ""
        }
    }

    Label {
        textFormat: Text.PlainText
        anchors.centerIn: parent
        visible: !root.row
        text: qsTr("Select a change to see it here.")
        color: root.tones.dim
    }

    Flickable {
        anchors.fill: parent
        anchors.margins: 20
        visible: !!root.row
        contentHeight: column.implicitHeight
        clip: true
        boundsBehavior: Flickable.StopAtBounds

        ColumnLayout {
            id: column
            width: parent.width
            spacing: 10

            Label {
                objectName: "detailSubject"
                Layout.fillWidth: true
                text: root.row ? root.row.subject : ""
                font.pixelSize: 18
                font.bold: true
                color: theme.foreground
                wrapMode: Text.WrapAnywhere
                textFormat: Text.PlainText
            }
            Label {
                Layout.fillWidth: true
                visible: text !== ""
                text: root.row ? qsTr(Model.rowStatus(root.row)) : ""
                color: root.row && root.row.crisis ? root.tones.urgentText : root.tones.accentText
                wrapMode: Text.Wrap
                textFormat: Text.PlainText
            }

            GridLayout {
                Layout.fillWidth: true
                columns: 2
                columnSpacing: 16
                rowSpacing: 6

                Repeater {
                    model: root.row ? [
                        { label: qsTr("What"), value: root.row.detail },
                        { label: qsTr("Source"), value: [root.row.source, root.row.kind].filter(function(p) { return p !== "" }).join(" · ") },
                        { label: qsTr("Who"), value: root.row.actor },
                        { label: qsTr("When"), value: root.row.day + " " + root.row.time },
                        { label: qsTr("Zone"), value: root.row.zone },
                        { label: qsTr("Case"), value: root.row.caseId },
                        { label: qsTr("Transaction"), value: root.row.alert }
                    ].filter(function(f) { return f.value && f.value.trim() !== "" }) : []

                    delegate: RowLayout {
                        required property var modelData
                        Layout.columnSpan: 2
                        Layout.fillWidth: true
                        spacing: 16

                        Label {
                            textFormat: Text.PlainText
                            Layout.preferredWidth: 96
                            text: modelData.label
                            color: root.tones.dim
                        }
                        Label {
                            Layout.fillWidth: true
                            text: modelData.value
                            color: theme.foreground
                            wrapMode: Text.WrapAnywhere
                            textFormat: Text.PlainText
                        }
                    }
                }
            }

            Rectangle {
                Layout.fillWidth: true
                Layout.topMargin: 8
                implicitHeight: 1
                color: root.tones.divider
                visible: !!root.item
            }

            // The resolution form.
            ColumnLayout {
                objectName: "driftForm"
                Layout.fillWidth: true
                visible: !!root.item
                spacing: 10

                RowLayout {
                    spacing: 8
                    Repeater {
                        model: [
                            { id: "link", label: qsTr("Link") },
                            { id: "explain", label: qsTr("Explain") },
                            { id: "dismiss", label: qsTr("Dismiss") }
                        ]
                        delegate: SeldonButton {
                            required property var modelData
                            objectName: "action-" + modelData.id
                            text: modelData.label
                            checkable: true
                            checked: root.action === modelData.id
                            // A tab, underlined when checked, so it does not
                            // look like the button that resolves.
                            tab: true
                            enabled: !root.pending
                            focusPolicy: Qt.NoFocus
                            onClicked: root.action = modelData.id
                        }
                    }
                }

                ComboBox {
                    id: caseBox
                    objectName: "caseBox"
                    Layout.fillWidth: true
                    visible: root.action === "link"
                    model: root.caseOptions
                    textRole: "label"
                    valueRole: "value"
                    currentIndex: root.caseAt
                    enabled: !root.pending
                    Material.roundedScale: theme.radius
                    onActivated: function(at) { root.caseId = root.caseOptions[at].value }

                    // Case titles come from the logbook: plain text, elided.
                    contentItem: Label {
                        leftPadding: 12
                        rightPadding: caseBox.indicator ? caseBox.indicator.width : 0
                        text: caseBox.currentIndex >= 0 ? caseBox.displayText : qsTr("Pick a case")
                        textFormat: Text.PlainText
                        elide: Text.ElideRight
                        verticalAlignment: Text.AlignVCenter
                        color: theme.foreground
                    }
                    delegate: ItemDelegate {
                        required property var modelData
                        required property int index
                        objectName: "caseOption"
                        width: ListView.view ? ListView.view.width : implicitWidth
                        highlighted: caseBox.highlightedIndex === index
                        contentItem: Label {
                            text: modelData.label
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            verticalAlignment: Text.AlignVCenter
                            color: theme.foreground
                        }
                    }
                }

                TextField {
                    id: reasonField
                    objectName: "reasonField"
                    Layout.fillWidth: true
                    visible: root.action !== "link"
                    placeholderText: root.action === "explain" ? qsTr("Why it changed") : qsTr("Why it needs no case")
                    text: root.reason
                    enabled: !root.pending
                    Material.roundedScale: theme.radius
                    onTextEdited: root.reason = text
                    // Once per press: a held Enter's repeat never confirms
                    // what its first press armed (SPEC-PLUGIN §5.3).
                    Keys.onReturnPressed: function(event) { event.accepted = true; if (!event.isAutoRepeat) root.press() }
                    Keys.onEnterPressed: function(event) { event.accepted = true; if (!event.isAutoRepeat) root.press() }
                }

                CheckBox {
                    objectName: "onlyBox"
                    visible: !!root.item && root.item.grouped
                    text: qsTr("Only this event, not its group")
                    checked: root.only
                    enabled: !root.pending
                    focusPolicy: Qt.NoFocus
                    onToggled: root.only = checked
                }

                RowLayout {
                    spacing: 12

                    SeldonButton {
                        objectName: "resolveButton"
                        text: root.armed ? qsTr("Press again to confirm")
                            : root.action === "link" ? qsTr("Link") : root.action === "explain" ? qsTr("Explain") : qsTr("Dismiss")
                        highlighted: true
                        enabled: !root.pending
                        focusPolicy: Qt.NoFocus
                        onClicked: root.press()
                    }
                    Label {
                        Layout.fillWidth: true
                        visible: root.armed
                        text: root.summary
                        color: root.tones.accentText
                        wrapMode: Text.Wrap
                        textFormat: Text.PlainText
                    }
                }

                Label {
                    objectName: "formError"
                    Layout.fillWidth: true
                    visible: text !== ""
                    text: root.error
                    color: root.tones.urgentText
                    wrapMode: Text.Wrap
                    textFormat: Text.PlainText
                }
                Label {
                    objectName: "formAnswer"
                    Layout.fillWidth: true
                    visible: text !== ""
                    text: root.answer
                    color: root.tones.dim
                    wrapMode: Text.Wrap
                    textFormat: Text.PlainText
                }
            }
        }
    }
}
