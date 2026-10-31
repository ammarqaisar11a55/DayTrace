"""Fills a disposable test database with a plausible month of activity.

This exists only so that end-to-end tests and design review can see populated
pages. It is never used by, or shipped with, the application.
"""
import random
import sqlite3
import sys
import time
from datetime import datetime, timedelta

database = sqlite3.connect(sys.argv[1])
database.execute("PRAGMA foreign_keys = ON")
# Entries made earlier in the run would overlap the generated history.
database.execute("DELETE FROM activity_sessions")
now_ms = int(time.time() * 1000)
random.seed(7)

applications = [
    ("code", "Visual Studio Code", "/usr/share/code/code", "Development"),
    ("firefox", "Firefox", "/usr/lib/firefox/firefox", "Research"),
    ("org.gnome.terminal", "Terminal", "/usr/bin/gnome-terminal", "Development"),
    ("notion", "Notion", "/opt/notion/notion", "Administration"),
    ("slack", "Slack", "/usr/bin/slack", "Communication"),
    ("figma-linux", "Figma", "/opt/figma/figma", "Design"),
    ("spotify", "Spotify", "/usr/bin/spotify", "Entertainment"),
]

category_ids = dict(database.execute("SELECT name, id FROM categories"))
application_ids = {}
for identifier, name, path, _ in applications:
    cursor = database.execute(
        "INSERT INTO applications (platform_identifier, name, executable_path, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
        (identifier, name, path, now_ms, now_ms),
    )
    application_ids[identifier] = cursor.lastrowid


def insert(kind, start, end, application=None):
    if end <= start:
        return
    identifier, _, _, category = application or (None, None, None, None)
    database.execute(
        """INSERT INTO activity_sessions (application_id, category_id, start_time, end_time,
           session_type, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)""",
        (application_ids.get(identifier), category_ids.get(category), start, end, kind, now_ms, now_ms),
    )


today = datetime.now().replace(hour=0, minute=0, second=0, microsecond=0)
for days_ago in range(30, -1, -1):
    day = today - timedelta(days=days_ago)
    if days_ago and day.weekday() >= 5 and random.random() < 0.6:
        continue
    cursor = day + timedelta(hours=8, minutes=random.randint(30, 80))
    stop = day + timedelta(hours=random.randint(16, 20), minutes=random.randint(0, 59))
    while cursor < stop:
        start = int(cursor.timestamp() * 1000)
        if start >= now_ms - 60_000:
            break
        roll = random.random()
        if roll < 0.10:
            minutes = random.randint(8, 40)
            kind, application = ("idle" if random.random() < 0.7 else "locked"), None
        elif roll < 0.16:
            minutes = random.randint(5, 25)
            kind, application = "unknown", None
        else:
            weights = [34, 22, 14, 6, 10, 8, 6]
            application = random.choices(applications, weights)[0]
            minutes = random.randint(3, 55)
            kind = "active"
        end = min(int((cursor + timedelta(minutes=minutes)).timestamp() * 1000), now_ms - 60_000)
        insert(kind, start, end, application)
        cursor += timedelta(minutes=minutes)
        if random.random() < 0.12:
            cursor += timedelta(minutes=random.randint(5, 45))

database.execute(
    """INSERT INTO classification_rules (name, rule_condition, target_category_id, priority, created_at, updated_at)
       VALUES ('Terminal is development', ?, ?, 10, ?, ?)""",
    (
        '[{"field":"application","operator":"contains","value":"terminal"}]',
        category_ids["Development"],
        now_ms,
        now_ms,
    ),
)
database.execute(
    "INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES ('trackingEnabled', 'true', ?), ('onboardingCompleted', 'true', ?)",
    (now_ms, now_ms),
)
database.commit()
database.close()
