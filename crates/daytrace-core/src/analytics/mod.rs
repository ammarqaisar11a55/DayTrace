//! Statistics computed directly from stored sessions.
//!
//! Sessions are clipped to the requested range and split at local day and hour
//! boundaries, so midnight-crossing sessions and daylight saving days stay
//! correct. Nothing here is cached: the sessions are the source of truth.

use std::collections::{BTreeMap, HashMap};

use chrono::{Datelike, Days, Local, NaiveDate, TimeZone, Timelike, Utc};
use rusqlite::{params, Connection};
use serde::Serialize;

use crate::error::Result;
use crate::models::SessionType;
use crate::time::{local_date, local_range, monday_of, now_ms, offset_ms, parse_date, start_of_day, HOUR_MS};

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Totals {
    pub active_ms: i64,
    pub idle_ms: i64,
    pub unknown_ms: i64,
    pub manual_ms: i64,
    pub total_ms: i64,
    pub untracked_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RankedItem {
    pub id: Option<i64>,
    pub name: String,
    pub color: Option<String>,
    pub ms: i64,
    pub share: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bucket {
    pub key: String,
    pub active_ms: i64,
    pub idle_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HourCell {
    pub weekday: u32,
    pub hour: u32,
    pub ms: i64,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub from: String,
    pub to: String,
    pub totals: Totals,
    pub applications: Vec<RankedItem>,
    pub projects: Vec<RankedItem>,
    pub categories: Vec<RankedItem>,
    pub hourly: Vec<Bucket>,
    pub daily: Vec<Bucket>,
    pub weekly: Vec<Bucket>,
    pub monthly: Vec<Bucket>,
    pub heatmap: Vec<HourCell>,
}

struct Row {
    start: i64,
    end: i64,
    session_type: SessionType,
    application_id: Option<i64>,
    project_id: Option<i64>,
    category_id: Option<i64>,
}

fn load_rows(connection: &Connection, from: i64, to: i64) -> Result<Vec<Row>> {
    let mut statement = connection.prepare_cached(
        "SELECT start_time, end_time, session_type, application_id, project_id, category_id
         FROM activity_sessions
         WHERE end_time > ?1 AND start_time < ?2
         ORDER BY start_time",
    )?;
    let rows = statement.query_map(params![from, to], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get::<_, String>(2)?, row.get(3)?, row.get(4)?, row.get(5)?))
    })?;

    let mut result = Vec::new();
    for row in rows {
        let (start, end, session_type, application_id, project_id, category_id) = row?;
        result.push(Row {
            start,
            end,
            session_type: SessionType::parse(&session_type)?,
            application_id,
            project_id,
            category_id,
        });
    }
    Ok(result)
}

type Names = HashMap<i64, (String, Option<String>)>;

