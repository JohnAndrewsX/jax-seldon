import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Layouts
import "Model.js" as Model
import "Tones.js" as Tones

// The window: a header with the views, then either the state that keeps
// the index from being shown (each with its one fix) or the view: Changes
// (1) or Plan (2). Keys live here and go to the view shown; the views only
// lay out what Model.js derives from the index. Both views stay loaded, so
// a view change keeps their drafts; it disarms what was armed.
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
    readonly property bool showsViews: !!win.index && (backend.status === "ok" || backend.status === "indexStale")
    // "changes" or "plan".
    property string view: "changes"
    readonly property bool inChanges: win.showsViews && win.view === "changes"
    readonly property bool inPlan: win.showsViews && win.view === "plan"
    // The view's own keys are off while a field or a picker has them.
    readonly property bool viewKeys: !win.typing && !win.helpVisible
        && (win.inChanges ? !changes.formFocused : win.inPlan ? !plan.formFocused : false)
    // A key typed into a field is text, not a shortcut.
    readonly property bool typing: !!win.activeFocusItem && win.activeFocusItem.cursorPosition !== undefined
    property bool helpVisible: false
    property bool quitAsked: false

    // Space: the main action. The state's fix while one is shown; in
    // Changes a capture (what changed since the last one), in Plan the
    // selected case's first action (as Enter).
    function mainAction() {
        if (backend.status === "indexStale" || !win.showsViews)
            stateCard.fix()
        else if (win.view === "plan")
            plan.activate()
        else
            changes.capture()
    }

    function showView(view) {
        if (win.view === view) return
        win.view = view
        changes.discardAsked = false
    }

    // Q asks first while a typed text would be lost.
    function quit() {
        if ((changes.hasDraft || plan.hasDraft) && !win.quitAsked) {
            win.quitAsked = true
            return
        }
        Qt.quit()
    }

    Shortcut { sequences: ["?", "Shift+?"]; context: Qt.ApplicationShortcut; enabled: !win.typing; onActivated: win.helpVisible = !win.helpVisible }
    Shortcut { sequence: "Q"; context: Qt.ApplicationShortcut; enabled: !win.typing; onActivated: win.quit() }
    Shortcut { sequence: "1"; context: Qt.ApplicationShortcut; enabled: !win.typing && win.showsViews; onActivated: win.showView("changes") }
    Shortcut { sequence: "2"; context: Qt.ApplicationShortcut; enabled: !win.typing && win.showsViews; onActivated: win.showView("plan") }
    // The case picker keeps Space for itself (it opens its list).
    Shortcut { sequence: "Space"; context: Qt.ApplicationShortcut; enabled: !win.typing && !changes.formFocused && !win.helpVisible; onActivated: win.mainAction() }
    Shortcut {
        sequence: "Esc"
        context: Qt.ApplicationShortcut
        onActivated: {
            if (win.helpVisible) win.helpVisible = false
            else if (win.quitAsked) win.quitAsked = false
            else if (win.inPlan) plan.leave()
            else if (changes.discardAsked) changes.discardAsked = false
            else changes.leave()
        }
    }
    Shortcut { sequences: ["Down", "J"]; context: Qt.ApplicationShortcut; enabled: win.viewKeys || win.helpVisible; onActivated: win.helpVisible ? help.scroll(1) : win.inPlan ? plan.move(1) : changes.move(1) }
    Shortcut { sequences: ["Up", "K"]; context: Qt.ApplicationShortcut; enabled: win.viewKeys || win.helpVisible; onActivated: win.helpVisible ? help.scroll(-1) : win.inPlan ? plan.move(-1) : changes.move(-1) }
    Shortcut { sequence: "F"; context: Qt.ApplicationShortcut; enabled: win.viewKeys && win.inChanges; onActivated: changes.cycleChip(1) }
    Shortcut { sequence: "Shift+F"; context: Qt.ApplicationShortcut; enabled: win.viewKeys && win.inChanges; onActivated: changes.cycleChip(-1) }
    Shortcut { sequences: ["Return", "Enter"]; context: Qt.ApplicationShortcut; enabled: !win.typing && win.showsViews; onActivated: win.inPlan ? plan.activate() : changes.activate() }
    // Plan's letters (HelpOverlay lists them).
    Repeater {
        model: ["A", "X", "R", "E", "I", "N", "B"]
        delegate: Item {
            required property string modelData
            Shortcut { sequence: modelData; context: Qt.ApplicationShortcut; enabled: win.viewKeys && win.inPlan; onActivated: plan.key(modelData.toLowerCase()) }
        }
    }
    Shortcut { sequences: ["/", "Shift+/"]; context: Qt.ApplicationShortcut; enabled: win.viewKeys && win.inPlan; onActivated: plan.key("/") }

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
                    textFormat: Text.PlainText
                    text: "Seldon"
                    font.pixelSize: 20
                    font.bold: true
                    color: theme.foreground
                }
                // The views, as tabs; 1 and 2 reach them too.
                Repeater {
                    model: [{ id: "changes", label: qsTr("Changes") }, { id: "plan", label: qsTr("Plan") }]
                    delegate: SeldonButton {
                        required property var modelData
                        objectName: "view-" + modelData.id
                        visible: win.showsViews
                        text: modelData.label
                        tab: true
                        checkable: true
                        checked: win.view === modelData.id
                        focusPolicy: Qt.NoFocus
                        onClicked: win.showView(modelData.id)
                    }
                }
                Item { Layout.fillWidth: true }
                Label {
                    objectName: "devModeLine"
                    textFormat: Text.PlainText
                    visible: backend.devMode
                    text: qsTr("Dev mode: read-only")
                    color: win.tones.accentText
                }
                Label {
                    objectName: "staleLine"
                    textFormat: Text.PlainText
                    visible: backend.status === "indexStale" && win.showsViews
                    text: qsTr("The index is stale")
                    color: win.tones.accentText
                }
                Label {
                    textFormat: Text.PlainText
                    text: qsTr("? for keys")
                    color: win.tones.dim
                }
            }

            Rectangle {
                Layout.fillWidth: true
                implicitHeight: 1
                color: win.tones.divider
            }

            // ADR-0051: an index of a newer contract this window can still
            // read shows, with a quiet note (CONTRACT.md rule 3).
            Label {
                objectName: "contractNotice"
                Layout.fillWidth: true
                Layout.margins: 8
                Layout.leftMargin: 16
                visible: win.showsViews && backend.contractNewer
                text: qsTr("The engine writes index v%1; this window reads v%2 — update Seldon.")
                    .arg(backend.indexContractVersion).arg(Model.CONTRACT_VERSION)
                color: win.tones.dim
                wrapMode: Text.Wrap
                textFormat: Text.PlainText
            }

            StateCard {
                id: stateCard
                objectName: "stateCard"
                Layout.fillWidth: true
                Layout.fillHeight: !win.showsViews
                Layout.preferredHeight: win.showsViews ? implicitHeight : -1
                tones: win.tones
                status: backend.status
                compact: win.showsViews
                visible: !win.showsViews || backend.status === "indexStale"
            }

            ChangesView {
                id: changes
                objectName: "changesView"
                Layout.fillWidth: true
                Layout.fillHeight: true
                visible: win.inChanges
                index: win.showsViews ? win.index : null
                tones: win.tones
            }

            PlanView {
                id: plan
                objectName: "planView"
                Layout.fillWidth: true
                Layout.fillHeight: true
                visible: win.inPlan
                active: win.inPlan
                index: win.showsViews ? win.index : null
                tones: win.tones
            }
        }

        HelpOverlay {
            id: help
            objectName: "helpOverlay"
            anchors.fill: parent
            visible: win.helpVisible
            view: win.view
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
                textFormat: Text.PlainText
                anchors.centerIn: parent
                text: qsTr("Your text is not sent yet. Press Q again to quit, Esc to stay.")
                color: theme.foreground
            }
        }
    }
}
