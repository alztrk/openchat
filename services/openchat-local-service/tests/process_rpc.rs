//! End-to-end checks for the sidecar's standard-input/output RPC process.

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
/// Verifies that a child service becomes ready and shuts down in an isolated profile.
async fn child_service_handles_health_and_shutdown_rpc_in_an_isolated_profile()
-> Result<(), Box<dyn Error>> {
    let profile = TemporaryProfile::new()?;
    let mut child = Command::new(env!("CARGO_BIN_EXE_openchat-local-service"))
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
    assert!(health["result"].get("storage_root").is_none());
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
/// Verifies invalid RPC frames are rejected without consuming the next frame.
async fn child_service_rejects_complex_json_without_losing_rpc_frame_alignment()
-> Result<(), Box<dyn Error>> {
    const PARAMETER_NODE_LIMIT: usize = 100_000;

    let profile = TemporaryProfile::new()?;
    let mut child = Command::new(env!("CARGO_BIN_EXE_openchat-local-service"))
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

    let invalid_params = json!({
        "id": "invalid-params",
        "method": "system.health",
        "params": [1],
    });
    let invalid = exchange_request(&mut stdin, &mut stdout, &invalid_params).await?;
    assert_eq!(invalid["id"], "invalid-params");
    assert_eq!(invalid["error"]["code"], "invalid_request_params");

    let invalid_method = json!({
        "id": "invalid-method",
        "method": "m".repeat(129),
        "params": {},
    });
    let rejected_method = exchange_request(&mut stdin, &mut stdout, &invalid_method).await?;
    assert_eq!(rejected_method["id"], "invalid-method");
    assert_eq!(rejected_method["error"]["code"], "invalid_request_method");

    let complex_request = json!({
        "id": "complex",
        "method": "system.health",
        "params": {"values": vec![Value::Null; PARAMETER_NODE_LIMIT]},
    });
    let rejected = exchange_request(&mut stdin, &mut stdout, &complex_request).await?;
    assert_eq!(rejected["id"], "complex");
    assert_eq!(rejected["error"]["code"], "request_too_complex");

    let health = exchange(
        &mut stdin,
        &mut stdout,
        "health-after-complex",
        "system.health",
    )
    .await?;
    assert_eq!(health["id"], "health-after-complex");
    assert_eq!(health["result"]["status"], "ready");

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
/// Measures debug startup phases, health-RPC latency, process CPU cycles, and memory counters.
async fn benchmarks_debug_service_startup_and_health_rpc() -> Result<(), Box<dyn Error>> {
    const STARTUP_SAMPLES: usize = 100;
    const RPC_WARMUP_SAMPLES: usize = 20;
    const RPC_SAMPLES: usize = 200;

    let profile = TemporaryProfile::new()?;
    let mut cold_start_samples = Vec::with_capacity(STARTUP_SAMPLES);
    let mut cold_spawn_and_pipe_samples = Vec::with_capacity(STARTUP_SAMPLES);
    let mut cold_ready_after_spawn_samples = Vec::with_capacity(STARTUP_SAMPLES);
    #[cfg(windows)]
    let mut cold_cpu_cycle_samples = Vec::with_capacity(STARTUP_SAMPLES);
    #[cfg(windows)]
    let mut cold_working_set_samples = Vec::with_capacity(STARTUP_SAMPLES);
    #[cfg(windows)]
    let mut cold_private_commit_samples = Vec::with_capacity(STARTUP_SAMPLES);
    for sample in 0..STARTUP_SAMPLES {
        let data_root = profile.path().join(format!("cold-{sample}"));
        fs::create_dir(&data_root)?;

        let started = Instant::now();
        let mut service = RunningService::start(&data_root)?;
        cold_spawn_and_pipe_samples.push(service.spawn_and_pipe_elapsed);
        let readiness_started = Instant::now();
        service.health_check(&format!("cold-{sample}")).await?;
        cold_ready_after_spawn_samples.push(readiness_started.elapsed());
        cold_start_samples.push(started.elapsed());
        #[cfg(windows)]
        {
            let metrics = service.process_metrics()?;
            cold_cpu_cycle_samples.push(metrics.cpu_cycles);
            cold_working_set_samples.push(metrics.peak_working_set_bytes);
            cold_private_commit_samples.push(metrics.private_commit_bytes);
        }
        service.shutdown().await?;
    }

    let warm_root = profile.path().join("warm");
    fs::create_dir(&warm_root)?;
    let mut warm_service = RunningService::start(&warm_root)?;
    warm_service.health_check("warm-prime").await?;
    warm_service.shutdown().await?;

    let mut warm_start_samples = Vec::with_capacity(STARTUP_SAMPLES);
    let mut warm_spawn_and_pipe_samples = Vec::with_capacity(STARTUP_SAMPLES);
    let mut warm_ready_after_spawn_samples = Vec::with_capacity(STARTUP_SAMPLES);
    #[cfg(windows)]
    let mut warm_cpu_cycle_samples = Vec::with_capacity(STARTUP_SAMPLES);
    #[cfg(windows)]
    let mut warm_working_set_samples = Vec::with_capacity(STARTUP_SAMPLES);
    #[cfg(windows)]
    let mut warm_private_commit_samples = Vec::with_capacity(STARTUP_SAMPLES);
    for sample in 0..STARTUP_SAMPLES {
        let started = Instant::now();
        let mut service = RunningService::start(&warm_root)?;
        warm_spawn_and_pipe_samples.push(service.spawn_and_pipe_elapsed);
        let readiness_started = Instant::now();
        service.health_check(&format!("warm-{sample}")).await?;
        warm_ready_after_spawn_samples.push(readiness_started.elapsed());
        warm_start_samples.push(started.elapsed());
        #[cfg(windows)]
        {
            let metrics = service.process_metrics()?;
            warm_cpu_cycle_samples.push(metrics.cpu_cycles);
            warm_working_set_samples.push(metrics.peak_working_set_bytes);
            warm_private_commit_samples.push(metrics.private_commit_bytes);
        }
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

    #[cfg(windows)]
    let health_rpc_cpu_started_at = service.process_metrics()?.cpu_cycles;
    let mut rpc_samples = Vec::with_capacity(RPC_SAMPLES);
    for sample in 0..RPC_SAMPLES {
        let id = format!("rpc-{sample}");
        let started = Instant::now();
        let response = service.request(&id, "system.health").await?;
        rpc_samples.push(started.elapsed());
        assert_ready(&response, &id);
    }
    #[cfg(windows)]
    let health_rpc_metrics = service.process_metrics()?;

    eprintln!(
        "service-startup cold debug n={STARTUP_SAMPLES}; {}; warm debug n={STARTUP_SAMPLES}; {}",
        format_percentiles_milliseconds(&cold_start_samples),
        format_percentiles_milliseconds(&warm_start_samples),
    );
    eprintln!(
        "service-startup phases debug; spawn+pipe cold {}; ready-after-spawn cold {}; spawn+pipe warm {}; ready-after-spawn warm {}",
        format_percentiles_milliseconds(&cold_spawn_and_pipe_samples),
        format_percentiles_milliseconds(&cold_ready_after_spawn_samples),
        format_percentiles_milliseconds(&warm_spawn_and_pipe_samples),
        format_percentiles_milliseconds(&warm_ready_after_spawn_samples),
    );
    eprintln!(
        "service-health NDJSON round-trip debug n={RPC_SAMPLES}; warmup={RPC_WARMUP_SAMPLES}; {}",
        format_percentiles_microseconds(&rpc_samples),
    );
    #[cfg(windows)]
    {
        let health_rpc_cpu_per_request = health_rpc_metrics
            .cpu_cycles
            .saturating_sub(health_rpc_cpu_started_at)
            / u64::try_from(RPC_SAMPLES)?;
        eprintln!(
            "service-process resources debug; cold CPU through readiness {}; cold peak working set {}; cold private commit {}; warm CPU through readiness {}; warm peak working set {}; warm private commit {}; health RPC average process CPU={:.1}kcycles; post-RPC peak working set={:.2}MiB; post-RPC private commit={:.2}MiB",
            format_percentiles_megacycles(&cold_cpu_cycle_samples),
            format_percentiles_mebibytes(&cold_working_set_samples),
            format_percentiles_mebibytes(&cold_private_commit_samples),
            format_percentiles_megacycles(&warm_cpu_cycle_samples),
            format_percentiles_mebibytes(&warm_working_set_samples),
            format_percentiles_mebibytes(&warm_private_commit_samples),
            health_rpc_cpu_per_request as f64 / 1_000.0,
            health_rpc_metrics.peak_working_set_bytes as f64 / (1024.0 * 1024.0),
            health_rpc_metrics.private_commit_bytes as f64 / (1024.0 * 1024.0),
        );
    }

    service.shutdown().await?;
    Ok(())
}

/// Owns one child service and the pipes used by its line-delimited RPC protocol.
struct RunningService {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    spawn_and_pipe_elapsed: Duration,
}

#[cfg(windows)]
/// Process-resource counters sampled from one benchmark child on Windows.
struct ProcessMetrics {
    cpu_cycles: u64,
    peak_working_set_bytes: usize,
    private_commit_bytes: usize,
}

impl RunningService {
    /// Starts the service with an explicit data root and piped standard input/output.
    fn start(data_root: &Path) -> io::Result<Self> {
        let started = Instant::now();
        let mut child = Command::new(env!("CARGO_BIN_EXE_openchat-local-service"))
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
            spawn_and_pipe_elapsed: started.elapsed(),
        })
    }

    #[cfg(windows)]
    /// Reads cumulative CPU cycles and peak working set for the still-running child.
    fn process_metrics(&mut self) -> io::Result<ProcessMetrics> {
        let process_id = self
            .child
            .id()
            .ok_or_else(|| io::Error::other("service process is no longer running"))?;
        process_metrics(process_id)
    }

    /// Sends a health request and asserts that the service has completed startup.
    async fn health_check(&mut self, id: &str) -> Result<(), Box<dyn Error>> {
        let health = self.request(id, "system.health").await?;
        assert_ready(&health, id);
        Ok(())
    }

    /// Sends one RPC request and returns its decoded JSON response.
    async fn request(&mut self, id: &str, method: &str) -> Result<Value, Box<dyn Error>> {
        exchange(&mut self.stdin, &mut self.stdout, id, method).await
    }

    /// Requests shutdown and waits at most ten seconds for a successful child exit.
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

#[cfg(windows)]
/// Reads process CPU-cycle and memory counters using a handle scoped to this benchmark.
fn process_metrics(process_id: u32) -> io::Result<ProcessMetrics> {
    use std::mem::size_of;

    use windows_sys::Win32::System::{
        ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX},
        Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ},
        WindowsProgramming::QueryProcessCycleTime,
    };

    // SAFETY: `process_id` comes from the running child process created by this benchmark.
    let process =
        unsafe { OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, 0, process_id) };
    if process.is_null() {
        return Err(io::Error::last_os_error());
    }
    let process = ProcessHandle(process);

    let mut memory = PROCESS_MEMORY_COUNTERS_EX {
        cb: u32::try_from(size_of::<PROCESS_MEMORY_COUNTERS_EX>())
            .map_err(|_| io::Error::other("process memory counter size is invalid"))?,
        ..Default::default()
    };
    // SAFETY: `process` is an open process handle and `memory` has its native structure size set.
    if unsafe {
        K32GetProcessMemoryInfo(
            process.0,
            (&mut memory as *mut PROCESS_MEMORY_COUNTERS_EX).cast(),
            memory.cb,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }

    let mut cpu_cycles = 0;
    // SAFETY: `process` is open and `cpu_cycles` is writable storage for the counter result.
    if unsafe { QueryProcessCycleTime(process.0, &mut cpu_cycles) } == 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(ProcessMetrics {
        cpu_cycles,
        peak_working_set_bytes: memory.PeakWorkingSetSize,
        private_commit_bytes: memory.PrivateUsage,
    })
}

