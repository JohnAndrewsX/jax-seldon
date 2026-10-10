import QtQuick
import qs.Commons
import ".."
import "../../Model.js" as Model

// The notices under the desk's header (ADR-0034 §2: today's banner states,
// with their one-click fixes): the restart notice after a plugin update
// (WP-090), the status banner (WP-010), the engine newer than the plugin
// (ADR-0051), snapper not readable (ADR-0026),
// the outdated agent rules (WP-101) and what their update did (WP-111),
// and the last capture's warnings (WP-085). The service builds them
// (Service.qml); a click on a fix goes back to Service.fix with the
// banner's id. The header's chip folds the whole strip. The setup states
// (engine not installed yet, logbook not created, snapshots not readable)
// are not notices while the setup card in Today shows them (WP-119,
// Model.isSetupNotice), and the snapshot one not after its Not now.
Column {
  id: root

  property var service: null
  property bool folded: false
  property color foreground: Color.popups.text
  property color urgent: Color.urgent
  property string fontFamily: Style.font.family

  // The notices shown, in order: { id, banner }.
  readonly property var items: {
    var s = root.service
    if (!s) return []
    var all = [
      { id: "restart", banner: s.restartNotice },
      { id: "status", banner: root.shown("status", s.banner) },
      { id: "contract", banner: s.contractNotice },
      { id: "snapper", banner: root.shown("snapper", s.snapperBanner) },
      { id: "rules", banner: s.rulesBanner },
      { id: "rulesNotice", banner: s.rulesNotice },
      { id: "capture", banner: s.captureNotice }
    ]
    return all.filter(function(n) { return !!n.banner })
  }

  // The banner, or null while the setup card stands for it.
  function shown(id, banner) {
    var s = root.service
    return s && Model.isSetupNotice(id, banner, s.setup, s.setupLater) ? null : banner
  }
  readonly property var statusBanner: statusItem
  readonly property var snapperBanner: snapperItem
  readonly property var captureBanner: captureItem

  function fix(id, actionId) {
    if (root.service) root.service.fix(actionId, id)
  }

  spacing: Style.spacing.md
  visible: root.items.length > 0 && !root.folded
  topPadding: visible ? Style.spacing.lg : 0
  bottomPadding: visible ? Style.spacing.lg : 0
  leftPadding: Style.spacing.huge
  rightPadding: Style.spacing.huge

  Banner {
    width: root.width - root.leftPadding - root.rightPadding
    banner: root.service ? root.service.restartNotice : null
    foreground: root.foreground
    urgent: root.urgent
    fontFamily: root.fontFamily
    onActionRequested: function(actionId) { root.fix("restart", actionId) }
  }

  Banner {
    id: statusItem
    width: root.width - root.leftPadding - root.rightPadding
    banner: root.service ? root.shown("status", root.service.banner) : null
    foreground: root.foreground
    urgent: root.urgent
    fontFamily: root.fontFamily
    onActionRequested: function(actionId) { root.fix("status", actionId) }
  }

  Banner {
    width: root.width - root.leftPadding - root.rightPadding
    banner: root.service ? root.service.contractNotice : null
    foreground: root.foreground
    urgent: root.urgent
    fontFamily: root.fontFamily
    onActionRequested: function(actionId) { root.fix("contract", actionId) }
  }

  Banner {
    id: snapperItem
    width: root.width - root.leftPadding - root.rightPadding
    banner: root.service ? root.shown("snapper", root.service.snapperBanner) : null
    foreground: root.foreground
    urgent: root.urgent
    fontFamily: root.fontFamily
    onActionRequested: function(actionId) { root.fix("snapper", actionId) }
  }

  Banner {
    width: root.width - root.leftPadding - root.rightPadding
    banner: root.service ? root.service.rulesBanner : null
    foreground: root.foreground
    urgent: root.urgent
    fontFamily: root.fontFamily
    onActionRequested: function(actionId) { root.fix("rules", actionId) }
  }

  Banner {
    width: root.width - root.leftPadding - root.rightPadding
    banner: root.service ? root.service.rulesNotice : null
    foreground: root.foreground
    urgent: root.urgent
    fontFamily: root.fontFamily
  }

  Banner {
    id: captureItem
    width: root.width - root.leftPadding - root.rightPadding
    banner: root.service ? root.service.captureNotice : null
    foreground: root.foreground
    urgent: root.urgent
    fontFamily: root.fontFamily
  }
}
