use rusqlite::{params, params_from_iter, types::Value, Connection, OptionalExtension, Row};

use crate::error::{invalid, CoreError, Result};
use crate::models::{ManualSessionInput, Session, SessionFilter, SessionPatch, SessionType};
use crate::time::now_ms;

const SELECT: &str = "SELECT s.id, s.application_id, COALESCE(a.display_name, a.name), a.executable_path,
        s.category_id, c.name, c.color,
        s.start_time, s.end_time, s.duration_seconds, s.session_type,
        s.window_title, s.label, s.manual_override, s.is_open
    FROM activity_sessions s
    LEFT JOIN applications a ON a.id = s.application_id
    LEFT JOIN categories c ON c.id = s.category_id";

fn from_row(row: &Row<'_>) -> rusqlite::Result<(Session, String)> {
    let session_type: String = row.get(10)?;
    Ok((
        Session {
            id: row.get(0)?,
            application_id: row.get(1)?,
            application_name: row.get(2)?,
            executable_path: row.get(3)?,
            category_id: row.get(4)?,
            category_name: row.get(5)?,
            category_color: row.get(6)?,
            start_time: row.get(7)?,
            end_time: row.get(8)?,
            duration_seconds: row.get(9)?,
            session_type: SessionType::Idle,
            window_title: row.get(11)?,
            label: row.get(12)?,
            manual_override: row.get(13)?,
            is_open: row.get(14)?,
        },
        session_type,
    ))
}

fn parse((mut session, session_type): (Session, String)) -> Result<Session> {
    session.session_type = SessionType::parse(&session_type)?;
    Ok(session)
}

pub fn list(connection: &Connection, filter: &SessionFilter) -> Result<Vec<Session>> {
    let mut sql = format!("{SELECT} WHERE s.end_time > ?1 AND s.start_time < ?2");
    let mut values = vec![Value::Integer(filter.from), Value::Integer(filter.to)];

    let filters = [("s.application_id", filter.application_id), ("s.category_id", filter.category_id)];
    for (column, value) in filters {
        if let Some(value) = value {
            values.push(Value::Integer(value));
            sql.push_str(&format!(" AND {column} = ?{}", values.len()));
        }
    }

    let order = if filter.newest_first.unwrap_or(false) { "DESC" } else { "ASC" };
    sql.push_str(&format!(" ORDER BY s.start_time {order}, s.id {order}"));
    if let Some(limit) = filter.limit {
        values.push(Value::Integer(limit));
        sql.push_str(&format!(" LIMIT ?{}", values.len()));
    }

    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params_from_iter(values), from_row)?;
    rows.map(|row| parse(row?)).collect()
}

pub fn get(connection: &Connection, id: i64) -> Result<Session> {
    let row = connection
        .query_row(&format!("{SELECT} WHERE s.id = ?1"), [id], from_row)
        .optional()?
        .ok_or(CoreError::NotFound("Session"))?;
    parse(row)
}

fn overlaps(connection: &Connection, ignore_id: i64, start: i64, end: i64) -> Result<bool> {
    Ok(connection.query_row(
        "SELECT EXISTS (
            SELECT 1 FROM activity_sessions
            WHERE id != ?1 AND start_time < ?3 AND end_time > ?2
         )",
        params![ignore_id, start, end],
        |row| row.get(0),
    )?)
}

fn clean_label(label: Option<&str>) -> Option<&str> {
    label.map(str::trim).filter(|label| !label.is_empty())
}

pub fn update(connection: &Connection, id: i64, patch: &SessionPatch) -> Result<Session> {
    let session = get(connection, id)?;
    let start = patch.start_time.unwrap_or(session.start_time);
    let end = patch.end_time.unwrap_or(session.end_time);

    if end <= start {
        return invalid("A session must end after it starts");
    }
    if end > now_ms() + 60_000 {
        return invalid("A session cannot end in the future");
    }
    if (start, end) != (session.start_time, session.end_time) {
        if session.is_open {
            return invalid("Times cannot be changed while the session is in progress");
        }
        if overlaps(connection, id, start, end)? {
            return Err(CoreError::Overlap);
        }
    }

    // A label or time correction leaves the session open to later rule changes.
    // Only a changed category counts as a manual assignment.
    let reassigned = patch.category_id != session.category_id;
    connection.execute(
        "UPDATE activity_sessions
         SET category_id = ?2, label = ?3, start_time = ?4, end_time = ?5,
             manual_override = manual_override OR ?6, updated_at = ?7
         WHERE id = ?1",
        params![id, patch.category_id, clean_label(patch.label.as_deref()), start, end, reassigned, now_ms()],
    )?;
    get(connection, id)
}

