use rusqlite::{params, Connection};
use serde::Serialize;

use super::{Classifier, Subject};
use crate::database::{applications, rules};
use crate::error::Result;
use crate::time::now_ms;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleChange {
    pub session_id: i64,
    pub application_name: String,
    pub start_time: i64,
    pub duration_seconds: i64,
    pub old_category_id: Option<i64>,
    pub new_category_id: Option<i64>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RulePreview {
    pub examined: i64,
    pub changed: i64,
    pub skipped_manual: i64,
    pub samples: Vec<RuleChange>,
}

const SAMPLE_LIMIT: usize = 50;

struct Candidate {
    id: i64,
    application_id: Option<i64>,
    category_id: Option<i64>,
    start_time: i64,
    duration_seconds: i64,
    window_title: Option<String>,
    manual_override: bool,
}

fn candidates(connection: &Connection, from: i64, to: i64) -> Result<Vec<Candidate>> {
    let mut statement = connection.prepare(
        "SELECT id, application_id, category_id, start_time, duration_seconds,
                window_title, manual_override
         FROM activity_sessions
         WHERE session_type = 'active' AND end_time > ?1 AND start_time < ?2
         ORDER BY start_time",
    )?;
    let rows = statement.query_map(params![from, to], |row| {
        Ok(Candidate {
            id: row.get(0)?,
            application_id: row.get(1)?,
            category_id: row.get(2)?,
            start_time: row.get(3)?,
            duration_seconds: row.get(4)?,
            window_title: row.get(5)?,
            manual_override: row.get(6)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn classify_all(
    connection: &Connection,
    from: i64,
    to: i64,
    mut visit: impl FnMut(&Candidate, Option<i64>) -> Result<()>,
) -> Result<RulePreview> {
    let applications = applications::list(connection)?;
    let classifier = Classifier::new(rules::list(connection)?, &applications);

    let mut preview = RulePreview::default();
    for candidate in candidates(connection, from, to)? {
        preview.examined += 1;
        if candidate.manual_override {
            preview.skipped_manual += 1;
            continue;
        }
        let Some(application) =
            applications.iter().find(|application| Some(application.id) == candidate.application_id)
        else {
            continue;
        };

        let subject = Subject {
            identifier: &application.platform_identifier,
            name: &application.name,
            display_name: application.display_name.as_deref(),
            executable_path: application.executable_path.as_deref(),
            window_title: candidate.window_title.as_deref(),
        };
        // Rules add or change an assignment. When nothing applies to a session
        // it keeps what it has, so one new rule cannot erase earlier work.
        let category_id = classifier.classify(Some(application.id), &subject).or(candidate.category_id);
        if category_id == candidate.category_id {
            continue;
        }

        preview.changed += 1;
        if preview.samples.len() < SAMPLE_LIMIT {
            preview.samples.push(RuleChange {
                session_id: candidate.id,
                application_name: application.label().to_string(),
                start_time: candidate.start_time,
                duration_seconds: candidate.duration_seconds,
                old_category_id: candidate.category_id,
                new_category_id: category_id,
            });
        }
        visit(&candidate, category_id)?;
    }
    Ok(preview)
}

/// Shows what applying the current rules would change, without changing it.
pub fn preview(connection: &Connection, from: i64, to: i64) -> Result<RulePreview> {
    classify_all(connection, from, to, |_, _| Ok(()))
}

/// Reclassifies stored sessions. Sessions the user corrected are never touched.
pub fn apply(connection: &Connection, from: i64, to: i64) -> Result<RulePreview> {
    let now = now_ms();
    classify_all(connection, from, to, |candidate, category_id| {
        connection.execute(
            "UPDATE activity_sessions SET category_id = ?2, updated_at = ?3 WHERE id = ?1",
            params![candidate.id, category_id, now],
        )?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classification::{Condition, ConditionField, ConditionOperator, RuleInput};
    use crate::database::sessions::{self, insert_tracked, TrackedSession};
    use crate::database::test_support::database;
    use crate::database::Database;
    use crate::models::{SessionPatch, SessionType};

    const TARGET: i64 = 3;

    fn setup() -> (tempfile::TempDir, Database, i64, i64) {
        let (dir, db) = database();
        let app = db
            .write(|c| {
                let app = applications::upsert(c, "code", "Code", None, None)?;
                rules::create(
                    c,
                    &RuleInput {
                        name: "Code".into(),
                        conditions: vec![Condition {
                            field: ConditionField::Application,
                            operator: ConditionOperator::Equals,
                            value: "code".into(),
                        }],
                        target_category_id: TARGET,
                        enabled: true,
                    },
                )?;
                Ok(app)
            })
            .unwrap();
        let session = db
            .write(|c| {
                insert_tracked(
                    c,
                    &TrackedSession {
                        application_id: Some(app),
                        category_id: None,
                        start_time: 1_000,
                        end_time: 2_000,
                        session_type: SessionType::Active,
                        window_title: None,
                        label: None,
                        manual_override: false,
                        is_open: false,
                    },
                )
            })
            .unwrap();
        (dir, db, app, session)
    }

    #[test]
    fn preview_reports_changes_without_writing() {
        let (_dir, db, _, session) = setup();
        let result = db.read(|c| preview(c, 0, 10_000)).unwrap();
        assert_eq!(result.changed, 1);
        assert_eq!(result.samples[0].new_category_id, Some(TARGET));
        assert_eq!(db.read(|c| sessions::get(c, session)).unwrap().category_id, None);
    }

    #[test]
    fn apply_writes_and_then_finds_nothing_to_do() {
        let (_dir, db, _, session) = setup();
        db.write(|c| apply(c, 0, 10_000)).unwrap();
        assert_eq!(db.read(|c| sessions::get(c, session)).unwrap().category_id, Some(TARGET));
        assert_eq!(db.read(|c| preview(c, 0, 10_000)).unwrap().changed, 0);
    }

    #[test]
    fn manual_assignments_are_never_overwritten() {
        let (_dir, db, _, session) = setup();
        let patch = SessionPatch { category_id: Some(1), label: None, start_time: None, end_time: None };
        db.write(|c| sessions::update(c, session, &patch)).unwrap();
        let result = db.write(|c| apply(c, 0, 10_000)).unwrap();
        assert_eq!(result.changed, 0);
        assert_eq!(result.skipped_manual, 1);
    }

    #[test]
    fn sessions_no_rule_matches_keep_their_assignment() {
        let (_dir, db, _, _) = setup();
        let other = db.write(|c| applications::upsert(c, "slack", "Slack", None, None)).unwrap();
        let session = db
            .write(|c| {
                insert_tracked(
                    c,
                    &TrackedSession {
                        application_id: Some(other),
                        category_id: Some(1),
                        start_time: 3_000,
                        end_time: 4_000,
                        session_type: SessionType::Active,
                        window_title: None,
                        label: None,
                        manual_override: false,
                        is_open: false,
                    },
                )
            })
            .unwrap();
        db.write(|c| apply(c, 0, 10_000)).unwrap();
        assert_eq!(db.read(|c| sessions::get(c, session)).unwrap().category_id, Some(1));
    }

    #[test]
    fn sessions_outside_the_range_are_ignored() {
        let (_dir, db, _, _) = setup();
        assert_eq!(db.read(|c| preview(c, 5_000, 10_000)).unwrap().examined, 0);
    }
}
