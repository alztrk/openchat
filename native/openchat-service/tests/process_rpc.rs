use std::{error::Error, fs, io, path::PathBuf, process::Stdio, time::Duration};

use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{ChildStdin, ChildStdout, Command},
    time::timeout,
};
use uuid::Uuid;

#[tokio::test]
async fn child_service_handles_health_and_shutdown_rpc_in_an_isolated_profile()
-> Result<(), Box<dyn Error>> {
    let profile = TemporaryProfile::new()?;
    let mut child = Command::new(env!("CARGO_BIN_EXE_openchat_service"))
        .arg("--data-root")
        .arg(profile.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;
    let mut stdin = child.stdin.take().ok_or("service stdin was not piped")?;
    let stdout = child.stdout.take().ok_or("service stdout was not piped")?;
    let mut stdout = BufReader::new(stdout);

    let health = exchange(&mut stdin, &mut stdout, "health", "system.health").await?;
    assert_eq!(health["id"], "health");
    assert_eq!(health["result"]["status"], "ready");
    assert_eq!(
        health["result"]["database_path"].as_str(),
        Some(
            profile
                .path()
                .join("db")
                .join("openchat.sqlite3")
                .to_string_lossy()
                .as_ref()
        )
    );

    let shutdown = exchange(&mut stdin, &mut stdout, "shutdown", "system.shutdown").await?;
    assert_eq!(shutdown["id"], "shutdown");
    assert_eq!(shutdown["result"]["stopping"], true);
    drop(stdin);
    let status = timeout(Duration::from_secs(10), child.wait()).await??;
    assert!(status.success(), "service child returned {status}");
    Ok(())
}

struct TemporaryProfile(PathBuf);

impl TemporaryProfile {
    fn new() -> io::Result<Self> {
        let path =
            std::env::temp_dir().join(format!("openchat-process-rpc-{}", Uuid::new_v4().simple()));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }

    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TemporaryProfile {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

async fn exchange(
    stdin: &mut ChildStdin,
    stdout: &mut BufReader<ChildStdout>,
    id: &str,
    method: &str,
) -> Result<Value, Box<dyn Error>> {
    let request = json!({"id": id, "method": method, "params": {}});
    let mut line = serde_json::to_vec(&request)?;
    line.push(b'\n');
    stdin.write_all(&line).await?;
    stdin.flush().await?;

    let mut response_line = String::new();
    let response_bytes = timeout(
        Duration::from_secs(10),
        stdout.read_line(&mut response_line),
    )
    .await??;
    if response_bytes == 0 {
        return Err("service child closed stdout before replying".into());
    }
    Ok(serde_json::from_str(&response_line)?)
}
