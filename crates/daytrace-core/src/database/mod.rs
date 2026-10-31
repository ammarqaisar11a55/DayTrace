pub mod applications;
pub mod categories;
pub mod exclusions;
mod migrations;
pub mod rules;
pub mod sessions;
pub mod settings;
pub mod transfer;

use std::fs;
use std::io::ErrorKind;
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
         PRAGMA secure_delete = ON;
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

        self.remove_safety_copy()?;
        let safety_copy = self.safety_copy_path();
        self.backup_to(&safety_copy)?;

        let mut writer = lock(&self.writer);
        let copy = backup::Backup::new(&source, &mut writer)?;
        copy.run_to_completion(256, Duration::from_millis(10), None)?;
        drop(copy);
        writer.execute_batch("PRAGMA foreign_keys = ON; PRAGMA secure_delete = ON;")?;
        migrations::run(&mut writer)?;
        drop(writer);
        self.scrub()?;
        Ok(safety_copy)
    }

    /// Runs a deletion and then clears its traces from the database files.
    pub fn erase<T>(&self, change: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let result = self.write(change)?;
        self.scrub()?;
        Ok(result)
    }

    /// Deleted rows linger in the write-ahead log and in free pages. Compacting
    /// the database and emptying the log leaves them in neither file.
    fn scrub(&self) -> Result<()> {
        let writer = lock(&self.writer);
        writer.execute_batch("VACUUM;")?;
        let busy: i64 = writer.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| row.get(0))?;
        if busy != 0 {
            return Err(CoreError::Invalid(
                "The data was deleted, but the database log could not be cleared. Restart DayTrace and try again."
                    .into(),
            ));
        }
        Ok(())
    }

    /// Deletes every session and recorded application, and the copy a restore
    /// left beside the database. Categories, rules and exclusions are the user's
    /// own settings, so they stay.
    pub fn delete_all_history(&self) -> Result<usize> {
        let count = self.write(|connection| {
            let count = sessions::delete_all(connection)?;
            connection.execute("DELETE FROM applications", [])?;
            Ok(count)
        })?;
        self.remove_safety_copy()?;
        self.scrub()?;
        Ok(count)
    }

    /// Deletes sessions that ended before the cutoff, here and in the copy a
    /// restore left behind. The copy stays usable for undoing the restore.
    pub fn apply_retention(&self, cutoff: i64) -> Result<usize> {
        let count = self.write(|connection| sessions::delete_before(connection, cutoff))?;
        self.expire_safety_copy(cutoff)?;
        if count > 0 {
            self.scrub()?;
        }
        Ok(count)
    }

    fn safety_copy_path(&self) -> PathBuf {
        self.path.with_extension("before-restore.db")
    }

    fn remove_safety_copy(&self) -> Result<()> {
        for suffix in ["", "-wal", "-shm"] {
            let mut path = self.safety_copy_path().into_os_string();
            path.push(suffix);
            match fs::remove_file(path) {
                Err(error) if error.kind() != ErrorKind::NotFound => return Err(error.into()),
                _ => {}
            }
        }
        Ok(())
    }

    /// A copy that cannot be trimmed is removed: losing the undo is better than
    /// keeping data the user asked to expire.
    fn expire_safety_copy(&self, cutoff: i64) -> Result<()> {
        let path = self.safety_copy_path();
        if !path.exists() {
            return Ok(());
        }
        let trim = || -> Result<()> {
            let copy = Connection::open(&path)?;
            if sessions::delete_before(&copy, cutoff)? > 0 {
                copy.execute_batch("VACUUM;")?;
            }
            Ok(())
        };
        if trim().is_err() {
            self.remove_safety_copy()?;
        }
        Ok(())
    }

    pub fn integrity_ok(&self) -> Result<bool> {
        self.read(check_integrity)
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
    use crate::classification::{Condition, ConditionField, ConditionOperator, RuleInput};
    use crate::database::sessions::{insert_manual, list};
    use crate::models::{ExclusionKind, ManualSessionInput, SessionFilter};

    fn add_session(db: &Database, start: i64) {
        let input = ManualSessionInput {
            start_time: start,
            end_time: start + 1_000,
            label: Some("Work".into()),
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
        let categories: i64 =
            db.read(|c| Ok(c.query_row("SELECT COUNT(*) FROM categories", [], |row| row.get(0))?)).unwrap();
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
        let categories: i64 =
            db.read(|c| Ok(c.query_row("SELECT COUNT(*) FROM categories", [], |row| row.get(0))?)).unwrap();
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

    /// Writes a database as the first released schema left it, with one session.
    fn version_one_database(path: &Path) {
        let connection = Connection::open(path).unwrap();
        connection.execute_batch(include_str!("../../migrations/0001_initial.sql")).unwrap();
        connection
            .execute_batch(
                "PRAGMA user_version = 1;
                 INSERT INTO projects (id, name, color, created_at, updated_at) VALUES (5, 'Thesis', '#000', 0, 0);
                 INSERT INTO applications (id, platform_identifier, name, created_at, updated_at)
                    VALUES (3, 'code', 'Code', 0, 0);
                 INSERT INTO activity_sessions
                    (application_id, project_id, category_id, start_time, end_time, session_type,
                     created_at, updated_at)
                    VALUES (3, 5, 1, 1000, 61000, 'active', 0, 0);",
            )
            .unwrap();
    }

    fn assert_session_survived(db: &Database) {
        let filter = SessionFilter { from: 0, to: i64::MAX, ..Default::default() };
        let stored = db.read(|c| list(c, &filter)).unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].application_name.as_deref(), Some("Code"));
        assert_eq!(stored[0].category_name.as_deref(), Some("Development"));
        assert_eq!(stored[0].duration_seconds, 60);
        assert!(db.integrity_ok().unwrap());
    }

    #[test]
    fn opening_a_version_one_database_upgrades_it_in_place() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("old.db");
        version_one_database(&path);

        assert_session_survived(&Database::open(&path).unwrap());
        assert_session_survived(&Database::open(&path).unwrap());
    }

    #[test]
    fn restoring_a_version_one_backup_upgrades_it() {
        let (dir, db) = database();
        add_session(&db, 500_000);
        let backup = dir.path().join("old.db");
        version_one_database(&backup);

        db.restore_from(&backup).unwrap();
        assert_session_survived(&db);
        assert!(db.write(foreign_keys_are_enforced).unwrap());
    }

    fn foreign_keys_are_enforced(connection: &Connection) -> Result<bool> {
        let refused = connection.execute(
            "INSERT INTO classification_rules
                (name, rule_condition, target_category_id, priority, created_at, updated_at)
                VALUES ('x', '[]', 9999, 1, 0, 0)",
            [],
        );
        Ok(refused.is_err())
    }

    /// A database holding the two sessions, after a restore that left a safety
    /// copy of the same two sessions.
    fn restored_database(first: i64, second: i64) -> (tempfile::TempDir, Database, PathBuf) {
        let (dir, db) = database();
        add_session(&db, first);
        add_session(&db, second);
        let backup = dir.path().join("backup.db");
        db.backup_to(&backup).unwrap();
        let safety = db.restore_from(&backup).unwrap();
        (dir, db, safety)
    }

    fn sessions_in(path: &Path) -> i64 {
        Connection::open(path)
            .unwrap()
            .query_row("SELECT COUNT(*) FROM activity_sessions", [], |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn deleting_all_history_also_removes_the_safety_copy() {
        let (_dir, db, safety) = restored_database(1_000, 5_000);
        assert_eq!(sessions_in(&safety), 2);
        let wal = safety.with_extension("db-wal");
        fs::write(&wal, "leftover").unwrap();

        assert_eq!(db.delete_all_history().unwrap(), 2);

        assert_eq!(count(&db), 0);
        assert!(!safety.exists(), "the old history must not stay on disk");
        assert!(!wal.exists());
    }

    #[test]
    fn deleting_all_history_works_without_a_safety_copy() {
        let (_dir, db) = database();
        add_session(&db, 1_000);
        assert_eq!(db.delete_all_history().unwrap(), 1);
    }

    #[test]
    fn retention_reaches_the_safety_copy_and_keeps_it_usable() {
        let (_dir, db, safety) = restored_database(1_000, 9_000);

        assert_eq!(db.apply_retention(5_000).unwrap(), 1);

        assert_eq!(count(&db), 1);
        assert_eq!(sessions_in(&safety), 1, "the expired session must leave the copy too");
        assert!(db.restore_from(&safety).is_ok(), "the copy can still undo the restore");
    }

    #[test]
    fn retention_trims_the_copy_even_when_the_live_database_has_nothing_to_expire() {
        let (_dir, db, safety) = restored_database(1_000, 9_000);
        db.write(|c| sessions::delete_before(c, 5_000)).unwrap();

        assert_eq!(db.apply_retention(5_000).unwrap(), 0);
        assert_eq!(sessions_in(&safety), 1);
    }

    #[test]
    fn a_safety_copy_that_cannot_be_trimmed_is_removed() {
        let (_dir, db, safety) = restored_database(1_000, 9_000);
        fs::write(&safety, "this is not a database").unwrap();

        db.apply_retention(5_000).unwrap();

        assert!(!safety.exists());
    }

    const SECRET: &[u8] = b"SECRET-LABEL";

    /// Searches the raw files, as someone reading the disk would.
    fn left_on_disk(db: &Database, needle: &[u8]) -> Vec<String> {
        ["", "-wal"]
            .iter()
            .filter_map(|suffix| {
                let mut path = db.path().to_path_buf().into_os_string();
                path.push(suffix);
                let bytes = fs::read(&path).unwrap_or_default();
                bytes.windows(needle.len()).any(|window| window == needle).then(|| format!("file{suffix}"))
            })
            .collect()
    }

    fn add_secret_sessions(db: &Database, application: Option<i64>) {
        for index in 0..200 {
            let input = ManualSessionInput {
                start_time: 1_000 + index * 2_000,
                end_time: 2_000 + index * 2_000,
                label: Some(format!("SECRET-LABEL-{index}")),
                category_id: None,
            };
            let id = db.write(|c| insert_manual(c, &input)).unwrap().id;
            if application.is_some() {
                db.write(|c| {
                    c.execute(
                        "UPDATE activity_sessions SET application_id = ?1 WHERE id = ?2",
                        (application, id),
                    )?;
                    Ok(())
                })
                .unwrap();
            }
        }
    }

    #[test]
    fn deleted_history_cannot_be_read_back_from_the_files() {
        let (_dir, db) = database();
        add_secret_sessions(&db, None);
        assert!(!left_on_disk(&db, SECRET).is_empty(), "the search must find live data");

        db.delete_all_history().unwrap();

        assert_eq!(left_on_disk(&db, SECRET), Vec::<String>::new());
        assert!(db.integrity_ok().unwrap());
    }

    #[test]
    fn deleting_all_history_clears_applications_but_keeps_the_users_settings() {
        let (_dir, db) = database();
        db.write(|c| {
            let application =
                applications::upsert(c, "secret-app", "SECRET-APP", Some("/opt/SECRET-PATH"), None)?;
            let input = RuleInput {
                name: "Keep me".into(),
                conditions: vec![Condition {
                    field: ConditionField::Application,
                    operator: ConditionOperator::Equals,
                    value: "code".into(),
                }],
                target_category_id: 1,
                enabled: true,
            };
            rules::create(c, &input)?;
            exclusions::add(c, ExclusionKind::Path, "/private")?;
            Ok(application)
        })
        .unwrap();

        db.delete_all_history().unwrap();

        assert!(db.read(applications::list).unwrap().is_empty());
        assert_eq!(left_on_disk(&db, b"SECRET-PATH"), Vec::<String>::new());
        assert_eq!(db.read(rules::list).unwrap().len(), 1);
        assert_eq!(db.read(exclusions::list).unwrap().len(), 1);
        assert_eq!(db.read(categories::list).unwrap().len(), 9);
    }

    #[test]
    fn deleting_one_applications_history_leaves_no_trace_of_it() {
        let (_dir, db) = database();
        let application = db.write(|c| applications::upsert(c, "secret-app", "Secret", None, None)).unwrap();
        add_secret_sessions(&db, Some(application));

        db.erase(|c| applications::delete_with_history(c, application)).unwrap();

        assert_eq!(left_on_disk(&db, SECRET), Vec::<String>::new());
    }

    #[test]
    fn expired_sessions_cannot_be_read_back_from_the_files() {
        let (_dir, db) = database();
        add_secret_sessions(&db, None);

        assert_eq!(db.apply_retention(i64::MAX).unwrap(), 200);

        assert_eq!(left_on_disk(&db, SECRET), Vec::<String>::new());
    }

    #[test]
    fn the_database_keeps_working_after_being_scrubbed() {
        let (_dir, db) = database();
        add_session(&db, 1_000);
        db.delete_all_history().unwrap();
        add_session(&db, 5_000);
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
