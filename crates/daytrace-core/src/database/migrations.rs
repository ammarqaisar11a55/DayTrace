use rusqlite::Connection;

use crate::error::{CoreError, Result};

const MIGRATIONS: &[&str] = &[include_str!("../../migrations/0001_initial.sql")];

pub const LATEST_VERSION: i64 = MIGRATIONS.len() as i64;

pub fn run(connection: &mut Connection) -> Result<()> {
    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version > LATEST_VERSION {
        return Err(CoreError::NewerSchema);
    }

    for (index, migration) in MIGRATIONS.iter().enumerate().skip(version as usize) {
        let transaction = connection.transaction()?;
        transaction.execute_batch(migration)?;
        transaction.execute_batch(&format!("PRAGMA user_version = {}", index + 1))?;
        transaction.commit()?;
    }
    Ok(())
}
