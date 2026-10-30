use chrono::{Datelike, Days, Local, LocalResult, NaiveDate, NaiveTime, Offset, TimeZone, Utc};

use crate::error::{invalid, Result};

pub const SECOND_MS: i64 = 1_000;
pub const MINUTE_MS: i64 = 60 * SECOND_MS;
pub const HOUR_MS: i64 = 60 * MINUTE_MS;

pub fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}

pub fn parse_date(value: &str) -> Result<NaiveDate> {
    match NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        Ok(date) => Ok(date),
        Err(_) => invalid(format!("'{value}' is not a valid date")),
    }
}

/// The first instant of a local calendar day, in UTC milliseconds.
///
/// A few zones change their clocks at midnight, so the local midnight may be
/// skipped or occur twice. The earliest valid instant of the day is used.
pub fn start_of_day<Tz: TimeZone>(tz: &Tz, date: NaiveDate) -> i64 {
    let midnight = date.and_time(NaiveTime::MIN);
    for minutes in (0..=180).step_by(15) {
        let candidate = midnight + chrono::Duration::minutes(minutes);
        match tz.from_local_datetime(&candidate) {
            LocalResult::Single(moment) => return moment.timestamp_millis(),
            LocalResult::Ambiguous(first, _) => return first.timestamp_millis(),
            LocalResult::None => continue,
        }
    }
    midnight.and_utc().timestamp_millis()
}

/// Half-open UTC range covering the inclusive local dates `from..=to`.
pub fn local_range<Tz: TimeZone>(tz: &Tz, from: NaiveDate, to: NaiveDate) -> (i64, i64) {
    (start_of_day(tz, from), start_of_day(tz, to + Days::new(1)))
}

pub fn local_range_str(from: &str, to: &str) -> Result<(i64, i64)> {
    let (from, to) = (parse_date(from)?, parse_date(to)?);
    if from > to {
        return invalid("The start date must not be after the end date");
    }
    Ok(local_range(&Local, from, to))
}

pub fn local_date<Tz: TimeZone>(tz: &Tz, ms: i64) -> NaiveDate {
    match tz.timestamp_millis_opt(ms) {
        LocalResult::Single(moment) => moment.date_naive(),
        LocalResult::Ambiguous(moment, _) => moment.date_naive(),
        LocalResult::None => Utc.timestamp_millis_opt(ms).unwrap().date_naive(),
    }
}

/// Milliseconds east of UTC at the given instant.
pub fn offset_ms<Tz: TimeZone>(tz: &Tz, ms: i64) -> i64 {
    let moment = Utc.timestamp_millis_opt(ms).unwrap().naive_utc();
    tz.offset_from_utc_datetime(&moment).fix().local_minus_utc() as i64 * SECOND_MS
}

pub fn monday_of(date: NaiveDate) -> NaiveDate {
    date - Days::new(date.weekday().num_days_from_monday() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono_tz::{America::Havana, America::New_York, Asia::Kolkata};

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn spring_forward_day_is_23_hours() {
        let (from, to) = local_range(&New_York, date(2026, 3, 8), date(2026, 3, 8));
        assert_eq!(to - from, 23 * HOUR_MS);
    }

    #[test]
    fn fall_back_day_is_25_hours() {
        let (from, to) = local_range(&New_York, date(2026, 11, 1), date(2026, 11, 1));
        assert_eq!(to - from, 25 * HOUR_MS);
    }

    #[test]
    fn skipped_midnight_uses_first_valid_instant() {
        // Havana springs forward at midnight, so 00:00 does not exist on this day.
        let start = start_of_day(&Havana, date(2026, 3, 8));
        assert_eq!(local_date(&Havana, start), date(2026, 3, 8));
        assert_eq!(local_date(&Havana, start - 1), date(2026, 3, 7));
    }

    #[test]
    fn half_hour_offsets_are_respected() {
        let start = start_of_day(&Kolkata, date(2026, 1, 1));
        assert_eq!(offset_ms(&Kolkata, start), 5 * HOUR_MS + 30 * MINUTE_MS);
        assert_eq!(local_date(&Kolkata, start), date(2026, 1, 1));
    }

    #[test]
    fn monday_of_handles_week_boundaries() {
        assert_eq!(monday_of(date(2026, 11, 1)), date(2026, 10, 26));
        assert_eq!(monday_of(date(2026, 10, 26)), date(2026, 10, 26));
    }

    #[test]
    fn inverted_range_is_rejected() {
        assert!(local_range_str("2026-02-02", "2026-02-01").is_err());
        assert!(local_range_str("nonsense", "2026-02-01").is_err());
    }
}