pub fn insert_manual(connection: &Connection, input: &ManualSessionInput) -> Result<Session> {
    if input.end_time <= input.start_time {
        return invalid("A session must end after it starts");
    }
    if input.end_time > now_ms() + 60_000 {
        return invalid("A session cannot end in the future");
    }
    if overlaps(connection, 0, input.start_time, input.end_time)? {
        return Err(CoreError::Overlap);
    }
    let now = now_ms();
    connection.execute(
        "INSERT INTO activity_sessions
            (category_id, start_time, end_time, session_type, label, manual_override, created_at, updated_at)
         VALUES (?1, ?2, ?3, 'manual', ?4, 1, ?5, ?5)",
        params![
            input.category_id,
            input.start_time,
            input.end_time,
            clean_label(input.label.as_deref()),
            now
        ],
    )?;
    get(connection, connection.last_insert_rowid())
}

pub fn delete(connection: &Connection, id: i64) -> Result<()> {
    connection.execute("DELETE FROM activity_sessions WHERE id = ?1 AND is_open = 0", [id])?;
    Ok(())
}

pub fn delete_all(connection: &Connection) -> Result<usize> {
    Ok(connection.execute("DELETE FROM activity_sessions", [])?)
}

/// Removes sessions that ended before the cutoff, for the retention setting.
pub fn delete_before(connection: &Connection, cutoff: i64) -> Result<usize> {
    Ok(connection.execute("DELETE FROM activity_sessions WHERE end_time < ?1 AND is_open = 0", [cutoff])?)
}

/// Closes sessions left open by a crash or power loss at their last checkpoint.
pub fn recover_open(connection: &Connection) -> Result<usize> {
    let now = now_ms();
    connection.execute("DELETE FROM activity_sessions WHERE is_open = 1 AND end_time <= start_time", [])?;
    Ok(connection
        .execute("UPDATE activity_sessions SET is_open = 0, updated_at = ?1 WHERE is_open = 1", [now])?)
}

/// A session as the tracker writes it.
#[derive(Clone, Debug, PartialEq)]
pub struct TrackedSession {
    pub application_id: Option<i64>,
    pub category_id: Option<i64>,
    pub start_time: i64,
    pub end_time: i64,
    pub session_type: SessionType,
    pub window_title: Option<String>,
    pub label: Option<String>,
    pub manual_override: bool,
    pub is_open: bool,
}

