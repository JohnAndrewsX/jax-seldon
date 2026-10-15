import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// One case in the Plan list (a row of Model.deskWork as Model.workView
// shows it). The first row of a group carries the group's header
// ("ACTIVE · 2"); the groups the index lists none of before this one
// (Model.workView's `lead`) come above it. An active case has the accent
// stripe with its word, so no state is told by colour alone. User content
// is plain text and elides; the detail shows it in full. The row imports
// no script, so a delegate does not evaluate Model.js again.
Item {
    id: root

    property var row: ({})
    property var tones: ({})
    property bool selected: false
    // "" or the group's header, translated by the view.
    property string header: ""
    // [{ header, text, command }]: groups with no listed row before this one.
    property var gaps: []
    // "agent working" while an agent's window is open on the case.
    property bool working: false

    readonly property string stripe: root.row.stripe || ""

    signal clicked()

    implicitHeight: column.implicitHeight

    Column {
        id: column
        width: parent.width

        Repeater {
            model: root.gaps
            delegate: GroupGap {
                required property var modelData
                width: column.width
                gap: modelData
                tones: root.tones
            }
        }

        Label {
            objectName: "groupHeader"
            visible: root.header !== ""
            width: parent.width
            leftPadding: 16
            topPadding: 12
            bottomPadding: 4
            text: root.header
            textFormat: Text.PlainText
            font.bold: true
            color: root.tones.dim
            elide: Text.ElideRight
        }

        Item {
            id: body
            width: parent.width
            implicitHeight: content.implicitHeight + 16

            Rectangle {
                anchors.fill: parent
                anchors.leftMargin: 8
                anchors.rightMargin: 8
                anchors.topMargin: 2
                anchors.bottomMargin: 2
                radius: Math.min(4, theme.radius)
                color: root.selected ? theme.selectedFill : mouse.containsMouse ? theme.hoverFill : theme.normalFill
            }
            Rectangle {
                objectName: "selectionBar"
                visible: root.selected
                anchors.left: parent.left
                anchors.leftMargin: 8
                width: 3
                height: parent.height - 4
                y: 2
                color: root.tones.accentUi
            }
            Rectangle {
                objectName: "stripe"
                visible: root.stripe !== ""
                anchors.left: parent.left
                anchors.leftMargin: 12
                width: 3
                height: parent.height - 4
                y: 2
                color: root.stripe === "crisis" ? theme.urgent : theme.accent
            }

            ColumnLayout {
                id: content
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                anchors.leftMargin: 24
                anchors.rightMargin: 20
                spacing: 2

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8

                    Label {
                        objectName: "caseTitle"
                        Layout.fillWidth: true
                        text: root.row.title || ""
                        font.bold: root.selected
                        color: theme.foreground
                        elide: Text.ElideRight
                        textFormat: Text.PlainText
                    }
                    Label {
                        visible: text !== ""
                        text: [root.working ? qsTr("agent working") : "", root.row.stepsText || ""]
                            .filter(function(p) { return p !== "" }).join(" · ")
                        color: root.working ? root.tones.accentText : root.tones.dim
                        textFormat: Text.PlainText
                        font.features: { "tnum": 1 }
                    }
                }
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8

                    Label {
                        visible: !!root.row.mark
                        text: root.row.mark ? qsTr(root.row.mark.word) : ""
                        color: root.tones.accentText
                        font.bold: true
                        textFormat: Text.PlainText
                    }
                    Label {
                        Layout.fillWidth: true
                        text: root.row.listMeta || ""
                        color: root.tones.dim
                        elide: Text.ElideRight
                        textFormat: Text.PlainText
                    }
                }
            }

            MouseArea {
                id: mouse
                anchors.fill: parent
                hoverEnabled: true
                onClicked: root.clicked()
            }
        }
    }
}
