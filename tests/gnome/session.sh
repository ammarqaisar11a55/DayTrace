#!/usr/bin/env bash
# Starts a disposable headless GNOME Shell with the DayTrace extension enabled,
# then runs the given command inside it. Nothing touches the real desktop: the
# session has its own D-Bus, Wayland socket, home and settings.
#
# usage: tests/gnome/session.sh [--no-extension] command [arguments...]
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
uuid=daytrace@ammarqaisar11a55.github.io
helper=daytrace-test-helper@ammarqaisar11a55.github.io
install=1
if [ "${1:-}" = "--no-extension" ]; then install=0; shift; fi

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
export HOME=$work/home XDG_DATA_HOME=$work/data XDG_CONFIG_HOME=$work/config XDG_CACHE_HOME=$work/cache
export XDG_RUNTIME_DIR=$work/run DAYTRACE_TEST_WORK=$work
mkdir -p "$HOME" "$XDG_DATA_HOME" "$XDG_CONFIG_HOME" "$XDG_CACHE_HOME"
mkdir -m 700 "$XDG_RUNTIME_DIR"

mkdir -p "$XDG_DATA_HOME/gnome-shell/extensions/$helper"
cp "$root"/tests/gnome/helper/* "$XDG_DATA_HOME/gnome-shell/extensions/$helper/"

if [ "$install" = 1 ]; then
  extension=$XDG_DATA_HOME/gnome-shell/extensions/$uuid
  mkdir -p "$extension"
  cp "$root/extensions/gnome-shell/metadata.json" "$root/extensions/gnome-shell/extension.js" "$extension/"
fi

unset DISPLAY WAYLAND_DISPLAY WAYLAND_SOCKET SESSION_MANAGER
export XDG_SESSION_TYPE=wayland XDG_CURRENT_DESKTOP=GNOME GDK_BACKEND=wayland
export GNOME_TEST_INSTALL=$install GNOME_TEST_UUID=$uuid GNOME_TEST_HELPER=$helper

exec dbus-run-session -- bash -c '
  set -u
  enabled="\"$GNOME_TEST_HELPER\""
  if [ "$GNOME_TEST_INSTALL" = 1 ]; then enabled="$enabled, \"$GNOME_TEST_UUID\""; fi
  gsettings set org.gnome.shell enabled-extensions "[$enabled]"
  gsettings set org.gnome.shell disable-user-extensions false
  gsettings set org.gnome.shell welcome-dialog-last-shown-version "999"

  gnome-shell --headless --wayland --no-x11 --wayland-display=wayland-daytrace \
    --virtual-monitor 1280x800 >"$DAYTRACE_TEST_WORK/shell.log" 2>&1 &
  shell=$!
  export WAYLAND_DISPLAY=wayland-daytrace

  for _ in $(seq 1 100); do
    if gdbus call --session -d org.gnome.Shell -o /org/gnome/Shell \
        -m org.freedesktop.DBus.Properties.Get org.gnome.Shell ShellVersion >/dev/null 2>&1; then
      break
    fi
    sleep 0.2
  done
  sleep 1.5

  "$@"
  status=$?
  kill "$shell" 2>/dev/null
  wait "$shell" 2>/dev/null
  exit $status
' daytrace-session "$@"
