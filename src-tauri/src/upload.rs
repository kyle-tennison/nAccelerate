//! Uploads files by streaming a tar archive into `tar -x` on the server, counting bytes as they go
//! so the UI can show real progress (scp prints none when it has no terminal).

use crate::remote::{self, shell_path};
use crate::settings::SelfHosted;
use serde::Serialize;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;
use tokio::sync::{mpsc, watch};

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub done: u64,
    pub total: u64,
}

pub enum Outcome {
    Done,
    Cancelled,
}

struct ChannelWriter(mpsc::Sender<Vec<u8>>);

impl Write for ChannelWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0
            .blocking_send(buf.to_vec())
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "upload stopped"))?;
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn size_of(path: &Path) -> u64 {
    match std::fs::metadata(path) {
        Ok(m) if m.is_dir() => std::fs::read_dir(path)
            .map(|rd| rd.filter_map(|e| e.ok()).map(|e| size_of(&e.path())).sum())
            .unwrap_or(0),
        Ok(m) => m.len(),
        Err(_) => 0,
    }
}

fn write_archive(entries: Vec<PathBuf>, out: ChannelWriter) -> std::io::Result<()> {
    let mut tar = tar::Builder::new(BufWriter::with_capacity(256 * 1024, out));
    for path in entries {
        let name = path.file_name().map(PathBuf::from).unwrap_or_default();
        if path.is_dir() {
            tar.append_dir_all(&name, &path)?;
        } else {
            tar.append_path_with_name(&path, &name)?;
        }
    }
    tar.into_inner()?.flush()
}

pub async fn upload(
    app: &AppHandle,
    sh: &SelfHosted,
    entries: Vec<PathBuf>,
    remote_dir: &str,
    cancel: &mut watch::Receiver<bool>,
) -> Result<Outcome, String> {
    let total: u64 = entries.iter().map(|p| size_of(p)).sum();
    let dir = shell_path(remote_dir);
    let mut cmd = remote::ssh(sh, &format!("mkdir -p {dir} && tar -xf - -C {dir}"), false);
    cmd.stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("Could not start ssh: {e}"))?;
    let mut stdin = child.stdin.take().ok_or("ssh has no input stream")?;

    let (tx, mut rx) = mpsc::channel::<Vec<u8>>(32);
    let writer = tokio::task::spawn_blocking(move || write_archive(entries, ChannelWriter(tx)));

    let mut sent = 0u64;
    let mut last_emit = Instant::now() - Duration::from_secs(1);
    let emit = |done: u64| {
        let _ = app.emit("job-progress", Progress { done: done.min(total), total });
    };
    emit(0);
    loop {
        tokio::select! {
            chunk = rx.recv() => match chunk {
                Some(chunk) => {
                    if stdin.write_all(&chunk).await.is_err() { break; }
                    sent += chunk.len() as u64;
                    if last_emit.elapsed() >= Duration::from_millis(100) {
                        emit(sent);
                        last_emit = Instant::now();
                    }
                }
                None => break,
            },
            _ = crate::job::cancelled(cancel) => {
                drop(rx);
                let _ = child.kill().await;
                return Ok(Outcome::Cancelled);
            }
        }
    }
    drop(stdin);
    let archived = writer.await.map_err(|e| e.to_string())?;
    let out = child.wait_with_output().await.map_err(|e| e.to_string())?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if err.is_empty() { format!("Upload failed ({})", out.status) } else { err });
    }
    archived.map_err(|e| format!("Could not read the project folder: {e}"))?;
    emit(total);
    Ok(Outcome::Done)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_round_trips_through_system_tar() {
        let src = std::env::temp_dir().join(format!("naccel-upload-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&src);
        std::fs::create_dir_all(src.join("sub")).unwrap();
        std::fs::write(src.join("part.ntop"), vec![7u8; 300_000]).unwrap();
        std::fs::write(src.join("sub/loads.csv"), b"a,b\n1,2\n").unwrap();

        let (tx, mut rx) = mpsc::channel(4);
        let entries = vec![src.join("part.ntop"), src.join("sub")];
        let writer = std::thread::spawn(move || write_archive(entries, ChannelWriter(tx)));
        let mut bytes = Vec::new();
        while let Some(chunk) = rx.blocking_recv() {
            bytes.extend(chunk);
        }
        writer.join().unwrap().unwrap();

        let out = src.join("out");
        std::fs::create_dir_all(&out).unwrap();
        let mut tar = std::process::Command::new("tar")
            .args(["-xf", "-", "-C"])
            .arg(&out)
            .stdin(Stdio::piped())
            .spawn()
            .unwrap();
        tar.stdin.take().unwrap().write_all(&bytes).unwrap();
        assert!(tar.wait().unwrap().success());
        assert_eq!(std::fs::read(out.join("part.ntop")).unwrap().len(), 300_000);
        assert_eq!(std::fs::read(out.join("sub/loads.csv")).unwrap(), b"a,b\n1,2\n");
        assert_eq!(size_of(&src.join("sub")), 8);
        std::fs::remove_dir_all(&src).unwrap();
    }
}
