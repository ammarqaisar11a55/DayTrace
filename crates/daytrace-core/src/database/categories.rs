use rusqlite::{params, Connection, ErrorCode, OptionalExtension, Row};

use crate::error::{invalid, CoreError, Result};
use crate::models::{Category, CategoryInput};
use crate::time::now_ms;

const COLUMNS: &str = "id, name, color, created_at, updated_at";

fn from_row(row: &Row<'_>) -> rusqlite::Result<Category> {
    Ok(Category {
        id: row.get(0)?,
        name: row.get(1)?,
        color: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
    })
}

fn map_duplicate(error: rusqlite::Error) -> CoreError {
    match error.sqlite_error_code() {
        Some(ErrorCode::ConstraintViolation) => {
            CoreError::Invalid("A category with this name already exists".into())
        }
        _ => error.into(),
    }
}

pub fn list(connection: &Connection) -> Result<Vec<Category>> {
    let mut statement =
        connection.prepare(&format!("SELECT {COLUMNS} FROM categories ORDER BY name COLLATE NOCASE"))?;
    let rows = statement.query_map([], from_row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn get(connection: &Connection, id: i64) -> Result<Category> {
    connection
        .query_row(&format!("SELECT {COLUMNS} FROM categories WHERE id = ?1"), [id], from_row)
        .optional()?
        .ok_or(CoreError::NotFound("Category"))
}

pub fn create(connection: &Connection, input: &CategoryInput) -> Result<Category> {
    let name = input.name.trim();
    if name.is_empty() {
        return invalid("A category needs a name");
    }
    connection
        .execute(
            "INSERT INTO categories (name, color, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
            params![name, input.color, now_ms()],
        )
        .map_err(map_duplicate)?;
    get(connection, connection.last_insert_rowid())
}

pub fn update(connection: &Connection, id: i64, input: &CategoryInput) -> Result<Category> {
    let name = input.name.trim();
    if name.is_empty() {
        return invalid("A category needs a name");
    }
    let changed = connection
        .execute(
            "UPDATE categories SET name = ?2, color = ?3, updated_at = ?4 WHERE id = ?1",
            params![id, name, input.color, now_ms()],
        )
        .map_err(map_duplicate)?;
    if changed == 0 {
        return Err(CoreError::NotFound("Category"));
    }
    get(connection, id)
}

/// Deletes a category. Sessions using it keep their time and lose the category,
/// and rules that assigned it go with it.
pub fn delete(connection: &Connection, id: i64) -> Result<()> {
    connection.execute("DELETE FROM categories WHERE id = ?1", [id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::test_support::database;

    fn input(name: &str) -> CategoryInput {
        CategoryInput { name: name.into(), color: "#000000".into() }
    }

    #[test]
    fn duplicate_names_are_rejected_case_insensitively() {
        let (_dir, db) = database();
        let error = db.write(|c| create(c, &input("development"))).unwrap_err();
        assert!(error.to_string().contains("already exists"));
    }

    #[test]
    fn rename_and_delete() {
        let (_dir, db) = database();
        let category = db.write(|c| create(c, &input("Deep work"))).unwrap();
        let renamed = db.write(|c| update(c, category.id, &input("Focus"))).unwrap();
        assert_eq!(renamed.name, "Focus");
        db.write(|c| delete(c, category.id)).unwrap();
        assert!(db.read(|c| get(c, category.id)).is_err());
    }
}
