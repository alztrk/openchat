use std::{net::SocketAddr, process::Stdio, sync::OnceLock, time::Duration};

use reqwest::Client;
use serde_json::{Value, json};
use tokio::{
    net::TcpListener,
    process::{Child, Command},
    sync::{Mutex, watch},
    time::{Instant, sleep},
};

use crate::{protocol::ServiceError, storage::AppStorage};

use super::{
    installer::{InstalledEngine, installed_engine},
    manifests, models,
};

const STARTUP_TIMEOUT: Duration = Duration::from_secs(300);
const HEALTH_POLL_INTERVAL: Duration = Duration::from_millis(500);

static RUNNING_SERVER: OnceLock<Mutex<Option<RunningServer>>> = OnceLock::new();
static HEALTH_CLIENT: OnceLock<Result<Client, reqwest::Error>> = OnceLock::new();

struct RunningServer {
    engine_id: String,
    model_id: String,
    port: u16,
    child: Child,
}

pub(crate) async fn start_model(
    storage: &AppStorage,
    model_id: &str,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<Value, ServiceError> {
    if *cancellation.borrow() {
        return Err(cancelled_error());
    }
    let model = models::find(storage, model_id)?.ok_or_else(model_unavailable_error)?;
    if model.engine_id != "llama_cpp" || model.path_kind != "file" || !model.path.is_file() {
        return Err(model_unavailable_error());
    }
    let installed = installed_llama_cpp(storage)?.ok_or_else(engine_not_installed_error)?;
    let state = running_state();
    let mut state = state.lock().await;

    if let Some(server) = state.as_mut() {
        let is_running = server
            .child
            .try_wait()
            .map_err(|_| runtime_error())?
            .is_none();
        if is_running
            && server.engine_id == model.engine_id
            && server.model_id == model.id
            && health_check_or_cancel(server.port, cancellation).await?
        {
            return Ok(status_json(Some(server), "running"));
        }
    }
    if *cancellation.borrow() {
        return Err(cancelled_error());
    }
    if state.is_some() {
        stop_server(storage, &mut state).await?;
    }

    let port = available_loopback_port().await?;
    let started_at = Instant::now();
    let child = match Command::new(&installed.entrypoint)
        .arg("--model")
        .arg(&model.path)
        .arg("--host")
        .arg("127.0.0.1")
        .arg("--port")
        .arg(port.to_string())
        .arg("--alias")
        .arg(&model.id)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
    {
        Ok(child) => child,
        Err(_) => {
            log_runtime_event(
                storage,
                "runtime_start_failed",
                Some("local_engine_runtime_unavailable"),
                Some(started_at.elapsed()),
            );
            return Err(runtime_error());
        }
    };
    *state = Some(RunningServer {
        engine_id: model.engine_id.clone(),
        model_id: model.id.clone(),
        port,
        child,
    });

    loop {
        let port = {
            let Some(server) = state.as_mut() else {
                return Err(runtime_error());
            };
            if let Some(exit_status) = server.child.try_wait().map_err(|_| runtime_error())? {
                let _ = exit_status;
                *state = None;
                log_runtime_event(
                    storage,
                    "runtime_start_failed",
                    Some("local_engine_start_failed"),
                    Some(started_at.elapsed()),
                );
                return Err(runtime_start_failed_error());
            }
            server.port
        };
        let ready = match health_check_or_cancel(port, cancellation).await {
            Ok(ready) => ready,
            Err(error) => {
                stop_server(storage, &mut state).await?;
                let event = if error.code == "operation_cancelled" {
                    "runtime_start_cancelled"
                } else {
                    "runtime_start_failed"
                };
                log_runtime_event(storage, event, Some(error.code), Some(started_at.elapsed()));
                return Err(error);
            }
        };
        if ready {
            log_runtime_event(storage, "runtime_started", None, Some(started_at.elapsed()));
            return Ok(status_json(state.as_ref(), "running"));
        }
        if started_at.elapsed() >= STARTUP_TIMEOUT {
            stop_server(storage, &mut state).await?;
            log_runtime_event(
                storage,
                "runtime_start_timeout",
                Some("local_engine_start_timeout"),
                Some(started_at.elapsed()),
            );
            return Err(runtime_start_timeout_error());
        }

        tokio::select! {
            changed = cancellation.changed() => {
                if changed.is_err() || *cancellation.borrow() {
                    stop_server(storage, &mut state).await?;
                    log_runtime_event(
                        storage,
                        "runtime_start_cancelled",
                        Some("operation_cancelled"),
                        Some(started_at.elapsed()),
                    );
                    return Err(cancelled_error());
                }
            }
            _ = sleep(HEALTH_POLL_INTERVAL) => {}
        }
    }
}

pub(crate) async fn stop(storage: &AppStorage) -> Result<Value, ServiceError> {
    let mut state = running_state().lock().await;
    stop_server(storage, &mut state).await?;
    Ok(status_json(None, "stopped"))
}

pub(crate) async fn status(storage: &AppStorage) -> Result<Value, ServiceError> {
    let mut state = running_state().lock().await;
    let Some(server) = state.as_mut() else {
        return Ok(status_json(None, "stopped"));
    };
    if server
        .child
        .try_wait()
        .map_err(|_| runtime_error())?
        .is_some()
    {
        *state = None;
        log_runtime_event(
            storage,
            "runtime_exited",
            Some("local_engine_runtime_unavailable"),
            None,
        );
        return Ok(status_json(None, "stopped"));
    }
    let status = if health_check(server.port).await {
        "running"
    } else {
        "unhealthy"
    };
    Ok(status_json(state.as_ref(), status))
}

pub(crate) async fn chat_url(
    storage: &AppStorage,
    model_id: &str,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<String, ServiceError> {
    let status = start_model(storage, model_id, cancellation).await?;
    let port = status
        .get("port")
        .and_then(Value::as_u64)
        .and_then(|value| u16::try_from(value).ok())
        .ok_or_else(runtime_error)?;
    Ok(format!("http://127.0.0.1:{port}/v1/chat/completions"))
}

pub(crate) fn is_model_available(
    storage: &AppStorage,
    model: &models::RegisteredModel,
) -> Result<bool, ServiceError> {
    if model.engine_id != "llama_cpp" || model.path_kind != "file" || !model.path.is_file() {
        return Ok(false);
    }
    Ok(installed_llama_cpp(storage)?.is_some())
}

async fn health_check(port: u16) -> bool {
    let client = HEALTH_CLIENT.get_or_init(|| {
        Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(2))
            .build()
    });
    let Ok(client) = client else {
        return false;
    };
    client
        .get(format!("http://127.0.0.1:{port}/health"))
        .send()
        .await
        .is_ok_and(|response| response.status().is_success())
}

async fn health_check_or_cancel(
    port: u16,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<bool, ServiceError> {
    if *cancellation.borrow() || cancellation.has_changed().is_err() {
        return Err(cancelled_error());
    }
    tokio::select! {
        changed = cancellation.changed() => {
            if changed.is_err() || *cancellation.borrow() {
                Err(cancelled_error())
            } else {
                Ok(false)
            }
        }
        ready = health_check(port) => Ok(ready),
    }
}

async fn available_loopback_port() -> Result<u16, ServiceError> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|_| runtime_error())?;
    let address: SocketAddr = listener.local_addr().map_err(|_| runtime_error())?;
    Ok(address.port())
}

