use std::io::Write;

use rusqlite::Connection;
use serde::Serialize;

use crate::config::Settings;
use crate::database::{applications, categories, exclusions, projects, rules, sessions, settings};
use crate::error::Result;
use crate::models::{Application, Category, Exclusion, Project, Session, SessionFilter};
use crate::projects::Rule;
use crate::time::now_ms;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonalData {
    pub exported_at: i64,
    pub settings: Settings,
    pub applications: Vec<Application>,
    pub projects: Vec<Project>,
    pub categories: Vec<Category>,
    pub rules: Vec<Rule>,
    pub exclusions: Vec<Exclusion>,
    pub sessions: Vec<Session>,
}

fn all_sessions(connection: &Connection) -> Result<Vec<Session>> {
    let filter = SessionFilter { from: i64::MIN, to: i64::MAX, ..Default::default() };
    sessions::list(connection, &filter)
}

pub fn collect(connection: &Connection) -> Result<PersonalData> {
    Ok(PersonalData {
        exported_at: now_ms(),
        settings: settings::load(connection)?,
        applications: applications::list(connection)?,
        projects: projects::list(connection, true)?,
        categories: categories::list(connection)?,
        rules: rules::list(connection)?,
        exclusions: exclusions::list(connection)?,
        sessions: all_sessions(connection)?,
    })
}

/// Quotes a CSV field, and neutralizes values a spreadsheet would run as a
/// formula. Window titles are attacker-influenced text.
fn csv_field(value: &str) -> String {
    let guarded = if value.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{value}")
    } else {
        value.to_string()
    };
    if guarded.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", guarded.replace('"', "\"\""))
    } else {
        guarded
    }
}

pub fn write_csv(connection: &Connection, output: &mut impl Write) -> Result<()> {
    writeln!(
        output,
        "id,type,application,project,category,label,window_title,start_utc,end_utc,duration_seconds"
    )?;
    for session in all_sessions(connection)? {
        let timestamp = |ms: i64| {
            chrono::DateTime::from_timestamp_millis(ms).map(|t| t.to_rfc3339()).unwrap_or_default()
        };
        let optional = |value: &Option<String>| csv_field(value.as_deref().unwrap_or(""));
        writeln!(
            output,
            "{},{},{},{},{},{},{},{},{},{}",
            session.id,
            session.session_type.as_str(),
            optional(&session.application_name),
            optional(&session.project_name),
            optional(&session.category_name),
            optional(&session.label),
            optional(&session.window_title),
            timestamp(session.start_time),
            timestamp(session.end_time),
            session.duration_seconds
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::test_support::database;
    use crate::models::ManualSessionInput;

    #[test]
    fn csv_fields_are_quoted_and_formulas_neutralized() {
        assert_eq!(csv_field("plain"), "plain");
        assert_eq!(csv_field("a,b"), "\"a,b\"");
        assert_eq!(csv_field("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(csv_field("=HYPERLINK(\"x\")"), "\"'=HYPERLINK(\"\"x\"\")\"");
        assert_eq!(csv_field("-1+1"), "'-1+1");
    }

    #[test]
    fn exports_contain_recorded_sessions() {
        let (_dir, db) = database();
        let input = ManualSessionInput {
            start_time: 1_000,
            end_time: 61_000,
            label: Some("Writing, thesis".into()),
            project_id: None,
            category_id: None,
        };
        db.write(|c| sessions::insert_manual(c, &input)).unwrap();

        let data = db.read(collect).unwrap();
        assert_eq!(data.sessions.len(), 1);
        assert_eq!(data.categories.len(), 9);

        let mut csv = Vec::new();
        db.read(|c| write_csv(c, &mut csv)).unwrap();
        let csv = String::from_utf8(csv).unwrap();
        assert_eq!(csv.lines().count(), 2);
        assert!(csv.contains("\"Writing, thesis\""));
        assert!(csv.contains(",60"));
    }
}
