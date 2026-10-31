use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::error::{CoreError, Result};
use crate::models::{Application, ApplicationPatch, ApplicationUsage};
use crate::time::now_ms;

const COLUMNS: &str = "id, platform_identifier, name, display_name, executable_path, icon_path,
    default_category_id, created_at";

fn from_row(row: &Row<'_>) -> rusqlite::Result<Application> {
    Ok(Application {
        id: row.get(0)?,
        platform_identifier: row.get(1)?,
        name: row.get(2)?,
        display_name: row.get(3)?,
        executable_path: row.get(4)?,
        icon_path: row.get(5)?,
        default_category_id: row.get(6)?,
        created_at: row.get(7)?,
    })
}

/// Records an application the first time it is seen, and refreshes details the
/// platform may have learned since. User-chosen display names are untouched.
pub fn upsert(
    connection: &Connection,
    identifier: &str,
    name: &str,
    executable_path: Option<&str>,
    icon_path: Option<&str>,
) -> Result<i64> {
    let now = now_ms();
    connection.execute(
        "INSERT INTO applications (platform_identifier, name, executable_path, icon_path, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)
         ON CONFLICT (platform_identifier) DO UPDATE SET
            name = excluded.name,
            executable_path = COALESCE(excluded.executable_path, executable_path),
            icon_path = COALESCE(excluded.icon_path, icon_path),
            updated_at = excluded.updated_at
         WHERE name != excluded.name
            OR executable_path IS NOT excluded.executable_path
            OR icon_path IS NOT excluded.icon_path",
        params![identifier, name, executable_path, icon_path, now],
    )?;
    Ok(connection.query_row(
        "SELECT id FROM applications WHERE platform_identifier = ?1",
        [identifier],
        |row| row.get(0),
    )?)
}

pub fn get(connection: &Connection, id: i64) -> Result<Application> {
    connection
        .query_row(&format!("SELECT {COLUMNS} FROM applications WHERE id = ?1"), [id], from_row)
        .optional()?
        .ok_or(CoreError::NotFound("Application"))
}

pub fn list(connection: &Connection) -> Result<Vec<Application>> {
    let mut statement = connection.prepare(&format!(
        "SELECT {COLUMNS} FROM applications ORDER BY COALESCE(display_name, name) COLLATE NOCASE"
    ))?;
    let rows = statement.query_map([], from_row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Applications with their use inside the half-open range, clipped to it.
pub fn list_usage(connection: &Connection, from: i64, to: i64) -> Result<Vec<ApplicationUsage>> {
    let columns =
        COLUMNS.split(',').map(|column| format!("a.{}", column.trim())).collect::<Vec<_>>().join(", ");
    let mut statement = connection.prepare(&format!(
        "SELECT {columns},
            COALESCE(SUM(MIN(s.end_time, ?2) - MAX(s.start_time, ?1)), 0),
            COUNT(s.id),
            (SELECT MAX(start_time) FROM activity_sessions WHERE application_id = a.id)
         FROM applications a
         LEFT JOIN activity_sessions s
            ON s.application_id = a.id AND s.end_time > ?1 AND s.start_time < ?2
         GROUP BY a.id
         ORDER BY 9 DESC, COALESCE(a.display_name, a.name) COLLATE NOCASE"
    ))?;
    let rows = statement.query_map(params![from, to], |row| {
        Ok(ApplicationUsage {
            application: from_row(row)?,
            total_ms: row.get(8)?,
            session_count: row.get(9)?,
            last_used: row.get(10)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn update(connection: &Connection, id: i64, patch: &ApplicationPatch) -> Result<()> {
    let display_name = patch.display_name.as_deref().map(str::trim).filter(|name| !name.is_empty());
    let changed = connection.execute(
        "UPDATE applications
         SET display_name = ?2, default_category_id = ?3, updated_at = ?4
         WHERE id = ?1",
        params![id, display_name, patch.default_category_id, now_ms()],
    )?;
    if changed == 0 {
        return Err(CoreError::NotFound("Application"));
    }
    Ok(())
}

/// Deletes an application and every session recorded for it.
pub fn delete_with_history(connection: &Connection, id: i64) -> Result<()> {
    connection.execute("DELETE FROM activity_sessions WHERE application_id = ?1", [id])?;
    connection.execute("DELETE FROM applications WHERE id = ?1", [id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::test_support::database;

    #[test]
    fn upsert_is_idempotent_and_keeps_known_details() {
        let (_dir, db) = database();
        let first = db.write(|c| upsert(c, "code", "Code", Some("/usr/bin/code"), None)).unwrap();
        let second = db.write(|c| upsert(c, "code", "Code", None, Some("/icon.png"))).unwrap();
        assert_eq!(first, second);
        let app = db.read(|c| get(c, first)).unwrap();
        assert_eq!(app.executable_path.as_deref(), Some("/usr/bin/code"));
        assert_eq!(app.icon_path.as_deref(), Some("/icon.png"));
    }

    #[test]
    fn display_name_survives_upsert() {
        let (_dir, db) = database();
        let id = db.write(|c| upsert(c, "code", "Code", None, None)).unwrap();
        let patch = ApplicationPatch { display_name: Some("  Editor ".into()), default_category_id: None };
        db.write(|c| update(c, id, &patch)).unwrap();
        db.write(|c| upsert(c, "code", "Visual Studio Code", None, None)).unwrap();
        let app = db.read(|c| get(c, id)).unwrap();
        assert_eq!(app.label(), "Editor");
        assert_eq!(app.name, "Visual Studio Code");
    }

    #[test]
    fn usage_is_clipped_to_the_range() {
        let (_dir, db) = database();
        let id = db.write(|c| upsert(c, "code", "Code", None, None)).unwrap();
        db.write(|c| {
            c.execute(
                "INSERT INTO activity_sessions (application_id, start_time, end_time, session_type, created_at, updated_at)
                 VALUES (?1, 500, 1500, 'active', 0, 0), (?1, 1800, 2500, 'active', 0, 0)",
                [id],
            )?;
            Ok(())
        })
        .unwrap();
        let usage = db.read(|c| list_usage(c, 1_000, 2_000)).unwrap();
        assert_eq!(usage[0].total_ms, 700);
        assert_eq!(usage[0].session_count, 2);
        assert_eq!(usage[0].last_used, Some(1_800));
    }
}
