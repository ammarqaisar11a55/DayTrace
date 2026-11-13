use rusqlite::{params, Connection};
use serde_json::{Map, Value};

use crate::config::Settings;
use crate::error::Result;
use crate::time::now_ms;

pub fn load(connection: &Connection) -> Result<Settings> {
    let mut statement = connection.prepare("SELECT key, value FROM settings")?;
    let rows = statement.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?;

    let mut values = Map::new();
    for row in rows {
        let (key, value) = row?;
        values.insert(key, serde_json::from_str(&value)?);
    }
    Ok(serde_json::from_value(Value::Object(values))?)
}

pub fn save(connection: &Connection, settings: &Settings) -> Result<()> {
    settings.validate()?;
    let Value::Object(values) = serde_json::to_value(settings)? else {
        unreachable!("settings serialize to an object");
    };

    let mut statement = connection.prepare(
        "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at
         WHERE value != excluded.value",
    )?;
    let now = now_ms();
    for (key, value) in values {
        statement.execute(params![key, value.to_string(), now])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Theme, UpdateChannel};
    use crate::database::test_support::database;

    #[test]
    fn missing_settings_use_defaults() {
        let (_dir, db) = database();
        assert_eq!(db.read(load).unwrap(), Settings::default());
    }

    #[test]
    fn settings_round_trip() {
        let (_dir, db) = database();
        let settings = Settings {
            tracking_enabled: true,
            tracking_consent_at: Some(42),
            theme: Theme::Light,
            update_channel: UpdateChannel::Beta,
            idle_threshold_secs: 600,
            ..Settings::default()
        };
        db.write(|c| save(c, &settings)).unwrap();
        assert_eq!(db.read(load).unwrap(), settings);
    }

    #[test]
    fn invalid_settings_are_not_saved() {
        let (_dir, db) = database();
        let settings = Settings { poll_interval_secs: 0, ..Settings::default() };
        assert!(db.write(|c| save(c, &settings)).is_err());
    }

    #[test]
    fn unknown_keys_from_future_versions_are_ignored() {
        let (_dir, db) = database();
        db.write(|c| {
            c.execute(
                "INSERT INTO settings (key, value, updated_at) VALUES ('futureOption', 'true', 0)",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        assert_eq!(db.read(load).unwrap(), Settings::default());
    }
}
