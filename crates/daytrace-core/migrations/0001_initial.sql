-- All timestamps are UTC epoch milliseconds.

CREATE TABLE projects (
    id INTEGER PRIMARY KEY,
    parent_id INTEGER REFERENCES projects (id) ON DELETE SET NULL,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    color TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'archived')),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX idx_projects_parent ON projects (parent_id);

CREATE TABLE categories (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE COLLATE NOCASE,
    color TEXT NOT NULL,
    default_project_id INTEGER REFERENCES projects (id) ON DELETE SET NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE applications (
    id INTEGER PRIMARY KEY,
    platform_identifier TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    display_name TEXT,
    executable_path TEXT,
    icon_path TEXT,
    default_project_id INTEGER REFERENCES projects (id) ON DELETE SET NULL,
    default_category_id INTEGER REFERENCES categories (id) ON DELETE SET NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE activity_sessions (
    id INTEGER PRIMARY KEY,
    application_id INTEGER REFERENCES applications (id) ON DELETE SET NULL,
    project_id INTEGER REFERENCES projects (id) ON DELETE SET NULL,
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

CREATE INDEX idx_sessions_start ON activity_sessions (start_time);
CREATE INDEX idx_sessions_end ON activity_sessions (end_time);
CREATE INDEX idx_sessions_app ON activity_sessions (application_id, start_time);
CREATE INDEX idx_sessions_project ON activity_sessions (project_id, start_time);
CREATE INDEX idx_sessions_category ON activity_sessions (category_id, start_time);
CREATE INDEX idx_sessions_open ON activity_sessions (is_open) WHERE is_open = 1;

CREATE TABLE classification_rules (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    rule_type TEXT NOT NULL DEFAULT 'all_conditions' CHECK (rule_type IN ('all_conditions')),
    rule_condition TEXT NOT NULL,
    target_project_id INTEGER REFERENCES projects (id) ON DELETE SET NULL,
    target_category_id INTEGER REFERENCES categories (id) ON DELETE SET NULL,
    priority INTEGER NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    CHECK (target_project_id IS NOT NULL OR target_category_id IS NOT NULL)
);

CREATE INDEX idx_rules_priority ON classification_rules (priority, id);

CREATE TABLE exclusions (
    id INTEGER PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('application', 'path', 'project')),
    value TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    UNIQUE (kind, value)
);

CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);

INSERT INTO categories (name, color, created_at, updated_at) VALUES
    ('Development', '#2a78d6', unixepoch() * 1000, unixepoch() * 1000),
    ('Research', '#eb6834', unixepoch() * 1000, unixepoch() * 1000),
    ('Education', '#1baf7a', unixepoch() * 1000, unixepoch() * 1000),
    ('Communication', '#eda100', unixepoch() * 1000, unixepoch() * 1000),
    ('Entertainment', '#e87ba4', unixepoch() * 1000, unixepoch() * 1000),
    ('Design', '#008300', unixepoch() * 1000, unixepoch() * 1000),
    ('Administration', '#6250d6', unixepoch() * 1000, unixepoch() * 1000),
    ('Personal', '#e34948', unixepoch() * 1000, unixepoch() * 1000),
    ('Other', '#64748b', unixepoch() * 1000, unixepoch() * 1000);
