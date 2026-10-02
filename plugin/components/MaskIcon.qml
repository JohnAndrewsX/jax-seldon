import QtQuick
import QtQuick.Window
import Quickshell.Io
import qs.Commons
import "../Model.js" as Model

// One Prime Radiant mask from plugin/assets/ (the bar glyph, the header
// mark, a state pictogram, a timeline marker) painted in a theme colour.
//
// The masks are `currentColor` with their fallback colour on the <svg> root
// (assets/README.md), so setting the root's `color` tints the whole file —
// the tint the designer built them for (design round 3, F1). The file is
// read once through FileView (asynchronous, like index.json) and handed to
// Image as a data URL with the theme colour on the root (Model.tintedSvg).
// That works the same in the shell and in the software renderer of the
// headless harness, where MultiEffect colorization paints nothing. Nothing
// here names a colour: `color` comes from Color/Style or the bar.
//
// `crisp` is for a hand-hinted pixel grid drawn at its own size: no
// smoothing, decoded at device pixels.
Item {
  id: root

  // A file name in plugin/assets/, e.g. "a4-bar-glyph-16.svg"; "" draws nothing.
  property string file: ""
  property color color: Color.foreground
  property bool crisp: false

  property string svgText: ""
  readonly property bool ready: image.status === Image.Ready
  readonly property string rgb: Qt.rgba(root.color.r, root.color.g, root.color.b, 1).toString()

  function localPath(name) {
    return name === "" ? "" : decodeURIComponent(Qt.resolvedUrl("../assets/" + name).toString().replace(/^file:\/\//, ""))
  }

  onFileChanged: root.svgText = ""

  FileView {
    id: asset
    path: root.localPath(root.file)
    printErrors: false
    onLoaded: root.svgText = asset.text()
    onLoadFailed: root.svgText = ""
  }

  Image {
    id: image
    anchors.fill: parent
    visible: root.file !== ""
    source: Model.tintedSvg(root.svgText, root.rgb)
    sourceSize.width: Math.max(1, Math.round(root.width * Screen.devicePixelRatio))
    sourceSize.height: Math.max(1, Math.round(root.height * Screen.devicePixelRatio))
    fillMode: Image.PreserveAspectFit
    smooth: !root.crisp
    cache: false
    opacity: root.color.a
  }
}
