//! Running the solve command on the server: building its shell script, streaming its output, and stopping it.

use crate::cancel::{self, Outcome, Token};
use crate::events;
use crate::remote::{self, shell_path, shell_quote};
use crate::settings::SelfHosted;
use std::process::Stdio;
use tauri::AppHandle;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// The remote shell prints this with its PID first, so a cancel can stop its whole process group.
const PID_MARK: &str = "NACCEL_PID ";

pub struct Launch<'a> {
    pub job: &'a str,
    pub job_dir: &'a str,
    pub folder: &'a str,
    pub email: &'a str,
    pub cpus: u32,
    pub mem_gb: u32,
    pub gpus: u32,
}

/// The remote shell script: report the PID, read the password from stdin (so it never appears in the
/// process list), export the login and resources, then run the solve command in the job folder.
pub fn script(sh: &SelfHosted, l: &Launch) -> String {
    let cpu_last = l.cpus.saturating_sub(1);
    let command = sh
        .command
        .replace("{folder}", &shell_quote(l.folder))
        .replace("{job}", l.job)
        .replace("{cpus}", &l.cpus.to_string())
        .replace("{cpu_last}", &cpu_last.to_string())
        .replace("{mem_gb}", &l.mem_gb.to_string())
        .replace("{gpus}", &l.gpus.to_string());
    let visible_gpus = (0..l.gpus).map(|i| i.to_string()).collect::<Vec<_>>().join(",");
    format!(
        "exec 2>&1; echo \"{PID_MARK}$$\"; IFS= read -r NTOP_PASSWORD || true; \
         export NTOP_PASSWORD NTOP_EMAIL={email} NACCEL_JOB={job} NACCEL_CPUS={cpus} NACCEL_CPUSET=0-{cpu_last} \
         NACCEL_MEM_GB={mem} NACCEL_GPUS={gpus} OMP_NUM_THREADS={cpus} CUDA_VISIBLE_DEVICES={visible} PYTHONUNBUFFERED=1; \
         cd {dir} && {command}",
        email = shell_quote(l.email),
        job = shell_quote(l.job),
        cpus = l.cpus,
        mem = l.mem_gb,
        gpus = l.gpus,
        visible = shell_quote(&visible_gpus),
        dir = shell_path(l.job_dir),
    )
}

pub struct Run {
    /// The remote shell's PID, once it has reported it.
    pub pid: Option<u32>,
    /// The result: the remote path from the solver's `Saved: <path>` line, or why there is none.
    pub saved: Outcome<Result<String, String>>,
}

/// Runs the script, forwarding every output line to the UI.
pub async fn run(app: &AppHandle, sh: &SelfHosted, script: &str, password: Option<&str>, token: &mut Token) -> Run {
    let mut cmd = remote::ssh(sh, script);
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return Run {
                pid: None,
                saved: Outcome::Done(Err(remote::spawn_error(e))),
            }
        }
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin
            .write_all(format!("{}\n", password.unwrap_or_default()).as_bytes())
            .await;
    }
    let Some(stdout) = child.stdout.take() else {
        return Run {
            pid: None,
            saved: Outcome::Done(Err("ssh produced no output stream".into())),
        };
    };

    let mut lines = BufReader::new(stdout).lines();
    let (mut pid, mut saved) = (None, None);
    loop {
        tokio::select! {
            line = lines.next_line() => match line {
                Ok(Some(line)) => {
                    let line = line.trim_end();
                    if line.is_empty() { continue; }
                    if let Some(p) = line.strip_prefix(PID_MARK) { pid = p.trim().parse().ok(); continue; }
                    if let Some(p) = line.strip_prefix("Saved:") { saved = Some(p.trim().to_string()); }
                    events::raw_log(app, line);
                }
                _ => break,
            },
            _ = cancel::requested(token) => {
                let _ = child.kill().await;
                return Run { pid, saved: Outcome::Cancelled };
            }
        }
    }

    let result = match (child.wait().await, saved) {
        (Ok(status), Some(path)) if status.success() => Ok(path),
        (Ok(status), _) if status.success() => {
            Err("The solver finished without a \"Saved:\" line, so there is nothing to download.".into())
        }
        (Ok(status), _) => Err(format!("The solver exited with {status}.")),
        (Err(e), _) => Err(e.to_string()),
    };
    Run {
        pid,
        saved: Outcome::Done(result),
    }
}

/// Closing ssh does not stop a solver running in Docker, so this terminates the remote process group, waits
/// up to 10 s for it to exit (then kills it), and runs the user's cancel command. Returns whether it is gone.
pub async fn stop(sh: &SelfHosted, pid: Option<u32>, job: &str) -> bool {
    let mut script = String::new();
    if let Some(pid) = pid {
        script.push_str(&format!(
            "kill -TERM -- -{pid} 2>/dev/null; \
             for i in $(seq 1 20); do kill -0 -- -{pid} 2>/dev/null || break; sleep 0.5; done; \
             kill -KILL -- -{pid} 2>/dev/null; "
        ));
    }
    let cancel = sh.cancel_command.trim();
    if !cancel.is_empty() {
        script.push_str(&format!("{{ {}; }} >/dev/null 2>&1; ", cancel.replace("{job}", job)));
    }
    match pid {
        Some(pid) => script.push_str(&format!(
            "sleep 0.2; kill -0 -- -{pid} 2>/dev/null && echo running || echo stopped"
        )),
        None => script.push_str("echo stopped"),
    }
    matches!(remote::run(sh, &script).await, Ok(out) if out.trim().ends_with("stopped"))
}
