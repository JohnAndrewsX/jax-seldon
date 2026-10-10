import QtQuick
import qs.Commons
import "../Tones.js" as Tones

// The text and UI tones of the active theme (SPEC-PLUGIN §7 "Theming",
// WP-177), as one object per component: `readonly property Tone tone:
// Tone {}`, then `color: root.tone.dim`. Derived by Model.deskTones from
// the desk's surface (Color.popups.*) and Style's fills; every instance
// shares one result per theme (Tones.js). Text in a role colour takes the
// tone, never the raw role; stripes, bars of state, charts and the pill
// keep the raw roles.
QtObject {
  readonly property var tones: Tones.of(Color.popups.text, Color.popups.background, Color.accent, Color.urgent, Color.muted,
    Style.normalFillFor(Color.popups.text, Color.accent), Style.hoverFillFor(Color.popups.text, Color.accent),
    Style.selectedFillFor(Color.popups.text, Color.accent), Style.focusBorderFor(Color.popups.text, Color.accent),
    Style.focusBorderWidth)

  // Secondary text (meta lines, captions, labels, hints).
  readonly property color dim: tones.dim
  // Text in the accent and in the urgent colour.
  readonly property color accentText: tones.accentText
  readonly property color urgentText: tones.urgentText
  // The selection's accent bar (≥ 3:1).
  readonly property color accentUi: tones.accentUi
  // A line or a ring that has to be seen (≥ 3:1).
  readonly property color ui: tones.ui
  // The ring on Seldon's own controls while they have the keys: the
  // theme's focus border where it reaches 3:1, else `ui` at two pixels.
  readonly property bool themeFocus: tones.themeFocus
  readonly property color focusRing: tones.focusRing
  readonly property int focusRingWidth: themeFocus ? Style.focusBorderWidth : Math.max(2, Style.space(2))
  // The hairlines between header, notices, list and detail.
  readonly property color divider: tones.divider
}
