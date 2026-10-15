import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// `?`: every key of the window: those of the view shown, then those of
// the whole window. Keys stay as they are in every language.
Rectangle {
    id: root

    property var tones: ({})
    // "changes" or "plan": whose keys come first.
    property string view: "changes"

    signal closed()

    color: theme.background
    opacity: 0.97

    readonly property var changesKeys: [
        { key: "↑ ↓  j k", what: qsTr("Move through the changes") },
        { key: "f  F", what: qsTr("Next or previous chip") },
        { key: "Enter", what: qsTr("Open the form of an open change; in the form, press (twice to confirm)") },
        { key: "Space", what: qsTr("Capture now, or the fix of the state shown") },
        { key: "Esc", what: qsTr("Leave the form, disarm, close this overlay") }
    ]
    readonly property var planKeys: [
        { key: "↑ ↓  j k", what: qsTr("Move through the cases") },
        { key: "Enter  Space", what: qsTr("The case's next step: Start, To verification, Complete (twice to confirm); in New case, Create") },
        { key: "a", what: qsTr("Hand the case to an agent (twice), or Focus the agent working on it") },
        { key: "x", what: qsTr("Drop the case (twice; final)") },
        { key: "r", what: qsTr("Reopen a completed case as a new one (twice)") },
        { key: "e", what: qsTr("Open the case file in the editor") },
        { key: "i", what: qsTr("New case in one sentence: type, then Enter twice starts an agent") },
        { key: "n", what: qsTr("New case by hand: title, zone, risk, priority, area") },
        { key: "/", what: qsTr("Search the cases") },
        { key: "b", what: qsTr("By agent: only the completed cases an agent closed") },
        { key: "Esc", what: qsTr("Leave the field, disarm, close the form, clear the search") }
    ]
    readonly property var windowKeys: [
        { key: "1  2", what: qsTr("Changes or Plan") },
        { key: "?", what: qsTr("Show or hide the keys") },
        { key: "Q", what: qsTr("Quit (asks first while a text is not sent)") }
    ]
    readonly property var keys: (root.view === "plan" ? root.planKeys : root.changesKeys).concat(root.windowKeys)

    // ↑/↓ while the overlay shows: a short window scrolls it.
    function scroll(dy) {
        flick.contentY = Math.max(0, Math.min(flick.contentHeight - flick.height, flick.contentY + dy * 48))
    }

    // Scrolls when the window is shorter than the keys; a click anywhere
    // closes it.
    Flickable {
        id: flick
        objectName: "helpFlick"
        anchors.fill: parent
        anchors.margins: 24
        contentWidth: width
        contentHeight: Math.max(column.implicitHeight, height)
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar {}

        MouseArea {
            width: flick.contentWidth
            height: flick.contentHeight
            onClicked: root.closed()
        }

        ColumnLayout {
            id: column
            x: Math.max(0, (flick.width - column.width) / 2)
            y: Math.max(0, (flick.height - column.implicitHeight) / 2)
            width: Math.min(flick.width, 560)
            spacing: 10

            Label {
                objectName: "helpTitle"
                textFormat: Text.PlainText
                text: root.view === "plan" ? qsTr("Keys · Plan") : qsTr("Keys · Changes")
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
                        objectName: "helpKey"
                        textFormat: Text.PlainText
                        Layout.preferredWidth: 110
                        Layout.alignment: Qt.AlignTop
                        text: modelData.key
                        font.bold: true
                        color: root.tones.accentText
                    }
                    Label {
                        textFormat: Text.PlainText
                        Layout.fillWidth: true
                        text: modelData.what
                        color: theme.foreground
                        wrapMode: Text.Wrap
                    }
                }
            }
        }
    }
}
