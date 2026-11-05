//! Streams CPU, memory and GPU usage from a Linux host over one long-lived ssh connection.

use crate::remote;
use crate::settings::SelfHosted;
use serde::Serialize;
use std::process::Stdio;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};

const SCRIPT: &str = "while true; do grep '^cpu[0-9]' /proc/stat; grep -E '^(MemTotal|MemAvailable):' /proc/meminfo; \
nvidia-smi --query-gpu=utilization.gpu,memory.used,memory.total --format=csv,noheader,nounits 2>/dev/null; echo ---; sleep 1; done";

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Gpu {
    pub util: f32,
    pub vram: f32,
}

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub cores: Vec<f32>,
    pub mem_gb: f32,
    pub gpus: Vec<Gpu>,
}

#[derive(Default)]
struct Block {
    cpu: Vec<(u64, u64)>,
    mem_total_kb: u64,
    mem_avail_kb: u64,
    gpus: Vec<Gpu>,
}

fn parse_line(block: &mut Block, line: &str) {
    if let Some(rest) = line.strip_prefix("cpu") {
        let fields: Vec<u64> = rest.split_whitespace().skip(1).filter_map(|f| f.parse().ok()).collect();
        if fields.len() >= 4 {
            let total: u64 = fields.iter().sum();
            let idle = fields[3] + fields.get(4).copied().unwrap_or(0);
            block.cpu.push((total, idle));
        }
    } else if let Some(v) = line.strip_prefix("MemTotal:") {
        block.mem_total_kb = v.trim().trim_end_matches("kB").trim().parse().unwrap_or(0);
    } else if let Some(v) = line.strip_prefix("MemAvailable:") {
        block.mem_avail_kb = v.trim().trim_end_matches("kB").trim().parse().unwrap_or(0);
    } else {
        let f: Vec<f32> = line.split(',').filter_map(|x| x.trim().parse().ok()).collect();
        if f.len() == 3 && f[2] > 0.0 {
            block.gpus.push(Gpu { util: f[0] / 100.0, vram: f[1] / f[2] });
        }
    }
}

/// Runs until the task is aborted. Only the first `cores` CPUs are reported, matching the cores the job is pinned to.
pub async fn stream(app: AppHandle, sh: SelfHosted, cores: usize) {
    let mut cmd = remote::ssh(&sh, SCRIPT, false);
    cmd.stdout(Stdio::piped()).stderr(Stdio::null());
    let Ok(mut child) = cmd.spawn() else { return };
    let Some(stdout) = child.stdout.take() else { return };
    let mut lines = BufReader::new(stdout).lines();
    let mut prev: Vec<(u64, u64)> = Vec::new();
    let mut block = Block::default();
    while let Ok(Some(line)) = lines.next_line().await {
        if line.trim() != "---" {
            parse_line(&mut block, line.trim());
            continue;
        }
        let done = std::mem::take(&mut block);
        if prev.len() == done.cpu.len() {
            let usage = done
                .cpu
                .iter()
                .zip(&prev)
                .take(cores.max(1))
                .map(|(&(t, i), &(pt, pi))| {
                    let dt = t.saturating_sub(pt) as f32;
                    if dt <= 0.0 { 0.0 } else { 1.0 - i.saturating_sub(pi) as f32 / dt }
                })
                .collect();
            let mem_gb = done.mem_total_kb.saturating_sub(done.mem_avail_kb) as f32 / 1024.0 / 1024.0;
            let _ = app.emit("job-stats", Stats { cores: usage, mem_gb, gpus: done.gpus });
        }
        prev = done.cpu;
    }
    let _ = child.kill().await;
}
