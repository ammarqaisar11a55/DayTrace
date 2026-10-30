use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::error::{invalid, CoreError, Result};
use crate::models::{Project, ProjectInput};
use crate::time::now_ms;

const COLUMNS: &str = "id, parent_id, name, description, color, status, created_at, updated_at";

fn from_row(row: &Row<'_>) -> rusqlite::Result<Project> {
    Ok(Project {
        id: row.get(0)?,
        parent_id: row.get(1)?,
        name: row.get(2)?,
        description: row.get(3)?,
        color: row.get(4)?,
        status: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn validate(input: &ProjectInput) -> Result<&str> {
    let name = input.name.trim();
    if name.is_empty() {
        return invalid("A project needs a name");
    }
    Ok(name)
}

pub fn list(connection: &Connection, include_archived: bool) -> Result<Vec<Project>> {
    let filter = if include_archived { "" } else { "WHERE status = 'active'" };
    let mut statement = connection
        .prepare(&format!("SELECT {COLUMNS} FROM projects {filter} ORDER BY name COLLATE NOCASE"))?;
    let rows = statement.query_map([], from_row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn get(connection: &Connection, id: i64) -> Result<Project> {
    connection
        .query_row(&format!("SELECT {COLUMNS} FROM projects WHERE id = ?1"), [id], from_row)
        .optional()?
        .ok_or(CoreError::NotFound("Project"))
}

pub fn create(connection: &Connection, input: &ProjectInput) -> Result<Project> {
    let name = validate(input)?;
    let now = now_ms();
    connection.execute(
        "INSERT INTO projects (parent_id, name, description, color, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        params![input.parent_id, name, input.description.trim(), input.color, now],
    )?;
    get(connection, connection.last_insert_rowid())
}

fn would_create_cycle(connection: &Connection, id: i64, parent_id: i64) -> Result<bool> {
    Ok(connection.query_row(
        "WITH RECURSIVE ancestors (id, parent_id) AS (
            SELECT id, parent_id FROM projects WHERE id = ?2
            UNION
            SELECT p.id, p.parent_id FROM projects p JOIN ancestors a ON p.id = a.parent_id
         )
         SELECT EXISTS (SELECT 1 FROM ancestors WHERE id = ?1)",
        params![id, parent_id],
        |row| row.get(0),
    )?)
}

pub fn update(connection: &Connection, id: i64, input: &ProjectInput) -> Result<Project> {
    let name = validate(input)?;
    if let Some(parent_id) = input.parent_id {
        if would_create_cycle(connection, id, parent_id)? {
            return invalid("A project cannot be placed inside itself");
        }
    }
    let changed = connection.execute(
        "UPDATE projects
         SET parent_id = ?2, name = ?3, description = ?4, color = ?5, updated_at = ?6
         WHERE id = ?1",
        params![id, input.parent_id, name, input.description.trim(), input.color, now_ms()],
    )?;
    if changed == 0 {
        return Err(CoreError::NotFound("Project"));
    }
    get(connection, id)
}

pub fn set_archived(connection: &Connection, id: i64, archived: bool) -> Result<()> {
    let status = if archived { "archived" } else { "active" };
    let changed = connection.execute(
        "UPDATE projects SET status = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, status, now_ms()],
    )?;
    if changed == 0 {
        return Err(CoreError::NotFound("Project"));
    }
    Ok(())
}

/// Deletes a project. Its sessions remain and become unassigned, and its
/// children move up to the top level. A rule that assigned nothing else goes
/// with it.
pub fn delete(connection: &Connection, id: i64) -> Result<()> {
    connection.execute(
        "DELETE FROM classification_rules WHERE target_project_id = ?1 AND target_category_id IS NULL",
        [id],
    )?;
    connection.execute("DELETE FROM exclusions WHERE kind = 'project' AND value = ?1", [id.to_string()])?;
    connection.execute("DELETE FROM projects WHERE id = ?1", [id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::test_support::database;

    fn input(name: &str, parent_id: Option<i64>) -> ProjectInput {
        ProjectInput { parent_id, name: name.into(), description: String::new(), color: "#2563eb".into() }
    }

    #[test]
    fn hierarchy_and_archive() {
        let (_dir, db) = database();
        let parent = db.write(|c| create(c, &input("University", None))).unwrap();
        let child = db.write(|c| create(c, &input("Databases", Some(parent.id)))).unwrap();
        assert_eq!(child.parent_id, Some(parent.id));

        db.write(|c| set_archived(c, child.id, true)).unwrap();
        assert_eq!(db.read(|c| list(c, false)).unwrap().len(), 1);
        assert_eq!(db.read(|c| list(c, true)).unwrap().len(), 2);
    }

    #[test]
    fn cycles_are_rejected() {
        let (_dir, db) = database();
        let a = db.write(|c| create(c, &input("A", None))).unwrap();
        let b = db.write(|c| create(c, &input("B", Some(a.id)))).unwrap();
        assert!(db.write(|c| update(c, a.id, &input("A", Some(b.id)))).is_err());
        assert!(db.write(|c| update(c, a.id, &input("A", Some(a.id)))).is_err());
    }

    #[test]
    fn blank_names_are_rejected() {
        let (_dir, db) = database();
        assert!(db.write(|c| create(c, &input("   ", None))).is_err());
    }

    #[test]
    fn deleting_a_parent_keeps_children() {
        let (_dir, db) = database();
        let parent = db.write(|c| create(c, &input("P", None))).unwrap();
        let child = db.write(|c| create(c, &input("C", Some(parent.id)))).unwrap();
        db.write(|c| delete(c, parent.id)).unwrap();
        assert_eq!(db.read(|c| get(c, child.id)).unwrap().parent_id, None);
    }
}
