use std::{
    error::Error,
    fs, io,
    path::{Path, PathBuf},
    process::Stdio,
    time::{Duration, Instant},
};

use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
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

#[tokio::test]
#[ignore = "manual debug/test process startup and IPC benchmark"]
async fn benchmarks_debug_service_startup_and_health_rpc() -> Result<(), Box<dyn Error>> {
    const STARTUP_SAMPLES: usize = 100;
    const RPC_WARMUP_SAMPLES: usize = 20;
    const RPC_SAMPLES: usize = 200;

    let profile = TemporaryProfile::new()?;
    let mut cold_start_samples = Vec::with_capacity(STARTUP_SAMPLES);
    for sample in 0..STARTUP_SAMPLES {
        let data_root = profile.path().join(format!("cold-{sample}"));
        fs::create_dir(&data_root)?;

        let started = Instant::now();
        let mut service = RunningService::start(&data_root)?;
        service.health_check(&format!("cold-{sample}")).await?;
        cold_start_samples.push(started.elapsed());
        service.shutdown().await?;
    }

    let warm_root = profile.path().join("warm");
    fs::create_dir(&warm_root)?;
    let mut warm_service = RunningService::start(&warm_root)?;
    warm_service.health_check("warm-prime").await?;
    warm_service.shutdown().await?;

    let mut warm_start_samples = Vec::with_capacity(STARTUP_SAMPLES);
    for sample in 0..STARTUP_SAMPLES {
        let started = Instant::now();
        let mut service = RunningService::start(&warm_root)?;
        service.health_check(&format!("warm-{sample}")).await?;
        warm_start_samples.push(started.elapsed());
        service.shutdown().await?;
    }

    let mut service = RunningService::start(&warm_root)?;
    service.health_check("rpc-prime").await?;
    for sample in 0..RPC_WARMUP_SAMPLES {
        let response = service
            .request(&format!("warmup-{sample}"), "system.health")
            .await?;
        assert_ready(&response, &format!("warmup-{sample}"));
    }

    let mut rpc_samples = Vec::with_capacity(RPC_SAMPLES);
    for sample in 0..RPC_SAMPLES {
        let id = format!("rpc-{sample}");
        let started = Instant::now();
        let response = service.request(&id, "system.health").await?;
        rpc_samples.push(started.elapsed());
        assert_ready(&response, &id);
    }

    eprintln!(
        "service-startup cold debug n={STARTUP_SAMPLES}; {}; warm debug n={STARTUP_SAMPLES}; {}",
        format_percentiles_milliseconds(&cold_start_samples),
        format_percentiles_milliseconds(&warm_start_samples),
    );
    eprintln!(
        "service-health NDJSON round-trip debug n={RPC_SAMPLES}; warmup={RPC_WARMUP_SAMPLES}; {}",
        format_percentiles_microseconds(&rpc_samples),
    );

    service.shutdown().await?;
    Ok(())
}

struct RunningService {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl RunningService {
    fn start(data_root: &Path) -> io::Result<Self> {
        let mut child = Command::new(env!("CARGO_BIN_EXE_openchat_service"))
            .arg("--data-root")
            .arg(data_root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("service stdin was not piped"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("service stdout was not piped"))?;
        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
        })
    }

    async fn health_check(&mut self, id: &str) -> Result<(), Box<dyn Error>> {
        let health = self.request(id, "system.health").await?;
        assert_ready(&health, id);
        Ok(())
    }

    async fn request(&mut self, id: &str, method: &str) -> Result<Value, Box<dyn Error>> {
        exchange(&mut self.stdin, &mut self.stdout, id, method).await
    }

    async fn shutdown(mut self) -> Result<(), Box<dyn Error>> {
        let response = self.request("shutdown", "system.shutdown").await?;
        assert_eq!(response["id"], "shutdown");
        assert_eq!(response["result"]["stopping"], true);
        self.stdin.shutdown().await?;
        let status = timeout(Duration::from_secs(10), self.child.wait()).await??;
        assert!(status.success(), "service child returned {status}");
        Ok(())
    }
}

fn assert_ready(response: &Value, id: &str) {
    assert_eq!(response["id"], id);
    assert_eq!(response["result"]["status"], "ready");
}

fn format_percentiles_milliseconds(samples: &[Duration]) -> String {
    format!(
        "p50={:.3}ms p95={:.3}ms p99={:.3}ms",
        percentile_milliseconds(samples, 0.50),
        percentile_milliseconds(samples, 0.95),
        percentile_milliseconds(samples, 0.99),
    )
}

fn format_percentiles_microseconds(samples: &[Duration]) -> String {
    format!(
        "p50={:.1}us p95={:.1}us p99={:.1}us",
        percentile_microseconds(samples, 0.50),
        percentile_microseconds(samples, 0.95),
        percentile_microseconds(samples, 0.99),
    )
}

fn percentile_milliseconds(samples: &[Duration], probability: f64) -> f64 {
    percentile_nanoseconds(samples, probability) / 1_000_000.0
}

fn percentile_microseconds(samples: &[Duration], probability: f64) -> f64 {
    percentile_nanoseconds(samples, probability) / 1_000.0
}

fn percentile_nanoseconds(samples: &[Duration], probability: f64) -> f64 {
    let mut sorted = samples.iter().map(Duration::as_nanos).collect::<Vec<_>>();
    sorted.sort_unstable();
    let position = probability * (sorted.len() - 1) as f64;
    let lower_index = position.floor() as usize;
    let upper_index = position.ceil() as usize;
    let fraction = position - lower_index as f64;
    let lower = sorted[lower_index] as f64;
    let upper = sorted[upper_index] as f64;
    lower + (upper - lower) * fraction
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
