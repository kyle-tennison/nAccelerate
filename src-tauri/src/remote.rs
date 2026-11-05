//! Thin wrappers around the system OpenSSH client, which ships with Windows 10+, macOS and Linux.

use crate::settings::SelfHosted;
use std::process::{ExitStatus, Stdio};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

fn expand_home(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\")) {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_default();
        return format!("{home}/{rest}");
    }
    path.to_string()
}

/// Quotes a path for the remote POSIX shell while keeping a leading `~/` unquoted so it still expands.
pub fn shell_path(path: &str) -> String {
    match path.strip_prefix("~/") {
        Some(rest) => format!("~/{}", shell_quote(rest)),
        None if path == "~" => "~".into(),
        None => shell_quote(path),
    }
}

pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

pub fn target(sh: &SelfHosted) -> String {
    if sh.user.trim().is_empty() {
        sh.host.trim().to_string()
    } else {
        format!("{}@{}", sh.user.trim(), sh.host.trim())
    }
}

/// An `ssh` command that runs `script` on the server. Stdin is closed unless the caller pipes it.
pub fn ssh(sh: &SelfHosted, script: &str) -> Command {
    let mut cmd = Command::new("ssh");
    cmd.arg("-p").arg(sh.port.to_string());
    cmd.args([
        "-o",
        "BatchMode=yes",
        "-o",
        "ConnectTimeout=10",
        "-o",
        "StrictHostKeyChecking=accept-new",
        "-o",
        "ServerAliveInterval=15",
    ]);
    if !sh.key_path.trim().is_empty() {
        cmd.arg("-i").arg(expand_home(sh.key_path.trim()));
    }
    cmd.arg(target(sh)).arg(script);
    cmd.stdin(Stdio::null()).kill_on_drop(true);
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

pub fn spawn_error(e: std::io::Error) -> String {
    format!("Could not start ssh: {e}. Is OpenSSH installed?")
}

/// Turns ssh's stderr into a readable message, dropping the `@@@@` banner lines it wraps warnings in.
pub fn failure(stderr: &[u8], status: ExitStatus) -> String {
    let text = String::from_utf8_lossy(stderr)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.chars().all(|c| c == '@'))
        .collect::<Vec<_>>()
        .join("\n");
    if text.is_empty() {
        format!("Remote command failed ({status})")
    } else {
        text
    }
}

/// Runs `script` on the server and returns its stdout.
pub async fn run(sh: &SelfHosted, script: &str) -> Result<String, String> {
    run_with_input(sh, script, None).await
}

/// Runs `script` on the server with `input` written to its stdin, and returns its stdout.
pub async fn run_with_input(sh: &SelfHosted, script: &str, input: Option<&str>) -> Result<String, String> {
    let mut cmd = ssh(sh, script);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    if input.is_some() {
        cmd.stdin(Stdio::piped());
    }
    let mut child = cmd.spawn().map_err(spawn_error)?;
    if let (Some(input), Some(mut stdin)) = (input, child.stdin.take()) {
        stdin.write_all(input.as_bytes()).await.map_err(|e| e.to_string())?;
    }
    let out = child.wait_with_output().await.map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(failure(&out.stderr, out.status))
    }
}
