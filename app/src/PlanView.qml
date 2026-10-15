import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Layouts
import "Model.js" as Model

// Plan: the cases (the desk's Work section, Model.deskWork and workView):
// the one-sentence start, the WIP line, By agent, New case and the search
// over the groups Active · Verification · Queued · Completed; the selected
// case beside the list (CaseDetail), or the New case form there.
//
// Every act that writes is pressed twice: the first press arms and says
// what will run, the second runs exactly the argument list the first one
// armed; whatever changes that list (another case, a new index that moves
// or removes the case, another action, a changed form or sentence)
// disarms. Focus and Open in editor write nothing and run at once. One
// call at a time; its answer is matched to the argument list that ran.
// The argument lists are Model.js's (planArgs, agentArgs, agentNewArgs,
// agentFocusArgs) and `edit <caseId> --editor --json` (ADR-0058); the
// backend checks each against CONTRACT.md's forms again.
//
// One agent per case (WP-156): while the view shows, the engine is asked
// which agents it launched still run (`agent sessions --json`, its own
// process); an active case an agent works on shows Focus in place of Hand
// to agent and "agent working" on its row.
Item {
    id: root

    property var index: null
    property var tones: ({})
    // The view is on screen: sessions are asked, arming ends when it goes.
    property bool active: false

    // "" or Model.COMPLETED_FILTER_AGENT: Completed lists only the cases an
    // agent closed (the spot check, ADR-0027 §5).
    property string filter: ""
    property string search: ""
    property bool searchOpen: false
    property string selectedId: ""
    property int cursorRow: 0
    property bool formOpen: false
    property var sessions: ({})

    // What the first press armed: "case:<action>", "new" or "run"; the
    // argv it armed, and the case it names.
    property string armedWhat: ""
    property var armedArgs: []
    property string armedCase: ""
    // The call the engine is running for this view and what it was.
    property var pendingArgs: []
    property string pendingWhat: ""
    property string pendingCase: ""
    readonly property bool pending: root.pendingArgs.length > 0
    // The last answer: { ok, text, what }.
    property var result: null
    // A refused New case shows in the form, beside the title it kept.
    property string formError: ""
    property string sentTitle: ""
    property string sentIntent: ""

    readonly property var prepared: Model.deskWork(root.index)
    readonly property var work: Model.workView(root.prepared, root.filter, root.search)
    readonly property var rows: root.work.rows
    readonly property var detailData: Model.withSession(Model.caseDetail(root.index, root.prepared, root.selectedId), root.sessions)
    readonly property var current: root.detailData ? root.detailData.row : null
    readonly property var actions: Model.caseDeskActions(root.current)
    readonly property var wip: Model.wipStatus(root.index, Model.WIP_LIMIT_DEFAULT)
    readonly property var listState: Model.emptyState("work", { index: !!root.index, search: root.search,
        shown: root.rows.length, footer: root.work.tail.length > 0, canWrite: !backend.devMode, mode: "pro" })
    readonly property bool hasDraft: intentField.text.trim() !== "" || form.hasDraft
    // A field or choice of the view has the keys.
    readonly property bool formFocused: form.formFocused
    readonly property string armedAction: root.armedWhat.indexOf("case:") === 0 && root.armedCase === root.selectedId
        ? root.armedWhat.slice(5) : ""

    // What the second press would run now; an armed act whose argv this
    // no longer is disarms. `built` reads armedWhat, so the disarm comes
    // right after this change, not inside it; a press in between finds
    // the argv changed and runs nothing.
    readonly property string built: root.armedWhat === "" ? "" : JSON.stringify(root.argsFor(root.armedWhat).args || [])
    onBuiltChanged: if (root.stale()) Qt.callLater(function() { if (root.stale()) root.disarm() })

    function stale() {
        return root.armedWhat !== "" && root.built !== JSON.stringify(root.armedArgs)
    }

    onActiveChanged: {
        root.disarm()
        if (root.active) backend.askSessions()
    }
    onIndexChanged: if (root.active) backend.askSessions()
    // A first index with cases selects the first one, behind an open form:
    // an arriving index never closes the form nor takes its keys.
    onRowsChanged: if (root.selectedId === "" && root.rows.length > 0) root.selectedId = root.rows[0].id
    Component.onCompleted: if (root.selectedId === "" && root.rows.length > 0) root.selectedId = root.rows[0].id
    // A closed form gives its keys back to the list: a hidden field never
    // keeps typing.
    onFormOpenChanged: if (!root.formOpen && form.activeFocus) list.forceActiveFocus()

    // The engine's argv for an act, Model.js's { args } or { error }.
    function argsFor(what) {
        if (what === "new") return Model.planArgs("new", form.form)
        if (what === "run") return Model.agentNewArgs(intentField.text)
        var id = what.indexOf("case:") === 0 ? what.slice(5) : ""
        var c = root.current
        var action = Model.caseDeskAction(c, id)
        if (!action) return { error: qsTr("This case has no such action now.") }
        if (action.review)
            return { error: qsTr("This window does not start an imported case: read its whole Intent in the editor (e), then run seldon plan start %1 in a terminal.").arg(c.id) }
        if (id === "agent") return Model.agentArgs(c.id)
        if (id === "focus") return Model.agentFocusArgs(c.id)
        // Open in editor: the `edit` row of CONTRACT.md (ADR-0058).
        if (id === "open") return Model.CASE_ID.test(c.id) ? { args: ["edit", c.id, "--editor", "--json"] } : { error: "Not a case id: " + c.id }
        var verb = Model.caseActionVerb(id)
        return verb !== "" ? Model.planArgs(verb, c.id) : { error: qsTr("This case has no such action now.") }
    }

    function sameArgs(a, b) {
        return JSON.stringify(a) === JSON.stringify(b)
    }

    function disarm() {
        root.armedWhat = ""
        root.armedArgs = []
        root.armedCase = ""
    }

    // The armed act's question, in the place it was asked.
    function armHint(what) {
        if (root.armedWhat !== what) return ""
        if (what === "new") return qsTr("Create the case “%1”? Press again to confirm, Esc cancels.").arg(form.title)
        if (what === "run") return qsTr("Create the case and start an agent on it? Press again to confirm, Esc cancels.")
        var action = Model.caseDeskAction(root.current, what.slice(5))
        if (!action) return ""
        return (action.final ? qsTr("%1 %2? Press again to confirm, Esc cancels. This is final.")
            : qsTr("%1 %2? Press again to confirm, Esc cancels.")).arg(qsTr(action.label)).arg(root.armedCase)
    }

    function say(ok, text, what) {
        root.result = { ok: ok, text: text, what: what }
        if (what === "new" && !ok) root.formError = text
    }

    // Press an act: "case:<action>", "new" or "run". Acts that write arm
    // on the first press and run on the second; Focus and Open at once.
    function press(what) {
        if (root.pending) return false
        if (what === "new") root.formError = ""
        var built = root.argsFor(what)
        if (built.error) {
            root.disarm()
            root.say(false, built.error, what)
            return false
        }
        if (backend.devMode) {
            root.disarm()
            root.say(false, qsTr("Dev mode is read-only: nothing is changed."), what)
            return false
        }
        var atOnce = what === "case:focus" || what === "case:open"
        if (!atOnce && root.armedWhat !== what) {
            // The argv before the act: `built` follows armedWhat and is
            // compared with this argv at once.
            root.armedArgs = built.args
            root.armedCase = root.current && what.indexOf("case:") === 0 ? root.current.id : ""
            root.armedWhat = what
            return true
        }
        // The second press runs what the first one armed, or nothing.
        if (!atOnce && !root.sameArgs(built.args, root.armedArgs)) {
            root.disarm()
            root.say(false, qsTr("Something changed after the first press; nothing ran."), what)
            return false
        }
        root.disarm()
        if (!backend.run(built.args)) {
            root.say(false, qsTr("The engine could not be asked."), what)
            return false
        }
        root.pendingArgs = built.args
        root.pendingWhat = what
        root.pendingCase = root.current && what.indexOf("case:") === 0 ? root.current.id : ""
        if (what === "new") root.sentTitle = form.title
        if (what === "run") root.sentIntent = intentField.text
        root.result = { ok: true, pending: true, text: qsTr("Working…"), what: what }
        return true
    }

    Connections {
        target: backend
        function onFinished(args, exitCode, output, errorOutput) {
            if (!root.pending || !root.sameArgs(args, root.pendingArgs)) return
            var what = root.pendingWhat
            root.pendingArgs = []
            root.pendingWhat = ""
            root.pendingCase = ""
            var r = args[0] === "plan" ? Model.planResult(exitCode, output, errorOutput)
                : args[0] === "edit" ? Model.openResult(exitCode, output, errorOutput)
                : args[1] === "focus" ? Model.focusResult(exitCode, output, errorOutput)
                : Model.agentResult(exitCode, output, errorOutput)
            root.say(r.ok, r.text, what)
            if (args[0] === "agent") backend.askSessions()
            if (!r.ok) return
            if (what === "new") {
                // Typed on meanwhile: the new text stays.
                if (form.title === root.sentTitle) form.reset()
                root.formOpen = false
            }
            if (what === "run" && intentField.text === root.sentIntent) intentField.text = ""
            // The case a New case, a sentence or a reopen made: select it
            // (it arrives with the index).
            if ((what === "new" || what === "run" || what === "case:reopen") && r.caseId) {
                root.filter = ""
                root.selectedId = r.caseId
            }
        }
        function onSessionsFinished(exitCode, output) {
            var found = Model.sessionsResult(exitCode, output)
            if (found !== null) root.sessions = found
        }
    }

    // An agent's window closed by hand ends its session without an index
    // change: look again while the view shows.
    Timer {
        interval: Model.SESSIONS_POLL_MS
        repeat: true
        running: root.active && backend.engineState === "present" && !backend.devMode
        onTriggered: backend.askSessions()
    }

    function rowIndex(id) {
        for (var i = 0; i < root.rows.length; i++) if (root.rows[i].id === id) return i
        return -1
    }

    function selectRow(i) {
        if (root.rows.length === 0) return
        var at = Math.max(0, Math.min(root.rows.length - 1, i))
        root.cursorRow = at
        root.selectedId = root.rows[at].id
        root.formOpen = false
        list.positionViewAtIndex(at, ListView.Contain)
    }

    function move(dy) {
        var at = root.rowIndex(root.selectedId)
        root.selectRow(at === -1 ? root.cursorRow + (dy > 0 ? 0 : -1) : at + dy)
    }

    // Enter and Space: in the form, Create; else the case's first action
    // that launches nothing (Enter never starts an agent).
    function activate() {
        if (root.formOpen) return root.press("new")
        var enter = Model.caseEnterAction(root.current)
        return enter ? root.press("case:" + enter.id) : false
    }

    // The view's letter keys; true when the key was the view's. While the
    // New case form is open the case behind it is hidden, so its keys
    // (a x r e) do nothing: no act is armed whose ask cannot be seen.
    function key(k) {
        if (root.formOpen && ["a", "x", "r", "e"].indexOf(k) !== -1) return true
        if (k === "a") {
            if (Model.caseDeskAction(root.current, "agent")) root.press("case:agent")
            else if (Model.caseDeskAction(root.current, "focus")) root.press("case:focus")
        } else if (k === "x") {
            if (Model.caseDeskAction(root.current, "drop")) root.press("case:drop")
        } else if (k === "r") {
            if (Model.caseDeskAction(root.current, "reopen")) root.press("case:reopen")
        } else if (k === "e") {
            if (Model.caseDeskAction(root.current, "open")) root.press("case:open")
        } else if (k === "i") {
            root.disarm()
            intentField.forceActiveFocus()
        } else if (k === "n") {
            root.openForm()
        } else if (k === "/") {
            root.searchOpen = true
            searchField.forceActiveFocus()
        } else if (k === "b") {
            root.filter = root.filter === "" ? Model.COMPLETED_FILTER_AGENT : ""
        } else {
            return false
        }
        return true
    }

    function openForm() {
        root.disarm()
        root.formOpen = true
        Qt.callLater(form.focusTitle)
    }

    function clearSearch() {
        root.search = ""
        searchField.text = ""
        root.searchOpen = false
        list.forceActiveFocus()
    }

    // Esc: out of a field, then disarm, then close the form, then clear
    // the search. False when there was nothing to do.
    function leave() {
        if (intentField.activeFocus || searchField.activeFocus) {
            list.forceActiveFocus()
            return true
        }
        if (form.leave()) {
            list.forceActiveFocus()
            return true
        }
        if (root.armedWhat !== "") {
            root.disarm()
            return true
        }
        if (root.formOpen) {
            root.formOpen = false
            return true
        }
        if (root.search !== "" || root.searchOpen) {
            root.clearSearch()
            return true
        }
        return false
    }

    function groupName(id) {
        return id === "active" ? qsTr("Active") : id === "verification" ? qsTr("Verification")
            : id === "queued" ? qsTr("Queued") : qsTr("Completed")
    }

    // "ACTIVE · 2", "COMPLETED · 1 / 2" while the filter or the search
    // hides some or the index lists fewer than the summary counts.
    function groupHeader(id) {
        translator.language
        var shown = root.work.counts[id] || 0
        var total = root.work.totals[id] || 0
        return root.groupName(id).toLocaleUpperCase() + " · " + shown + (shown !== total ? " / " + total : "")
    }

    function gapsOf(list) {
        return (list || []).map(function(g) { return { header: root.groupHeader(g.group), command: g.command } })
    }

    readonly property string wipText: !root.index ? ""
        : root.wip.tone === "urgent" ? qsTr("%1 / %2 active · over the limit").arg(root.wip.active).arg(root.wip.limit)
        : root.wip.tone === "accent" ? qsTr("%1 / %2 active · at the limit").arg(root.wip.active).arg(root.wip.limit)
        : qsTr("%1 / %2 active").arg(root.wip.active).arg(root.wip.limit)

    RowLayout {
        anchors.fill: parent
        spacing: 0

        ColumnLayout {
            id: listColumn
            objectName: "planListColumn"
            // A layout fills by default; the detail takes the rest.
            Layout.fillWidth: false
            Layout.preferredWidth: Math.max(360, root.width * 0.42)
            Layout.fillHeight: true
            spacing: 8

            // The one-sentence start (ADR-0027 §6): the engine makes the
            // case from the sentence and starts an agent on it.
            RowLayout {
                objectName: "planIntentRow"
                Layout.fillWidth: true
                Layout.topMargin: 16
                Layout.leftMargin: 16
                Layout.rightMargin: 16
                spacing: 8

                TextField {
                    id: intentField
                    objectName: "intentField"
                    Layout.fillWidth: true
                    placeholderText: qsTr("New case: say what to do")
                    // Read-only, not disabled, while the engine works: the
                    // field keeps the keys (a refusal leaves the sentence
                    // there to fix), so typing on never reaches the view's
                    // keys.
                    readOnly: root.pending
                    Material.roundedScale: theme.radius
                    onTextEdited: if (root.armedWhat === "run") root.disarm()
                    // Once per press: a held Enter's repeat does nothing.
                    Keys.onReturnPressed: function(event) { event.accepted = true; if (!event.isAutoRepeat) root.press("run") }
                    Keys.onEnterPressed: function(event) { event.accepted = true; if (!event.isAutoRepeat) root.press("run") }
                }
                SeldonButton {
                    objectName: "runButton"
                    text: root.armedWhat === "run" ? qsTr("Press again to confirm")
                        : root.pending && root.pendingWhat === "run" ? qsTr("Working…") : qsTr("Run")
                    highlighted: root.armedWhat === "run"
                    enabled: !root.pending && intentField.text.trim() !== ""
                    focusPolicy: Qt.NoFocus
                    onClicked: root.press("run")
                }
            }
            Label {
                objectName: "runHint"
                Layout.fillWidth: true
                Layout.leftMargin: 16
                Layout.rightMargin: 16
                visible: text !== ""
                text: root.armHint("run")
                color: root.tones.accentText
                wrapMode: Text.Wrap
                textFormat: Text.PlainText
            }

            // The WIP line ("2 / 3 active", warns, never blocks), then the
            // buttons, which wrap.
            Label {
                objectName: "wipLine"
                Layout.fillWidth: true
                Layout.leftMargin: 16
                Layout.rightMargin: 16
                text: root.wipText
                color: root.wip.tone === "urgent" ? root.tones.urgentText : root.wip.tone === "accent" ? root.tones.accentText : theme.foreground
                font.bold: true
                wrapMode: Text.Wrap
                textFormat: Text.PlainText
            }
            Flow {
                objectName: "planButtons"
                Layout.fillWidth: true
                Layout.leftMargin: 16
                Layout.rightMargin: 16
                spacing: 8

                SeldonButton {
                    objectName: "byAgent"
                    text: qsTr("By agent")
                    checkable: true
                    checked: root.filter === Model.COMPLETED_FILTER_AGENT
                    highlighted: checked
                    focusPolicy: Qt.NoFocus
                    onClicked: root.filter = root.filter === "" ? Model.COMPLETED_FILTER_AGENT : ""
                }
                SeldonButton {
                    objectName: "newCaseButton"
                    text: qsTr("New case")
                    checkable: true
                    checked: root.formOpen
                    highlighted: checked
                    focusPolicy: Qt.NoFocus
                    onClicked: root.formOpen ? (root.formOpen = false) : root.openForm()
                }
            }
            TextField {
                id: searchField
                objectName: "searchField"
                Layout.fillWidth: true
                Layout.leftMargin: 16
                Layout.rightMargin: 16
                visible: root.searchOpen || root.search !== ""
                placeholderText: qsTr("Search the cases")
                Material.roundedScale: theme.radius
                onTextEdited: root.search = text
                Keys.onReturnPressed: function(event) { event.accepted = true; list.forceActiveFocus() }
                Keys.onEnterPressed: function(event) { event.accepted = true; list.forceActiveFocus() }
            }
            // The engine's answer to the last act; a refused New case shows
            // in the form, beside its fields.
            Label {
                objectName: "planResult"
                Layout.fillWidth: true
                Layout.leftMargin: 16
                Layout.rightMargin: 16
                visible: text !== ""
                text: root.result && !(root.formOpen && root.result.what === "new") ? root.result.text : ""
                color: root.result && !root.result.ok ? root.tones.urgentText : root.tones.dim
                wrapMode: Text.Wrap
                maximumLineCount: 3
                elide: Text.ElideRight
                textFormat: Text.PlainText
            }

            // The list, and over it the list's data state.
            Item {
                Layout.fillWidth: true
                Layout.fillHeight: true

                ListView {
                    id: list
                    objectName: "planList"
                    anchors.fill: parent
                    clip: true
                    focus: true
                    model: root.rows
                    currentIndex: root.rowIndex(root.selectedId)
                    keyNavigationEnabled: false
                    boundsBehavior: Flickable.StopAtBounds
                    ScrollBar.vertical: ScrollBar {}

                    delegate: CaseRow {
                        required property var modelData
                        required property int index
                        width: ListView.view.width
                        row: modelData
                        tones: root.tones
                        selected: modelData.id === root.selectedId && !root.formOpen
                        header: index === 0 || root.rows[index - 1].group !== modelData.group ? root.groupHeader(modelData.group) : ""
                        gaps: root.gapsOf(root.work.lead[index])
                        working: modelData.status === "active" && !!root.sessions[modelData.id]
                        onClicked: {
                            root.cursorRow = index
                            root.selectedId = modelData.id
                            root.formOpen = false
                        }
                    }

                    // The groups after the last row the index lists none of.
                    footer: Column {
                        width: ListView.view ? ListView.view.width : 0
                        Repeater {
                            model: root.gapsOf(root.work.tail)
                            delegate: GroupGap {
                                required property var modelData
                                width: parent.width
                                gap: modelData
                                tones: root.tones
                            }
                        }
                    }
                }

                // The list's data state: no cases yet with New case, or no
                // match with Clear search.
                ColumnLayout {
                    objectName: "planEmpty"
                    anchors.centerIn: parent
                    width: parent.width - 48
                    visible: root.listState.text !== "" && !!root.index
                    spacing: 12

                    Label {
                        objectName: "planEmptyText"
                        Layout.fillWidth: true
                        horizontalAlignment: Text.AlignHCenter
                        wrapMode: Text.Wrap
                        color: root.tones.dim
                        textFormat: Text.PlainText
                        text: root.search.trim() !== "" ? qsTr("Nothing here matches “%1”.").arg(root.search.trim())
                            : qsTr("No cases yet. Say what should happen above, or make one by hand with New case.")
                    }
                    SeldonButton {
                        objectName: "planEmptyAction"
                        Layout.alignment: Qt.AlignHCenter
                        visible: !!root.listState.action
                        text: root.listState.action && root.listState.action.id === "clearSearch" ? qsTr("Clear search") : qsTr("New case")
                        focusPolicy: Qt.NoFocus
                        onClicked: root.listState.action.id === "clearSearch" ? root.clearSearch() : root.openForm()
                    }
                }
            }
        }

        Rectangle {
            Layout.fillHeight: true
            implicitWidth: 1
            color: root.tones.divider
        }

        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true

            CaseDetail {
                id: detail
                objectName: "caseDetail"
                anchors.fill: parent
                visible: !root.formOpen && !!root.detailData
                detail: root.detailData
                actions: root.actions
                tones: root.tones
                armedAction: root.armedAction
                pending: root.pending
                pendingAction: root.pending && root.pendingCase === root.selectedId && root.pendingWhat.indexOf("case:") === 0
                    ? root.pendingWhat.slice(5) : ""
                hint: root.armedAction !== "" ? root.armHint(root.armedWhat)
                    : root.current && root.current.imported && Model.caseDeskAction(root.current, "start")
                        ? qsTr("This window does not start an imported case: read its whole Intent in the editor (e), then run seldon plan start %1 in a terminal.").arg(root.current.id)
                    : ""
                onPressed: function(actionId) { root.press("case:" + actionId) }
            }

            NewCaseForm {
                id: form
                objectName: "newCaseForm"
                anchors.fill: parent
                anchors.margins: 20
                visible: root.formOpen
                tones: root.tones
                armed: root.armedWhat === "new"
                pending: root.pending && root.pendingWhat === "new"
                hint: root.armHint("new")
                error: root.formError
                onPressed: root.press("new")
                onCloseRequested: root.formOpen = false
            }

            Label {
                anchors.centerIn: parent
                visible: !root.formOpen && !root.detailData
                text: qsTr("Select a case to see it here.")
                color: root.tones.dim
                textFormat: Text.PlainText
            }
        }
    }
}
