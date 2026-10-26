//! Runs optional, locally installed language servers behind bounded read-only tool queries.

use std::path::Path;

use serde_json::Value;
#[cfg(not(windows))]
use serde_json::json;

#[cfg(not(windows))]
use crate::protocol::ServiceError;

const MAX_LOCATIONS: usize = 25;
const MAX_LSP_QUERY_BYTES: u64 = 2 * 1024 * 1024;

#[cfg(windows)]
mod windows {
    use std::{
        collections::HashMap,
        ffi::OsString,
        fs,
        hash::{Hash, Hasher},
        io,
        path::{Path, PathBuf},
        sync::{Arc, OnceLock},
        time::{Duration, Instant},
    };

    use serde_json::{Value, json};
    use sha2::{Digest, Sha256};
    use tokio::{
        fs::File,
        io::{AsyncReadExt, AsyncWriteExt, BufReader},
        sync::{Mutex, OwnedSemaphorePermit, Semaphore, watch},
        time::{Instant as TokioInstant, timeout, timeout_at},
    };

    use crate::{
        protocol::ServiceError,
        sandbox::{SandboxJobLimits, SandboxOptions, SandboxedProcess},
    };

    use super::{MAX_LOCATIONS, MAX_LSP_QUERY_BYTES};

    const MAX_ACTIVE_SESSIONS: usize = 2;
    const MAX_LSP_MESSAGE_BYTES: usize = 1024 * 1024;
    const MAX_LSP_HEADER_BYTES: usize = 8 * 1024;
    const MAX_HOVER_BYTES: usize = 8 * 1024;
    const LSP_REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
    const LSP_STARTUP_TIMEOUT: Duration = Duration::from_secs(30);
    const LSP_DIAGNOSTIC_WAIT: Duration = Duration::from_secs(2);
    const LSP_SESSION_IDLE_TTL: Duration = Duration::from_secs(5 * 60);