#[cfg(windows)]
/// Closes its owned Windows process handle when the benchmark sample completes.
struct ProcessHandle(windows_sys::Win32::Foundation::HANDLE);

#[cfg(windows)]
impl Drop for ProcessHandle {
    /// Releases the Windows handle exactly once.
    fn drop(&mut self) {
        // SAFETY: This handle is owned by the guard and has not been closed elsewhere.
        unsafe { windows_sys::Win32::Foundation::CloseHandle(self.0) };
    }
}

/// Asserts that an RPC response matches its request and reports the ready state.
fn assert_ready(response: &Value, id: &str) {
    assert_eq!(response["id"], id);
    assert_eq!(response["result"]["status"], "ready");
}

/// Formats latency samples as interpolated p50, p95, and p99 milliseconds.
fn format_percentiles_milliseconds(samples: &[Duration]) -> String {
    format!(
        "p50={:.3}ms p95={:.3}ms p99={:.3}ms",
        percentile_milliseconds(samples, 0.50),
        percentile_milliseconds(samples, 0.95),
        percentile_milliseconds(samples, 0.99),
    )
}

/// Formats latency samples as interpolated p50, p95, and p99 microseconds.
fn format_percentiles_microseconds(samples: &[Duration]) -> String {
    format!(
        "p50={:.1}us p95={:.1}us p99={:.1}us",
        percentile_microseconds(samples, 0.50),
        percentile_microseconds(samples, 0.95),
        percentile_microseconds(samples, 0.99),
    )
}

