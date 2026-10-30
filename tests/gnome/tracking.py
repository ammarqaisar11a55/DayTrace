"""End-to-end tracking test on a real GNOME Wayland session.

Run through session.sh, which provides a disposable headless GNOME Shell:

    tests/gnome/session.sh python3 -I tests/gnome/tracking.py <daytrace binary>

It starts the real application, switches focus between real applications, and
checks what reached the database.
"""
import json
import os
import signal
import sqlite3
import subprocess
import sys
import time

BINARY = sys.argv[1]
DATA = os.environ["XDG_DATA_HOME"]
DATABASE = f"{DATA}/io.github.ammarqaisar11a55.DayTrace/daytrace.db"
ENV = {**os.environ, "WEBKIT_DISABLE_COMPOSITING_MODE": "1", "WEBKIT_DISABLE_DMABUF_RENDERER": "1"}


def evaluate(script):
    result = subprocess.run(
        ["gdbus", "call", "--session", "-d", "org.gnome.Shell", "-o", "/org/gnome/Shell",
         "-m", "org.gnome.Shell.Eval", script],
        capture_output=True, text=True,
    )
    return result.stdout.strip()


def activate(title):
    script = (
        "global.get_window_actors().map(a => a.meta_window)"
        f".filter(w => w.get_title() && w.get_title().includes('{title}'))"
        ".forEach(w => w.activate(global.get_current_time()))"
    )
    evaluate(script)


def start_app():
    return subprocess.Popen([BINARY], env=ENV, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def wait_for_database():
    for _ in range(100):
        if os.path.exists(DATABASE):
            return
        time.sleep(0.1)
    raise SystemExit("the application did not create its database")


def set_settings(**values):
    connection = sqlite3.connect(DATABASE)
    for key, value in values.items():
        connection.execute(
            "INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES (?, ?, 0)",
            (key, json.dumps(value)),
        )
    connection.commit()
    connection.close()


def sessions():
    connection = sqlite3.connect(DATABASE)
    rows = connection.execute(
        """SELECT COALESCE(a.name, s.session_type), s.session_type, a.executable_path, s.window_title,
                  s.start_time, s.end_time, s.is_open
           FROM activity_sessions s LEFT JOIN applications a ON a.id = s.application_id
           ORDER BY s.start_time"""
    ).fetchall()
    connection.close()
    return rows


def check(condition, message):
    print(f"{'ok' if condition else 'FAILED'} - {message}")
    if not condition:
        for row in sessions():
            print("   ", row)
        raise SystemExit(1)


def stop(process, sig):
    process.send_signal(sig)
    process.wait(timeout=15)


calculator = subprocess.Popen(["gnome-calculator"], env=ENV, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
editor = subprocess.Popen(["gnome-text-editor"], env=ENV, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
time.sleep(4)
print("windows:", evaluate("JSON.stringify(global.get_window_actors().map(a => a.meta_window.get_title()))"))

first_run = start_app()
wait_for_database()
time.sleep(3)
stop(first_run, signal.SIGTERM)
check(sessions() == [], "nothing is recorded before consent")

set_settings(trackingEnabled=True, onboardingCompleted=True, pollIntervalSecs=1)
app = start_app()
time.sleep(4)

activate("Calculator")
time.sleep(4)
activate("Text Editor")
time.sleep(4)
activate("Calculator")
time.sleep(4)
stop(app, signal.SIGTERM)

rows = sessions()
named = [row for row in rows if row[1] == "active"]
order = [row[0] for row in named]
print("recorded:", [(row[0], round((row[5] - row[4]) / 1000, 1)) for row in rows])

check(any("Calculator" in name for name in order), "Calculator was identified by name")
check(any("Text Editor" in name for name in order), "Text Editor was identified by name")
calc = [i for i, name in enumerate(order) if "Calculator" in name]
text = [i for i, name in enumerate(order) if "Text Editor" in name]
check(calc[0] < text[0] < calc[-1], "focus switches were recorded in order")
check(all(row[2] and row[2].startswith("/") for row in named), "executable paths were recorded")
check(all(row[3] is None for row in rows), "window titles are not stored by default")
check(all(row[6] == 0 for row in rows), "a graceful exit closes every session")
check(all(row[5] >= row[4] for row in rows), "no session has a negative duration")
for earlier, later in zip(rows, rows[1:]):
    check(earlier[5] <= later[4], f"sessions do not overlap at {earlier[0]} -> {later[0]}")

before = len(rows)
set_settings(collectWindowTitles=True)
app = start_app()
time.sleep(3)
activate("Text Editor")
time.sleep(3)
activate("Calculator")
time.sleep(35)
stop(app, signal.SIGKILL)

rows = sessions()
open_rows = [row for row in rows if row[6] == 1]
check(len(open_rows) == 1, "a crash leaves the open session at its last checkpoint")
check(any(row[3] for row in rows[before:]), "window titles are stored once the user opts in")

app = start_app()
time.sleep(3)
stop(app, signal.SIGTERM)
rows = sessions()
check(all(row[6] == 0 for row in rows), "restarting recovers the interrupted session")

before = len(sessions())
set_settings(collectWindowTitles=False)
app = start_app()
time.sleep(4)
check("DayTrace" in evaluate("JSON.stringify(global.get_window_actors().map(a => a.meta_window.get_title()))"),
      "the window opens")
evaluate("global.get_window_actors().map(a => a.meta_window)"
         ".filter(w => w.get_title() === 'DayTrace').forEach(w => w.delete(global.get_current_time()))")
time.sleep(2)
titles = evaluate("JSON.stringify(global.get_window_actors().map(a => a.meta_window.get_title()))")
check("DayTrace" not in titles, "closing the window removes it")
check(app.poll() is None, "the application keeps running after its window closes")

activate("Text Editor")
time.sleep(3)
activate("Calculator")
time.sleep(3)
check(app.poll() is None, "tracking continues in the background")

second = start_app()
second.wait(timeout=10)
time.sleep(3)
check("DayTrace" in evaluate("JSON.stringify(global.get_window_actors().map(a => a.meta_window.get_title()))"),
      "launching again reopens the window of the running instance")
stop(app, signal.SIGTERM)
later = sessions()[before:]
check(any("Text Editor" in row[0] for row in later) and any("Calculator" in row[0] for row in later),
      "tracking survives closing the window")

set_settings(runInBackground=False)
app = start_app()
time.sleep(4)
evaluate("global.get_window_actors().map(a => a.meta_window)"
         ".filter(w => w.get_title() === 'DayTrace').forEach(w => w.delete(global.get_current_time()))")
try:
    app.wait(timeout=8)
    exited = True
except subprocess.TimeoutExpired:
    exited = False
    app.kill()
check(exited, "with background running off, closing the window quits the application")
check(all(row[6] == 0 for row in sessions()), "quitting that way still closes every session")

calculator.terminate()
editor.terminate()
print("\nGNOME Wayland tracking checks passed")