    #[derive(Clone, Copy)]
    struct ServerSpec {
        id: &'static str,
        program_names: &'static [&'static str],
        arguments: &'static [&'static str],
    }

    const TYPESCRIPT: ServerSpec = ServerSpec {
        id: "typescript-language-server",
        program_names: &["typescript-language-server.exe"],
        arguments: &["--stdio"],
    };
    const PYRIGHT: ServerSpec = ServerSpec {
        id: "pyright",
        program_names: &["pyright-langserver.exe"],
        arguments: &["--stdio"],
    };
    const RUST_ANALYZER: ServerSpec = ServerSpec {
        id: "rust-analyzer",
        program_names: &["rust-analyzer.exe"],
        arguments: &[],
    };
    const GOPLS: ServerSpec = ServerSpec {
        id: "gopls",
        program_names: &["gopls.exe"],
        arguments: &[],
    };
    const CLANGD: ServerSpec = ServerSpec {
        id: "clangd",
        program_names: &["clangd.exe"],
        arguments: &[],
    };
    const CSHARP_LS: ServerSpec = ServerSpec {
        id: "csharp-ls",
        program_names: &["csharp-ls.exe"],
        arguments: &["--stdio"],
    };
    const PHP_ACTOR: ServerSpec = ServerSpec {
        id: "phpactor",
        program_names: &["phpactor.exe"],
        arguments: &["language-server"],
    };
    const BASH_LANGUAGE_SERVER: ServerSpec = ServerSpec {
        id: "bash-language-server",
        program_names: &["bash-language-server.exe"],
        arguments: &["start"],
    };
    const RUBY_LSP: ServerSpec = ServerSpec {
        id: "ruby-lsp",
        program_names: &["ruby-lsp.exe"],
        arguments: &["--stdio"],
    };
    const LUA_LANGUAGE_SERVER: ServerSpec = ServerSpec {
        id: "lua-language-server",
        program_names: &["lua-language-server.exe"],
        arguments: &["--stdio"],
    };
    const SQLS: ServerSpec = ServerSpec {
        id: "sqls",
        program_names: &["sqls.exe"],
        arguments: &["-stdio"],
    };
    const DART: ServerSpec = ServerSpec {
        id: "dart",
        program_names: &["dart.exe"],
        arguments: &["language-server"],
    };
    const BIOME: ServerSpec = ServerSpec {
        id: "biome",
        program_names: &["biome.exe"],
        arguments: &["lsp-proxy"],
    };

    #[derive(Clone, Eq)]
    struct SessionKey {
        root: PathBuf,
        server_id: &'static str,
    }

    impl PartialEq for SessionKey {
        fn eq(&self, other: &Self) -> bool {
            self.root == other.root && self.server_id == other.server_id
        }
    }

    impl Hash for SessionKey {
        fn hash<H: Hasher>(&self, state: &mut H) {
            self.root.hash(state);
            self.server_id.hash(state);
        }
    }

    struct CachedSession {
        session: Arc<Mutex<LspSession>>,
        last_used: Instant,
    }

    static SESSIONS: OnceLock<Mutex<HashMap<SessionKey, CachedSession>>> = OnceLock::new();
    static SESSION_LIMITER: OnceLock<Arc<Semaphore>> = OnceLock::new();

    struct LspSession {
        process: SandboxedProcess,
        stdin: File,
        stdout: BufReader<File>,
        next_id: u64,
        root_uri: String,
        capabilities: Value,
        pushed_diagnostics: Option<Value>,
        open_document_uri: Option<String>,
        _permit: OwnedSemaphorePermit,
    }

    struct LspDocumentRequest<'a> {
        action: &'a str,
        uri: &'a str,
        language_id: &'a str,
        source: &'a str,
        line: Option<usize>,
        character: Option<usize>,
        limit: usize,
    }

    pub(super) async fn execute(
        cache_root: &Path,
        root: &Path,
        relative_path: &str,
        action: &str,
        line: Option<usize>,
        character: Option<usize>,
        limit: usize,
        cancellation: &mut watch::Receiver<bool>,
    ) -> Value {
        match query(
            cache_root,
            root,
            relative_path,
            action,
            line,
            character,
            limit,
            cancellation,
        )
        .await
        {
            Ok(value) => value,
            Err(error) if matches!(error.code, "operation_cancelled" | "request_cancelled") => {
                json!({"status":"cancelled"})
            }
            Err(error) => {
                let mut output = json!({"error": {"code": error.code, "message": error.message}});
                if let Some(details) = error.details {
                    output["error"]["details"] = json!(details);
                }
                output
            }
        }
    }

    async fn query(
        cache_root: &Path,
        root: &Path,
        relative_path: &str,
        action: &str,
        line: Option<usize>,
        character: Option<usize>,
        limit: usize,
        cancellation: &mut watch::Receiver<bool>,
    ) -> Result<Value, ServiceError> {
        let root = fs::canonicalize(root).map_err(|_| project_root_error())?;
        if !root.is_dir() {
            return Err(project_root_error());
        }
        let file = root.join(relative_path);
        let file = fs::canonicalize(file).map_err(|_| lsp_file_error())?;
        if !file.starts_with(&root) || !file.is_file() {
            return Err(lsp_file_error());
        }
        let extension = file
            .extension()
            .and_then(|value| value.to_str())
            .map(str::to_ascii_lowercase)
            .unwrap_or_default();
        let spec = server_for_extension(&root, &file, &extension, action)
            .ok_or_else(unsupported_language_error)?;
        let (program, arguments) =
            find_server_command(spec).ok_or_else(|| missing_server_error(spec.id))?;
        let relative_file = file.strip_prefix(&root).map_err(|_| lsp_file_error())?;
        let source = crate::filesystem::read_bounded_file_under_root(
            &root,
            relative_file,
            MAX_LSP_QUERY_BYTES as usize,
        )
        .map_err(|_| lsp_file_error())?;
        if source.len() as u64 > MAX_LSP_QUERY_BYTES {
            return Err(ServiceError::new(
                "lsp_file_too_large",
                "The selected source file exceeds the language server read limit.",
                false,
            ));
        }
        let source = String::from_utf8(source).map_err(|_| {
            ServiceError::new(
                "lsp_file_not_text",
                "The selected source file is not valid UTF-8 text.",
                false,
            )
        })?;
        let uri = url::Url::from_file_path(&file)
            .map_err(|_| lsp_file_error())?
            .to_string();
        let language_id = language_id(&extension).ok_or_else(unsupported_language_error)?;
        let key = SessionKey {
            root: root.clone(),
            server_id: spec.id,
        };
        let session = session_for(
            key.clone(),
            cache_root,
            &root,
            spec,
            &program,
            &arguments,
            cancellation,
        )
        .await?;
        let session_to_lock = Arc::clone(&session);
        let mut session = tokio::select! {
            guard = session_to_lock.lock_owned() => guard,
            _ = wait_for_cancellation(cancellation) => {
                return Err(crate::permissions::operation_cancelled_error());
            }
        };
        let request = LspDocumentRequest {
            action,
            uri: &uri,
            language_id,
            source: &source,
            line,
            character,
            limit,
        };
        let query_result = tokio::select! {
            result = session.execute(request) => Some(result),
            _ = wait_for_cancellation(cancellation) => None,
        };
        let result = match query_result {
            Some(result) => result,
            None => {
                session.kill();
                Err(crate::permissions::operation_cancelled_error())
            }
        };
        drop(session);
        let mut sessions = SESSIONS
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .await;
        if result.is_err() {
            sessions.remove(&key);
        } else if let Some(cached) = sessions.get_mut(&key) {
            cached.last_used = Instant::now();
        }
        result
    }

    async fn wait_for_cancellation(cancellation: &mut watch::Receiver<bool>) {
        loop {
            if *cancellation.borrow() {
                return;
            }
            if cancellation.changed().await.is_err() {
                return;
            }
        }
    }

    async fn session_for(
        key: SessionKey,
        cache_root: &Path,
        root: &Path,
        spec: ServerSpec,
        program: &Path,
        arguments: &[OsString],
        cancellation: &mut watch::Receiver<bool>,
    ) -> Result<Arc<Mutex<LspSession>>, ServiceError> {
        let sessions = SESSIONS.get_or_init(|| Mutex::new(HashMap::new()));
        {
            let mut sessions = sessions.lock().await;
            sessions.retain(|_, session| session.last_used.elapsed() < LSP_SESSION_IDLE_TTL);
            if let Some(session) = sessions.get_mut(&key) {
                session.last_used = Instant::now();
                return Ok(Arc::clone(&session.session));
            }
            evict_oldest_session_if_full(&mut sessions);
        }

        let permit = tokio::select! {
            result = Arc::clone(SESSION_LIMITER.get_or_init(|| {
                Arc::new(Semaphore::new(MAX_ACTIVE_SESSIONS))
            })).acquire_owned() => result.map_err(|_| lsp_start_error(spec.id))?,
            _ = wait_for_cancellation(cancellation) => {
                return Err(crate::permissions::operation_cancelled_error());
            }
        };
        let mut sessions = sessions.lock().await;
        sessions.retain(|_, session| session.last_used.elapsed() < LSP_SESSION_IDLE_TTL);
        if let Some(session) = sessions.get_mut(&key) {
            session.last_used = Instant::now();
            return Ok(Arc::clone(&session.session));
        }
        evict_oldest_session_if_full(&mut sessions);
        let session = Arc::new(Mutex::new(LspSession::spawn(
            cache_root, root, spec, program, arguments, permit,
        )?));
        sessions.insert(
            key.clone(),
            CachedSession {
                session: Arc::clone(&session),
                last_used: Instant::now(),
            },
        );
        tokio::spawn(expire_session_when_idle(key));
        Ok(session)
    }

    async fn expire_session_when_idle(key: SessionKey) {
        loop {
            let remaining = {
                let sessions = SESSIONS.get_or_init(|| Mutex::new(HashMap::new()));
                let sessions = sessions.lock().await;
                let Some(session) = sessions.get(&key) else {
                    return;
                };
                LSP_SESSION_IDLE_TTL.saturating_sub(session.last_used.elapsed())
            };
            if remaining.is_zero() {
                let sessions = SESSIONS.get_or_init(|| Mutex::new(HashMap::new()));
                let mut sessions = sessions.lock().await;
                if sessions
                    .get(&key)
                    .is_some_and(|session| session.last_used.elapsed() >= LSP_SESSION_IDLE_TTL)
                {
                    sessions.remove(&key);
                    return;
                }
                continue;
            }
            tokio::time::sleep(remaining).await;
        }
    }

    fn evict_oldest_session_if_full(sessions: &mut HashMap<SessionKey, CachedSession>) {
        if sessions.len() >= MAX_ACTIVE_SESSIONS
            && let Some(oldest_key) = sessions
                .iter()
                .min_by_key(|(_, session)| session.last_used)
                .map(|(key, _)| key.clone())
        {
            sessions.remove(&oldest_key);
        }
    }

    fn server_for_extension(
        root: &Path,
        file: &Path,
        extension: &str,
        action: &str,
    ) -> Option<ServerSpec> {
        Some(match extension {
            "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" | "mts" | "cts"
                if action == "diagnostics" && has_biome_config(root, file) =>
            {
                BIOME
            }
            "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" | "mts" | "cts" => TYPESCRIPT,
            "py" | "pyi" => PYRIGHT,
            "rs" => RUST_ANALYZER,
            "go" => GOPLS,
            "c" | "h" | "cc" | "cpp" | "cxx" | "hh" | "hpp" | "hxx" => CLANGD,
            "cs" => CSHARP_LS,
            "php" => PHP_ACTOR,
            "bash" | "sh" | "zsh" => BASH_LANGUAGE_SERVER,
            "rb" | "rake" => RUBY_LSP,
            "dart" => DART,
            "lua" => LUA_LANGUAGE_SERVER,
            "sql" => SQLS,
            _ => return None,
        })
    }

    fn has_biome_config(root: &Path, file: &Path) -> bool {
        let mut current = file.parent();
        while let Some(directory) = current.filter(|directory| directory.starts_with(root)) {
            if ["biome.json", "biome.jsonc"].iter().any(|name| {
                fs::symlink_metadata(directory.join(name))
                    .is_ok_and(|metadata| metadata.is_file() && !metadata.file_type().is_symlink())
            }) {
                return true;
            }
            if directory == root {
                break;
            }
            current = directory.parent();
        }
        false
    }

    fn language_id(extension: &str) -> Option<&'static str> {
        Some(match extension {
            "js" | "mjs" | "cjs" => "javascript",
            "jsx" => "javascriptreact",
            "ts" | "mts" | "cts" => "typescript",
            "tsx" => "typescriptreact",
            "py" => "python",
            "pyi" => "python",
            "rs" => "rust",
            "go" => "go",
            "c" | "h" => "c",
            "cc" | "cpp" | "cxx" | "hh" | "hpp" | "hxx" => "cpp",
            "cs" => "csharp",
            "php" => "php",
            "bash" | "sh" | "zsh" => "shellscript",
            "rb" | "rake" => "ruby",
            "lua" => "lua",
            "sql" => "sql",
            "dart" => "dart",
            _ => return None,
        })
    }

    fn find_server_command(spec: ServerSpec) -> Option<(PathBuf, Vec<OsString>)> {
        let path = std::env::var_os("PATH")?;
        for directory in std::env::split_paths(&path) {
            for program_name in spec.program_names {
                let candidate = directory.join(program_name);
                if candidate.is_file()
                    && let Ok(canonical) = fs::canonicalize(candidate)
                {
                    return Some((
                        canonical,
                        spec.arguments.iter().map(OsString::from).collect(),
                    ));
                }
            }
        }
        if spec.id == DART.id
            && let Some(program) = find_dart_sdk_program(&path)
        {
            return Some((program, spec.arguments.iter().map(OsString::from).collect()));
        }
        let (package, entry) = npm_server_entry(spec.id)?;
        let node = find_program_in_path(&path, &["node.exe"])?;
        let script = find_npm_entry(&path, package, entry)?;
        let mut arguments = vec![script.into_os_string()];
        arguments.extend(spec.arguments.iter().map(OsString::from));
        Some((node, arguments))
    }

    fn find_program_in_path(path: &std::ffi::OsStr, names: &[&str]) -> Option<PathBuf> {
        for directory in std::env::split_paths(path) {
            for name in names {
                let candidate = directory.join(name);
                if candidate.is_file()
                    && let Ok(canonical) = fs::canonicalize(candidate)
                {
                    return Some(canonical);
                }
            }
        }
        None
    }

    fn find_dart_sdk_program(path: &std::ffi::OsStr) -> Option<PathBuf> {
        for directory in std::env::split_paths(path) {
            let launcher = directory.join("dart.bat");
            let candidate = directory.join("cache/dart-sdk/bin/dart.exe");
            if launcher.is_file()
                && candidate.is_file()
                && let Ok(candidate) = fs::canonicalize(candidate)
            {
                return Some(candidate);
            }
        }
        None
    }

    fn npm_server_entry(server_id: &str) -> Option<(&'static str, &'static str)> {
        match server_id {
            "typescript-language-server" => Some(("typescript-language-server", "lib/cli.mjs")),
            "pyright" => Some(("pyright", "langserver.index.js")),
            "bash-language-server" => Some(("bash-language-server", "out/cli.js")),
            "biome" => Some(("@biomejs/biome", "bin/biome")),
            _ => None,
        }
    }

    fn find_npm_entry(path: &std::ffi::OsStr, package: &str, entry: &str) -> Option<PathBuf> {
        let entry = Path::new(entry);
        for directory in std::env::split_paths(path) {
            let candidate = directory.join("node_modules").join(package).join(entry);
            if candidate.is_file() {
                return fs::canonicalize(candidate).ok();
            }
        }
        None
    }

    impl LspSession {
        fn spawn(
            cache_root: &Path,
            root: &Path,
            spec: ServerSpec,
            program: &Path,
            arguments: &[OsString],
            permit: OwnedSemaphorePermit,
        ) -> Result<Self, ServiceError> {
            let cache_directory = lsp_cache_directory(cache_root, root, spec.id);
            fs::create_dir_all(&cache_directory).map_err(|_| lsp_start_error(spec.id))?;
            let mut read_only_paths = vec![root.to_path_buf()];
            if let Some(program_directory) = program.parent() {
                read_only_paths.push(program_directory.to_path_buf());
            }
            if spec.id == DART.id
                && let Some(sdk_directory) = program.parent().and_then(Path::parent)
            {
                read_only_paths.push(sdk_directory.to_path_buf());
            }
            if let Some(script) = arguments
                .first()
                .map(|argument| PathBuf::from(argument.as_os_str()))
                && script.is_file()
                && let Some(package_directory) = script.parent().and_then(Path::parent)
            {
                read_only_paths.push(package_directory.to_path_buf());
                if let Some(dependency_directory) = package_directory.parent()
                    && dependency_directory
                        .file_name()
                        .is_some_and(|name| name == "node_modules")
                {
                    read_only_paths.push(dependency_directory.to_path_buf());
                }
            }
            let options = SandboxOptions {
                additional_read_only_paths: &read_only_paths,
                job_limits: Some(SandboxJobLimits {
                    active_process_limit: 32,
                    process_memory_limit_bytes: 1024 * 1024 * 1024,
                    job_memory_limit_bytes: 2 * 1024 * 1024 * 1024,
                }),
                ..SandboxOptions::default()
            };
            let mut process_arguments = arguments.to_vec();
            if spec.id == DART.id {
                let mut cache_argument = OsString::from("--cache=");
                cache_argument.push(cache_directory.as_os_str());
                process_arguments.push(cache_argument);
            }
            let mut process = SandboxedProcess::spawn_program_with_options(
                program,
                &process_arguments,
                &cache_directory,
                &[],
                options,
            )
            .map_err(|error| {
                lsp_start_error(spec.id).with_details(format!(
                    "Sandbox I/O kind: {:?}; operating-system code: {:?}.",
                    error.kind(),
                    error.raw_os_error()
                ))
            })?;
            let stdin = process
                .take_stdin()
                .ok_or_else(|| lsp_start_error(spec.id))?;
            let stdout = process
                .take_stdout()
                .ok_or_else(|| lsp_start_error(spec.id))?;
            if let Some(mut stderr) = process.take_stderr() {
                tokio::spawn(async move {
                    let mut buffer = [0_u8; 4096];
                    loop {
                        match stderr.read(&mut buffer).await {
                            Ok(0) | Err(_) => break,
                            Ok(_) => {}
                        }
                    }
                });
            }
            let root_uri = url::Url::from_directory_path(root)
                .map_err(|_| lsp_start_error(spec.id))?
                .to_string();
            Ok(Self {
                process,
                stdin,
                stdout: BufReader::new(stdout),
                next_id: 0,
                root_uri,
                capabilities: Value::Null,
                pushed_diagnostics: None,
                open_document_uri: None,
                _permit: permit,
            })
        }

        async fn execute(
            &mut self,
            request: LspDocumentRequest<'_>,
        ) -> Result<Value, ServiceError> {
            let LspDocumentRequest {
                action,
                uri,
                language_id,
                source,
                line,
                character,
                limit,
            } = request;
            if self.capabilities.is_null() {
                timeout(LSP_STARTUP_TIMEOUT, self.initialize())
                    .await
                    .map_err(|_| lsp_timeout_error())??;
            }
            self.notify(
                "textDocument/didOpen",
                json!({"textDocument":{"uri":uri,"languageId":language_id,"version":1,"text":source}}),
            )
            .await?;
            self.open_document_uri = Some(uri.to_owned());

            let result = async {
                match action {
                    "definition" | "references" | "hover" => {
                        let line = line.unwrap_or(1).saturating_sub(1);
                        let character = character.unwrap_or(1).saturating_sub(1);
                        let method = match action {
                            "definition" => "textDocument/definition",
                            "references" => "textDocument/references",
                            _ => "textDocument/hover",
                        };
                        let mut params = json!({
                            "textDocument":{"uri":uri},
                            "position":{"line":line,"character":character}
                        });
                        if action == "references" {
                            params["context"] = json!({"includeDeclaration":true});
                        }
                        let result = timeout(LSP_REQUEST_TIMEOUT, self.request(method, params))
                            .await
                            .map_err(|_| lsp_timeout_error())??;
                        if action == "hover" {
                            normalize_hover(result)
                        } else {
                            normalize_locations(result, &self.root_uri, limit)
                        }
                    }
                    "diagnostics" => self.diagnostics(uri, limit).await,
                    _ => Err(unsupported_action_error()),
                }
            }
            .await;
            let close_result = self
                .notify("textDocument/didClose", json!({"textDocument":{"uri":uri}}))
                .await
                .map_err(|_| lsp_protocol_error());
            self.open_document_uri = None;
            self.pushed_diagnostics = None;
            match (result, close_result) {
                (Err(error), _) => Err(error),
                (Ok(_), Err(error)) => Err(error),
                (Ok(value), Ok(())) => Ok(value),
            }
        }

        async fn initialize(&mut self) -> Result<(), ServiceError> {
            let response = self
                .request(
                    "initialize",
                    json!({
                        "processId": Value::Null,
                        "clientInfo":{"name":"OpenChat","version":env!("CARGO_PKG_VERSION")},
                        "rootUri":self.root_uri,
                        "workspaceFolders":[{"uri":self.root_uri,"name":"project"}],
                        "capabilities":{
                            "workspace":{"configuration":true,"workspaceFolders":true},
                            "general":{"positionEncodings":["utf-16"]},
                            "textDocument":{
                                "definition":{"dynamicRegistration":false,"linkSupport":true},
                                "references":{"dynamicRegistration":false},
                                "hover":{"dynamicRegistration":false,"contentFormat":["markdown","plaintext"]},
                                "publishDiagnostics":{"relatedInformation":false,"versionSupport":false}
                            }
                        },
                        "trace":"off"
                    }),
                )
                .await?;
            self.capabilities = response.get("capabilities").cloned().unwrap_or(Value::Null);
            self.notify("initialized", json!({})).await?;
            Ok(())
        }

        async fn diagnostics(&mut self, uri: &str, limit: usize) -> Result<Value, ServiceError> {
            let supports_pull = self
                .capabilities
                .pointer("/textDocument/diagnosticProvider")
                .is_some_and(|value| !value.is_null() && value != &Value::Bool(false));
            let diagnostics = if supports_pull {
                let result = timeout(
                    LSP_REQUEST_TIMEOUT,
                    self.request(
                        "textDocument/diagnostic",
                        json!({"textDocument":{"uri":uri}}),
                    ),
                )
                .await
                .map_err(|_| lsp_timeout_error())??;
                result
                    .get("items")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default()
            } else {
                self.pushed_diagnostics = None;
                let deadline = TokioInstant::now() + LSP_DIAGNOSTIC_WAIT;
                loop {
                    if let Some(value) = self.pushed_diagnostics.take() {
                        break value.as_array().cloned().unwrap_or_default();
                    }
                    match timeout_at(deadline, self.read_message()).await {
                        Ok(Ok(message)) => self.handle_incoming(message).await?,
                        Ok(Err(error)) => return Err(lsp_protocol_io_error(&error)),
                        Err(_) => return Err(diagnostics_unavailable_error()),
                    }
                }
            };
            let items = diagnostics
                .into_iter()
                .take(limit.min(MAX_LOCATIONS))
                .map(|diagnostic| {
                    json!({
                        "line": diagnostic.pointer("/range/start/line").and_then(Value::as_u64).unwrap_or(0) + 1,
                        "character": diagnostic.pointer("/range/start/character").and_then(Value::as_u64).unwrap_or(0) + 1,
                        "severity": diagnostic.get("severity").and_then(Value::as_u64),
                        "code": diagnostic.get("code"),
                        "message": diagnostic.get("message").and_then(Value::as_str).unwrap_or_default().chars().take(2048).collect::<String>(),
                        "source": diagnostic.get("source").and_then(Value::as_str)
                    })
                })
                .collect::<Vec<_>>();
            let count = items.len();
            Ok(json!({"diagnostics":items,"count":count}))
        }

        async fn request(&mut self, method: &str, params: Value) -> Result<Value, ServiceError> {
            self.next_id = self.next_id.saturating_add(1);
            let request_id = self.next_id;
            self.write_message(
                &json!({"jsonrpc":"2.0","id":request_id,"method":method,"params":params}),
            )
            .await
            .map_err(|error| lsp_protocol_io_error(&error))?;
            loop {
                let message = self
                    .read_message()
                    .await
                    .map_err(|error| lsp_protocol_io_error(&error))?;
                if message.get("id") == Some(&json!(request_id)) {
                    if let Some(error) = message.get("error") {
                        let code = error
                            .get("code")
                            .and_then(Value::as_i64)
                            .unwrap_or_default();
                        let message = error
                            .get("message")
                            .and_then(Value::as_str)
                            .unwrap_or_default();
                        return Err(ServiceError::new(
                            "lsp_request_failed",
                            "The language server could not complete the requested code query.",
                            true,
                        )
                        .with_details(format!(
                            "LSP error {code}: {}",
                            message.chars().take(512).collect::<String>()
                        )));
                    }
                    return Ok(message.get("result").cloned().unwrap_or(Value::Null));
                }
                self.handle_incoming(message).await?;
            }
        }

        async fn notify(&mut self, method: &str, params: Value) -> Result<(), ServiceError> {
            self.write_message(&json!({"jsonrpc":"2.0","method":method,"params":params}))
                .await
                .map_err(|error| lsp_protocol_io_error(&error))
        }

        async fn handle_incoming(&mut self, message: Value) -> Result<(), ServiceError> {
            if message.get("method").and_then(Value::as_str)
                == Some("textDocument/publishDiagnostics")
                && let (Some(uri), Some(diagnostics)) = (
                    message.pointer("/params/uri").and_then(Value::as_str),
                    message.pointer("/params/diagnostics"),
                )
                && self.open_document_uri.as_deref() == Some(uri)
            {
                self.pushed_diagnostics = Some(diagnostics.clone());
            }
            if let (Some(method), Some(id)) = (
                message.get("method").and_then(Value::as_str),
                message.get("id"),
            ) {
                let result = match method {
                    "workspace/configuration" => message
                        .pointer("/params/items")
                        .and_then(Value::as_array)
                        .map(|items| Value::Array(vec![Value::Null; items.len()]))
                        .unwrap_or_else(|| json!([])),
                    "workspace/workspaceFolders" => json!([{"uri":self.root_uri,"name":"project"}]),
                    "workspace/applyEdit" => {
                        json!({"applied":false,"failureReason":"OpenChat code-intelligence tools do not apply workspace edits."})
                    }
                    _ => Value::Null,
                };
                self.write_message(&json!({"jsonrpc":"2.0","id":id,"result":result}))
                    .await
                    .map_err(|error| lsp_protocol_io_error(&error))?;
            }
            Ok(())
        }

        async fn read_message(&mut self) -> io::Result<Value> {
            let mut header = Vec::with_capacity(128);
            while !header.ends_with(b"\r\n\r\n") {
                if header.len() >= MAX_LSP_HEADER_BYTES {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "LSP header is too large.",
                    ));
                }
                header.push(self.stdout.read_u8().await?);
            }
            let header = std::str::from_utf8(&header)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            let content_length = header
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.trim()
                        .eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .filter(|length| *length <= MAX_LSP_MESSAGE_BYTES)
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "LSP message length is invalid.")
                })?;
            let mut body = vec![0; content_length];
            self.stdout.read_exact(&mut body).await?;
            serde_json::from_slice(&body)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
        }

        async fn write_message(&mut self, value: &Value) -> io::Result<()> {
            let body = serde_json::to_vec(value)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            if body.len() > MAX_LSP_MESSAGE_BYTES {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "LSP message is too large.",
                ));
            }
            self.stdin
                .write_all(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes())
                .await?;
            self.stdin.write_all(&body).await?;
            self.stdin.flush().await
        }

        fn kill(&mut self) {
            if self.process.start_kill().is_err() {
                eprintln!("lsp_process_termination_failed");
            }
        }
    }

    fn normalize_locations(
        result: Value,
        root_uri: &str,
        limit: usize,
    ) -> Result<Value, ServiceError> {
        let locations = match result {
            Value::Array(locations) => locations,
            Value::Null => Vec::new(),
            location => vec![location],
        };
        let root = url::Url::parse(root_uri)
            .ok()
            .and_then(|url| url.to_file_path().ok())
            .ok_or_else(lsp_protocol_error)?;
        let locations = locations
            .into_iter()
            .take(limit.min(MAX_LOCATIONS))
            .filter_map(|location| normalize_location(location, &root))
            .collect::<Vec<_>>();
        let count = locations.len();
        Ok(json!({"locations":locations,"count":count}))
    }

    fn normalize_location(location: Value, root: &Path) -> Option<Value> {
        let uri = location
            .get("uri")
            .or_else(|| location.get("targetUri"))
            .and_then(Value::as_str)?;
        let file = url::Url::parse(uri).ok()?.to_file_path().ok()?;
        if !file.starts_with(root) {
            return None;
        }
        let file = fs::canonicalize(file).ok()?;
        if !file.starts_with(root) {
            return None;
        }
        let range = location
            .get("range")
            .or_else(|| location.get("targetSelectionRange"))
            .or_else(|| location.get("targetRange"))?;
        let start = range.get("start")?;
        let end = range.get("end")?;
        Some(json!({
            "path":file.to_string_lossy(),
            "line":start.get("line").and_then(Value::as_u64).unwrap_or(0) + 1,
            "character":start.get("character").and_then(Value::as_u64).unwrap_or(0) + 1,
            "endLine":end.get("line").and_then(Value::as_u64).unwrap_or(0) + 1,
            "endCharacter":end.get("character").and_then(Value::as_u64).unwrap_or(0) + 1
        }))
    }

    fn normalize_hover(result: Value) -> Result<Value, ServiceError> {
        if result.is_null() {
            return Ok(json!({"contents":""}));
        }
        let contents = result.get("contents").cloned().unwrap_or(Value::Null);
        let text = match contents {
            Value::String(text) => text,
            Value::Object(object) => object
                .get("value")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            Value::Array(items) => items
                .iter()
                .filter_map(|item| match item {
                    Value::String(text) => Some(text.as_str()),
                    Value::Object(object) => object.get("value").and_then(Value::as_str),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n\n"),
            _ => String::new(),
        };
        Ok(json!({"contents":text.chars().take(MAX_HOVER_BYTES).collect::<String>()}))
    }

    fn lsp_cache_directory(cache_root: &Path, root: &Path, server_id: &str) -> PathBuf {
        let mut hasher = Sha256::new();
        hasher.update(root.to_string_lossy().as_bytes());
        hasher.update(server_id.as_bytes());
        let digest = hasher.finalize();
        let key = digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        cache_root.join("lsp").join(key)
    }

    fn project_root_error() -> ServiceError {
        ServiceError::new(
            "project_unavailable",
            "The selected project folder is unavailable to the language server.",
            false,
        )
    }

    fn lsp_file_error() -> ServiceError {
        ServiceError::new(
            "lsp_file_unavailable",
            "The requested source file is unavailable inside the project.",
            false,
        )
    }

    fn unsupported_language_error() -> ServiceError {
        ServiceError::new(
            "lsp_language_unsupported",
            "No configured language server supports the requested file and action.",
            false,
        )
    }

    fn missing_server_error(server_id: &str) -> ServiceError {
        ServiceError::new(
            "lsp_server_not_found",
            format!("The {server_id} language server executable was not found on PATH."),
            false,
        )
    }

    fn lsp_start_error(server_id: &str) -> ServiceError {
        ServiceError::new(
            "lsp_server_start_failed",
            format!("The {server_id} language server could not start in the local sandbox."),
            true,
        )
    }

    fn lsp_protocol_error() -> ServiceError {
        ServiceError::new(
            "lsp_protocol_failed",
            "The language server returned an invalid or incomplete protocol response.",
            true,
        )
    }

    fn lsp_protocol_io_error(error: &io::Error) -> ServiceError {
        lsp_protocol_error().with_details(format!(
            "LSP transport I/O kind: {:?}; operating-system code: {:?}.",
            error.kind(),
            error.raw_os_error()
        ))
    }

    fn lsp_timeout_error() -> ServiceError {
        ServiceError::new(
            "lsp_timeout",
            "The language server did not respond before the query timed out.",
            true,
        )
    }

    fn diagnostics_unavailable_error() -> ServiceError {
        ServiceError::new(
            "lsp_diagnostics_unavailable",
            "The selected language server did not provide diagnostics for this file.",
            true,
        )
    }

    fn unsupported_action_error() -> ServiceError {
        ServiceError::new(
            "lsp_action_unsupported",
            "The requested language server action is not supported.",
            false,
        )
    }

    #[cfg(test)]
    mod tests {
        use std::{
            fs,
            path::{Path, PathBuf},
        };

        use super::{find_dart_sdk_program, find_npm_entry, npm_server_entry};

        struct TemporaryProject {
            root: std::path::PathBuf,
            cache_root: std::path::PathBuf,
            directory: std::path::PathBuf,
        }

        impl TemporaryProject {
            fn new() -> Self {
                let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("target")
                    .join(format!("openchat-lsp-integration-{}", uuid::Uuid::new_v4()));
                let root = directory.join("project");
                let cache_root = directory.join("app-data");
                fs::create_dir_all(&root).expect("create temporary LSP project");
                fs::create_dir_all(&cache_root).expect("create temporary LSP cache root");
                Self {
                    root,
                    cache_root,
                    directory,
                }
            }
        }

        impl Drop for TemporaryProject {
            fn drop(&mut self) {
                fs::remove_dir_all(&self.directory).expect("remove temporary LSP project");
            }
        }

        async fn stop_cached_session(root: &Path, server_id: &'static str) {
            let key = super::SessionKey {
                root: root.to_path_buf(),
                server_id,
            };
            super::SESSIONS
                .get_or_init(|| tokio::sync::Mutex::new(std::collections::HashMap::new()))
                .lock()
                .await
                .remove(&key);
        }

        #[test]
        fn npm_backed_servers_resolve_their_package_entry_points() {
            assert_eq!(
                npm_server_entry("typescript-language-server"),
                Some(("typescript-language-server", "lib/cli.mjs"))
            );
            assert_eq!(
                npm_server_entry("pyright"),
                Some(("pyright", "langserver.index.js"))
            );
            assert_eq!(
                npm_server_entry("bash-language-server"),
                Some(("bash-language-server", "out/cli.js"))
            );
            assert_eq!(
                npm_server_entry("biome"),
                Some(("@biomejs/biome", "bin/biome"))
            );
            assert_eq!(npm_server_entry("gopls"), None);
        }

        #[test]
        fn npm_entry_lookup_finds_an_install_under_a_path_entry() {
            let root =
                std::env::temp_dir().join(format!("openchat-lsp-test-{}", uuid::Uuid::new_v4()));
            let npm_directory = root.join("npm");
            let package_entry = npm_directory
                .join("node_modules")
                .join("typescript-language-server")
                .join("lib")
                .join("cli.mjs");
            fs::create_dir_all(package_entry.parent().expect("entry parent"))
                .expect("create package fixture");
            fs::write(&package_entry, "").expect("create package entry");

            let entry = find_npm_entry(
                npm_directory.as_os_str(),
                "typescript-language-server",
                "lib/cli.mjs",
            );

            assert_eq!(entry, package_entry.canonicalize().ok());
            fs::remove_dir_all(root).expect("remove package fixture");
        }

        #[test]
        fn flutter_path_resolves_its_bundled_dart_language_server_binary() {
            let root =
                std::env::temp_dir().join(format!("openchat-dart-test-{}", uuid::Uuid::new_v4()));
            let bin_directory = root.join("flutter/bin");
            let dart_binary = bin_directory.join("cache/dart-sdk/bin/dart.exe");
            fs::create_dir_all(dart_binary.parent().expect("Dart binary parent"))
                .expect("create Flutter Dart SDK fixture");
            fs::write(bin_directory.join("dart.bat"), "@echo off\n")
                .expect("create Flutter Dart launcher");
            fs::write(&dart_binary, "fixture").expect("create Dart executable fixture");

            let resolved = find_dart_sdk_program(bin_directory.as_os_str());

            assert_eq!(resolved, dart_binary.canonicalize().ok());
            fs::remove_dir_all(root).expect("remove Flutter Dart SDK fixture");
        }

        #[test]
        fn biome_diagnostics_respect_project_configuration() {
            let root =
                std::env::temp_dir().join(format!("openchat-lsp-test-{}", uuid::Uuid::new_v4()));
            let nested = root.join("packages/app/src");
            fs::create_dir_all(&nested).expect("create project fixture");
            let file = nested.join("index.ts");

            assert_eq!(
                super::server_for_extension(&root, &file, "ts", "diagnostics").map(|spec| spec.id),
                Some("typescript-language-server")
            );
            fs::write(root.join("biome.json"), "{}\n").expect("write Biome config");
            assert_eq!(
                super::server_for_extension(&root, &file, "ts", "diagnostics").map(|spec| spec.id),
                Some("biome")
            );
            assert_eq!(
                super::server_for_extension(&root, &file, "ts", "hover").map(|spec| spec.id),
                Some("typescript-language-server")
            );

            fs::remove_dir_all(root).expect("remove project fixture");
        }

        #[test]
        fn popular_source_extensions_select_a_server_and_language_id() {
            let root =
                std::env::temp_dir().join(format!("openchat-lsp-test-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&root).expect("create project fixture");
            let cases = [
                ("ts", "typescript-language-server"),
                ("py", "pyright"),
                ("rs", "rust-analyzer"),
                ("go", "gopls"),
                ("c", "clangd"),
                ("cs", "csharp-ls"),
                ("php", "phpactor"),
                ("sh", "bash-language-server"),
                ("rb", "ruby-lsp"),
                ("lua", "lua-language-server"),
                ("sql", "sqls"),
                ("dart", "dart"),
            ];

            for (extension, server_id) in cases {
                let path = root.join(format!("source.{extension}"));
                assert_eq!(
                    super::server_for_extension(&root, &path, extension, "hover")
                        .map(|server| server.id),
                    Some(server_id)
                );
                assert!(super::language_id(extension).is_some());
            }
            assert!(
                super::server_for_extension(&root, &root.join("source.txt"), "txt", "hover")
                    .is_none()
            );
            fs::remove_dir_all(root).expect("remove project fixture");
        }

        #[tokio::test]
        #[ignore = "manual integration test requires Rust Analyzer on PATH"]
        async fn installed_rust_analyzer_resolves_a_project_definition() {
            use tokio::sync::watch;

            let project = TemporaryProject::new();
            fs::create_dir_all(project.root.join("src")).expect("create source directory");
            fs::write(
                project.root.join("Cargo.toml"),
                "[package]\nname = \"openchat_lsp_fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
            )
            .expect("write fixture manifest");
            fs::write(project.root.join("src/lib.rs"), "pub fn target() {}\n")
                .expect("write definition");
            fs::write(
                project.root.join("src/main.rs"),
                "use openchat_lsp_fixture::target;\nfn main() { target(); }\n",
            )
            .expect("write reference");
            let root = fs::canonicalize(&project.root).expect("canonicalize fixture root");
            let (_cancel, mut cancellation) = watch::channel(false);

            let result = super::query(
                &project.cache_root,
                &root,
                "src/main.rs",
                "definition",
                Some(2),
                Some(13),
                10,
                &mut cancellation,
            )
            .await;
            stop_cached_session(&root, super::RUST_ANALYZER.id).await;
            let result = result.expect("Rust Analyzer should resolve the reference");
            let expected = fs::canonicalize(root.join("src/lib.rs"))
                .expect("canonicalize definition file")
                .to_string_lossy()
                .into_owned();
            assert!(
                result["locations"]
                    .as_array()
                    .is_some_and(|locations| locations
                        .iter()
                        .any(|location| { location["path"].as_str() == Some(expected.as_str()) })),
                "Rust Analyzer returned {result}"
            );
        }

        #[tokio::test]
        #[ignore = "manual integration test requires Dart SDK on PATH"]
        async fn installed_dart_language_server_answers_hover() {
            use tokio::sync::watch;

            let project = TemporaryProject::new();
            fs::write(
                project.root.join("main.dart"),
                "void main() { print('hello'); }\n",
            )
            .expect("write Dart source");
            let root = fs::canonicalize(&project.root).expect("canonicalize fixture root");
            let (_cancel, mut cancellation) = watch::channel(false);

            let result = super::query(
                &project.cache_root,
                &root,
                "main.dart",
                "hover",
                Some(1),
                Some(6),
                10,
                &mut cancellation,
            )
            .await;
            stop_cached_session(&root, super::DART.id).await;
            let result = result.expect("Dart language server should answer hover");
            assert!(
                result["contents"]
                    .as_str()
                    .is_some_and(|contents| contents.contains("main")),
                "Dart language server returned {result}"
            );
        }
    }
}

#[cfg(windows)]
pub(crate) async fn execute_lsp_query(
    cache_root: &Path,
    root: &Path,
    relative_path: &str,
    action: &str,
    line: Option<usize>,
    character: Option<usize>,
    limit: usize,
    cancellation: &mut tokio::sync::watch::Receiver<bool>,
) -> Value {
    windows::execute(
        cache_root,
        root,
        relative_path,
        action,
        line,
        character,
        limit,
        cancellation,
    )
    .await
}

#[cfg(not(windows))]
pub(crate) async fn execute_lsp_query(
    _cache_root: &Path,
    _root: &Path,
    _relative_path: &str,
    _action: &str,
    _line: Option<usize>,
    _character: Option<usize>,
    _limit: usize,
    _cancellation: &mut tokio::sync::watch::Receiver<bool>,
) -> Value {
    let error = ServiceError::new(
        "lsp_platform_unsupported",
        "Sandboxed language server tools are not available on this platform.",
        false,
    );
    json!({"error":{"code":error.code,"message":error.message}})
}
