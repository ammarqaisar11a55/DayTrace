use serde::{Deserialize, Serialize};

use crate::error::{invalid, Result};

pub const MIN_POLL_SECS: u32 = 1;
pub const MAX_POLL_SECS: u32 = 60;
pub const MIN_IDLE_SECS: u32 = 30;
pub const MAX_IDLE_SECS: u32 = 3_600;
pub const MIN_UPDATE_HOURS: u32 = 6;
pub const MAX_UPDATE_HOURS: u32 = 168;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdateChannel {
    #[default]
    Stable,
    Beta,
}

/// Application settings. Every privacy-sensitive option defaults to off.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub onboarding_completed: bool,
    pub tracking_enabled: bool,
    pub tracking_consent_at: Option<i64>,
    pub poll_interval_secs: u32,
    pub idle_threshold_secs: u32,
    pub collect_window_titles: bool,
    pub browser_integration: bool,
    pub retention_days: u32,
    pub theme: Theme,
    pub start_on_login: bool,
    pub start_hidden: bool,
    pub update_channel: UpdateChannel,
    pub update_check_on_startup: bool,
    pub update_check_interval_hours: u32,
    pub last_update_check: Option<i64>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            onboarding_completed: false,
            tracking_enabled: false,
            tracking_consent_at: None,
            poll_interval_secs: 5,
            idle_threshold_secs: 300,
            collect_window_titles: false,
            browser_integration: false,
            retention_days: 0,
            theme: Theme::System,
            start_on_login: false,
            start_hidden: false,
            update_channel: UpdateChannel::Stable,
            update_check_on_startup: true,
            update_check_interval_hours: 24,
            last_update_check: None,
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<()> {
        if !(MIN_POLL_SECS..=MAX_POLL_SECS).contains(&self.poll_interval_secs) {
            return invalid(format!(
                "The polling interval must be between {MIN_POLL_SECS} and {MAX_POLL_SECS} seconds"
            ));
        }
        if !(MIN_IDLE_SECS..=MAX_IDLE_SECS).contains(&self.idle_threshold_secs) {
            return invalid(format!(
                "The idle threshold must be between {MIN_IDLE_SECS} seconds and {} minutes",
                MAX_IDLE_SECS / 60
            ));
        }
        if !(MIN_UPDATE_HOURS..=MAX_UPDATE_HOURS).contains(&self.update_check_interval_hours) {
            return invalid(format!(
                "The update check interval must be between {MIN_UPDATE_HOURS} and {MAX_UPDATE_HOURS} hours"
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn privacy_sensitive_defaults_are_off() {
        let settings = Settings::default();
        assert!(!settings.tracking_enabled);
        assert!(!settings.collect_window_titles);
        assert!(!settings.browser_integration);
        assert_eq!(settings.idle_threshold_secs, 300);
        assert!(settings.validate().is_ok());
    }

    #[test]
    fn out_of_range_values_are_rejected() {
        let mut settings = Settings::default();
        settings.poll_interval_secs = 0;
        assert!(settings.validate().is_err());
        settings = Settings { idle_threshold_secs: 5, ..Settings::default() };
        assert!(settings.validate().is_err());
    }
}
