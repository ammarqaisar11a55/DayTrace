use serde::{Deserialize, Serialize};

use crate::error::{invalid, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionType {
    Active,
    Unknown,
    Manual,
    Idle,
    Locked,
}

impl SessionType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Unknown => "unknown",
            Self::Manual => "manual",
            Self::Idle => "idle",
            Self::Locked => "locked",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "active" => Ok(Self::Active),
            "unknown" => Ok(Self::Unknown),
            "manual" => Ok(Self::Manual),
            "idle" => Ok(Self::Idle),
            "locked" => Ok(Self::Locked),
            other => invalid(format!("Unknown session type '{other}'")),
        }
    }

    pub fn is_active(self) -> bool {
        matches!(self, Self::Active | Self::Unknown | Self::Manual)
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Application {
    pub id: i64,
    pub platform_identifier: String,
    pub name: String,
    pub display_name: Option<String>,
    pub executable_path: Option<String>,
    pub icon_path: Option<String>,
    pub default_project_id: Option<i64>,
    pub default_category_id: Option<i64>,
    pub created_at: i64,
}

impl Application {
    pub fn label(&self) -> &str {
        self.display_name.as_deref().unwrap_or(&self.name)
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationUsage {
    #[serde(flatten)]
    pub application: Application,
    pub total_ms: i64,
    pub session_count: i64,
    pub last_used: Option<i64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationPatch {
    pub display_name: Option<String>,
    pub default_project_id: Option<i64>,
    pub default_category_id: Option<i64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: i64,
    pub parent_id: Option<i64>,
    pub name: String,
    pub description: String,
    pub color: String,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInput {
    pub parent_id: Option<i64>,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub color: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub default_project_id: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryInput {
    pub name: String,
    pub color: String,
    pub default_project_id: Option<i64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: i64,
    pub application_id: Option<i64>,
    pub application_name: Option<String>,
    pub executable_path: Option<String>,
    pub project_id: Option<i64>,
    pub project_name: Option<String>,
    pub project_color: Option<String>,
    pub category_id: Option<i64>,
    pub category_name: Option<String>,
    pub category_color: Option<String>,
    pub start_time: i64,
    pub end_time: i64,
    pub duration_seconds: i64,
    pub session_type: SessionType,
    pub window_title: Option<String>,
    pub label: Option<String>,
    pub manual_override: bool,
    pub is_open: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionFilter {
    pub from: i64,
    pub to: i64,
    pub application_id: Option<i64>,
    pub project_id: Option<i64>,
    pub category_id: Option<i64>,
    pub limit: Option<i64>,
    pub newest_first: Option<bool>,
}

/// A user correction to a session. Omitted times keep their stored values.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPatch {
    pub project_id: Option<i64>,
    pub category_id: Option<i64>,
    pub label: Option<String>,
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualSessionInput {
    pub start_time: i64,
    pub end_time: i64,
    pub label: Option<String>,
    pub project_id: Option<i64>,
    pub category_id: Option<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExclusionKind {
    Application,
    Path,
    Project,
}

impl ExclusionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Application => "application",
            Self::Path => "path",
            Self::Project => "project",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "application" => Ok(Self::Application),
            "path" => Ok(Self::Path),
            "project" => Ok(Self::Project),
            other => invalid(format!("Unknown exclusion kind '{other}'")),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Exclusion {
    pub id: i64,
    pub kind: ExclusionKind,
    pub value: String,
    pub created_at: i64,
}
