//! Preparing and inspecting the self-hosted server.

use crate::remote;
use crate::settings::{HostInfo, SelfHosted};

/// The ntop runner the app installs on the server, so the default solve command works without setup.
const RUNNER: &str = include_str!("../resources/ntop-run.py");

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

pub async fn install_runner(sh: &SelfHosted) -> Result<(), String> {
    let script = "mkdir -p ~/.naccelerate && cat > ~/.naccelerate/ntop-run.py && chmod +x ~/.naccelerate/ntop-run.py";
    remote::run_with_input(sh, script, Some(RUNNER)).await.map(|_| ())
}

/// Reads core count, memory, GPUs and OS from a Linux server, and checks that Python and Docker are there for the runner.
pub async fn probe(sh: &SelfHosted) -> Result<HostInfo, String> {
    check(sh)?;
    let script = "nproc; awk '/MemTotal/ {print $2}' /proc/meminfo; \
         (nvidia-smi -L 2>/dev/null | grep -c '^GPU') || true; \
         uname -sr; command -v python3 >/dev/null && command -v docker >/dev/null && echo yes || echo no";
    let out = remote::run(sh, script).await?;
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
