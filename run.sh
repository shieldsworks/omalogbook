#!/usr/bin/env bash
# Opens the ship's log in a window of its own, from this checkout, in its
# own Quickshell process. The window reads the day through omalogbook
# (OMALOGBOOK_BIN, default this checkout's release build); the log itself
# only keeps the watch when `omalogbook run` is going.
set -euo pipefail
cd "$(dirname "$0")"
exec quickshell -p ui/shell.qml "$@"
