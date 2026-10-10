.pragma library
.import "Model.js" as Model

// The desk's tones, shared by the whole plugin (WP-177). A library: QML
// loads it once, so every components/Tone.qml reads the same memo in this
// one copy of Model.js and the derivation (Model.deskTones) runs once per
// theme change, not once per row.

function of(foreground, background, accent, urgent, muted, normal, hover, selected, focusBorder, focusBorderWidth) {
  return Model.deskTones({
    foreground: foreground, background: background, accent: accent, urgent: urgent, muted: muted,
    normal: normal, hover: hover, selected: selected, focusBorder: focusBorder, focusBorderWidth: focusBorderWidth
  })
}
