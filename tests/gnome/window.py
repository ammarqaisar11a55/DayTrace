"""Window control test on a real GNOME Wayland session.

Run through session.sh, which provides a disposable headless GNOME Shell:

    tests/gnome/session.sh --no-extension python3 -I tests/gnome/window.py <daytrace binary>

DayTrace draws its own title bar. This test moves a virtual pointer through
Mutter, so the clicks, drags and edge resizes are real input events, and it
reads the window state back from Mutter rather than from the application.
"""
import json
import os
import re
import subprocess
import sys
import time

BINARY = sys.argv[1]
ENV = {**os.environ, "WEBKIT_DISABLE_COMPOSITING_MODE": "1", "WEBKIT_DISABLE_DMABUF_RENDERER": "1"}
TITLE_BAR = 44
# Below the notification that announces unsafe mode.
X, Y = 60, 170
CONTROL = 46
MIN_WIDTH, MIN_HEIGHT = 860, 560


def evaluate(script):
    # Eval is refused until the helper extension has enabled unsafe mode.
    for _ in range(150):
        result = subprocess.run(
            ["gdbus", "call", "--session", "-d", "org.gnome.Shell", "-o", "/org/gnome/Shell",
             "-m", "org.gnome.Shell.Eval", script],
            capture_output=True, text=True,
        )
        match = re.match(r"\((true|false), '(.*)'\)$", result.stdout.strip(), re.S)
        if match and match.group(1) == "true":
            break
        time.sleep(0.2)
    else:
        raise SystemExit(f"Eval failed: {result.stdout or result.stderr}")
    # GVariant quotes the result, and Eval returns it as JSON, so a script
    # that returns JSON.stringify(...) comes back encoded twice.
    text = re.sub(r"\\(.)", r"\1", match.group(2))
    value = json.loads(text) if text else None
    return json.loads(value) if isinstance(value, str) and value else value


WINDOW = "global.get_window_actors().map(a => a.meta_window).find(w => w.get_title() === 'DayTrace')"
POINTER = """
(() => {
  const { Clutter, GLib } = imports.gi;
  const seat = Clutter.get_default_backend().get_default_seat();
  if (!global._daytracePointer)
    global._daytracePointer = seat.create_virtual_device(Clutter.InputDeviceType.POINTER_DEVICE);
  return { seat, pointer: global._daytracePointer, now: () => GLib.get_monotonic_time() };
})()
"""


def state():
    return evaluate(f"""
      (() => {{
        const w = {WINDOW};
        if (!w) return JSON.stringify(null);
        const r = w.get_frame_rect();
        const maximized = typeof w.is_maximized === 'function'
          ? w.is_maximized()
          : w.maximized_horizontally && w.maximized_vertically;
        return JSON.stringify({{ x: r.x, y: r.y, width: r.width, height: r.height,
          maximized, minimized: w.minimized, decorated: w.decorated }});
      }})()
    """)


def color_at(x, y):
    """The screen color at a point, read back from GNOME Shell."""
    evaluate(f"""
      (() => {{
        global._daytraceColor = null;
        const shot = new imports.gi.Shell.Screenshot();
        shot.pick_color({x}, {y}, (source, result) => {{
          const [, color] = source.pick_color_finish(result);
          global._daytraceColor = [color.red, color.green, color.blue];
        }});
        return '';
      }})()
    """)
    for _ in range(20):
        time.sleep(0.1)
        color = evaluate("JSON.stringify(global._daytraceColor)")
        if color:
            return color
    return None


