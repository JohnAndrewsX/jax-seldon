import QtQuick

// Arm twice (SPEC-PLUGIN §5): a writing action asks for a second press.
// press(id) arms on the first press and returns false; the same id again
// returns true (go) and disarms. The desk disarms on every key the press
// did not come from (`touched` tells it), so any other key cancels; the
// sticky action bar shows `hint` while armed. Shared by the sections.
QtObject {
  id: root

  property string armedId: ""
  property string hint: ""
  // Set by press(); the desk clears it before a key and disarms after a
  // key that did not press.
  property bool touched: false

  function press(id, hintText) {
    root.touched = true
    if (root.armedId === id) {
      root.disarm()
      return true
    }
    root.armedId = id
    root.hint = hintText || "Press again to confirm"
    return false
  }

  function isArmed(id) {
    return root.armedId !== "" && root.armedId === id
  }

  function disarm() {
    root.armedId = ""
    root.hint = ""
  }
}
