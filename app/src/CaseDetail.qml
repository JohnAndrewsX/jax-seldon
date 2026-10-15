import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// The selected case in full (Model.caseDetail, from the index only,
// AGENTS.md §3): its bar of actions by status (Model.caseDeskActions), the
// hint of the armed one, then key/values, the first paragraph of Intent
// and Result, the plan's progress, the log and the linked changes. The
// rest is in the case file, which Open in editor shows. Every text that
// comes from the logbook is plain text. The view owns arming and the
// engine calls; this only lays them out and reports a press.
Item {
    id: root

    property var detail: null
    property var actions: []
    property var tones: ({})
    // The action armed on this case ("" for none) and the one the engine
    // is running for it.
    property string armedAction: ""
    property string pendingAction: ""
    property bool pending: false
    property string hint: ""

    signal pressed(string actionId)

    // Model.js says the linked-change remainder as "+N older changes …";
    // the window says it in the user's language.
    function olderCount(text) {
        var n = parseInt(String(text || "").slice(1), 10)
        return isFinite(n) ? n : 0
    }

    Flickable {
        id: flick
        objectName: "caseDetailFlick"
        anchors.fill: parent
        anchors.margins: 20
        contentHeight: column.implicitHeight
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar {}

        ColumnLayout {
            id: column
            width: flick.width
            spacing: 10

            Label {
                objectName: "caseHeading"
                Layout.fillWidth: true
                text: root.detail ? root.detail.id + " · " + (root.detail.closedByAgent ? qsTr("completed by agent") : qsTr(root.detail.status)) : ""
                color: root.tones.dim
                textFormat: Text.PlainText
                elide: Text.ElideRight
            }
            Label {
                objectName: "caseDetailTitle"
                Layout.fillWidth: true
                text: root.detail ? root.detail.title : ""
                font.pixelSize: 18
                font.bold: true
                color: theme.foreground
                wrapMode: Text.Wrap
                textFormat: Text.PlainText
            }
            Label {
                Layout.fillWidth: true
                visible: text !== ""
                text: !root.detail ? "" : [root.detail.closedByAgent ? qsTr("completed by agent") : "",
                    root.detail.reopens !== "" ? qsTr("reopens %1").arg(root.detail.reopens) : ""]
                    .filter(function(p) { return p !== "" }).join(" · ")
                color: root.tones.accentText
                font.italic: true
                textFormat: Text.PlainText
            }

            // The bar: the case's actions by status, the armed one asks.
            Flow {
                objectName: "caseActions"
                Layout.fillWidth: true
                spacing: 8

                Repeater {
                    model: root.actions
                    delegate: SeldonButton {
                        required property var modelData
                        objectName: "caseAction-" + modelData.id
                        text: root.armedAction === modelData.id ? qsTr("Press again to confirm")
                            : root.pendingAction === modelData.id ? qsTr("Working…")
                            : qsTr(modelData.label)
                        // Filled: the act Enter takes (never Hand to
                        // agent: Enter never starts an agent), or the armed one.
                        highlighted: modelData.enter || root.armedAction === modelData.id
                        enabled: !root.pending && !modelData.review
                        focusPolicy: Qt.NoFocus
                        onClicked: root.pressed(modelData.id)
                    }
                }
            }
            Label {
                objectName: "caseHint"
                Layout.fillWidth: true
                visible: text !== ""
                text: root.hint
                color: root.tones.accentText
                wrapMode: Text.Wrap
                textFormat: Text.PlainText
            }

            GridLayout {
                Layout.fillWidth: true
                Layout.topMargin: 6
                columns: 2
                columnSpacing: 16
                rowSpacing: 6

                Repeater {
                    model: root.detail ? root.detail.kv : []
                    delegate: RowLayout {
                        required property var modelData
                        Layout.columnSpan: 2
                        Layout.fillWidth: true
                        spacing: 16

                        Label {
                            Layout.preferredWidth: 110
                            Layout.alignment: Qt.AlignTop
                            // Model.caseDetail's fixed labels; each has a catalog entry.
                            text: qsTr(modelData[0])
                            color: root.tones.dim
                            elide: Text.ElideRight
                            textFormat: Text.PlainText
                        }
                        Label {
                            Layout.fillWidth: true
                            // A status, zone or priority is one of the
                            // engine's words, which have catalog entries;
                            // the rest is the index's text.
                            text: ["Status", "Zone", "Priority"].indexOf(modelData[0]) !== -1 ? qsTr(modelData[1]) : modelData[1]
                            color: theme.foreground
                            wrapMode: Text.WrapAtWordBoundaryOrAnywhere
                            textFormat: Text.PlainText
                        }
                    }
                }
            }

            // The first paragraph of Intent and Result (ADR-0038), each
            // hidden when the index has none.
            Repeater {
                model: root.detail ? [{ head: qsTr("Intent"), body: root.detail.intent },
                    { head: qsTr("Result"), body: root.detail.result }].filter(function(p) { return p.body !== "" }) : []
                delegate: ColumnLayout {
                    required property var modelData
                    Layout.fillWidth: true
                    Layout.topMargin: 6
                    spacing: 4

                    Label {
                        text: modelData.head
                        color: root.tones.dim
                        font.bold: true
                        textFormat: Text.PlainText
                    }
                    Label {
                        Layout.fillWidth: true
                        text: modelData.body
                        color: theme.foreground
                        wrapMode: Text.Wrap
                        textFormat: Text.PlainText
                    }
                }
            }

            Label {
                Layout.topMargin: 6
                text: qsTr("Plan")
                color: root.tones.dim
                font.bold: true
                textFormat: Text.PlainText
            }
            ProgressBar {
                Layout.preferredWidth: Math.min(column.width, 260)
                visible: !!root.detail && root.detail.plan.total > 0
                value: root.detail ? root.detail.plan.progress : 0
            }
            Label {
                objectName: "casePlan"
                Layout.fillWidth: true
                text: !root.detail ? "" : root.detail.plan.total > 0
                    ? qsTr("%1 of %2 steps done").arg(root.detail.plan.done).arg(root.detail.plan.total)
                    : qsTr("No steps in the plan yet")
                color: theme.foreground
                wrapMode: Text.Wrap
                textFormat: Text.PlainText
            }
            Label {
                Layout.fillWidth: true
                visible: !!root.detail && root.detail.actionable
                text: qsTr("The steps and the whole Intent and Result are in the case file; e opens it in the editor.")
                color: root.tones.dim
                wrapMode: Text.Wrap
                textFormat: Text.PlainText
            }

            Label {
                Layout.topMargin: 6
                text: qsTr("Log")
                color: root.tones.dim
                font.bold: true
                textFormat: Text.PlainText
            }
            Label {
                visible: !!root.detail && root.detail.log.length === 0
                text: qsTr("Nothing in the index yet.")
                color: root.tones.dim
                textFormat: Text.PlainText
            }
            Repeater {
                model: root.detail ? root.detail.log : []
                delegate: KeyValueRow {
                    required property var modelData
                    Layout.fillWidth: true
                    key: modelData[0]
                    value: modelData[1]
                    tones: root.tones
                }
            }

            Label {
                Layout.topMargin: 6
                text: qsTr("Linked changes · %1").arg(root.detail ? root.detail.linked.length : 0)
                color: root.tones.dim
                font.bold: true
                textFormat: Text.PlainText
            }
            Label {
                visible: !!root.detail && root.detail.linked.length === 0 && root.detail.linkedMore === ""
                text: qsTr("No change is linked to this case.")
                color: root.tones.dim
                textFormat: Text.PlainText
            }
            Repeater {
                model: root.detail ? root.detail.linked : []
                delegate: KeyValueRow {
                    required property var modelData
                    Layout.fillWidth: true
                    key: modelData[0]
                    value: modelData[1]
                    tones: root.tones
                }
            }
            Label {
                Layout.fillWidth: true
                visible: !!root.detail && root.detail.linkedMore !== ""
                text: qsTr("%n older change(s) the index no longer lists", "", root.olderCount(root.detail ? root.detail.linkedMore : ""))
                color: root.tones.dim
                wrapMode: Text.Wrap
                textFormat: Text.PlainText
            }
        }
    }
}
