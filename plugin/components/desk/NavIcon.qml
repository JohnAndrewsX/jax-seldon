import QtQuick
import QtQuick.Shapes
import qs.Commons

// A section icon: one 24-unit SVG path (Model.DESK_SECTIONS[].icon, from the
// approved prototype) stroked in `color`, scaled to the item's size. No font
// glyph, so it looks the same with every Omarchy font.
Item {
  id: root

  property string path: ""
  property color color: Color.foreground

  implicitWidth: Style.space(16)
  implicitHeight: Style.space(16)

  Shape {
    width: 24
    height: 24
    scale: Math.min(root.width, root.height) / 24
    transformOrigin: Item.TopLeft
    preferredRendererType: Shape.CurveRenderer

    ShapePath {
      strokeColor: root.color
      strokeWidth: 1.6
      fillColor: "transparent"
      capStyle: ShapePath.RoundCap
      joinStyle: ShapePath.RoundJoin
      PathSvg { path: root.path }
    }
  }
}
