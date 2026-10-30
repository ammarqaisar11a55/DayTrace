use rusqlite::{params, Connection};

use crate::error::{invalid, Result};
use crate::models::{Exclusion, ExclusionKind};
use crate::time::now_ms;

pub fn list(connection: &Connection) -> Result<Vec<Exclusion>> {
    let mut statement =
        connection.prepare("SELECT id, kind, value, created_at FROM exclusions ORDER BY kind, value")?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, i64>(3)?))
    })?;

    let mut exclusions = Vec::new();
    for row in rows {
        let (id, kind, value, created_at) = row?;
        exclusions.push(Exclusion { id, kind: ExclusionKind::parse(&kind)?, value, created_at });
    }
    Ok(exclusions)
}

pub fn add(connection: &Connection, kind: ExclusionKind, value: &str) -> Result<()> {
    let value = value.trim();
    if value.is_empty() {
        return invalid("An exclusion needs a value");
    }
    connection.execute(
        "INSERT OR IGNORE INTO exclusions (kind, value, created_at) VALUES (?1, ?2, ?3)",
        params![kind.as_str(), value, now_ms()],
    )?;
    Ok(())
}

pub fn remove(connection: &Connection, id: i64) -> Result<()> {
    connection.execute("DELETE FROM exclusions WHERE id = ?1", [id])?;
    Ok(())
}
