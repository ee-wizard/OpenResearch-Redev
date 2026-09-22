//! Stop a running local `orx up` dashboard.
//!
//! `orx up` writes its pid to `~/.openresearch/orx-up.pid`; the companion
//! LAN proxy (`scripts/lan-proxy.cjs`) writes to `~/.openresearch/lan-proxy.pid`.
//! `orx --shutdown` reads those files, terminates the processes gracefully,
//! then falls back to scanning `/proc` on Unix for any stray `orx up` or
//! `lan-proxy.cjs` process.

use std::path::Path;
use std::time::Duration;

use tokio::process::Command;
use tokio::time::sleep;

use std::process::Stdio;

use crate::error::{anyhow, Result};

const UP_PID_FILE: &str = "orx-up.pid";
const PROXY_PID_FILE: &str = "lan-proxy.pid";

pub async fn run() -> Result<()> {
    let dir = dirs::home_dir()
        .ok_or_else(|| anyhow!("no home directory"))?
        .join(".openresearch");
    std::fs::create_dir_all(&dir)?;

    let mut stopped = false;
    stopped |= stop_by_pid_file(&dir, UP_PID_FILE, "orx up").await?;
    // The proxy is optional; don't fail the whole shutdown if it isn't there.
    let _ = stop_by_pid_file(&dir, PROXY_PID_FILE, "lan-proxy").await;

    #[cfg(unix)]
    if !stopped {
        for pid in find_pids(is_orx_up) {
            let _ = stop_pid(pid, "orx up").await;
        }
        for pid in find_pids(|cmd| cmd.iter().any(|arg| arg.contains("lan-proxy.cjs"))) {
            let _ = stop_pid(pid, "lan-proxy").await;
        }
    }

    println!("orx shutdown: done");
    Ok(())
}

async fn stop_by_pid_file(dir: &Path, name: &str, label: &str) -> Result<bool> {
    let path = dir.join(name);
    let Some(pid) = read_pid(&path) else {
        return Ok(false);
    };
    if !process_alive(pid).await {
        let _ = std::fs::remove_file(&path);
        return Ok(false);
    }
    stop_pid(pid, label).await?;
    let _ = std::fs::remove_file(&path);
    Ok(true)
}

fn read_pid(path: &Path) -> Option<u32> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

async fn process_alive(pid: u32) -> bool {
    Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .stderr(Stdio::null())
        .status()
        .await
        .is_ok_and(|s| s.success())
}

async fn stop_pid(pid: u32, label: &str) -> Result<()> {
    if !process_alive(pid).await {
        return Ok(());
    }
    println!("orx shutdown: stopping {label} (pid {pid})");
    Command::new("kill")
        .arg("-TERM")
        .arg(pid.to_string())
        .stderr(Stdio::null())
        .status()
        .await?;
    for _ in 0..50 {
        sleep(Duration::from_millis(100)).await;
        if !process_alive(pid).await {
            return Ok(());
        }
    }
    eprintln!("orx shutdown: {label} did not exit, sending SIGKILL");
    Command::new("kill")
        .arg("-KILL")
        .arg(pid.to_string())
        .stderr(Stdio::null())
        .status()
        .await?;
    Ok(())
}

#[cfg(unix)]
fn find_pids(predicate: impl Fn(&[String]) -> bool) -> Vec<u32> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return out;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let pid_str = name.to_string_lossy();
        let Ok(pid) = pid_str.parse::<u32>() else {
            continue;
        };
        let cmdline_path = entry.path().join("cmdline");
        let Ok(content) = std::fs::read_to_string(cmdline_path) else {
            continue;
        };
        let parts: Vec<String> = content
            .split('\0')
            .map(String::from)
            .filter(|s| !s.is_empty())
            .collect();
        if predicate(&parts) {
            out.push(pid);
        }
    }
    out
}

#[cfg(unix)]
fn is_orx_up(cmd: &[String]) -> bool {
    let Some(bin) = cmd.first() else {
        return false;
    };
    if !bin.ends_with("orx") && !bin.contains("/orx") {
        return false;
    }
    cmd.get(1).map(|s| s.as_str()) == Some("up")
}
