//! Everything the backend tells the UI about a job goes through these helpers.

use serde::Serialize;
use tauri::{AppHandle, Emitter};

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Idle,
    Queued,
    Upload,
    Solve,
    Download,
    Cancelling,
    Done,
}

#[derive(Serialize, Clone, Copy)]
pub struct Progress {
    pub done: u64,
    pub total: u64,
}

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Gpu {
    pub util: f32,
    pub vram: f32,
}

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub cores: Vec<f32>,
    pub mem_gb: f32,
    pub gpus: Vec<Gpu>,
}

#[derive(Clone, Copy)]
pub enum Level {
    Info,
    Warn,
    Error,
}

pub fn phase(app: &AppHandle, phase: Phase) {
    let _ = app.emit("job-status", phase);
}

/// A line the app writes itself, formatted like ntopcl's own log lines.
pub fn log(app: &AppHandle, level: Level, text: &str) {
    let tag = match level {
        Level::Info => 'I',
        Level::Warn => 'W',
        Level::Error => 'E',
    };
    raw_log(
        app,
        &format!("{} [{tag}]: {text}", chrono::Local::now().format("%H:%M:%S")),
    );
}

/// A line passed through unchanged from the solver.
pub fn raw_log(app: &AppHandle, line: &str) {
    let _ = app.emit("job-log", line);
}

pub fn progress(app: &AppHandle, done: u64, total: u64) {
    let _ = app.emit(
        "job-progress",
        Progress {
            done: done.min(total),
            total,
        },
    );
}

pub fn stats(app: &AppHandle, stats: Stats) {
    let _ = app.emit("job-stats", stats);
}

pub fn error(app: &AppHandle, message: &str) {
    let _ = app.emit("job-error", message);
}
