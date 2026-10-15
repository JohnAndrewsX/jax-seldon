import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Material
import QtQuick.Layouts
import "Model.js" as Model

// New case, the hand-made one (SPEC-PLUGIN §5.4, the desk's sheet): a
// title, zone, risk and priority at the engine's defaults, an optional
// area. Create sends `seldon plan new --zone <z> --risk <r> [--area <a>]
// [--priority <p>] --json -- <title>` (Model.planArgs): the first press
// arms, the second runs. The fields keep their text until the engine has
// made the case, so a refusal (a blank title here, the lock held there)
// never loses what was typed; closing the form keeps it too.
//
// Keys: Tab walks title → zone → risk → priority → area; on a choice ←/→
// or h/l pick; Enter in a field (or on a choice) presses Create; Esc
// leaves the field, then disarms, then closes the form.
FocusScope {
    id: root

    property var tones: ({})
    property bool armed: false
    property bool pending: false
    property string hint: ""
    property string error: ""

    property alias title: titleField.text
    property alias area: areaField.text
    property string zone: Model.NEW_CASE_DEFAULTS.zone
    property string risk: Model.NEW_CASE_DEFAULTS.risk
    property string priority: Model.NEW_CASE_DEFAULTS.priority

    readonly property bool hasDraft: root.title.trim() !== "" || root.area.trim() !== ""
    readonly property bool formFocused: root.activeFocus
    readonly property var form: ({ title: root.title, zone: root.zone, risk: root.risk, area: root.area, priority: root.priority })

    signal pressed()
    signal closeRequested()

    // Return or Enter in a field: once per press; a held key's repeat
    // does nothing (SPEC-PLUGIN §5.3: no field acts on `accepted`, which a
    // repeat emits too).
    function enterPressed(event) {
        event.accepted = true
        if (!event.isAutoRepeat) root.pressed()
    }

    function focusTitle() {
        titleField.forceActiveFocus()
    }

    // Out of a field or choice; false when nothing had the focus.
    function leave() {
        if (!root.activeFocus) return false
        root.focus = false
        return true
    }

    function reset() {
        root.title = ""
        root.area = ""
        root.zone = Model.NEW_CASE_DEFAULTS.zone
        root.risk = Model.NEW_CASE_DEFAULTS.risk
        root.priority = Model.NEW_CASE_DEFAULTS.priority
    }

    implicitHeight: column.implicitHeight

    // One labelled choice: the words as tabs, the picked one underlined;
    // ←/→ or h/l move the pick while it has the focus.
    component Choice: FocusScope {
        id: choice

        property string label: ""
        property var options: []
        property string value: ""
        property var words: ({})
        signal picked(string value)

        function step(d) {
            var at = choice.options.indexOf(choice.value)
            var next = Math.max(0, Math.min(choice.options.length - 1, at + d))
            if (next !== at) choice.picked(choice.options[next])
        }

        activeFocusOnTab: true
        implicitHeight: row.implicitHeight
        Keys.onLeftPressed: choice.step(-1)
        Keys.onRightPressed: choice.step(1)
        Keys.onPressed: function(event) {
            if (event.key === Qt.Key_H) { choice.step(-1); event.accepted = true }
            else if (event.key === Qt.Key_L) { choice.step(1); event.accepted = true }
        }

        // The label over its words, which wrap in a narrow window.
        Column {
            id: row
            width: choice.width
            spacing: 4

            Label {
                width: row.width
                text: choice.label
                color: choice.activeFocus ? theme.foreground : root.tones.dim
                font.bold: choice.activeFocus
                elide: Text.ElideRight
                textFormat: Text.PlainText
            }
            Flow {
                width: row.width
                spacing: 8

                Repeater {
                    model: choice.options
                    delegate: SeldonButton {
                        required property string modelData
                        objectName: "choice-" + modelData
                        text: choice.words[modelData] || modelData
                        tab: true
                        checkable: true
                        checked: choice.value === modelData
                        enabled: !root.pending
                        focusPolicy: Qt.NoFocus
                        onClicked: choice.picked(modelData)
                    }
                }
            }
        }
        Rectangle {
            anchors.fill: parent
            anchors.margins: -4
            color: "transparent"
            radius: Math.min(4, theme.radius)
            border.width: choice.activeFocus ? 1 : 0
            border.color: root.tones.accentUi
        }
    }

    ColumnLayout {
        id: column
        width: root.width
        spacing: 12

        Label {
            text: qsTr("New case")
            font.pixelSize: 18
            font.bold: true
            color: theme.foreground
            textFormat: Text.PlainText
        }

        TextField {
            id: titleField
            objectName: "newCaseTitle"
            Layout.fillWidth: true
            placeholderText: qsTr("Title; Enter creates the case (twice to confirm)")
            // Read-only, not disabled, while the engine works: the field
            // keeps the keys, so typing on never reaches the view's keys.
            readOnly: root.pending
            Material.roundedScale: theme.radius
            Keys.onReturnPressed: function(event) { root.enterPressed(event) }
            Keys.onEnterPressed: function(event) { root.enterPressed(event) }
        }

        Choice {
            objectName: "newCaseZone"
            Layout.fillWidth: true
            label: qsTr("Zone")
            options: Model.ZONES
            words: ({ green: qsTr("green"), yellow: qsTr("yellow"), red: qsTr("red") })
            value: root.zone
            onPicked: function(value) { root.zone = value }
        }
        Choice {
            objectName: "newCaseRisk"
            Layout.fillWidth: true
            label: qsTr("Risk")
            options: Model.RISKS
            value: root.risk
            onPicked: function(value) { root.risk = value }
        }
        Choice {
            objectName: "newCasePriority"
            Layout.fillWidth: true
            label: qsTr("Priority")
            options: Model.PRIORITIES
            words: ({ high: qsTr("high"), normal: qsTr("normal"), low: qsTr("low") })
            value: root.priority
            onPicked: function(value) { root.priority = value }
        }

        TextField {
            id: areaField
            objectName: "newCaseArea"
            Layout.fillWidth: true
            placeholderText: qsTr("Area (optional): lowercase letters, digits and -")
            readOnly: root.pending
            Material.roundedScale: theme.radius
            Keys.onReturnPressed: function(event) { root.enterPressed(event) }
            Keys.onEnterPressed: function(event) { root.enterPressed(event) }
        }

        RowLayout {
            spacing: 12

            SeldonButton {
                objectName: "newCaseCreate"
                text: root.armed ? qsTr("Press again to confirm") : root.pending ? qsTr("Working…") : qsTr("Create")
                highlighted: true
                enabled: !root.pending
                focusPolicy: Qt.NoFocus
                onClicked: root.pressed()
            }
            SeldonButton {
                objectName: "newCaseClose"
                text: qsTr("Close")
                flat: true
                focusPolicy: Qt.NoFocus
                onClicked: root.closeRequested()
            }
        }

        Label {
            objectName: "newCaseHint"
            Layout.fillWidth: true
            visible: text !== ""
            text: root.hint
            color: root.tones.accentText
            wrapMode: Text.Wrap
            textFormat: Text.PlainText
        }
        Label {
            objectName: "newCaseError"
            Layout.fillWidth: true
            visible: text !== ""
            text: root.error
            color: root.tones.urgentText
            wrapMode: Text.Wrap
            textFormat: Text.PlainText
        }
    }
}
