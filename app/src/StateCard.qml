import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Layouts
import "Model.js" as Model

// The states that keep the index from being shown (AGENTS.md §7: engine
// missing, logbook not initialised, index missing, index stale; and a
// contract this window cannot read), each with its one fix. A terminal fix
// opens Omarchy's presentation terminal with one of Model.js's fixed
// scripts; the others are engine calls. `compact` is the stale strip above
// the list, which still shows.
Item {
    id: root

    property string status: ""
    property var tones: ({})
    property bool compact: false
    property string answer: ""
    // A fix the engine is working on: its answer replaces "Working…".
    property bool waiting: false

    readonly property var words: root.wordsFor(root.status)

    implicitHeight: column.implicitHeight + 32

    onStatusChanged: {
        root.answer = ""
        root.waiting = false
    }

    function wordsFor(status) {
        if (status === "engineMissing")
            return { title: qsTr("The Seldon engine is missing"),
                     detail: qsTr("This window shows what the engine records. Install opens a terminal that downloads seldon from the Seldon release on GitHub into ~/.local/bin and checks it. It runs as your user; no password."),
                     fix: qsTr("Install the engine"), terminal: true }
        if (status === "notInitialised")
            return { title: qsTr("Create your logbook"),
                     detail: qsTr("Seldon has no logbook yet. Create opens a terminal that creates it and starts recording; the last 90 days become history before Seldon. No questions, no password."),
                     fix: qsTr("Create logbook"), terminal: true }
        if (status === "indexMissing") {
            var error = backend.parseError
            var unreadable = error !== "" && error !== "missing" && error !== "empty"
            return { title: unreadable ? qsTr("The index cannot be read") : qsTr("No index yet"),
                     detail: error === "size"
                         ? qsTr("index.json is %1 MB, over the 1 MB this window reads. Build it again from the logbook; if it stays this large, seldon status in a terminal names the largest part.")
                             .arg((backend.indexSize / 1000000).toLocaleString(Qt.locale(), 'f', 1))
                         : unreadable ? qsTr("index.json could not be read. Building it again from the logbook fixes this.")
                         : qsTr("The engine has not written index.json yet."),
                     fix: qsTr("Build index"), terminal: false }
        }
        if (status === "indexStale")
            return { title: qsTr("The index is stale"),
                     detail: qsTr("Last update %1. A capture brings it up to date.").arg(root.lastUpdate()),
                     fix: qsTr("Capture now"), terminal: false }
        if (status === "contractMismatch")
            return { title: qsTr("Index format mismatch"),
                     detail: qsTr("The index uses contract v%1 and this window reads v%2. Update Seldon, then check again.")
                         .arg(backend.indexContractVersion).arg(Model.CONTRACT_VERSION),
                     fix: qsTr("Check again"), terminal: false }
        if (status === "loading")
            return { title: qsTr("Reading the index…"), detail: "", fix: "", terminal: false }
        return { title: "", detail: "", fix: "", terminal: false }
    }

    function lastUpdate() {
        var at = backend.hasIndex ? new Date(backend.index.generatedAt) : null
        translator.language
        return at && isFinite(at.getTime()) ? at.toLocaleString(Qt.locale(), Locale.ShortFormat) : qsTr("unknown")
    }

    // The state's one fix (also Space).
    function fix() {
        if (root.words.fix === "")
            return
        if (backend.devMode) {
            root.answer = qsTr("Dev mode is read-only: fixes are off.")
            return
        }
        if (root.words.terminal) {
            var argv = Model.terminalArgv(Model.bannerFor(root.status, {}))
            root.answer = backend.openTerminal(argv || [])
                ? qsTr("A terminal opened. This window moves on by itself when the step is done.")
                : qsTr("The terminal could not be opened.")
        } else if (root.status === "contractMismatch") {
            backend.recheck()
        } else {
            root.waiting = backend.fix()
            root.answer = root.waiting ? qsTr("Working…") : qsTr("The engine could not be asked; check again.")
        }
    }

    Connections {
        target: backend
        function onFinished(args, exitCode, output, errorOutput) {
            if (!root.waiting || backend.busy)
                return
            root.waiting = false
            root.answer = exitCode === 0 ? "" : Model.engineError(output, errorOutput, exitCode)
        }
    }

    // Placed by y, not by anchors that switch between top and centre: an
    // anchor switch at runtime left the column at the height the two
    // anchors had stretched it to, over the list below the strip.
    ColumnLayout {
        id: column
        x: root.compact ? 16 : Math.max(16, (root.width - 640) / 2)
        y: root.compact ? 16 : Math.max(16, (root.height - column.implicitHeight) / 2)
        width: root.width - 2 * column.x
        height: column.implicitHeight
        spacing: 12

        Label {
            objectName: "stateTitle"
            textFormat: Text.PlainText
            Layout.fillWidth: true
            text: root.words.title
            font.pixelSize: root.compact ? 15 : 22
            font.bold: true
            color: root.status === "contractMismatch" ? root.tones.urgentText : theme.foreground
            wrapMode: Text.Wrap
            visible: !root.compact
        }
        Label {
            objectName: "stateDetail"
            textFormat: Text.PlainText
            Layout.fillWidth: true
            text: root.words.detail
            color: root.compact ? root.tones.accentText : root.tones.dim
            wrapMode: Text.Wrap
            visible: text !== ""
        }
        RowLayout {
            spacing: 12
            visible: root.words.fix !== ""

            SeldonButton {
                objectName: "stateFix"
                text: root.words.fix
                highlighted: true
                enabled: !backend.busy
                onClicked: root.fix()
            }
            SeldonButton {
                objectName: "stateRecheck"
                text: qsTr("Check again")
                flat: true
                visible: root.words.terminal
                onClicked: backend.recheck()
            }
            BusyIndicator {
                running: backend.busy
                visible: running
                implicitWidth: 28
                implicitHeight: 28
            }
        }
        Label {
            objectName: "stateAnswer"
            textFormat: Text.PlainText
            Layout.fillWidth: true
            text: root.answer
            color: root.tones.dim
            wrapMode: Text.Wrap
            visible: text !== ""
        }
    }
}
