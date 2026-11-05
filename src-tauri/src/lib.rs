mod job;
pub mod notebook;
mod remote;
mod settings;
mod stats;
mod transfer;

use serde::Serialize;
use settings::{HostInfo, Provider, Settings};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::watch;

const KEYRING_SERVICE: &str = "nAccelerate";

#[derive(Default)]
struct JobState(Mutex<Option<watch::Sender<bool>>>);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FolderInfo {
    path: String,
    name: String,
    notebook: Option<String>,
    steps: Option<u32>,
    blocks: Vec<notebook::Block>,
}

#[tauri::command]
fn load_settings(app: AppHandle) -> Settings {
    settings::load(&app)
}

#[tauri::command]
fn save_settings(app: AppHandle, settings: Settings) -> Result<(), String> {
    settings::save(&app, &settings)
}

#[tauri::command]
async fn test_connection(settings: Settings) -> Result<HostInfo, String> {
    remote::probe(&settings.self_hosted).await
}

#[tauri::command]
fn save_password(user: String, password: String) -> Result<(), String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, &user).map_err(|e| e.to_string())?;
    entry.set_password(&password).map_err(|e| e.to_string())
}

#[tauri::command]
fn has_password(user: String) -> bool {
    keyring::Entry::new(KEYRING_SERVICE, &user).and_then(|e| e.get_password()).is_ok()
}

#[tauri::command]
fn inspect_folder(path: String) -> Result<FolderInfo, String> {
    let dir = std::path::Path::new(&path);
    let mut notebooks: Vec<String> = std::fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.to_lowercase().ends_with(".ntop") && !n.to_lowercase().ends_with("_solved.ntop"))
        .collect();
    notebooks.sort();
    let notebook = notebooks.into_iter().next();
    let plan = notebook.as_ref().and_then(|n| notebook::count_steps(&dir.join(n)).ok());
    Ok(FolderInfo {
        name: dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.clone()),
        path,
        notebook,
        steps: plan.as_ref().map(|c| c.steps),
        blocks: plan.map(|c| c.blocks).unwrap_or_default(),
    })
}

#[tauri::command]
fn start_job(app: AppHandle, state: State<JobState>, request: job::JobRequest) -> Result<(), String> {
    let settings = settings::load(&app);
    if settings.provider == Provider::Aws {
        return Err("AWS isn't available yet. Choose a self-hosted server in Settings.".into());
    }
    remote::check(&settings.self_hosted)?;
    let mut slot = state.0.lock().map_err(|e| e.to_string())?;
    if slot.as_ref().is_some_and(|tx| !tx.is_closed()) {
        return Err("A job is already running.".into());
    }
    let password = keyring::Entry::new(KEYRING_SERVICE, &settings.ntop_user).and_then(|e| e.get_password()).ok();
    let (tx, rx) = watch::channel(false);
    *slot = Some(tx);
    tauri::async_runtime::spawn(job::run(app, settings, request, password, rx));
    Ok(())
}

#[tauri::command]
fn cancel_job(app: AppHandle, state: State<JobState>) {
    if let Ok(slot) = state.0.lock() {
        if let Some(tx) = slot.as_ref().filter(|tx| !tx.is_closed()) {
            let _ = app.emit("job-status", "cancelling");
            let _ = tx.send(true);
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(JobState::default())
        .invoke_handler(tauri::generate_handler![
            load_settings,
            save_settings,
            test_connection,
            save_password,
            has_password,
            inspect_folder,
            start_job,
            cancel_job
        ])
        .run(tauri::generate_context!())
        .expect("error while running nAccelerate");
}
