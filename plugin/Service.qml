import QtQuick

// Headless data service for jax.seldon (SPEC-PLUGIN §3).
//
// Stub (WP-001): loads and exposes the state surface the bar widget and
// overlay will bind to. Watching index.json and running the engine arrive
// in later work packages.
Item {
  id: root

  // Injected by omarchy-shell (capability-scoped facade for third parties).
  property var shell: null
  property var manifest: null

  // ok | engineMissing | notInitialised | indexMissing | indexStale | contractMismatch
  property string status: "indexMissing"
  property var index: null
  readonly property int contractVersion: 1
}