def wait_for_page():
    """Waits until the title bar has drawn the purple logo, so the page is ready for input."""
    end = time.time() + 30
    while time.time() < end:
        color = color_at(X + 18, Y + TITLE_BAR // 2)
        if color and color[2] > 150 and color[1] < 110 and color[0] < 220:
            print("ok - the title bar is drawn with the logo")
            return
        leave_overview()
        place(X, Y, 900, 600)
    print(f"FAILED - the title bar never drew its logo: {color_at(X + 18, Y + TITLE_BAR // 2)}")
    screenshot(os.environ.get("DAYTRACE_TEST_SHOT"))
    raise SystemExit(1)


def screenshot(path):
    """Saves the whole screen, when a path is given, to show what a failure looked like."""
    if not path:
        return
    evaluate(f"""
      (() => {{
        const {{ Shell, Gio }} = imports.gi;
        const stream = Gio.File.new_for_path('{path}').replace(null, false, 0, null);
        new Shell.Screenshot().screenshot(false, stream, (source, result) => {{
          try {{ source.screenshot_finish(result); }} catch (error) {{}}
          stream.close(null);
        }});
        return '';
      }})()
    """)
    time.sleep(1.5)


def leave_overview():
    """GNOME sometimes starts in the Activities overview, which shows windows as thumbnails.

    Escape closes it, and does nothing on the desktop.
    """
    evaluate("""
      (() => {
        const { Clutter, GLib } = imports.gi;
        if (!global._daytraceKeyboard)
          global._daytraceKeyboard = Clutter.get_default_backend().get_default_seat()
            .create_virtual_device(Clutter.InputDeviceType.KEYBOARD_DEVICE);
        const keyboard = global._daytraceKeyboard;
        keyboard.notify_keyval(GLib.get_monotonic_time(), Clutter.KEY_Escape, Clutter.KeyState.PRESSED);
        keyboard.notify_keyval(GLib.get_monotonic_time(), Clutter.KEY_Escape, Clutter.KeyState.RELEASED);
        return '';
      })()
    """)
    time.sleep(1)


def place(x, y, width, height):
    evaluate(f"""
      (() => {{
        const w = {WINDOW};
        if (typeof w.is_maximized === 'function' ? w.is_maximized() : w.maximized_horizontally)
          try {{ w.unmaximize(); }} catch (e) {{ w.unmaximize(3); }}
        w.move_resize_frame(true, {x}, {y}, {width}, {height});
        w.activate(global.get_current_time());
        return '';
      }})()
    """)
    time.sleep(0.8)


def pointer(steps):
    """Runs pointer actions: ("move", x, y), ("by", dx, dy), ("down",), ("up",), ("wait", ms).

    Absolute moves warp the pointer, because the virtual device drops axes of
    absolute motion events. Moves during a drag are relative, like a mouse.
    """
    for step in steps:
        if step[0] == "wait":
            time.sleep(step[1] / 1000)
            continue
        if step[0] == "move":
            action = (f"p.seat.warp_pointer({step[1]}, {step[2]}); "
                      "p.pointer.notify_relative_motion(p.now(), 1, 0); "
                      "p.pointer.notify_relative_motion(p.now(), -1, 0)")
        elif step[0] == "by":
            action = f"p.pointer.notify_relative_motion(p.now(), {step[1]}, {step[2]})"
        else:
            pressed = "PRESSED" if step[0] == "down" else "RELEASED"
            action = f"p.pointer.notify_button(p.now(), 1, imports.gi.Clutter.ButtonState.{pressed})"
        evaluate(f"(() => {{ const p = {POINTER}; {action}; return ''; }})()")
        time.sleep(0.04)


def click(x, y):
    pointer([("move", x, y), ("wait", 150), ("down",), ("wait", 60), ("up",)])


def drag(x, y, dx, dy):
    moves = [("by", dx / 10, dy / 10)] * 10
    pointer([("move", x, y), ("wait", 150), ("down",), ("wait", 100), *moves, ("wait", 150), ("up",)])
    time.sleep(0.8)


def wait_for(condition, message, timeout=6):
    end = time.time() + timeout
    while time.time() < end:
        current = state()
        if current and condition(current):
            print(f"ok - {message}")
            # Let the animation and the page's layout settle before the next step.
            time.sleep(1)
            return current
        time.sleep(0.2)
    print(f"FAILED - {message}: {state()}")
    raise SystemExit(1)


def control(current, index):
    """Centre of a window control, counting from the right edge."""
    return current["x"] + current["width"] - CONTROL * index - CONTROL // 2, current["y"] + TITLE_BAR // 2


log = open(os.environ.get("DAYTRACE_TEST_LOG", os.devnull), "w")
app = subprocess.Popen([BINARY], env=ENV, stdout=log, stderr=log)
try:
    wait_for(lambda s: s["width"] > 0, "the window opens", timeout=30)
    leave_overview()
    time.sleep(1)
    pointer([("move", 10, 790)])
    wait_for(lambda s: not s["decorated"], "the window has no system decorations")

    place(X, Y, 900, 600)
    start = wait_for(lambda s: (s["width"], s["height"]) == (900, 600), "the window can be placed for the test")
    wait_for_page()
    # The virtual device loses its first press, so spend it on empty space.
    for _ in range(2):
        click(X + 40, Y + 560)
        time.sleep(0.7)

    click(*control(start, 1))
    wait_for(lambda s: s["maximized"], "the maximize button maximizes the window")
    click(*control(state(), 1))
    wait_for(lambda s: not s["maximized"] and s["width"] == 900, "the same button restores it")

    title = (start["x"] + 420, start["y"] + TITLE_BAR // 2)
    pointer([("move", *title), ("wait", 150), ("down",), ("up",), ("down",), ("up",)])
    wait_for(lambda s: s["maximized"], "double-clicking the title bar maximizes")
    current = state()
    pointer([("move", current["x"] + 420, current["y"] + TITLE_BAR // 2), ("wait", 150),
             ("down",), ("up",), ("down",), ("up",)])
    wait_for(lambda s: not s["maximized"], "double-clicking again restores")

    place(X, Y, 900, 600)
    drag(start["x"] + 420, start["y"] + TITLE_BAR // 2, 120, -70)
    wait_for(lambda s: abs(s["x"] - (X + 120)) <= 12 and abs(s["y"] - (Y - 70)) <= 12 and s["width"] == 900,
             "dragging empty title-bar space moves the window")

    place(X, Y, 900, 600)
    drag(start["x"] + 300, start["y"] + 150, 120, 70)
    wait_for(lambda s: (s["x"], s["y"]) == (X, Y), "dragging page content does not move the window")

    place(X, Y, 900, 600)
    drag(X + 900 - 2, Y + 300, 140, 0)
    wait_for(lambda s: abs(s["width"] - 1040) <= 12 and s["x"] == X, "the right edge resizes the window")

    place(X, Y, 900, 600)
    drag(X + 900 - 2, Y + 600 - 2, 100, 20)
    wait_for(lambda s: abs(s["width"] - 1000) <= 12 and abs(s["height"] - 620) <= 12,
             "the bottom-right corner resizes both ways")

    place(X, Y, 900, 600)
    drag(X + 1, Y + 300, -40, 0)
    wait_for(lambda s: abs(s["width"] - 940) <= 12 and abs(s["x"] - (X - 40)) <= 12, "the left edge resizes the window")

    place(X, Y, 900, 600)
    drag(X + 900 - 2, Y + 600 - 2, -500, -400)
    wait_for(lambda s: s["width"] >= MIN_WIDTH and s["height"] >= MIN_HEIGHT, "resizing stops at the minimum size")

    place(X, Y, 900, 600)
    click(*control(state(), 2))
    wait_for(lambda s: s["minimized"], "the minimize button minimizes the window")
    evaluate(f"(() => {{ const w = {WINDOW}; w.unminimize(); w.activate(global.get_current_time()); return ''; }})()")
    wait_for(lambda s: not s["minimized"], "the window comes back")

    time.sleep(0.5)
    click(*control(state(), 0))
    end = time.time() + 6
    while time.time() < end and state() is not None:
        time.sleep(0.2)
    print(f"{'ok' if state() is None else 'FAILED'} - the close button closes the window")
    alive = app.poll() is None
    print(f"{'ok' if alive else 'FAILED'} - DayTrace keeps running in the background after close")
    if state() is not None or not alive:
        raise SystemExit(1)
finally:
    app.terminate()
    try:
        app.wait(timeout=10)
    except subprocess.TimeoutExpired:
        app.kill()
print("window controls passed")
