#!/bin/bash
# Seldon: record an Omarchy theme change the moment it happens (optional).
#
# Installed only when the user opts in during `seldon init`, with
#   omarchy hook install theme-set <this file>
# which copies it into Omarchy's theme-set.d hook directory.
# omarchy-theme-set runs it as `bash <file> <theme-slug>` after it has
# written the new slug to theme.name. See engine/hooks/README.md.
#
# Without this hook the theme collector still finds the change on the next
# `seldon capture`; with it, the event carries the time of the switch and
# the collector does not record it a second time.
#
# It must never get in the way of a theme switch: no output, always exit 0.
# The slug is passed as one argument, never evaluated.

[[ -n ${1:-} ]] || exit 0
command -v seldon >/dev/null 2>&1 || exit 0
seldon event theme theme-set --subject "$1" >/dev/null 2>&1
exit 0
