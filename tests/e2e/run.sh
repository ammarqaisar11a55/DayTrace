#!/usr/bin/env bash
# Runs the end-to-end suite against a built binary, in a disposable home and
# D-Bus session on a virtual display. It never touches real user data.
#
# Needs: DAYTRACE_BINARY, tauri-driver, WebKitWebDriver and Xvfb.
set -euo pipefail

root=$(cd "$(dirname "$0")" && pwd)
work=$(mktemp -d)
shots=${DAYTRACE_SHOTS:-$work/screenshots}
display=:${DAYTRACE_DISPLAY:-93}
webdriver=${WEBKIT_WEBDRIVER:-WebKitWebDriver}

cleanup() {
  jobs -p | xargs -r kill 2>/dev/null || true
  rm -rf "$work"
}
trap cleanup EXIT

export HOME="$work/home" XDG_DATA_HOME="$work/data" XDG_CONFIG_HOME="$work/config" XDG_CACHE_HOME="$work/cache"
mkdir -p "$HOME" "$XDG_DATA_HOME" "$XDG_CONFIG_HOME" "$XDG_CACHE_HOME"

Xvfb "$display" -screen 0 1280x860x24 -nolisten tcp >/dev/null 2>&1 &
sleep 1

export DISPLAY=$display GDK_BACKEND=x11 XDG_SESSION_TYPE=x11 XDG_CURRENT_DESKTOP=test
export WEBKIT_DISABLE_COMPOSITING_MODE=1 WEBKIT_DISABLE_DMABUF_RENDERER=1
unset WAYLAND_DISPLAY
export DAYTRACE_DATABASE="$XDG_DATA_HOME/io.github.ammarqaisar11a55.DayTrace/daytrace.db"
export DAYTRACE_SHOTS=$shots

dbus-run-session -- bash -c "
  tauri-driver --native-driver \"\$(command -v $webdriver)\" >/dev/null 2>&1 &
  sleep 1
  node '$root/run.mjs'
"
