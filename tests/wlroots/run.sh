#!/usr/bin/env bash
# Tests the wlroots backend against a real headless sway, in a disposable home.
#
# usage: tests/wlroots/run.sh <probe binary>
#
# Build the probe with: cargo build -p daytrace-core --example probe
set -euo pipefail

probe=$1
work=$(mktemp -d)
pids=()
cleanup() {
  kill "${pids[@]}" 2>/dev/null || true
  rm -rf "$work"
}
trap cleanup EXIT

export HOME=$work/home XDG_DATA_HOME=$work/data XDG_CONFIG_HOME=$work/config XDG_CACHE_HOME=$work/cache
export XDG_RUNTIME_DIR=$work/run
mkdir -p "$HOME" "$XDG_DATA_HOME" "$XDG_CONFIG_HOME" "$XDG_CACHE_HOME"
mkdir -m 700 "$XDG_RUNTIME_DIR"
unset DISPLAY WAYLAND_DISPLAY SWAYSOCK

export WLR_BACKENDS=headless WLR_RENDERER=pixman WLR_LIBINPUT_NO_DEVICES=1 WLR_HEADLESS_OUTPUTS=1
export XDG_SESSION_TYPE=wayland XDG_CURRENT_DESKTOP=sway GDK_BACKEND=wayland

echo 'output HEADLESS-1 resolution 1280x800' >"$work/config.sway"
sway -c "$work/config.sway" >"$work/sway.log" 2>&1 &
pids+=($!)
sleep 2

WAYLAND_DISPLAY=$(find "$XDG_RUNTIME_DIR" -maxdepth 1 -name 'wayland-[0-9]' -printf '%f\n' | head -1)
SWAYSOCK=$(find "$XDG_RUNTIME_DIR" -maxdepth 1 -name 'sway-ipc.*' | head -1)
export WAYLAND_DISPLAY SWAYSOCK

gnome-calculator >/dev/null 2>&1 &
pids+=($!)
sleep 1
gnome-text-editor >/dev/null 2>&1 &
pids+=($!)
sleep 3

"$probe" 10 --titles >"$work/probe.log" 2>&1 &
pids+=($!)
sleep 4
swaymsg '[app_id="org.gnome.Calculator"] focus' >/dev/null
sleep 3
swaymsg '[app_id="org.gnome.TextEditor"] focus' >/dev/null
sleep 4

check() {
  if grep -q "$1" "$work/probe.log"; then
    echo "ok - $2"
  else
    echo "FAILED - $2"
    cat "$work/probe.log"
    exit 1
  fi
}

check '"backend": "wlr-foreign-toplevel"' "the wlroots backend is selected"
check 'id=org.gnome.calculator' "Calculator is identified"
check 'id=org.gnome.texteditor' "Text Editor is identified"
check 'FocusChanged' "focus changes wake the runtime"
check 'idle_ms=Some' "idle time is reported through ext-idle-notify"
check 'does not expose which process owns a window' "the missing program path is reported honestly"
echo "wlroots checks passed"