fn installed_llama_cpp(storage: &AppStorage) -> Result<Option<InstalledEngine>, ServiceError> {
    let manifest = manifests()?
        .into_iter()
        .find(|manifest| manifest.engine_id == "llama_cpp")
        .ok_or_else(engine_not_installed_error)?;
    let variants = manifest.variants.iter().filter(|variant| {
        variant.os == std::env::consts::OS && variant.architecture == std::env::consts::ARCH
    });
    for variant in variants {
        if let Some(engine) = installed_engine(storage, &manifest, variant)? {
            return Ok(Some(engine));
        }
    }
    Ok(None)
}

fn running_state() -> &'static Mutex<Option<RunningServer>> {
    RUNNING_SERVER.get_or_init(|| Mutex::new(None))
}

async fn stop_server(
    storage: &AppStorage,
    state: &mut Option<RunningServer>,
) -> Result<(), ServiceError> {
    let Some(mut server) = state.take() else {
        return Ok(());
    };
    if server
        .child
        .try_wait()
        .map_err(|_| runtime_error())?
        .is_none()
    {
        if server.child.kill().await.is_err() || server.child.wait().await.is_err() {
            log_runtime_event(
                storage,
                "runtime_stop_failed",
                Some("local_engine_runtime_unavailable"),
                None,
            );
            return Err(runtime_error());
        }
    }
    log_runtime_event(storage, "runtime_stopped", None, None);
    Ok(())
}

