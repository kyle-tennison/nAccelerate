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
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
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

pub async fn run(app: AppHandle, settings: Settings, req: JobRequest, password: Option<String>, cancel: watch::Receiver<bool>) {
    let mut ctx = Ctx { app, cancel };
    let host = settings.self_hosted.host.clone();
    match solve(&mut ctx, &settings, &req, password).await {
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

/// Docker container names allow letters, digits, `_`, `.` and `-`.
fn job_name(folder: &str) -> String {
    let clean: String = folder.chars().map(|c| if c.is_ascii_alphanumeric() || "_.-".contains(c) { c } else { '_' }).collect();
    format!("naccel-{}-{}", clean, chrono::Local::now().format("%Y%m%d-%H%M%S"))
}

/// Everything in the folder except other `.ntop` files, since the solver expects exactly one notebook.
fn upload_entries(folder: &Path, notebook: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(folder).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.to_lowercase().ends_with(".ntop") && name != notebook {
            continue;
        }
        out.push(entry.path().to_string_lossy().into_owned());
    }
    Ok(out)
}

async fn solve(ctx: &mut Ctx, settings: &Settings, req: &JobRequest, password: Option<String>) -> Result<(), Stop> {
    let sh = &settings.self_hosted;
    let local = PathBuf::from(&req.folder);
    let folder_name = local
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or_else(|| "The selected folder has no name.".to_string())?;
    let job = job_name(&folder_name);
    let job_dir = format!("{}/{}", sh.remote_dir.trim().trim_end_matches('/'), job);
    let remote_folder = format!("{job_dir}/{folder_name}");
    let target = remote::target(sh);

    ctx.status("queued");
    ctx.log('I', &format!("Connecting to {target}"));
    ctx.step(remote::run(sh, &format!("mkdir -p {}", shell_path(&remote_folder)))).await?;
    ctx.log('I', &format!("Connected to {}", sh.host));

    ctx.status("upload");
    let entries = upload_entries(&local, &req.notebook)?;
    ctx.log('I', &format!("Uploading {folder_name}/ ({} items)", entries.len()));
    let dest = format!("{target}:{}/", scp_path(&remote_folder));
    ctx.step(remote::output(remote::scp(sh, &entries, &dest))).await?;
    ctx.log('I', &format!("Uploaded {folder_name}/"));

    ctx.status("solve");
    let command = sh
        .command
        .replace("{folder}", &shell_quote(&folder_name))
        .replace("{job}", &job)
        .replace("{cpus}", &req.cpu.to_string())
        .replace("{cpu_last}", &req.cpu.saturating_sub(1).to_string())
        .replace("{mem_gb}", &req.mem.to_string())
        .replace("{gpus}", &req.gpu.to_string());
    let visible_gpus = (0..req.gpu).map(|i| i.to_string()).collect::<Vec<_>>().join(",");
    // The password arrives on stdin rather than in the command line, so it never shows up in the remote process list.
    let script = format!(
        "exec 2>&1; echo \"{PID_MARK}$$\"; IFS= read -r NTOP_PASSWORD || true; \
         export NTOP_PASSWORD NTOP_EMAIL={email} NACCEL_JOB={job} NACCEL_CPUS={cpus} NACCEL_CPUSET=0-{last} \
         NACCEL_MEM_GB={mem} NACCEL_GPUS={gpus} OMP_NUM_THREADS={cpus} CUDA_VISIBLE_DEVICES={visible} PYTHONUNBUFFERED=1; \
         cd {dir} && {command}",
        email = shell_quote(&settings.ntop_user),
        job = shell_quote(&job),
        cpus = req.cpu,
        last = req.cpu.saturating_sub(1),
        mem = req.mem,
        gpus = req.gpu,
        visible = shell_quote(&visible_gpus),
        dir = shell_path(&job_dir),
    );
    let stats_task = tokio::spawn(stats::stream(ctx.app.clone(), sh.clone(), req.cpu as usize));
    let solved = stream_solver(ctx, remote::ssh(sh, &script, false), password).await;
    stats_task.abort();
    let saved = match solved {
        Ok(p) => p,
        Err((stop, pid)) => {
            if matches!(stop, Stop::Cancelled) {
                stop_remote(sh, pid, &job).await;
            }
            return Err(stop);
        }
    };

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
    ctx.step(remote::output(remote::scp(sh, &[from], &local_target.to_string_lossy()))).await?;
    ctx.log('I', &format!("Received {}", local_target.display()));

    let _ = remote::run(sh, &format!("rm -rf {}", shell_path(&job_dir))).await;
    ctx.log('I', &format!("Disconnected from {}", sh.host));
    ctx.log('I', "Complete");
    Ok(())
}

const PID_MARK: &str = "NACCEL_PID ";

/// Closing ssh does not stop a solver running in Docker, so cancel kills the remote process group and runs the user's cancel command.
async fn stop_remote(sh: &crate::settings::SelfHosted, pid: Option<u32>, job: &str) {
    let mut script = String::new();
    if let Some(pid) = pid {
        script.push_str(&format!("kill -TERM -- -{pid} 2>/dev/null; "));
    }
    let cancel = sh.cancel_command.trim();
    if !cancel.is_empty() {
        script.push_str(&format!("{} >/dev/null 2>&1; ", cancel.replace("{job}", job)));
    }
    script.push_str("true");
    let _ = remote::run(sh, &script).await;
}

/// Forwards every solver line to the UI and returns the remote path from the solver's `Saved: <path>` line.
/// On failure it also returns the remote shell's PID so a cancel can stop it.
async fn stream_solver(ctx: &mut Ctx, mut cmd: tokio::process::Command, password: Option<String>) -> Result<String, (Stop, Option<u32>)> {
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
    let mut child = cmd.spawn().map_err(|e| (Stop::Failed(format!("Could not start ssh: {e}")), None))?;
    if let Some(mut stdin) = child.stdin.take() {
        let line = format!("{}\n", password.unwrap_or_default());
        let _ = stdin.write_all(line.as_bytes()).await;
    }
    let stdout = child.stdout.take().ok_or_else(|| (Stop::Failed("ssh produced no output stream".into()), None))?;
    let mut lines = BufReader::new(stdout).lines();
    let mut saved = None;
    let mut pid = None;
    loop {
        tokio::select! {
            line = lines.next_line() => match line {
                Ok(Some(line)) => {
                    let line = line.trim_end_matches('\r').trim_end();
                    if line.is_empty() { continue; }
                    if let Some(p) = line.strip_prefix(PID_MARK) { pid = p.trim().parse().ok(); continue; }
                    if let Some(p) = line.strip_prefix("Saved:") { saved = Some(p.trim().to_string()); }
                    let _ = ctx.app.emit("job-log", line.to_string());
                }
                _ => break,
            },
            _ = cancelled(&mut ctx.cancel) => {
                let _ = child.kill().await;
                return Err((Stop::Cancelled, pid));
            }
        }
    }
    let status = child.wait().await.map_err(|e| (Stop::Failed(e.to_string()), pid))?;
    match saved {
        Some(p) if status.success() => Ok(p),
        _ if status.success() => Err((Stop::Failed("The solver finished without a \"Saved:\" line, so there is nothing to download.".into()), pid)),
        _ => Err((Stop::Failed(format!("The solver exited with {status}.")), pid)),
    }
}
