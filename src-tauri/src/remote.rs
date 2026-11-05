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

/// scp resolves relative paths against the remote home directory, so `~/x` becomes `x`.
pub fn scp_path(path: &str) -> String {
    path.strip_prefix("~/").unwrap_or(path).to_string()
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

/// An `ssh` command that runs `script` on the remote host. A pseudo-terminal makes the remote solver
/// line-buffer its output, and closing the connection hangs it up, which is how cancel stops it.
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

pub fn scp(sh: &SelfHosted, from: &str, to: &str) -> Command {
    let mut cmd = Command::new("scp");
    cmd.arg("-P").arg(sh.port.to_string()).arg("-r");
    common_opts(&mut cmd, sh);
    cmd.arg(from).arg(to);
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

/// Reads core count, memory, GPUs and OS from a Linux host, and checks that the solve command exists.
pub async fn probe(sh: &SelfHosted) -> Result<HostInfo, String> {
    check(sh)?;
    let program = sh
        .command
        .split_whitespace()
        .filter(|w| !w.contains('{') && !w.starts_with('-') && !w.contains('='))
        .find(|w| !w.chars().all(|c| c.is_ascii_digit()) && *w != "taskset")
        .unwrap_or("ntop")
        .to_string();
    let script = format!(
        "nproc; awk '/MemTotal/ {{print $2}}' /proc/meminfo; \
         (nvidia-smi -L 2>/dev/null | grep -c '^GPU') || true; \
         uname -sr; command -v {} >/dev/null && echo yes || echo no",
        shell_quote(&program)
    );
    let out = run(sh, &script).await?;
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