pub fn insert_tracked(connection: &Connection, session: &TrackedSession) -> Result<i64> {
    let now = now_ms();
    connection.execute(
        "INSERT INTO activity_sessions
            (application_id, category_id, start_time, end_time, session_type,
             window_title, label, manual_override, is_open, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
        params![
            session.application_id,
            session.category_id,
            session.start_time,
            session.end_time,
            session.session_type.as_str(),
            session.window_title,
            session.label,
            session.manual_override,
            session.is_open,
            now
        ],
    )?;
    Ok(connection.last_insert_rowid())
}

/// Updates only the tracker's own fields. Corrections the user made to a
/// running session, such as a new category, are kept.
pub fn update_tracked(connection: &Connection, id: i64, session: &TrackedSession) -> Result<()> {
    connection.execute(
        "UPDATE activity_sessions
         SET start_time = ?2, end_time = ?3, is_open = ?4, updated_at = ?5,
             category_id = CASE manual_override WHEN 1 THEN category_id ELSE ?6 END
         WHERE id = ?1",
        params![id, session.start_time, session.end_time, session.is_open, now_ms(), session.category_id],
    )?;
    Ok(())
}

pub fn remove_tracked(connection: &Connection, id: i64) -> Result<()> {
    connection.execute("DELETE FROM activity_sessions WHERE id = ?1", [id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::test_support::database;
    use crate::database::Database;

    fn tracked(start: i64, end: i64, is_open: bool) -> TrackedSession {
        TrackedSession {
            application_id: None,
            category_id: None,
            start_time: start,
            end_time: end,
            session_type: SessionType::Unknown,
            window_title: None,
            label: None,
            manual_override: false,
            is_open,
        }
    }

    fn all(db: &Database) -> Vec<Session> {
        let filter = SessionFilter { from: 0, to: i64::MAX, ..Default::default() };
        db.read(|c| list(c, &filter)).unwrap()
    }

    #[test]
    fn crash_recovery_closes_open_sessions_at_last_checkpoint() {
        let (_dir, db) = database();
        db.write(|c| insert_tracked(c, &tracked(1_000, 31_000, true))).unwrap();
        db.write(|c| insert_tracked(c, &tracked(40_000, 40_000, true))).unwrap();

        assert_eq!(db.write(recover_open).unwrap(), 1);
        let sessions = all(&db);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].end_time, 31_000);
        assert!(!sessions[0].is_open);
        assert_eq!(sessions[0].duration_seconds, 30);
    }

    #[test]
    fn negative_durations_cannot_be_stored() {
        let (_dir, db) = database();
        assert!(db.write(|c| insert_tracked(c, &tracked(2_000, 1_000, false))).is_err());
    }

    #[test]
    fn editing_times_rejects_overlaps() {
        let (_dir, db) = database();
        let first = db.write(|c| insert_tracked(c, &tracked(1_000, 2_000, false))).unwrap();
        db.write(|c| insert_tracked(c, &tracked(3_000, 4_000, false))).unwrap();
        let patch =
            SessionPatch { category_id: None, label: None, start_time: Some(1_000), end_time: Some(3_500) };
        assert!(matches!(db.write(|c| update(c, first, &patch)), Err(CoreError::Overlap)));
    }

    #[test]
    fn manual_entries_cannot_overlap_or_be_in_the_future() {
        let (_dir, db) = database();
        db.write(|c| insert_tracked(c, &tracked(1_000, 2_000, false))).unwrap();
        let mut input =
            ManualSessionInput { start_time: 1_500, end_time: 2_500, label: None, category_id: None };
        assert!(db.write(|c| insert_manual(c, &input)).is_err());
        input.start_time = 2_000;
        assert!(db.write(|c| insert_manual(c, &input)).is_ok());
        input.start_time = now_ms() + 3_600_000;
        input.end_time = input.start_time + 1;
        assert!(db.write(|c| insert_manual(c, &input)).is_err());
    }

    #[test]
    fn user_corrections_survive_tracker_checkpoints() {
        let (_dir, db) = database();
        let id = db.write(|c| insert_tracked(c, &tracked(1_000, 2_000, true))).unwrap();
        let patch = SessionPatch {
            category_id: Some(2),
            label: Some("  Review ".into()),
            start_time: None,
            end_time: None,
        };
        db.write(|c| update(c, id, &patch)).unwrap();
        db.write(|c| update_tracked(c, id, &tracked(1_000, 9_000, true))).unwrap();

        let session = db.read(|c| get(c, id)).unwrap();
        assert_eq!(session.category_id, Some(2));
        assert_eq!(session.label.as_deref(), Some("Review"));
        assert_eq!(session.end_time, 9_000);
    }

    #[test]
    fn retention_keeps_open_and_recent_sessions() {
        let (_dir, db) = database();
        db.write(|c| insert_tracked(c, &tracked(1_000, 2_000, false))).unwrap();
        db.write(|c| insert_tracked(c, &tracked(3_000, 4_000, true))).unwrap();
        db.write(|c| insert_tracked(c, &tracked(9_000, 10_000, false))).unwrap();
        assert_eq!(db.write(|c| delete_before(c, 5_000)).unwrap(), 1);
        assert_eq!(all(&db).len(), 2);
    }

    #[test]
    fn filters_and_clipping_range() {
        let (_dir, db) = database();
        db.write(|c| insert_tracked(c, &tracked(1_000, 2_000, false))).unwrap();
        db.write(|c| insert_tracked(c, &tracked(2_500, 3_500, false))).unwrap();
        let filter = SessionFilter { from: 3_000, to: 4_000, ..Default::default() };
        assert_eq!(db.read(|c| list(c, &filter)).unwrap().len(), 1);
        let newest = SessionFilter {
            from: 0,
            to: 9_000,
            limit: Some(1),
            newest_first: Some(true),
            ..Default::default()
        };
        assert_eq!(db.read(|c| list(c, &newest)).unwrap()[0].start_time, 2_500);
    }
}
