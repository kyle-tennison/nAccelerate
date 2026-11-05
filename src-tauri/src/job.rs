//! One remote solve: upload the project folder, run the solver over ssh while streaming its log and the
//! machine's stats, download the saved notebook, then clean up and disconnect.

use crate::remote::{self, scp_path, shell_path, shell_quote};
use crate::settings::{ResultMode, Settings};
use crate::stats;
use serde::Deserialize;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::watch;

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct JobRequest {
    pub folder: String,
    pub notebook: String,
    pub cpu: u32,
    pub mem: u32,
    pub gpu: u32,
}

enum Stop {
    Cancelled,
    Failed(String),
}

impl From<String> for Stop {
    fn from(e: String) -> Self {
        Stop::Failed(e)
    }
}

async fn cancelled(rx: &mut watch::Receiver<bool>) {
    let hit = rx.wait_for(|c| *c).await.is_ok();
    if !hit {
        std::future::pending::<()>().await;
    }
}

struct Ctx {
    app: AppHandle,
    cancel: watch::Receiver<bool>,
}

impl Ctx {
    fn status(&self, s: &str) {
        let _ = self.app.emit("job-status", s);
    }

    fn log(&self, level: char, text: &str) {
        let now = chrono::Local::now().format("%H:%M:%S");
        let _ = self.app.emit("job-log", format!("{now} [{level}]: {text}"));
    }

    /// Runs `fut` unless the user cancels first.
    async fn step<T>(&mut self, fut: impl Future<Output = Result<T, String>>) -> Result<T, Stop> {
        tokio::select! {
            r = fut => r.map_err(Stop::Failed),
            _ = cancelled(&mut self.cancel) => Err(Stop::Cancelled),
        }
    }
}

pub async fn run(app: AppHandle, settings: Settings, req: JobRequest, cancel: watch::Receiver<bool>) {
    let mut ctx = Ctx { app, cancel };
    let host = settings.self_hosted.host.clone();
    match solve(&mut ctx, &settings, &req).await {
        Ok(()) => ctx.status("done"),
        Err(stop) => {
            match stop {
                Stop::Cancelled => ctx.log('W', "Cancelled; remote solve stopped"),
                Stop::Failed(e) => {
                    ctx.log('E', &format!("nAccelerate: {e}"));
                    let _ = ctx.app.emit("job-error", e);
                }
            }
            ctx.log('I', &format!("Disconnected from {host}"));
            ctx.status("idle");
        }
    }
}

async fn solve(ctx: &mut Ctx, settings: &Settings, req: &JobRequest) -> Result<(), Stop> {
    let sh = &settings.self_hosted;
    let local = PathBuf::from(&req.folder);
    let folder_name = local
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or_else(|| "The selected folder has no name.".to_string())?;
    let job_id = format!("{}-{}", folder_name.replace(' ', "_"), chrono::Local::now().format("%Y%m%d-%H%M%S"));
    let job_dir = format!("{}/{}", sh.remote_dir.trim().trim_end_matches('/'), job_id);
    let target = remote::target(sh);

    ctx.status("queued");
    ctx.log('I', &format!("Connecting to {target}"));
    ctx.step(remote::run(sh, &format!("mkdir -p {}", shell_path(&job_dir)))).await?;
    ctx.log('I', &format!("Connected to {}", sh.host));

    ctx.status("upload");
    ctx.log('I', &format!("Uploading {folder_name}/"));
    let dest = format!("{target}:{}/", scp_path(&job_dir));
    ctx.step(remote::output(remote::scp(sh, &req.folder, &dest))).await?;
    ctx.log('I', &format!("Uploaded {folder_name}/"));

    ctx.status("solve");
    let command = sh
        .command
        .replace("{folder}", &shell_quote(&folder_name))
        .replace("{cpus}", &req.cpu.to_string())
        .replace("{cpu_last}", &req.cpu.saturating_sub(1).to_string())
        .replace("{mem_gb}", &req.mem.to_string())
        .replace("{gpus}", &req.gpu.to_string());
    let visible_gpus = (0..req.gpu).map(|i| i.to_string()).collect::<Vec<_>>().join(",");
    let script = format!(
        "cd {} && export PYTHONUNBUFFERED=1 OMP_NUM_THREADS={} CUDA_VISIBLE_DEVICES={} && {}",
        shell_path(&job_dir),
        req.cpu,
        shell_quote(&visible_gpus),
        command
    );
    let stats_task = tokio::spawn(stats::stream(ctx.app.clone(), sh.clone(), req.cpu as usize));
    let solved = stream_solver(ctx, remote::ssh(sh, &script, true)).await;
    stats_task.abort();
    let saved = solved?;

    ctx.status("download");
    let local_target = match settings.results.mode {
        ResultMode::Replace => local.join(&req.notebook),
        ResultMode::Copy => {
            let stem = Path::new(&req.notebook).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "notebook".into());
            local.join(format!("{stem}_solved.ntop"))
        }
    };
    ctx.log('I', &format!("Downloading {}", local_target.display()));
    let from = format!("{target}:{saved}");
    ctx.step(remote::output(remote::scp(sh, &from, &local_target.to_string_lossy()))).await?;
    ctx.log('I', &format!("Received {}", local_target.display()));

    let _ = remote::run(sh, &format!("rm -rf {}", shell_path(&job_dir))).await;
    ctx.log('I', &format!("Disconnected from {}", sh.host));
    ctx.log('I', "Complete");
    Ok(())
}

/// Forwards every solver line to the UI and returns the remote path from the solver's `Saved: <path>` line.
async fn stream_solver(ctx: &mut Ctx, mut cmd: tokio::process::Command) -> Result<String, Stop> {
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("Could not start ssh: {e}"))?;
    let stdout = child.stdout.take().ok_or_else(|| "ssh produced no output stream".to_string())?;
    let mut lines = BufReader::new(stdout).lines();
    let mut saved = None;
    loop {
        tokio::select! {
            line = lines.next_line() => match line {
                Ok(Some(line)) => {
                    let line = line.trim_end_matches('\r').trim_end();
                    if line.is_empty() { continue; }
                    if let Some(p) = line.strip_prefix("Saved:") { saved = Some(p.trim().to_string()); }
                    let _ = ctx.app.emit("job-log", line.to_string());
                }
                _ => break,
            },
            _ = cancelled(&mut ctx.cancel) => {
                let _ = child.kill().await;
                return Err(Stop::Cancelled);
            }
        }
    }
    let status = child.wait().await.map_err(|e| e.to_string())?;
    match saved {
        Some(p) => Ok(p),
        None if status.success() => Err("The solver finished without a \"Saved:\" line, so there is nothing to download.".to_string().into()),
        None => Err(format!("The solver exited with {status}.").into()),
    }
}
