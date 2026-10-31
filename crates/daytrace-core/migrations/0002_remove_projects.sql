-- Removes projects. SQLite cannot drop a column that is part of a foreign key,
-- so the tables that referenced projects are rebuilt. This runs with foreign key
-- enforcement off: otherwise dropping `applications` or `categories` would clear
-- those links on every session.

DELETE FROM classification_rules WHERE target_category_id IS NULL;
DELETE FROM exclusions WHERE kind = 'project';

CREATE TABLE categories_new (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE COLLATE NOCASE,
    color TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
INSERT INTO categories_new (id, name, color, created_at, updated_at)
    SELECT id, name, color, created_at, updated_at FROM categories;
DROP TABLE categories;
ALTER TABLE categories_new RENAME TO categories;

CREATE TABLE applications_new (
    id INTEGER PRIMARY KEY,
    platform_identifier TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    display_name TEXT,
    executable_path TEXT,
    icon_path TEXT,
    default_category_id INTEGER REFERENCES categories (id) ON DELETE SET NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
INSERT INTO applications_new
    (id, platform_identifier, name, display_name, executable_path, icon_path,
     default_category_id, created_at, updated_at)
    SELECT id, platform_identifier, name, display_name, executable_path, icon_path,
           default_category_id, created_at, updated_at
    FROM applications;
DROP TABLE applications;
ALTER TABLE applications_new RENAME TO applications;

CREATE TABLE activity_sessions_new (
    id INTEGER PRIMARY KEY,
    application_id INTEGER REFERENCES applications (id) ON DELETE SET NULL,
    category_id INTEGER REFERENCES categories (id) ON DELETE SET NULL,
    start_time INTEGER NOT NULL,
    end_time INTEGER NOT NULL,
    duration_seconds INTEGER GENERATED ALWAYS AS ((end_time - start_time) / 1000) STORED,
    session_type TEXT NOT NULL CHECK (session_type IN ('active', 'unknown', 'manual', 'idle', 'locked')),
    window_title TEXT,
    label TEXT,
    manual_override INTEGER NOT NULL DEFAULT 0,
    is_open INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    CHECK (end_time >= start_time)
);
INSERT INTO activity_sessions_new
    (id, application_id, category_id, start_time, end_time, session_type, window_title,
     label, manual_override, is_open, created_at, updated_at)
    SELECT id, application_id, category_id, start_time, end_time, session_type, window_title,
           label, manual_override, is_open, created_at, updated_at
    FROM activity_sessions;
DROP TABLE activity_sessions;
ALTER TABLE activity_sessions_new RENAME TO activity_sessions;

CREATE INDEX idx_sessions_start ON activity_sessions (start_time);
CREATE INDEX idx_sessions_end ON activity_sessions (end_time);
CREATE INDEX idx_sessions_app ON activity_sessions (application_id, start_time);
CREATE INDEX idx_sessions_category ON activity_sessions (category_id, start_time);
CREATE INDEX idx_sessions_open ON activity_sessions (is_open) WHERE is_open = 1;

CREATE TABLE classification_rules_new (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    rule_type TEXT NOT NULL DEFAULT 'all_conditions' CHECK (rule_type IN ('all_conditions')),
    rule_condition TEXT NOT NULL,
    target_category_id INTEGER NOT NULL REFERENCES categories (id) ON DELETE CASCADE,
    priority INTEGER NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
INSERT INTO classification_rules_new
    (id, name, rule_type, rule_condition, target_category_id, priority, enabled, created_at, updated_at)
    SELECT id, name, rule_type, rule_condition, target_category_id, priority, enabled, created_at, updated_at
    FROM classification_rules;
DROP TABLE classification_rules;
ALTER TABLE classification_rules_new RENAME TO classification_rules;

CREATE INDEX idx_rules_priority ON classification_rules (priority, id);

CREATE TABLE exclusions_new (
    id INTEGER PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('application', 'path')),
    value TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    UNIQUE (kind, value)
);
INSERT INTO exclusions_new (id, kind, value, created_at)
    SELECT id, kind, value, created_at FROM exclusions;
DROP TABLE exclusions;
ALTER TABLE exclusions_new RENAME TO exclusions;

DROP TABLE projects;
