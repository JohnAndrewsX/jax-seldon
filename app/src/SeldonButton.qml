import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material

// A Material button filled with a plain rectangle in the theme's colours:
// the accent when highlighted, Omarchy's normal fill otherwise, none when
// flat. Material draws its own fill through a shader layer that the
// software renderer (offscreen tests, the README screenshot) leaves out, so
// a highlighted button would lose its fill there. The text on the accent is
// whichever of black or white reads on it (Theme.accentForeground).
Button {
    id: control

    Material.roundedScale: theme.radius

    contentItem: Label {
        text: control.text
        font: control.font
        color: control.highlighted ? theme.accentForeground : theme.foreground
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
        opacity: control.enabled ? 1 : 0.5
    }

    background: Rectangle {
        implicitWidth: 64
        implicitHeight: 36
        radius: Math.min(4, theme.radius)
        color: control.highlighted ? theme.accent : control.flat ? "transparent" : theme.normalFill
        border.width: control.visualFocus ? 2 : 0
        border.color: theme.foreground

        Rectangle {
            anchors.fill: parent
            radius: parent.radius
            color: theme.hoverFill
            visible: control.hovered || control.down
        }
    }
}