fn load_names(connection: &Connection, sql: &str) -> Result<Names> {
    let mut statement = connection.prepare_cached(sql)?;
    let rows = statement.query_map([], |row| Ok((row.get(0)?, (row.get(1)?, row.get(2)?))))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn rank(
    usage: HashMap<Option<i64>, i64>,
    names: &Names,
    unassigned: &str,
    total: i64,
) -> Vec<RankedItem> {
    let mut items: Vec<RankedItem> = usage
        .into_iter()
        .filter(|(_, ms)| *ms > 0)
        .map(|(id, ms)| {
            let known = id.and_then(|id| names.get(&id));
            RankedItem {
                id,
                name: known.map_or_else(|| unassigned.to_string(), |(name, _)| name.clone()),
                color: known.and_then(|(_, color)| color.clone()),
                ms,
                share: if total > 0 { ms as f64 / total as f64 } else { 0.0 },
            }
        })
        .collect();
    items.sort_by(|a, b| b.ms.cmp(&a.ms).then_with(|| a.name.cmp(&b.name)));
    items
}

fn add(bucket: &mut Bucket, session_type: SessionType, ms: i64) {
    if session_type.is_active() {
        bucket.active_ms += ms;
    } else {
        bucket.idle_ms += ms;
    }
}

/// Calls `visit` for each piece of `[start, end)` that falls within one local
/// day, with that day's date.
fn for_each_day<Tz: TimeZone>(tz: &Tz, start: i64, end: i64, mut visit: impl FnMut(NaiveDate, i64, i64)) {
    let mut cursor = start;
    while cursor < end {
        let date = local_date(tz, cursor);
        let next = start_of_day(tz, date + Days::new(1)).min(end);
        visit(date, cursor, next);
        cursor = next;
    }
}

/// Calls `visit` for each piece of `[start, end)` within one local clock hour.
fn for_each_hour<Tz: TimeZone>(tz: &Tz, start: i64, end: i64, mut visit: impl FnMut(i64, i64)) {
    let mut cursor = start;
    while cursor < end {
        let offset = offset_ms(tz, cursor);
        let into_hour = (cursor + offset).rem_euclid(HOUR_MS);
        let next = (cursor + HOUR_MS - into_hour).min(end);
        visit(cursor, next);
        cursor = next;
    }
}

fn empty_buckets(keys: impl Iterator<Item = String>) -> BTreeMap<String, Bucket> {
    keys.map(|key| (key.clone(), Bucket { key, ..Default::default() })).collect()
}

fn week_key(date: NaiveDate) -> String {
    monday_of(date).format("%Y-%m-%d").to_string()
}

fn month_key(date: NaiveDate) -> String {
    date.format("%Y-%m").to_string()
}

fn dates(from: NaiveDate, to: NaiveDate) -> impl Iterator<Item = NaiveDate> {
    from.iter_days().take_while(move |date| *date <= to)
}

pub fn report<Tz: TimeZone>(
    connection: &Connection,
    tz: &Tz,
    from: NaiveDate,
    to: NaiveDate,
    now: i64,
    focus_categories: Option<&[i64]>,
) -> Result<Report> {
    let (range_start, range_end) = local_range(tz, from, to);
    let rows = load_rows(connection, range_start, range_end)?;

    let mut totals = Totals::default();
    let mut applications = HashMap::new();
    let mut projects = HashMap::new();
    let mut categories = HashMap::new();
    let mut hourly = empty_buckets((0..24).map(|hour| format!("{hour:02}")));
    let mut daily = empty_buckets(dates(from, to).map(|date| date.to_string()));
    let mut weekly = empty_buckets(dates(from, to).map(week_key));
    let mut monthly = empty_buckets(dates(from, to).map(month_key));
    let mut heatmap = BTreeMap::new();

    for row in &rows {
        let start = row.start.max(range_start);
        let end = row.end.min(range_end);
        let ms = end - start;
        if ms <= 0 {
            continue;
        }

        match row.session_type {
            SessionType::Unknown => totals.unknown_ms += ms,
            SessionType::Manual => totals.manual_ms += ms,
            _ => {}
        }
        if row.session_type.is_active() {
            totals.active_ms += ms;
            if row.session_type == SessionType::Active {
                *applications.entry(row.application_id).or_insert(0) += ms;
            }
            *projects.entry(row.project_id).or_insert(0) += ms;
            *categories.entry(row.category_id).or_insert(0) += ms;
        } else {
            totals.idle_ms += ms;
        }

        for_each_day(tz, start, end, |date, day_start, day_end| {
            let piece = day_end - day_start;
            add(daily.get_mut(&date.to_string()).expect("date in range"), row.session_type, piece);
            add(weekly.get_mut(&week_key(date)).expect("week in range"), row.session_type, piece);
            add(monthly.get_mut(&month_key(date)).expect("month in range"), row.session_type, piece);
        });

        let counts_for_focus = row.session_type.is_active()
            && focus_categories.is_none_or(|ids| row.category_id.is_some_and(|id| ids.contains(&id)));
        for_each_hour(tz, start, end, |hour_start, hour_end| {
            let piece = hour_end - hour_start;
            let moment = tz.timestamp_millis_opt(hour_start).earliest();
            let Some(moment) = moment else { return };
            let hour = moment.hour();
            add(hourly.get_mut(&format!("{hour:02}")).expect("hour of day"), row.session_type, piece);
            if counts_for_focus {
                let weekday = moment.weekday().num_days_from_monday();
                *heatmap.entry((weekday, hour)).or_insert(0) += piece;
            }
        });
    }

    totals.total_ms = totals.active_ms + totals.idle_ms;
    let elapsed = now.min(range_end) - range_start;
    totals.untracked_ms = (elapsed - totals.total_ms).max(0);

    let application_names = load_names(
        connection,
        "SELECT id, COALESCE(display_name, name), NULL FROM applications",
    )?;
    let project_names = load_names(connection, "SELECT id, name, color FROM projects")?;
    let category_names = load_names(connection, "SELECT id, name, color FROM categories")?;

    Ok(Report {
        from: from.to_string(),
        to: to.to_string(),
        applications: rank(applications, &application_names, "Removed application", totals.active_ms),
        projects: rank(projects, &project_names, "No project", totals.active_ms),
        categories: rank(categories, &category_names, "Uncategorized", totals.active_ms),
        hourly: hourly.into_values().collect(),
        daily: daily.into_values().collect(),
        weekly: weekly.into_values().collect(),
        monthly: monthly.into_values().collect(),
        heatmap: heatmap
            .into_iter()
            .map(|((weekday, hour), ms)| HourCell { weekday, hour, ms })
            .collect(),
        totals,
    })
}

/// Report for a range of local dates in the system timezone.
pub fn local_report(
    connection: &Connection,
    from: &str,
    to: &str,
    focus_categories: Option<&[i64]>,
) -> Result<Report> {
    let (from, to) = (parse_date(from)?, parse_date(to)?);
    if from > to {
        return crate::error::invalid("The start date must not be after the end date");
    }
    report(connection, &Local, from, to, now_ms(), focus_categories)
}

/// Report limited to one project and its descendants' sessions.
pub fn project_report(
    connection: &Connection,
    project_id: i64,
    from: i64,
    to: i64,
) -> Result<Vec<RankedItem>> {
    let mut statement = connection.prepare(
        "WITH RECURSIVE tree (id) AS (
            SELECT id FROM projects WHERE id = ?1
            UNION
            SELECT p.id FROM projects p JOIN tree t ON p.parent_id = t.id
         )
         SELECT s.application_id, COALESCE(a.display_name, a.name),
                SUM(MIN(s.end_time, ?3) - MAX(s.start_time, ?2))
         FROM activity_sessions s
         LEFT JOIN applications a ON a.id = s.application_id
         WHERE s.project_id IN tree AND s.end_time > ?2 AND s.start_time < ?3
         GROUP BY s.application_id
         ORDER BY 3 DESC",
    )?;
    let rows = statement.query_map(params![project_id, from, to], |row| {
        Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, Option<String>>(1)?, row.get::<_, i64>(2)?))
    })?;

    let rows = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    let total: i64 = rows.iter().map(|row| row.2).sum();
    Ok(rows
        .into_iter()
        .map(|(id, name, ms)| RankedItem {
            id,
            name: name.unwrap_or_else(|| "Manual entries".into()),
            color: None,
            ms,
            share: if total > 0 { ms as f64 / total as f64 } else { 0.0 },
        })
        .collect())
}

