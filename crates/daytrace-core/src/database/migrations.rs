use rusqlite::Connection;

use crate::error::{CoreError, Result};

const MIGRATIONS: &[&str] = &[
    include_str!("../../migrations/0001_initial.sql"),
    include_str!("../../migrations/0002_remove_projects.sql"),
];

pub const LATEST_VERSION: i64 = MIGRATIONS.len() as i64;

/// Applies pending migrations, each in its own transaction. Foreign keys are off
/// while they run, because rebuilding a table would otherwise fire its delete
/// actions on every row that references it. They are checked before committing.
pub fn run(connection: &mut Connection) -> Result<()> {
    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version > LATEST_VERSION {
        return Err(CoreError::NewerSchema);
    }

    connection.execute_batch("PRAGMA foreign_keys = OFF;")?;
    let result = apply(connection, version as usize);
    connection.execute_batch("PRAGMA foreign_keys = ON;")?;
    result
}

fn apply(connection: &mut Connection, from: usize) -> Result<()> {
    for (index, migration) in MIGRATIONS.iter().enumerate().skip(from) {
        let transaction = connection.transaction()?;
        transaction.execute_batch(migration)?;

        let broken = transaction.prepare("PRAGMA foreign_key_check")?.query_map([], |_| Ok(()))?.count();
        if broken > 0 {
            return Err(CoreError::Invalid(format!("Migration {} broke {broken} references", index + 1)));
        }

        transaction.execute_batch(&format!("PRAGMA user_version = {}", index + 1))?;
        transaction.commit()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A database as version 1 of the application left it, with projects in use.
    fn version_one() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        connection.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        connection.execute_batch(MIGRATIONS[0]).unwrap();
        connection
            .execute_batch(
                "PRAGMA user_version = 1;
                 INSERT INTO projects (id, name, color, created_at, updated_at) VALUES (5, 'Thesis', '#000', 0, 0);
                 UPDATE categories SET default_project_id = 5 WHERE id = 1;
                 INSERT INTO applications
                    (id, platform_identifier, name, default_project_id, default_category_id, created_at, updated_at)
                    VALUES (3, 'code', 'Code', 5, 2, 0, 0);
                 INSERT INTO activity_sessions
                    (id, application_id, project_id, category_id, start_time, end_time, session_type,
                     window_title, label, manual_override, is_open, created_at, updated_at)
                    VALUES (9, 3, 5, 1, 1000, 61000, 'active', 'A title', 'A label', 1, 1, 7, 8);
                 INSERT INTO classification_rules
                    (id, name, rule_condition, target_project_id, priority, created_at, updated_at)
                    VALUES (1, 'Only a project', '[]', 5, 10, 0, 0);
                 INSERT INTO classification_rules
                    (id, name, rule_condition, target_project_id, target_category_id, priority, created_at, updated_at)
                    VALUES (2, 'Both', '[]', 5, 2, 20, 0, 0);
                 INSERT INTO exclusions (kind, value, created_at) VALUES ('project', '5', 0), ('path', '/private', 0);",
            )
            .unwrap();
        connection
    }

    fn columns(connection: &Connection, table: &str) -> Vec<String> {
        let mut statement =
            connection.prepare(&format!("SELECT name FROM pragma_table_xinfo('{table}')")).unwrap();
        statement.query_map([], |row| row.get(0)).unwrap().map(|name| name.unwrap()).collect()
    }

    #[test]
    fn removing_projects_keeps_sessions_and_their_links() {
        let mut connection = version_one();
        run(&mut connection).unwrap();

        let session: (i64, i64, i64, i64, String, String, bool, bool, i64) = connection
            .query_row(
                "SELECT application_id, category_id, start_time, duration_seconds, window_title, label,
                        manual_override, is_open, updated_at
                 FROM activity_sessions WHERE id = 9",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                        row.get(8)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(session, (3, 1, 1000, 60, "A title".into(), "A label".into(), true, true, 8));

        let default_category: i64 = connection
            .query_row("SELECT default_category_id FROM applications WHERE id = 3", [], |row| row.get(0))
            .unwrap();
        assert_eq!(default_category, 2);
    }

    #[test]
    fn nothing_about_projects_is_left_behind() {
        let mut connection = version_one();
        run(&mut connection).unwrap();

        let tables: i64 = connection
            .query_row("SELECT COUNT(*) FROM sqlite_master WHERE name = 'projects'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(tables, 0);
        for table in ["activity_sessions", "applications", "categories", "classification_rules"] {
            assert!(!columns(&connection, table).iter().any(|column| column.contains("project")), "{table}");
        }
        let schema: String = connection
            .query_row("SELECT group_concat(sql) FROM sqlite_master WHERE sql IS NOT NULL", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert!(!schema.contains("project"));
    }

    #[test]
    fn rules_and_exclusions_that_only_concerned_projects_go() {
        let mut connection = version_one();
        run(&mut connection).unwrap();

        let rules: Vec<i64> = connection
            .prepare("SELECT id FROM classification_rules")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(|id| id.unwrap())
            .collect();
        assert_eq!(rules, vec![2]);

        let exclusions: Vec<String> = connection
            .prepare("SELECT kind FROM exclusions")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(|kind| kind.unwrap())
            .collect();
        assert_eq!(exclusions, vec!["path"]);
    }

    #[test]
    fn foreign_keys_work_again_afterwards() {
        let mut connection = version_one();
        run(&mut connection).unwrap();

        let enabled: bool = connection.query_row("PRAGMA foreign_keys", [], |row| row.get(0)).unwrap();
        assert!(enabled);

        connection.execute("DELETE FROM categories WHERE id = 1", []).unwrap();
        let category: Option<i64> = connection
            .query_row("SELECT category_id FROM activity_sessions WHERE id = 9", [], |row| row.get(0))
            .unwrap();
        assert_eq!(category, None);

        connection.execute("DELETE FROM categories WHERE id = 2", []).unwrap();
        let rules: i64 =
            connection.query_row("SELECT COUNT(*) FROM classification_rules", [], |row| row.get(0)).unwrap();
        assert_eq!(rules, 0, "a rule's category going takes the rule with it");
    }
}
