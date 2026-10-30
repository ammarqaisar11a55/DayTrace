//! Tracking engine, persistence and analytics for DayTrace.
//!
//! This crate has no dependency on Tauri so that it can be built and tested
//! without a webview toolkit.

pub mod analytics;
pub mod config;
pub mod database;
pub mod error;
pub mod models;
pub mod projects;
pub mod time;
pub mod tracker;
pub mod updates;

pub use error::{CoreError, Result};