fn log_runtime_event(
    storage: &AppStorage,
    event: &'static str,
    code: Option<&'static str>,
    duration: Option<Duration>,
) {
    if storage
        .log_local_engine_event(event, code, duration)
        .is_err()
    {
        eprintln!("local_engine_diagnostic_write_failed");
    }
}

fn status_json(server: Option<&RunningServer>, status: &str) -> Value {
    json!({
        "status": status,
        "engineId": server.map(|server| server.engine_id.as_str()),
        "modelId": server.map(|server| server.model_id.as_str()),
        "port": server.map(|server| server.port),
    })
}

fn engine_not_installed_error() -> ServiceError {
    ServiceError::new(
        "local_engine_not_installed",
        "The selected local inference engine is not installed.",
        false,
    )
}

fn model_unavailable_error() -> ServiceError {
    ServiceError::new(
        "local_model_unavailable",
        "The selected local model file is unavailable or unsupported.",
        false,
    )
}

fn runtime_start_failed_error() -> ServiceError {
    ServiceError::new(
        "local_engine_start_failed",
        "The local inference engine exited before its health check passed.",
        true,
    )
}

fn runtime_start_timeout_error() -> ServiceError {
    ServiceError::new(
        "local_engine_start_timeout",
        "The local inference engine did not become ready before the startup limit.",
        true,
    )
}

fn runtime_error() -> ServiceError {
    ServiceError::new(
        "local_engine_runtime_unavailable",
        "The local inference engine could not be controlled.",
        true,
    )
}

fn cancelled_error() -> ServiceError {
    ServiceError::new(
        "operation_cancelled",
        "Starting the local model was cancelled.",
        false,
    )
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use tokio::{
        net::TcpListener,
        sync::watch,
        time::{Duration, timeout},
    };
    use uuid::Uuid;

    use crate::storage::AppStorage;

    use super::{health_check_or_cancel, start_model};

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .join(format!("openchat-local-runtime-test-{}", Uuid::new_v4()));
            fs::create_dir_all(&path).expect("test directory should be created");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[tokio::test]
    async fn honors_cancellation_before_looking_up_or_starting_a_model() {
        let temporary = TestDirectory::new();
        let storage = AppStorage::open_at(temporary.0.clone()).expect("storage should open");
        let (_sender, mut cancellation) = watch::channel(true);

        let error = start_model(&storage, "missing-model", &mut cancellation)
            .await
            .expect_err("a cancelled start should not continue");

        assert_eq!(error.code, "operation_cancelled");
    }

    #[tokio::test]
    async fn readiness_wait_is_interrupted_by_cancel_or_operation_shutdown() {
        let (sender, mut cancellation) = watch::channel(false);
        sender
            .send(true)
            .expect("the active readiness request should receive cancellation");
        let error = health_check_or_cancel(0, &mut cancellation)
            .await
            .expect_err("a cancelled readiness wait should stop");
        assert_eq!(error.code, "operation_cancelled");

        let (sender, mut cancellation) = watch::channel(false);
        drop(sender);
        let error = health_check_or_cancel(0, &mut cancellation)
            .await
            .expect_err("a closed operation should stop its readiness wait");
        assert_eq!(error.code, "operation_cancelled");
    }

    #[tokio::test]
    async fn cancellation_interrupts_an_in_flight_health_request() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("the test health endpoint should bind");
        let port = listener
            .local_addr()
            .expect("the test health endpoint should have an address")
            .port();
        let (sender, mut cancellation) = watch::channel(false);
        let readiness =
            tokio::spawn(async move { health_check_or_cancel(port, &mut cancellation).await });
        let (_connection, _) = listener
            .accept()
            .await
            .expect("the health request should reach the test endpoint");

        sender
            .send(true)
            .expect("the active readiness request should receive cancellation");
        let result = timeout(Duration::from_millis(250), readiness)
            .await
            .expect("cancellation should interrupt the pending health request")
            .expect("the readiness task should finish");
        let error = result.expect_err("a cancelled readiness wait should stop");

        assert_eq!(error.code, "operation_cancelled");
    }
}
