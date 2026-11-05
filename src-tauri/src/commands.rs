//! The commands the UI can invoke.

use crate::notebook::{self, Block};
use crate::settings::{self, HostInfo, Provider, Settings};
use crate::{credentials, events, job, server};
use serde::Serialize;
use std::path::Path;
use std::sync::Mutex;
use tauri::{AppHandle, State};
use tokio::sync::watch;

/// The cancel switch for the job that is running, if any.
#[derive(Default)]
pub struct JobState(Mutex<Option<watch::Sender<bool>>>);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderInfo {
    path: String,
    name: String,
    notebook: Option<String>,
    /// How many steps the notebook should log, when its block list could be read.
    steps: Option<u32>,
    blocks: Vec<Block>,
}

#[tauri::command]
pub fn load_settings(app: AppHandle) -> Settings {
    settings::load(&app)
}

#[tauri::command]
pub fn save_settings(app: AppHandle, settings: Settings) -> Result<(), String> {
    settings::save(&app, &settings)
}

#[tauri::command]
pub async fn test_connection(settings: Settings) -> Result<HostInfo, String> {
    server::probe(&settings.self_hosted).await
}

#[tauri::command]
pub fn save_password(user: String, password: String) -> Result<(), String> {
    credentials::save(&user, &password)
}

#[tauri::command]
pub fn has_password(user: String) -> bool {
    credentials::load(&user).is_some()
}

/// Finds the notebook in a project folder (ignoring earlier `_solved` copies) and reads its step plan.
#[tauri::command]
pub fn inspect_folder(path: String) -> Result<FolderInfo, String> {
    // A dropped .ntop file stands for the folder it's in.
    let path = match Path::new(&path) {
        p if p.is_file() => p.parent().map(|d| d.to_string_lossy().into_owned()).unwrap_or(path),
        _ => path,
    };
    let dir = Path::new(&path);
    let mut notebooks: Vec<String> = std::fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| {
            let n = n.to_lowercase();
            n.ends_with(".ntop") && !n.ends_with("_solved.ntop")
        })
        .collect();
    notebooks.sort();
    let notebook = notebooks.into_iter().next();
    let plan = notebook.as_ref().and_then(|n| notebook::count_steps(&dir.join(n)).ok());
    Ok(FolderInfo {
        name: dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.clone()),
        path,
        notebook,
        steps: plan.as_ref().map(|c| c.steps),
        blocks: plan.map(|c| c.blocks).unwrap_or_default(),
    })
}

#[tauri::command]
pub fn start_job(app: AppHandle, state: State<JobState>, request: job::JobRequest) -> Result<(), String> {
    let settings = settings::load(&app);
    if settings.provider == Provider::Aws {
        return Err("AWS isn't available yet. Choose a self-hosted server in Settings.".into());
    }
    server::check(&settings.self_hosted)?;
    let mut slot = state.0.lock().map_err(|e| e.to_string())?;
    if slot.as_ref().is_some_and(|tx| !tx.is_closed()) {
        return Err("A job is already running.".into());
    }
    let password = credentials::load(&settings.ntop_user);
    let (tx, rx) = watch::channel(false);
    *slot = Some(tx);
    tauri::async_runtime::spawn(job::run(app, settings, request, password, rx));
    Ok(())
}

#[tauri::command]
pub fn cancel_job(app: AppHandle, state: State<JobState>) {
    if let Ok(slot) = state.0.lock() {
        if let Some(tx) = slot.as_ref().filter(|tx| !tx.is_closed()) {
            events::phase(&app, events::Phase::Cancelling);
            let _ = tx.send(true);
        }
    }
}
