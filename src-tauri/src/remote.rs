//! Thin wrappers around the system OpenSSH client (`ssh` and `scp`), which ships with Windows 10+, macOS and Linux.

use crate::settings::{HostInfo, SelfHosted};
use std::process::Stdio;
use tokio::process::Command;

fn expand_home(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\")) {
        let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_default();
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

/// Catches setup mistakes that ssh would otherwise report with a long, confusing message.
pub fn check(sh: &SelfHosted) -> Result<(), String> {
    if sh.host.trim().is_empty() {
        return Err("Enter a host first.".into());
    }
    let key = sh.key_path.trim();
    if key.to_lowercase().ends_with(".pub") {
        return Err(format!(
            "The SSH key should be the private key, not the .pub file. Try {}.",
            &key[..key.len() - 4]
        ));
    }
    Ok(())
}

fn common_opts(cmd: &mut Command, sh: &SelfHosted) {
    cmd.args(["-o", "BatchMode=yes", "-o", "ConnectTimeout=10", "-o", "StrictHostKeyChecking=accept-new", "-o", "ServerAliveInterval=15"]);
    if !sh.key_path.trim().is_empty() {
        cmd.arg("-i").arg(expand_home(sh.key_path.trim()));
    }
    cmd.stdin(Stdio::null()).kill_on_drop(true);
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
}

/// An `ssh` command that runs `script` on the remote host, optionally with a pseudo-terminal.
pub fn ssh(sh: &SelfHosted, script: &str, tty: bool) -> Command {
    let mut cmd = Command::new("ssh");
    cmd.arg("-p").arg(sh.port.to_string());
    common_opts(&mut cmd, sh);
    if tty {
        cmd.arg("-tt");
    }
    cmd.arg(target(sh)).arg(script);
    cmd
}

pub async fn output(mut cmd: Command) -> Result<String, String> {
    let out = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| format!("Could not start ssh: {e}. Is OpenSSH installed?"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        let err = String::from_utf8_lossy(&out.stderr)
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.chars().all(|c| c == '@'))
            .collect::<Vec<_>>()
            .join("\n");
        Err(if err.is_empty() { format!("Remote command failed ({})", out.status) } else { err })
    }
}

pub async fn run(sh: &SelfHosted, script: &str) -> Result<String, String> {
    output(ssh(sh, script, false)).await
}

/// Runs `script` with `input` on its stdin.
pub async fn run_with_input(sh: &SelfHosted, script: &str, input: &str) -> Result<String, String> {
    use tokio::io::AsyncWriteExt;
    let mut cmd = ssh(sh, script, false);
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("Could not start ssh: {e}. Is OpenSSH installed?"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input.as_bytes()).await.map_err(|e| e.to_string())?;
    }
    let out = child.wait_with_output().await.map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

/// The ntop runner the app installs on the server, so the default solve command works without setup.
pub const RUNNER: &str = include_str!("../resources/ntop-run.py");

pub async fn install_runner(sh: &SelfHosted) -> Result<(), String> {
    run_with_input(sh, "mkdir -p ~/.naccelerate && cat > ~/.naccelerate/ntop-run.py && chmod +x ~/.naccelerate/ntop-run.py", RUNNER)
        .await
        .map(|_| ())
}

/// Reads core count, memory, GPUs and OS from a Linux host, and checks that the solve command exists.
pub async fn probe(sh: &SelfHosted) -> Result<HostInfo, String> {
    check(sh)?;
    let script = "nproc; awk '/MemTotal/ {print $2}' /proc/meminfo; \
         (nvidia-smi -L 2>/dev/null | grep -c '^GPU') || true; \
         uname -sr; command -v python3 >/dev/null && command -v docker >/dev/null && echo yes || echo no";
    let out = run(sh, script).await?;
    let lines: Vec<&str> = out.lines().map(str::trim).collect();
    let num = |i: usize| lines.get(i).and_then(|l| l.parse::<f64>().ok()).unwrap_or(0.0);
    Ok(HostInfo {
        cores: num(0) as u32,
        mem_gb: (num(1) / 1024.0 / 1024.0) as f32,
        gpus: num(2) as u32,
        os: lines.get(3).unwrap_or(&"").to_string(),
        ntop_found: lines.get(4) == Some(&"yes"),
    })
}
