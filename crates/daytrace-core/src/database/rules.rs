use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::error::{CoreError, Result};
use crate::projects::{Rule, RuleInput};
use crate::time::now_ms;

const COLUMNS: &str =
    "id, name, rule_condition, target_project_id, target_category_id, priority, enabled";

fn from_row(row: &Row<'_>) -> rusqlite::Result<(Rule, String)> {
    Ok((
        Rule {
            id: row.get(0)?,
            name: row.get(1)?,
            conditions: Vec::new(),
            target_project_id: row.get(3)?,
            target_category_id: row.get(4)?,
            priority: row.get(5)?,
            enabled: row.get(6)?,
        },
        row.get(2)?,
    ))
}

fn parse((mut rule, conditions): (Rule, String)) -> Result<Rule> {
    rule.conditions = serde_json::from_str(&conditions)?;
    Ok(rule)
}

pub fn list(connection: &Connection) -> Result<Vec<Rule>> {
    let mut statement = connection.prepare(&format!(
        "SELECT {COLUMNS} FROM classification_rules ORDER BY priority, id"
    ))?;
    let rows = statement.query_map([], from_row)?;
    rows.map(|row| parse(row?)).collect()
}

pub fn get(connection: &Connection, id: i64) -> Result<Rule> {
    let row = connection
        .query_row(
            &format!("SELECT {COLUMNS} FROM classification_rules WHERE id = ?1"),
            [id],
            from_row,
        )
        .optional()?
        .ok_or(CoreError::NotFound("Rule"))?;
    parse(row)
}

pub fn create(connection: &Connection, input: &RuleInput) -> Result<Rule> {
    input.validate()?;
    let priority: i64 = connection.query_row(
        "SELECT COALESCE(MAX(priority), 0) + 10 FROM classification_rules",
        [],
        |row| row.get(0),
    )?;
    connection.execute(
        "INSERT INTO classification_rules
            (name, rule_condition, target_project_id, target_category_id, priority, enabled, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
        params![
            input.name.trim(),
            serde_json::to_string(&input.conditions)?,
            input.target_project_id,
            input.target_category_id,
            priority,
            input.enabled,
            now_ms()
        ],
    )?;
    get(connection, connection.last_insert_rowid())
}

pub fn update(connection: &Connection, id: i64, input: &RuleInput) -> Result<Rule> {
    input.validate()?;
    let changed = connection.execute(
        "UPDATE classification_rules
         SET name = ?2, rule_condition = ?3, target_project_id = ?4, target_category_id = ?5,
             enabled = ?6, updated_at = ?7
         WHERE id = ?1",
        params![
            id,
            input.name.trim(),
            serde_json::to_string(&input.conditions)?,
            input.target_project_id,
            input.target_category_id,
            input.enabled,
            now_ms()
        ],
    )?;
    if changed == 0 {
        return Err(CoreError::NotFound("Rule"));
    }
    get(connection, id)
}

pub fn delete(connection: &Connection, id: i64) -> Result<()> {
    connection.execute("DELETE FROM classification_rules WHERE id = ?1", [id])?;
    Ok(())
}

/// Renumbers rules to match the given order, first id evaluated first.
pub fn reorder(connection: &Connection, ids: &[i64]) -> Result<()> {
    let mut statement =
        connection.prepare("UPDATE classification_rules SET priority = ?2, updated_at = ?3 WHERE id = ?1")?;
    let now = now_ms();
    for (index, id) in ids.iter().enumerate() {
        statement.execute(params![id, (index as i64 + 1) * 10, now])?;
    }
    Ok(())
}
