use super::{ActivityTracker, Capabilities, Capability, Foreground, Sample, SampleOptions};

/// Used when no tracking integration exists. Automatic tracking records
/// nothing, and manual timers keep working.
pub struct UnsupportedTracker {
    reason: String,
}

impl UnsupportedTracker {
    pub fn new(reason: impl Into<String>) -> Self {
        Self { reason: reason.into() }
    }
}

impl ActivityTracker for UnsupportedTracker {
    fn capabilities(&mut self) -> Capabilities {
        let unavailable = || Capability::unavailable(self.reason.clone());
        Capabilities {
            platform: std::env::consts::OS.to_string(),
            display_server: "unknown".to_string(),
            desktop: "unknown".to_string(),
            backend: "none".to_string(),
            foreground_app: unavailable(),
            executable_path: unavailable(),
            window_title: unavailable(),
            idle: unavailable(),
            lock: unavailable(),
            suspend: unavailable(),
            integration: None,
        }
    }

    fn sample(&mut self, _options: SampleOptions) -> Sample {
        Sample { foreground: Foreground::Unknown, idle_ms: None, locked: false }
    }
}
