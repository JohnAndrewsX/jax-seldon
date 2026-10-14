import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// `?`: every key of the window. Keys stay as they are in every language.
Rectangle {
    id: root

    property var tones: ({})

    signal closed()

    color: theme.background
    opacity: 0.97

    readonly property var keys: [
        { key: "↑ ↓  j k", what: qsTr("Move through the changes") },
        { key: "f  F", what: qsTr("Next or previous chip") },
        { key: "Enter", what: qsTr("Open the form of an open change; in the form, press (twice to confirm)") },
        { key: "Space", what: qsTr("Capture now, or the fix of the state shown") },
        { key: "Esc", what: qsTr("Leave the form, disarm, close this overlay") },
        { key: "?", what: qsTr("Show or hide the keys") },
        { key: "Q", what: qsTr("Quit (asks first while a text is not sent)") }
    ]

    MouseArea {
        anchors.fill: parent
        onClicked: root.closed()
    }

    ColumnLayout {
        anchors.centerIn: parent
        width: Math.min(parent.width - 48, 560)
        spacing: 14

        Label {
            text: qsTr("Keys")
            font.pixelSize: 20
            font.bold: true
            color: theme.foreground
        }

        Repeater {
            model: root.keys
            delegate: RowLayout {
                required property var modelData
                Layout.fillWidth: true
                spacing: 20

                Label {
                    Layout.preferredWidth: 110
                    text: modelData.key
                    font.bold: true
                    color: root.tones.accentText
                }
                Label {
                    Layout.fillWidth: true
                    text: modelData.what
                    color: theme.foreground
                    wrapMode: Text.Wrap
                }
            }
        }
    }
}
