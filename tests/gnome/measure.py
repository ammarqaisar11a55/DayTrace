"""Measures real resource use of a release build in a disposable GNOME session.

    tests/gnome/session.sh python3 -I tests/gnome/measure.py <daytrace binary>
"""
import glob
import json
import os
import signal
import sqlite3
import subprocess
import sys
import time

BINARY = sys.argv[1]
DATABASE = f"{os.environ['XDG_DATA_HOME']}/io.github.ammarqaisar11a55.DayTrace/daytrace.db"
ENV = {**os.environ, "WEBKIT_DISABLE_COMPOSITING_MODE": "1", "WEBKIT_DISABLE_DMABUF_RENDERER": "1"}
TICKS = os.sysconf("SC_CLK_TCK")


def tree(root):
    children = {}
    for path in glob.glob("/proc/[0-9]*/stat"):
        try:
            text = open(path).read()
        except OSError:
            continue
        pid = int(text.split(" ", 1)[0])
        parent = int(text.rsplit(")", 1)[1].split()[1])
        children.setdefault(parent, []).append(pid)
    found, queue = [], [root]
    while queue:
        pid = queue.pop()
        found.append(pid)
        queue.extend(children.get(pid, []))
    return found


def memory_mb(pid):
    """Proportional memory, counting pages that are swapped out."""
    total = 0.0
    try:
        for line in open(f"/proc/{pid}/smaps_rollup"):
            if line.startswith(("Pss:", "SwapPss:")):
                total += int(line.split()[1]) / 1024
    except OSError:
        pass
    return total


def name(pid):
    try:
        return open(f"/proc/{pid}/comm").read().strip()
    except OSError:
        return "?"


def snapshot(root):
    processes = {pid: (name(pid), round(memory_mb(pid), 1)) for pid in tree(root)}
    return {"totalMb": round(sum(value[1] for value in processes.values()), 1), "processes": list(processes.values())}


def cpu_seconds(pid):
    fields = open(f"/proc/{pid}/stat").read().rsplit(")", 1)[1].split()
    return (int(fields[11]) + int(fields[12])) / TICKS


def evaluate(script):
    return subprocess.run(
        ["gdbus", "call", "--session", "-d", "org.gnome.Shell", "-o", "/org/gnome/Shell",
         "-m", "org.gnome.Shell.Eval", script], capture_output=True, text=True).stdout


def set_settings(**values):
    connection = sqlite3.connect(DATABASE)
    for key, value in values.items():
        connection.execute(
            "INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES (?, ?, 0)",
            (key, json.dumps(value)))
    connection.commit()
    connection.close()


calculator = subprocess.Popen(["gnome-calculator"], env=ENV, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
time.sleep(3)
first = subprocess.Popen([BINARY], env=ENV, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
time.sleep(4)
first.send_signal(signal.SIGTERM)
first.wait(timeout=15)
set_settings(trackingEnabled=True, onboardingCompleted=True)

app = subprocess.Popen([BINARY], env=ENV, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
time.sleep(10)
evaluate("global.get_window_actors().map(a => a.meta_window).filter(w => w.get_title() === 'Calculator').forEach(w => w.activate(global.get_current_time()))")
time.sleep(5)
window_open = snapshot(app.pid)

evaluate("global.get_window_actors().map(a => a.meta_window).filter(w => w.get_title() === 'DayTrace').forEach(w => w.delete(global.get_current_time()))")
time.sleep(12)
window_closed = snapshot(app.pid)

start = cpu_seconds(app.pid)
began = time.time()
time.sleep(60)
elapsed = time.time() - began
background_cpu = (cpu_seconds(app.pid) - start) / elapsed * 100
after = snapshot(app.pid)

print(json.dumps({
    "windowOpen": window_open,
    "windowClosed": window_closed,
    "backgroundCpuPercent": round(background_cpu, 3),
    "afterOneMinute": after,
}, indent=1))
app.send_signal(signal.SIGTERM)
app.wait(timeout=15)
calculator.terminate()
