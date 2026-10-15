import QtQuick
import QtQuick.Controls

// A group the summary counts but a cut index lists no row of (ADR-0053
// §2, Model.workView's gaps): its header, "QUEUED · 0 / 500", over a dim
// line and the command that lists them, on a line of its own so it never
// wraps inside a flag.
Column {
    id: root

    // { header, command }; the header translated by the view.
    property var gap: ({})
    property var tones: ({})

    objectName: "groupGap"
    bottomPadding: 8

    Label {
        width: root.width
        leftPadding: 16
        topPadding: 12
        bottomPadding: 4
        text: root.gap.header || ""
        textFormat: Text.PlainText
        font.bold: true
        color: root.tones.dim
        elide: Text.ElideRight
    }
    Label {
        width: root.width
        leftPadding: 24
        rightPadding: 16
        text: qsTr("The index lists none of them. This lists them:")
        textFormat: Text.PlainText
        color: root.tones.dim
        wrapMode: Text.Wrap
    }
    Label {
        width: root.width
        leftPadding: 24
        rightPadding: 16
        text: root.gap.command || ""
        textFormat: Text.PlainText
        color: theme.foreground
        font.family: "monospace"
        elide: Text.ElideRight
    }
}
