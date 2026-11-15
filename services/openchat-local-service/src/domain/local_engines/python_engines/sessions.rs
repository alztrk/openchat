use super::*;

use crate::local_engines::settings::LocalEngineSettings;

pub(crate) async fn launch_context(
    storage: &AppStorage,
    engine_id: &str,
) -> Result<RuntimePaths, ServiceError> {
    let engine = Engine::parse(engine_id)?;
    if let Some(paths) = runtime_paths_sync(storage, engine)? {
        if cached_release(&paths, engine).as_deref() != Some(engine.release()) {
            return Err(engine_unavailable_error());
        }
        return Ok(paths);
    }
    if engine == Engine::Sglang && cfg!(windows) {
        #[cfg(windows)]
        {
            let paths = runtime_paths(storage, engine).await?;
            if cached_release(&paths, engine).as_deref() != Some(engine.release()) {
                return Err(engine_unavailable_error());
            }
            return Ok(paths);
        }
    }
    Err(engine_unavailable_error())
}

pub(crate) async fn model_path_for_runtime(
    paths: &RuntimePaths,
    path: &Path,
) -> Result<String, ServiceError> {
    let path = path.to_str().ok_or_else(model_unavailable_error)?;
    if paths.wsl {
        let output = capture_wsl(paths.distro.as_deref(), &["wslpath", "-a", path]).await?;
        return String::from_utf8(output)
            .map(|value| value.trim().to_owned())
            .map_err(|_| model_unavailable_error());
    }
    Ok(path.to_owned())
}

pub(super) fn exllama_max_seq_len(
    model_context_length: Option<u32>,
    engine_default: Option<u32>,
) -> i64 {
    model_context_length
        .or(engine_default)
        .map_or(-1, i64::from)
}

pub(crate) async fn create_session(
    paths: &RuntimePaths,
    engine: Engine,
    model: &crate::local_engines::models::RegisteredModel,
    port: u16,
    settings: &LocalEngineSettings,
) -> Result<RuntimeSession, ServiceError> {
    let session = runtime_child_path(
        &paths.root,
        &format!(".sessions/{}", uuid::Uuid::new_v4()),
        paths.wsl,
    );
    if paths.wsl {
        let session_parent = session.parent().ok_or_else(storage_error)?;
        let mut cleanup_cancellation = watch::channel(false).1;
        create_wsl_directory(paths, session_parent, true, &mut cleanup_cancellation).await?;
        create_wsl_directory(paths, &session, false, &mut cleanup_cancellation).await?;
    } else {
        fs::create_dir_all(&session).map_err(|_| storage_error())?;
    }
    let result = async {
        let api_key = Zeroizing::new(uuid::Uuid::new_v4().simple().to_string());
        let auth_file = runtime_child_path(
            &session,
            if engine == Engine::Exllama {
                "api_tokens.yml"
            } else {
                ".api-key"
            },
            paths.wsl,
        );
        let auth_contents = if engine == Engine::Exllama {
            let admin_key = Zeroizing::new(uuid::Uuid::new_v4().simple().to_string());
            format!(
                "api_key: {}\nadmin_key: {}\n",
                api_key.as_str(),
                admin_key.as_str()
            )
        } else {
            format!("{}\n", api_key.as_str())
        };
        write_private_file(paths, &auth_file, auth_contents.as_bytes()).await?;

        let model_parent = model.path.parent().ok_or_else(model_unavailable_error)?;
        let model_name = model
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(model_unavailable_error)?;
        let model_directory = model_path_for_runtime(paths, model_parent).await?;
        let runner = if engine == Engine::Sglang {
            let runner = runtime_child_path(&session, "launch_sglang.py", paths.wsl);
            write_private_file(paths, &runner, SGLANG_LAUNCHER_SCRIPT.as_bytes()).await?;
            runner
        } else {
            runtime_child_path(&paths.root, "tabbyAPI/main.py", paths.wsl)
        };
        if engine == Engine::Exllama {
            let escaped_model_dir =
                serde_json::to_string(&model_directory).map_err(|_| storage_error())?;
            let escaped_model_name =
                serde_json::to_string(model_name).map_err(|_| storage_error())?;
            let cache_mode = settings.exllama_cache_mode.as_deref().unwrap_or("fp16");
            let max_seq_len =
                exllama_max_seq_len(model.context_length, settings.exllama_max_seq_len);
            let config_path = runtime_child_path(&session, "config.yml", paths.wsl);
            let contents = format!(
                "network:\n  host: 127.0.0.1\n  port: {port}\n  disable_auth: false\n  allowed_origins: []\n  web_ui: false\nlogging:\n  log_prompt: false\n  log_generation_params: false\n  log_requests: false\n  log_chat_completion_requests: false\nmodel:\n  model_dir: {escaped_model_dir}\n  model_name: {escaped_model_name}\n  max_seq_len: {max_seq_len}\n  cache_mode: {cache_mode}\n  max_batch_size: 1\n"
            );
            write_private_file(paths, &config_path, contents.as_bytes()).await?;
        }
        let pid_file = paths
            .wsl
            .then(|| runtime_child_path(&session, ".pid", true));
        Ok(RuntimeSession {
            directory: session.clone(),
            auth_file,
            remote_pid_file: pid_file,
            runner,
            api_key,
        })
    }
    .await;
    if result.is_err() {
        remove_session_directory(paths, &session).await?;
    }
    result
}

pub(crate) async fn remove_session(
    storage: &AppStorage,
    engine_id: &str,
    auth_file: &Path,
) -> Result<(), ServiceError> {
    let paths = launch_context(storage, engine_id).await?;
    let session = auth_file.parent().ok_or_else(installation_invalid_error)?;
    remove_session_directory(&paths, session).await
}

async fn remove_session_directory(
    paths: &RuntimePaths,
    session: &Path,
) -> Result<(), ServiceError> {
    let session_root = runtime_child_path(&paths.root, ".sessions", paths.wsl);
    if !session.starts_with(&session_root)
        || session.parent() != Some(session_root.as_path())
        || !session
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.len() == 36)
    {
        return Err(installation_invalid_error());
    }
    if paths.wsl {
        remove_wsl_directory(paths, session).await
    } else {
        let metadata = fs::symlink_metadata(session).map_err(|_| storage_error())?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(installation_invalid_error());
        }
        fs::remove_dir_all(session).map_err(|_| storage_error())
    }
}

pub(crate) async fn stop_wsl_process(
    storage: &AppStorage,
    engine_id: &str,
    pid_file: &Path,
    runner: &Path,
) -> Result<(), ServiceError> {
    let paths = launch_context(storage, engine_id).await?;
    if !paths.wsl || !pid_file.starts_with(&paths.root) || !runner.starts_with(&paths.root) {
        return Err(installation_invalid_error());
    }
    let runner_string = runner.to_str().ok_or_else(storage_error)?;
    run_python_script(
        &paths,
        "python3",
        MANAGED_PROCESS_STOP_SCRIPT,
        &[
            path_arg(pid_file)?,
            runner_string.to_owned(),
            "10".to_owned(),
        ],
        &mut watch::channel(false).1,
    )
    .await
}
