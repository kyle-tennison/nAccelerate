use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Provider {
    #[default]
    #[serde(rename = "self")]
    SelfHosted,
    #[serde(rename = "aws")]
    Aws,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum ResultMode {
    #[default]
    Copy,
    Replace,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct SelfHosted {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub key_path: String,
    pub remote_dir: String,
    pub command: String,
    pub cancel_command: String,
}

impl Default for SelfHosted {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: 22,
            user: String::new(),
            key_path: String::new(),
            remote_dir: "~/naccelerate/jobs".into(),
            command: "ntop --input {folder}".into(),
            cancel_command: "sg docker -c 'docker rm -f {job}'".into(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Aws {
    pub region: String,
    pub profile: String,
    pub spend_cap: String,
    pub spot: bool,
    pub terminate: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Results {
    pub mode: ResultMode,
    pub notify: bool,
    pub show_all_logs: bool,
}

impl Default for Results {
    fn default() -> Self {
        Self { mode: ResultMode::Copy, notify: true, show_all_logs: false }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct HostInfo {
    pub cores: u32,
    pub mem_gb: f32,
    pub gpus: u32,
    pub os: String,
    pub ntop_found: bool,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default)]
#[serde(default)]
pub struct Resources {
    pub cpu: u32,
    pub mem: u32,
    pub gpu: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub provider: Provider,
    pub ntop_user: String,
    pub self_hosted: SelfHosted,
    pub aws: Aws,
    pub results: Results,
    pub host_info: Option<HostInfo>,
    pub resources: Option<Resources>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            provider: Provider::SelfHosted,
            ntop_user: String::new(),
            self_hosted: SelfHosted::default(),
            aws: Aws { region: "us-east-2".into(), profile: "default".into(), spend_cap: "$25.00".into(), spot: false, terminate: true },
            results: Results::default(),
            host_info: None,
            resources: None,
        }
    }
}

fn path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    Ok(dir.join("settings.json"))
}

pub fn load(app: &AppHandle) -> Settings {
    path(app)
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let p = path(app)?;
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    std::fs::write(p, json).map_err(|e| e.to_string())
}
