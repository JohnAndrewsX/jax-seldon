import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// One line of a case's log or linked changes: the time, then what
// happened (plain text from the index).
RowLayout {
    id: root

    property string key: ""
    property string value: ""
    property var tones: ({})

    spacing: 16

    // The index's stamps are "YYYY-MM-DD HH:MM"; the column fits one.
    TextMetrics {
        id: stamp
        font: keyLabel.font
        text: "0000-00-00 00:00"
    }

    Label {
        id: keyLabel
        Layout.preferredWidth: Math.ceil(stamp.advanceWidth) + 2
        Layout.alignment: Qt.AlignTop
        text: root.key
        color: root.tones.dim
        textFormat: Text.PlainText
        font.features: { "tnum": 1 }
    }
    Label {
        Layout.fillWidth: true
        text: root.value
        color: theme.foreground
        wrapMode: Text.WrapAtWordBoundaryOrAnywhere
        textFormat: Text.PlainText
    }
}
