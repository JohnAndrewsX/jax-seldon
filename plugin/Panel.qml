import QtQuick
import qs.Commons
import qs.Ui
import "components"
import "Model.js" as Model

// The bar panel (SPEC-PLUGIN §5), opened by a left click on the pill or by
// the `jax.seldon.panel` IPC target that BarWidget.qml owns.
//
// Skeleton (WP-010): the status banner with its fix, and the summary counts.
// The tabs (Today, Changelog, Work, Decisions, System, Memory) arrive in a
// later work package.
Panel {
  id: root
  moduleName: "jax.seldon"
  // BarWidget.qml owns the IPC target so it exists before this panel loads.
  manageIpc: false

  // Injected by BarWidget.qml.
  property var anchorItem: null
  property var hostWidget: null
  property var service: null

  // The bar identifies a panel by the widget in its slot (see the clock).
  readonly property var barIdentity: hostWidget || root

  readonly property color foreground: bar ? bar.foreground : Color.foreground
  readonly property color urgent: bar ? bar.urgent : Color.urgent
  readonly property string fontFamily: bar ? bar.fontFamily : Style.font.family
  readonly property var counts: service ? service.counts : null

  function captureNow() {
    if (root.service) root.service.captureNow()
  }

  KeyboardPanel {
    id: panel
    anchorItem: root.anchorItem
    owner: root.barIdentity
    bar: root.bar
    open: root.opened
    focusTarget: keyCatcher
    contentWidth: panel.fittedContentWidth(Style.space(360))
    contentHeight: panel.fittedContentHeight(column.implicitHeight)

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      onCloseRequested: root.close()
      onTabRequested: function(direction) { root.switchPanel(direction) }
      onTextKey: function(t) { if (t === "c" || t === "C") root.captureNow() }

      Column {
        id: column
        width: parent.width
        spacing: Style.spacing.panelGap

        Text {
          textFormat: Text.PlainText
          text: "Seldon"
          color: root.foreground
          font.family: root.fontFamily
          font.pixelSize: Style.font.heading
          font.bold: true
        }

        Banner {
          width: parent.width
          banner: root.service ? root.service.banner : null
          foreground: root.foreground
          urgent: root.urgent
          fontFamily: root.fontFamily
          onActionRequested: function(actionId) { if (root.service) root.service.fix(actionId) }
        }

        Text {
          width: parent.width
          visible: !root.service
          textFormat: Text.PlainText
          text: "The Seldon service is not running. Enable the plugin in Setup > Plugins."
          color: root.foreground
          font.family: root.fontFamily
          font.pixelSize: Style.font.body
          wrapMode: Text.Wrap
        }

        Column {
          width: parent.width
          visible: root.counts !== null
          spacing: Style.spacing.sm

          PanelSectionHeader {
            text: "SUMMARY"
            foreground: root.foreground
            fontFamily: root.fontFamily
          }

          Text {
            textFormat: Text.PlainText
            text: root.counts
              ? Model.plural(root.counts.active, "active case", "active cases") + " · " + root.counts.queued + " queued"
              : ""
            color: root.foreground
            font.family: root.fontFamily
            font.pixelSize: Style.font.body
          }

          Text {
            textFormat: Text.PlainText
            text: root.counts
              ? Model.plural(root.counts.drift, "unexplained change", "unexplained changes")
                + (root.counts.crisis > 0 ? " · " + root.counts.crisis + " in the red zone" : "")
              : ""
            color: root.counts && root.counts.crisis > 0 ? root.urgent : root.foreground
            font.family: root.fontFamily
            font.pixelSize: Style.font.body
          }

          Text {
            textFormat: Text.PlainText
            text: root.service && root.service.lastCapture !== ""
              ? "Last capture " + Model.relativeAge(Model.timeMs(root.service.lastCapture), root.service.nowMs)
              : "Never captured"
            color: Qt.darker(root.foreground, 1.3)
            font.family: root.fontFamily
            font.pixelSize: Style.font.bodySmall
          }
        }

        Text {
          width: parent.width
          visible: text !== ""
          textFormat: Text.PlainText
          text: root.service ? root.service.lastError : ""
          color: root.urgent
          font.family: root.fontFamily
          font.pixelSize: Style.font.bodySmall
          wrapMode: Text.Wrap
        }

        Text {
          width: parent.width
          visible: !!root.service && root.service.devMode
          textFormat: Text.PlainText
          text: root.service ? "Dev mode: " + root.service.indexPath : ""
          color: Qt.darker(root.foreground, 1.3)
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
          wrapMode: Text.WrapAnywhere
        }
      }
    }
  }
}
