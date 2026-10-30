pub mod applications;
pub mod categories;
pub mod exclusions;
mod migrations;
pub mod projects;
pub mod rules;
pub mod sessions;
pub mod settings;
pub mod transfer;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use rusqlite::{backup, Connection, OpenFlags};

use crate::error::{CoreError, Result};

pub use migrations::LATEST_VERSION;

/// Separate reader and writer connections let the UI query while the tracker
/// flushes. WAL mode keeps them from blocking each other.
pub struct Database {
    writer: Mutex<Connection>,
    reader: Mutex<Connection>,
    path: PathBuf,
}

fn lock(connection: &Mutex<Connection>) -> MutexGuard<'_, Connection> {
    connection.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn connect(path: &Path) -> Result<Connection> {
    let connection = Connection::open(path)?;
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA foreign_keys = ON;
         PRAGMA temp_store = MEMORY;",
    )?;
    Ok(connection)
}

#[cfg(unix)]
fn restrict_permissions(path: &Path, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
    Ok(())
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &Path, _mode: u32) -> Result<()> {
    Ok(())
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
            restrict_permissions(parent, 0o700)?;
        }

        let mut writer = connect(&path)?;
        restrict_permissions(&path, 0o600)?;
        migrations::run(&mut writer)?;
        let reader = connect(&path)?;

        Ok(Self { writer: Mutex::new(writer), reader: Mutex::new(reader), path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn read<T>(&self, query: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        query(&lock(&self.reader))
    }

    /// Runs `change` in a transaction, which rolls back if it returns an error.
    pub fn write<T>(&self, change: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let mut connection = lock(&self.writer);
        let transaction = connection.transaction()?;
        let result = change(&transaction)?;
        transaction.commit()?;
        Ok(result)
    }

    pub fn backup_to(&self, target: &Path) -> Result<()> {
        let source = lock(&self.reader);
        let mut destination = Connection::open(target)?;
        let copy = backup::Backup::new(&source, &mut destination)?;
        copy.run_to_completion(256, Duration::from_millis(10), None)?;
        drop(copy);
        restrict_permissions(target, 0o600)
    }

    /// Replaces all data with a validated backup. The previous database is
    /// kept beside the live one in case the restored data is not what the user
    /// expected.
    pub fn restore_from(&self, source_path: &Path) -> Result<PathBuf> {
        let source = Connection::open_with_flags(source_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        validate_backup(&source)?;

        let safety_copy = self.path.with_extension("before-restore.db");
        if safety_copy.exists() {
            fs::remove_file(&safety_copy)?;
        }
        self.backup_to(&safety_copy)?;

        let mut writer = lock(&self.writer);
        let copy = backup::Backup::new(&source, &mut writer)?;
        copy.run_to_completion(256, Duration::from_millis(10), None)?;
        drop(copy);
        writer.execute_batch("PRAGMA foreign_keys = ON;")?;
        migrations::run(&mut writer)?;
        Ok(safety_copy)
    }

    pub fn integrity_ok(&self) -> Result<bool> {
        self.read(|connection| check_integrity(connection))
    }
}

fn check_integrity(connection: &Connection) -> Result<bool> {
    let result: String = connection.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    Ok(result == "ok")
}

fn validate_backup(source: &Connection) -> Result<()> {
    let version: i64 = source.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version > LATEST_VERSION {
        return Err(CoreError::NewerSchema);
    }
    if !check_integrity(source)? {
        return Err(CoreError::Invalid("The backup file is damaged".into()));
    }
    let has_sessions: bool = source.query_row(
        "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'activity_sessions')",
        [],
        |row| row.get(0),
    )?;
    if !has_sessions || version < 1 {
        return Err(CoreError::Invalid("This file is not a DayTrace backup".into()));
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::Database;
    use tempfile::TempDir;

    pub fn database() -> (TempDir, Database) {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("test.db")).unwrap();
        (directory, database)
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::database;
    use super::*;
    use crate::database::sessions::{insert_manual, list};
    use crate::models::{ManualSessionInput, SessionFilter};

    fn add_session(db: &Database, start: i64) {
        let input = ManualSessionInput {
            start_time: start,
            end_time: start + 1_000,
            label: Some("Work".into()),
            project_id: None,
            category_id: None,
        };
        db.write(|c| insert_manual(c, &input)).unwrap();
    }

    fn count(db: &Database) -> usize {
        let filter = SessionFilter { from: 0, to: i64::MAX, ..Default::default() };
        db.read(|c| list(c, &filter)).unwrap().len()
    }

    #[test]
    fn opening_runs_migrations_and_seeds_categories() {
        let (_dir, db) = database();
        let (version, categories): (i64, i64) = db
            .read(|c| {
                let version = c.query_row("PRAGMA user_version", [], |row| row.get(0))?;
                let total = c.query_row("SELECT COUNT(*) FROM categories", [], |row| row.get(0))?;
                Ok((version, total))
            })
            .unwrap();
        assert_eq!(version, LATEST_VERSION);
        assert_eq!(categories, 9);
        assert!(db.integrity_ok().unwrap());
    }

    #[test]
    fn reopening_does_not_rerun_migrations() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("test.db");
        Database::open(&path).unwrap();
        let db = Database::open(&path).unwrap();
        let categories: i64 = db
            .read(|c| Ok(c.query_row("SELECT COUNT(*) FROM categories", [], |row| row.get(0))?))
            .unwrap();
        assert_eq!(categories, 9);
    }

    #[test]
    fn failed_write_rolls_back() {
        let (_dir, db) = database();
        let result: Result<()> = db.write(|c| {
            c.execute("DELETE FROM categories", [])?;
            Err(CoreError::Invalid("stop".into()))
        });
        assert!(result.is_err());
        let categories: i64 = db
            .read(|c| Ok(c.query_row("SELECT COUNT(*) FROM categories", [], |row| row.get(0))?))
            .unwrap();
        assert_eq!(categories, 9);
    }

    #[test]
    fn newer_schema_is_refused() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("test.db");
        Database::open(&path).unwrap();
        let connection = Connection::open(&path).unwrap();
        connection.execute_batch(&format!("PRAGMA user_version = {}", LATEST_VERSION + 1)).unwrap();
        drop(connection);
        assert!(matches!(Database::open(&path), Err(CoreError::NewerSchema)));
    }

    #[test]
    fn backup_and_restore_round_trip() {
        let (dir, db) = database();
        add_session(&db, 1_000);
        let backup = dir.path().join("backup.db");
        db.backup_to(&backup).unwrap();

        add_session(&db, 5_000);
        assert_eq!(count(&db), 2);

        let safety = db.restore_from(&backup).unwrap();
        assert_eq!(count(&db), 1);
        assert!(safety.exists());
        assert!(db.integrity_ok().unwrap());
    }

    #[test]
    fn restore_rejects_unrelated_databases() {
        let (dir, db) = database();
        add_session(&db, 1_000);
        let other = dir.path().join("other.db");
        Connection::open(&other).unwrap().execute_batch("CREATE TABLE t (a);").unwrap();
        assert!(db.restore_from(&other).is_err());
        assert_eq!(count(&db), 1);
    }

    #[cfg(unix)]
    #[test]
    fn database_is_private_to_the_user() {
        use std::os::unix::fs::PermissionsExt;
        let (_dir, db) = database();
        let mode = fs::metadata(db.path()).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
