//! One remote solve: upload the project folder, run the solver while streaming its log and the server's
//! stats, download the saved notebook, then clean up and disconnect.

use crate::cancel::{self, Outcome, Stop, Token};
use crate::events::{self, Level, Phase};
use crate::remote::{self, shell_path};
use crate::settings::{ResultMode, SelfHosted, Settings};
use crate::{server, solver, stats, transfer};
use serde::Deserialize;
use std::future::Future;
use std::path::{Path, PathBuf};
use tauri::AppHandle;

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct JobRequest {
    pub folder: String,
    pub notebook: String,
    pub cpu: u32,
    pub mem: u32,
    pub gpu: u32,
}

/// Where a job lives locally and on the server.
struct Paths {
    local: PathBuf,
    folder_name: String,
    /// Also the Docker container name, so it only uses characters Docker allows.
    job: String,
    job_dir: String,
    remote_folder: String,
}

impl Paths {
    fn new(sh: &SelfHosted, req: &JobRequest) -> Result<Self, String> {
        let local = PathBuf::from(&req.folder);
        let folder_name = local
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .ok_or("The selected folder has no name.")?;
        let clean: String = folder_name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || "_.-".contains(c) {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let job = format!("naccel-{clean}-{}", chrono::Local::now().format("%Y%m%d-%H%M%S"));
        let job_dir = format!("{}/{job}", sh.remote_dir.trim().trim_end_matches('/'));
        let remote_folder = format!("{job_dir}/{folder_name}");
        Ok(Self {
            local,
            folder_name,
            job,
            job_dir,
            remote_folder,
        })
    }

    fn result_file(&self, mode: ResultMode, notebook: &str) -> PathBuf {
        match mode {
            ResultMode::Replace => self.local.join(notebook),
            ResultMode::Copy => {
                let stem = Path::new(notebook)
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "notebook".into());
                self.local.join(format!("{stem}_solved.ntop"))
            }
        }
    }

    /// Everything in the folder except other `.ntop` files, since the runner expects exactly one notebook.
    fn upload_entries(&self, notebook: &str) -> Result<Vec<PathBuf>, String> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(&self.local).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.to_lowercase().ends_with(".ntop") || name == notebook {
                out.push(entry.path());
            }
        }
        Ok(out)
    }
}

struct Job<'a> {
    app: AppHandle,
    token: Token,
    settings: &'a Settings,
    req: &'a JobRequest,
    password: Option<String>,
}

impl Job<'_> {
    fn sh(&self) -> &SelfHosted {
        &self.settings.self_hosted
    }

    fn log(&self, level: Level, text: &str) {
        events::log(&self.app, level, text);
    }

    /// Runs `fut` unless the user cancels first.
    async fn step<T>(&mut self, fut: impl Future<Output = Result<T, String>>) -> Result<T, Stop> {
        tokio::select! {
            r = fut => r.map_err(Stop::Failed),
            _ = cancel::requested(&mut self.token) => Err(Stop::Cancelled),
        }
    }

    async fn execute(&mut self, paths: &Paths) -> Result<(), Stop> {
        let sh = self.sh().clone();
        events::phase(&self.app, Phase::Queued);
        self.log(Level::Info, &format!("Connecting to {}", remote::target(&sh)));
        self.step(remote::run(&sh, "true")).await?;
        self.log(Level::Info, &format!("Connected to {}", sh.host));
        self.step(server::install_runner(&sh)).await?;

        events::phase(&self.app, Phase::Upload);
        let entries = paths.upload_entries(&self.req.notebook)?;
        self.log(
            Level::Info,
            &format!("Uploading {}/ ({} items)", paths.folder_name, entries.len()),
        );
        if let Outcome::Cancelled =
            transfer::upload(&self.app, &sh, entries, &paths.remote_folder, &mut self.token).await?
        {
            return Err(Stop::Cancelled);
        }
        self.log(Level::Info, &format!("Uploaded {}/", paths.folder_name));

        events::phase(&self.app, Phase::Solve);
        let saved = self.solve(&sh, paths).await?;

        events::phase(&self.app, Phase::Download);
        let target = paths.result_file(self.settings.results.mode, &self.req.notebook);
        self.log(Level::Info, &format!("Downloading {}", target.display()));
        if let Outcome::Cancelled = transfer::download(&self.app, &sh, &saved, &target, &mut self.token).await? {
            return Err(Stop::Cancelled);
        }
        self.log(Level::Info, &format!("Received {}", target.display()));
        Ok(())
    }

    /// Runs the solver with stats streaming alongside. On cancel the stats keep streaming until the
    /// solver is confirmed stopped, so the CPU load can be seen dropping before disconnecting.
    async fn solve(&mut self, sh: &SelfHosted, paths: &Paths) -> Result<String, Stop> {
        let script = solver::script(
            sh,
            &solver::Launch {
                job: &paths.job,
                job_dir: &paths.job_dir,
                folder: &paths.folder_name,
                email: &self.settings.ntop_user,
                cpus: self.req.cpu,
                mem_gb: self.req.mem,
                gpus: self.req.gpu,
            },
        );
        let stats_task = tokio::spawn(stats::stream(self.app.clone(), sh.clone(), self.req.cpu as usize));
        let run = solver::run(&self.app, sh, &script, self.password.as_deref(), &mut self.token).await;
        let result = match run.saved {
            Outcome::Done(saved) => saved.map_err(Stop::Failed),
            Outcome::Cancelled => {
                self.log(Level::Info, "Stopping the remote solve");
                if solver::stop(sh, run.pid, &paths.job).await {
                    self.log(Level::Info, "Remote solve stopped");
                } else {
                    self.log(Level::Warn, "The remote solve may still be running; check the server");
                }
                Err(Stop::Cancelled)
            }
        };
        stats_task.abort();
        result
    }
}

pub async fn run(app: AppHandle, settings: Settings, req: JobRequest, password: Option<String>, token: Token) {
    let sh = settings.self_hosted.clone();
    let paths = match Paths::new(&sh, &req) {
        Ok(p) => p,
        Err(e) => return finish(&app, &sh, Err(Stop::Failed(e))),
    };
    let mut job = Job {
        app: app.clone(),
        token,
        settings: &settings,
        req: &req,
        password,
    };
    let result = job.execute(&paths).await;
    // The job folder is removed whether the job finished or was cancelled; after a failure it is kept for inspection.
    if !matches!(result, Err(Stop::Failed(_))) {
        let _ = remote::run(&sh, &format!("rm -rf {}", shell_path(&paths.job_dir))).await;
        if matches!(result, Err(Stop::Cancelled)) {
            events::log(&app, Level::Info, "Removed the remote job folder");
        }
    }
    finish(&app, &sh, result);
}

fn finish(app: &AppHandle, sh: &SelfHosted, result: Result<(), Stop>) {
    match &result {
        Ok(()) => {}
        Err(Stop::Cancelled) => events::log(app, Level::Warn, "Cancelled"),
        Err(Stop::Failed(e)) => {
            events::log(app, Level::Error, &format!("nAccelerate: {e}"));
            events::error(app, e);
        }
    }
    events::log(app, Level::Info, &format!("Disconnected from {}", sh.host));
    match result {
        Ok(()) => {
            events::log(app, Level::Info, "Complete");
            events::phase(app, Phase::Done);
        }
        Err(_) => events::phase(app, Phase::Idle),
    }
}