/// The UTC millisecond for a local wall-clock time, for tests and fixtures.
pub fn local_ms<Tz: TimeZone>(tz: &Tz, date: NaiveDate, hour: u32, minute: u32) -> i64 {
    tz.from_local_datetime(&date.and_hms_opt(hour, minute, 0).expect("valid time"))
        .earliest()
        .map_or_else(|| Utc.from_utc_datetime(&date.and_hms_opt(hour, minute, 0).unwrap()).timestamp_millis(), |t| t.timestamp_millis())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::sessions::{insert_tracked, TrackedSession};
    use crate::database::test_support::database;
    use crate::database::{applications, Database};
    use crate::time::MINUTE_MS;
    use chrono_tz::{America::New_York, Asia::Kolkata, UTC};

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn session(db: &Database, app: Option<i64>, kind: SessionType, start: i64, end: i64) {
        db.write(|c| {
            insert_tracked(
                c,
                &TrackedSession {
                    application_id: app,
                    project_id: None,
                    category_id: None,
                    start_time: start,
                    end_time: end,
                    session_type: kind,
                    window_title: None,
                    label: None,
                    manual_override: false,
                    is_open: false,
                },
            )
        })
        .unwrap();
    }

    fn app(db: &Database, name: &str) -> i64 {
        db.write(|c| applications::upsert(c, name, name, None, None)).unwrap()
    }

    fn run<Tz: TimeZone>(db: &Database, tz: &Tz, from: NaiveDate, to: NaiveDate, now: i64) -> Report {
        db.read(|c| report(c, tz, from, to, now, None)).unwrap()
    }

    #[test]
    fn totals_separate_active_idle_and_untracked() {
        let (_dir, db) = database();
        let code = app(&db, "code");
        let day = date(2026, 5, 1);
        let at = |h, m| local_ms(&UTC, day, h, m);
        session(&db, Some(code), SessionType::Active, at(9, 0), at(10, 0));
        session(&db, None, SessionType::Idle, at(10, 0), at(10, 30));
        session(&db, None, SessionType::Unknown, at(10, 30), at(11, 0));
        session(&db, None, SessionType::Manual, at(11, 0), at(11, 15));
        session(&db, None, SessionType::Locked, at(11, 15), at(11, 30));

        let result = run(&db, &UTC, day, day, at(12, 0));
        assert_eq!(result.totals.active_ms, 105 * MINUTE_MS);
        assert_eq!(result.totals.unknown_ms, 30 * MINUTE_MS);
        assert_eq!(result.totals.manual_ms, 15 * MINUTE_MS);
        assert_eq!(result.totals.idle_ms, 45 * MINUTE_MS);
        assert_eq!(result.totals.total_ms, 150 * MINUTE_MS);
        assert_eq!(result.totals.untracked_ms, 570 * MINUTE_MS);
    }

    #[test]
    fn idle_never_counts_towards_application_usage() {
        let (_dir, db) = database();
        let code = app(&db, "code");
        let day = date(2026, 5, 1);
        let at = |h, m| local_ms(&UTC, day, h, m);
        session(&db, Some(code), SessionType::Active, at(9, 0), at(9, 30));
        session(&db, Some(code), SessionType::Idle, at(9, 30), at(11, 0));

        let result = run(&db, &UTC, day, day, at(12, 0));
        assert_eq!(result.applications.len(), 1);
        assert_eq!(result.applications[0].ms, 30 * MINUTE_MS);
    }

    #[test]
    fn unknown_activity_is_not_attributed_to_an_application() {
        let (_dir, db) = database();
        let day = date(2026, 5, 1);
        let at = |h, m| local_ms(&UTC, day, h, m);
        session(&db, None, SessionType::Unknown, at(9, 0), at(10, 0));
        let result = run(&db, &UTC, day, day, at(12, 0));
        assert!(result.applications.is_empty());
        assert_eq!(result.totals.active_ms, 60 * MINUTE_MS);
    }

    #[test]
    fn sessions_crossing_midnight_are_split_between_days() {
        let (_dir, db) = database();
        let code = app(&db, "code");
        let start = local_ms(&UTC, date(2026, 5, 1), 23, 30);
        let end = local_ms(&UTC, date(2026, 5, 2), 0, 45);
        session(&db, Some(code), SessionType::Active, start, end);

        let both = run(&db, &UTC, date(2026, 5, 1), date(2026, 5, 2), end);
        assert_eq!(both.daily[0].active_ms, 30 * MINUTE_MS);
        assert_eq!(both.daily[1].active_ms, 45 * MINUTE_MS);

        let second = run(&db, &UTC, date(2026, 5, 2), date(2026, 5, 2), end);
        assert_eq!(second.totals.active_ms, 45 * MINUTE_MS);
    }

    #[test]
    fn daylight_saving_days_keep_their_real_length() {
        let (_dir, db) = database();
        let code = app(&db, "code");
        let day = date(2026, 11, 1);
        let (start, end) = local_range(&New_York, day, day);
        session(&db, Some(code), SessionType::Active, start, end);

        let result = run(&db, &New_York, day, day, end);
        assert_eq!(result.totals.active_ms, 25 * HOUR_MS);
        // The repeated 01:00 hour holds two real hours.
        assert_eq!(result.hourly[1].active_ms, 2 * HOUR_MS);
        let sum: i64 = result.hourly.iter().map(|bucket| bucket.active_ms).sum();
        assert_eq!(sum, 25 * HOUR_MS);
    }

    #[test]
    fn spring_forward_day_has_an_empty_skipped_hour() {
        let (_dir, db) = database();
        let code = app(&db, "code");
        let day = date(2026, 3, 8);
        let (start, end) = local_range(&New_York, day, day);
        session(&db, Some(code), SessionType::Active, start, end);

        let result = run(&db, &New_York, day, day, end);
        assert_eq!(result.totals.active_ms, 23 * HOUR_MS);
        assert_eq!(result.hourly[2].active_ms, 0);
    }

    #[test]
    fn half_hour_timezones_bucket_by_local_hour() {
        let (_dir, db) = database();
        let code = app(&db, "code");
        let day = date(2026, 5, 1);
        let start = local_ms(&Kolkata, day, 9, 30);
        let end = local_ms(&Kolkata, day, 10, 30);
        session(&db, Some(code), SessionType::Active, start, end);

        let result = run(&db, &Kolkata, day, day, end);
        assert_eq!(result.hourly[9].active_ms, 30 * MINUTE_MS);
        assert_eq!(result.hourly[10].active_ms, 30 * MINUTE_MS);
    }

    #[test]
    fn future_time_is_not_untracked() {
        let (_dir, db) = database();
        let day = date(2026, 5, 1);
        let result = run(&db, &UTC, day, day, local_ms(&UTC, day, 6, 0));
        assert_eq!(result.totals.untracked_ms, 6 * HOUR_MS);
        let future = run(&db, &UTC, date(2026, 5, 2), date(2026, 5, 2), local_ms(&UTC, day, 6, 0));
        assert_eq!(future.totals.untracked_ms, 0);
    }

    #[test]
    fn rankings_are_sorted_with_shares() {
        let (_dir, db) = database();
        let (a, b) = (app(&db, "alpha"), app(&db, "beta"));
        let day = date(2026, 5, 1);
        let at = |h, m| local_ms(&UTC, day, h, m);
        session(&db, Some(a), SessionType::Active, at(9, 0), at(9, 15));
        session(&db, Some(b), SessionType::Active, at(9, 15), at(10, 0));

        let result = run(&db, &UTC, day, day, at(12, 0));
        assert_eq!(result.applications[0].name, "beta");
        assert!((result.applications[0].share - 0.75).abs() < 1e-9);
        assert_eq!(result.projects[0].name, "No project");
    }

    #[test]
    fn weekly_and_monthly_buckets_cover_the_range() {
        let (_dir, db) = database();
        let result = run(&db, &UTC, date(2026, 1, 26), date(2026, 2, 10), 0);
        assert_eq!(result.daily.len(), 16);
        assert_eq!(result.weekly.len(), 3);
        assert_eq!(result.monthly.len(), 2);
    }

    #[test]
    fn heatmap_can_be_limited_to_chosen_categories() {
        let (_dir, db) = database();
        let code = app(&db, "code");
        let day = date(2026, 5, 4);
        let at = |h, m| local_ms(&UTC, day, h, m);
        session(&db, Some(code), SessionType::Active, at(9, 0), at(10, 0));
        db.write(|c| {
            c.execute("UPDATE activity_sessions SET category_id = 1", [])?;
            Ok(())
        })
        .unwrap();

        let matching = db.read(|c| report(c, &UTC, day, day, at(12, 0), Some(&[1]))).unwrap();
        assert_eq!(matching.heatmap, vec![HourCell { weekday: 0, hour: 9, ms: HOUR_MS }]);
        let other = db.read(|c| report(c, &UTC, day, day, at(12, 0), Some(&[2]))).unwrap();
        assert!(other.heatmap.is_empty());
    }
}
