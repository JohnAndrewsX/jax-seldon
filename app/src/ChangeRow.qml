import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// One change in the list (a row of Model.deskChangelog, with its
// Model.rowStatus as statusLine; the row imports no script, so a delegate
// does not evaluate Model.js again): the stripe of open
// drift (crisis in the urgent colour, attention in the accent) with its word,
// so no state is told by colour alone; the time, the title over the meta
// line, and the resolution or the open note. User content is plain text and
// elides; the detail shows it in full.
Item {
    id: root

    property var row: ({})
    property var tones: ({})
    property bool selected: false

    readonly property string stripe: root.row.stripe || ""
    readonly property string statusLine: root.row.statusLine || ""

    signal clicked()

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

    // The selection's accent bar, and beside it the drift stripe.
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

    RowLayout {
        id: content
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        anchors.leftMargin: 24
        anchors.rightMargin: 20
        spacing: 12

        Label {
            textFormat: Text.PlainText
            Layout.alignment: Qt.AlignTop
            text: root.row.time || ""
            color: root.tones.dim
            font.features: { "tnum": 1 }
        }

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 2

            Label {
                Layout.fillWidth: true
                text: root.row.title || ""
                font.bold: root.selected
                color: theme.foreground
                elide: Text.ElideRight
                textFormat: Text.PlainText
            }
            RowLayout {
                Layout.fillWidth: true
                spacing: 8

                Label {
                    textFormat: Text.PlainText
                    visible: !!root.row.mark
                    text: root.row.mark ? qsTr(root.row.mark.word) : ""
                    color: root.stripe === "crisis" ? root.tones.urgentText : root.tones.accentText
                    font.bold: true
                }
                Label {
                    textFormat: Text.PlainText
                    visible: (root.row.alert || "") !== ""
                    text: qsTr(root.row.alert || "")
                    color: root.tones.urgentText
                    font.bold: true
                }
                Label {
                    Layout.fillWidth: true
                    text: root.row.listMeta || ""
                    color: root.tones.dim
                    elide: Text.ElideRight
                    textFormat: Text.PlainText
                }
            }
            Label {
                Layout.fillWidth: true
                visible: root.statusLine !== ""
                // Model.js says it in English; its fixed notes have a catalog entry.
                text: qsTr(root.statusLine)
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
