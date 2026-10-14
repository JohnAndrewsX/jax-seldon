import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Layouts
import "Model.js" as Model
import "Tones.js" as Tones

// The window: a header, then either the state that keeps the index from
// being shown (each with its one fix) or the Changes view. Keys live here;
// the views only lay out what Model.js derives from the index.
ApplicationWindow {
    id: win
    width: 1120
    height: 720
    minimumWidth: 720
    minimumHeight: 480
    visible: true
    title: "Seldon"
    color: theme.background

    Material.theme: Material.Dark
    Material.accent: theme.accent
    Material.background: theme.background
    Material.foreground: theme.foreground
    Material.roundedScale: theme.radius

    // The plugin's tones (Model.deskTones through Tones.js): text colours
    // that keep their contrast on this theme's surfaces.
    readonly property var tones: Tones.of(theme.foreground, theme.background, theme.accent, theme.urgent, theme.muted,
        theme.normalFill, theme.hoverFill, theme.selectedFill, theme.focusBorder, theme.focusBorderWidth)
    readonly property var index: backend.hasIndex ? backend.index : null
    readonly property bool showsChanges: !!win.index && (backend.status === "ok" || backend.status === "indexStale")
    // A key typed into a field is text, not a shortcut.
    readonly property bool typing: !!win.activeFocusItem && win.activeFocusItem.cursorPosition !== undefined
    property bool helpVisible: false
    property bool quitAsked: false

    // Space: the main action. The state's fix while one is shown, else a
    // capture (what changed since the last one).
    function mainAction() {
        if (backend.status === "indexStale" || !win.showsChanges)
            stateCard.fix()
        else
            changes.capture()
    }

    // Q asks first while a typed text would be lost.
    function quit() {
        if (changes.hasDraft && !win.quitAsked) {
            win.quitAsked = true
            return
        }
        Qt.quit()
    }

    Shortcut { sequences: ["?", "Shift+?"]; context: Qt.ApplicationShortcut; enabled: !win.typing; onActivated: win.helpVisible = !win.helpVisible }
    Shortcut { sequence: "Q"; context: Qt.ApplicationShortcut; enabled: !win.typing; onActivated: win.quit() }
    Shortcut { sequence: "Space"; context: Qt.ApplicationShortcut; enabled: !win.typing && !win.helpVisible; onActivated: win.mainAction() }
    Shortcut {
        sequence: "Esc"
        context: Qt.ApplicationShortcut
        onActivated: {
            if (win.helpVisible) win.helpVisible = false
            else if (win.quitAsked) win.quitAsked = false
            else changes.leave()
        }
    }
    Shortcut { sequences: ["Down", "J"]; context: Qt.ApplicationShortcut; enabled: !win.typing && !changes.formFocused && win.showsChanges; onActivated: changes.move(1) }
    Shortcut { sequences: ["Up", "K"]; context: Qt.ApplicationShortcut; enabled: !win.typing && !changes.formFocused && win.showsChanges; onActivated: changes.move(-1) }
    Shortcut { sequence: "F"; context: Qt.ApplicationShortcut; enabled: !win.typing && !changes.formFocused && win.showsChanges; onActivated: changes.cycleChip(1) }
    Shortcut { sequence: "Shift+F"; context: Qt.ApplicationShortcut; enabled: !win.typing && !changes.formFocused && win.showsChanges; onActivated: changes.cycleChip(-1) }
    Shortcut { sequences: ["Return", "Enter"]; context: Qt.ApplicationShortcut; enabled: !win.typing && win.showsChanges; onActivated: changes.activate() }

    // Right-to-left languages mirror the window; the Translator sets the
    // application's layout direction.
    Item {
        anchors.fill: parent
        LayoutMirroring.enabled: Qt.application.layoutDirection === Qt.RightToLeft
        LayoutMirroring.childrenInherit: true

        ColumnLayout {
            anchors.fill: parent
            spacing: 0

            RowLayout {
                Layout.fillWidth: true
                Layout.margins: 16
                spacing: 12

                Label {
                    text: "Seldon"
                    font.pixelSize: 20
                    font.bold: true
                    color: theme.foreground
                }
                Label {
                    text: qsTr("Changes")
                    font.pixelSize: 20
                    color: win.tones.dim
                    visible: win.showsChanges
                }
                Item { Layout.fillWidth: true }
                Label {
                    objectName: "devModeLine"
                    visible: backend.devMode
                    text: qsTr("Dev mode: read-only")
                    color: win.tones.accentText
                }
                Label {
                    objectName: "staleLine"
                    visible: backend.status === "indexStale" && win.showsChanges
                    text: qsTr("The index is stale")
                    color: win.tones.accentText
                }
                Label {
                    text: qsTr("? for keys")
                    color: win.tones.dim
                }
            }

            Rectangle {
                Layout.fillWidth: true
                implicitHeight: 1
                color: win.tones.divider
            }

            StateCard {
                id: stateCard
                objectName: "stateCard"
                Layout.fillWidth: true
                Layout.fillHeight: !win.showsChanges
                Layout.preferredHeight: win.showsChanges ? implicitHeight : -1
                tones: win.tones
                status: backend.status
                compact: win.showsChanges
                visible: !win.showsChanges || backend.status === "indexStale"
            }

            ChangesView {
                id: changes
                objectName: "changesView"
                Layout.fillWidth: true
                Layout.fillHeight: true
                visible: win.showsChanges
                index: win.showsChanges ? win.index : null
                tones: win.tones
            }
        }

        HelpOverlay {
            objectName: "helpOverlay"
            anchors.fill: parent
            visible: win.helpVisible
            tones: win.tones
            onClosed: win.helpVisible = false
        }

        // Q with a draft: say what goes, then Q again quits.
        Rectangle {
            objectName: "quitAsk"
            visible: win.quitAsked
            anchors.horizontalCenter: parent.horizontalCenter
            anchors.bottom: parent.bottom
            anchors.bottomMargin: 24
            width: quitText.implicitWidth + 32
            height: quitText.implicitHeight + 20
            radius: theme.radius
            color: theme.background
            border.color: theme.accent
            border.width: 1

            Label {
                id: quitText
                anchors.centerIn: parent
                text: qsTr("Your text is not sent yet. Press Q again to quit, Esc to stay.")
                color: theme.foreground
            }
        }
    }
}