#[cfg(windows)]
/// Formats process working-set samples as interpolated p50, p95, and p99 MiB.
fn format_percentiles_mebibytes(samples: &[usize]) -> String {
    format!(
        "p50={:.2}MiB p95={:.2}MiB p99={:.2}MiB",
        percentile_value(samples, 0.50) / (1024.0 * 1024.0),
        percentile_value(samples, 0.95) / (1024.0 * 1024.0),
        percentile_value(samples, 0.99) / (1024.0 * 1024.0),
    )
}

#[cfg(windows)]
/// Formats process CPU-cycle samples as interpolated p50, p95, and p99 Mcycles.
fn format_percentiles_megacycles(samples: &[u64]) -> String {
    format!(
        "p50={:.3}Mcycles p95={:.3}Mcycles p99={:.3}Mcycles",
        percentile_u64(samples, 0.50) / 1_000_000.0,
        percentile_u64(samples, 0.95) / 1_000_000.0,
        percentile_u64(samples, 0.99) / 1_000_000.0,
    )
}

#[cfg(windows)]
/// Returns a linearly interpolated percentile from non-empty byte-count samples.
fn percentile_value(samples: &[usize], probability: f64) -> f64 {
    let mut sorted = samples
        .iter()
        .map(|sample| *sample as f64)
        .collect::<Vec<_>>();
    sorted.sort_by(f64::total_cmp);
    let position = probability * (sorted.len() - 1) as f64;
    let lower_index = position.floor() as usize;
    let upper_index = position.ceil() as usize;
    let fraction = position - lower_index as f64;
    sorted[lower_index] + (sorted[upper_index] - sorted[lower_index]) * fraction
}

#[cfg(windows)]
/// Returns a linearly interpolated percentile from non-empty cycle-count samples.
fn percentile_u64(samples: &[u64], probability: f64) -> f64 {
    let mut sorted = samples
        .iter()
        .map(|sample| *sample as f64)
        .collect::<Vec<_>>();
    sorted.sort_by(f64::total_cmp);
    let position = probability * (sorted.len() - 1) as f64;
    let lower_index = position.floor() as usize;
    let upper_index = position.ceil() as usize;
    let fraction = position - lower_index as f64;
    sorted[lower_index] + (sorted[upper_index] - sorted[lower_index]) * fraction
}

/// Returns one linearly interpolated duration percentile in milliseconds.
fn percentile_milliseconds(samples: &[Duration], probability: f64) -> f64 {
    percentile_nanoseconds(samples, probability) / 1_000_000.0
}

/// Returns one linearly interpolated duration percentile in microseconds.
fn percentile_microseconds(samples: &[Duration], probability: f64) -> f64 {
    percentile_nanoseconds(samples, probability) / 1_000.0
}

/// Returns one linearly interpolated duration percentile in nanoseconds.
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

/// Creates and removes a unique temporary data root for one integration test.
struct TemporaryProfile(PathBuf);

impl TemporaryProfile {
    /// Creates an empty profile under the system temporary directory.
    fn new() -> io::Result<Self> {
        let path =
            std::env::temp_dir().join(format!("openchat-process-rpc-{}", Uuid::new_v4().simple()));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }

    /// Returns the absolute root passed to the child service.
    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TemporaryProfile {
    /// Removes test data after the service child has shut down.
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Sends a method-only request through the service's newline-delimited RPC stream.
async fn exchange(
    stdin: &mut ChildStdin,
    stdout: &mut BufReader<ChildStdout>,
    id: &str,
    method: &str,
) -> Result<Value, Box<dyn Error>> {
    let request = json!({"id": id, "method": method, "params": {}});
    exchange_request(stdin, stdout, &request).await
}

/// Serializes one request, writes one frame, and decodes its bounded-time response.
async fn exchange_request(
    stdin: &mut ChildStdin,
    stdout: &mut BufReader<ChildStdout>,
    request: &Value,
) -> Result<Value, Box<dyn Error>> {
    let mut line = serde_json::to_vec(request)?;
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
