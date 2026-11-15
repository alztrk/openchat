use std::{fs, path::PathBuf};

use uuid::Uuid;

use super::sessions::exllama_max_seq_len;
use super::{
    Engine, RuntimeMarker, SGLANG_CUDA_HEADER_PATCH_SCRIPT, SGLANG_INSTALL_VERIFICATION_SCRIPT,
    SGLANG_LAUNCHER_SCRIPT, SGLANG_RELEASE, SGLANG_RUNTIME_REQUIREMENTS, engine_json, install,
    installed_engine, installed_release_for_catalog, installed_runtime_paths, parse_runtime_marker,
    python_minor_version, runtime_child_path, runtime_paths_sync, uninstall,
    valid_managed_wsl_root, valid_release_tag, windows_python_launcher_from, write_marker,
};
#[cfg(windows)]
use super::{decode_wsl_output, parse_default_distro, valid_wsl_home};
use crate::{protocol::EventSink, storage::AppStorage};

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("openchat-python-engine-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&path).expect("test directory should be created");
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn identifies_supported_python_engines_and_variants() {
    let exllama = Engine::parse("exllama").expect("ExLlama should be supported");
    let sglang = Engine::parse("sglang").expect("SGLang should be supported");

    assert_eq!(exllama.id(), "exllama");
    assert_eq!(sglang.id(), "sglang");
    assert_eq!(exllama.variant(true), "windows-cu13".to_owned());
    assert_eq!(sglang.variant(true), "windows-wsl2-cuda130".to_owned());
    assert_eq!(
        Engine::parse("onnx")
            .expect_err("unknown engines must be rejected")
            .code,
        "local_engine_not_found"
    );
}

#[test]
fn exllama_context_prefers_model_then_engine_then_model_config() {
    assert_eq!(exllama_max_seq_len(Some(16_384), Some(8_192)), 16_384);
    assert_eq!(exllama_max_seq_len(None, Some(8_192)), 8_192);
    assert_eq!(exllama_max_seq_len(None, None), -1);
}

#[test]
fn finds_python_launcher_from_either_windows_directory_variable() {
    let temporary = TestDirectory::new();
    let system_root = temporary.0.join("system-root");
    let windows_directory = temporary.0.join("windows-directory");
    fs::create_dir_all(&system_root).expect("system root should be created");
    fs::create_dir_all(&windows_directory).expect("Windows directory should be created");
    let windir_launcher = windows_directory.join("py.exe");
    fs::write(&windir_launcher, b"launcher").expect("launcher fixture should be written");

    assert_eq!(
        windows_python_launcher_from(
            Some(system_root.as_os_str()),
            Some(windows_directory.as_os_str()),
            std::ffi::OsStr::new("py.exe"),
        ),
        windir_launcher.into_os_string()
    );
}

#[test]
fn managed_sglang_runtime_installs_and_uses_its_cuda_compiler() {
    assert!(SGLANG_RUNTIME_REQUIREMENTS.contains(&"sglang==0.5.23"));
    assert!(SGLANG_RUNTIME_REQUIREMENTS.contains(&"cuda-toolkit[nvcc,nvvm,cccl]==13.0.3"));
    assert!(
        SGLANG_CUDA_HEADER_PATCH_SCRIPT.contains("libc_name != \"glibc\" or version < (2, 42)")
    );
    assert!(SGLANG_CUDA_HEADER_PATCH_SCRIPT.contains("noexcept\\(true\\)"));
    assert!(SGLANG_CUDA_HEADER_PATCH_SCRIPT.contains("if len(matches) != 1:"));
    assert!(SGLANG_LAUNCHER_SCRIPT.contains("site.getsitepackages()"));
    assert!(SGLANG_LAUNCHER_SCRIPT.contains("cuda_libraries = os.path.join(cuda_home, \"lib64\")"));
    assert!(SGLANG_LAUNCHER_SCRIPT.contains("os.environ[\"CUDA_HOME\"] = cuda_home"));
    assert!(
        SGLANG_LAUNCHER_SCRIPT.contains("for variable in (\"LIBRARY_PATH\", \"LD_LIBRARY_PATH\"):")
    );
    assert!(SGLANG_LAUNCHER_SCRIPT.contains("/usr/lib/wsl/lib"));
    assert!(SGLANG_LAUNCHER_SCRIPT.contains("if __name__ == \"__main__\":"));
    assert!(SGLANG_INSTALL_VERIFICATION_SCRIPT.contains("torch.cuda.is_available()"));
}

#[test]
fn wsl_runtime_release_probe_reads_the_serialized_marker_field() {
    let marker = RuntimeMarker {
        engine_id: Engine::Sglang.id().to_owned(),
        release_tag: SGLANG_RELEASE.to_owned(),
        variant_id: Engine::Sglang.variant(true),
    };
    let marker_json = serde_json::to_vec(&marker).expect("runtime marker should serialize");
    let parsed = parse_runtime_marker(&marker_json, Engine::Sglang)
        .expect("matching runtime marker should be accepted");
    assert_eq!(parsed.release_tag, SGLANG_RELEASE);
    assert!(parse_runtime_marker(&marker_json, Engine::Exllama).is_none());
    assert!(parse_runtime_marker(&vec![b' '; 2049], Engine::Sglang).is_none());
}

#[test]
fn reads_supported_python_minor_versions_from_version_output() {
    assert_eq!(python_minor_version(b"Python 3.12.8\n"), Some(12));
    assert_eq!(python_minor_version(b"Python 2.7.18\n"), None);
    assert_eq!(python_minor_version(b"Python development build"), None);
}

#[cfg(windows)]
#[test]
fn parses_default_wsl_distribution_and_utf16_output() {
    let listing = "  NAME              STATE           VERSION\n* Ubuntu-24.04      Stopped         2\n  Debian            Stopped         2\n";
    assert_eq!(
        parse_default_distro(listing),
        Some(("Ubuntu-24.04".to_owned(), "2".to_owned()))
    );

    let utf16 = "* Ubuntu-24.04 Stopped 2\n"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    assert_eq!(decode_wsl_output(&utf16), "* Ubuntu-24.04 Stopped 2\n");
    assert_eq!(parse_default_distro("Ubuntu-24.04 Stopped 2\n"), None);
}

#[test]
fn restricts_managed_wsl_paths_to_the_engine_runtime_root() {
    assert!(valid_managed_wsl_root(
        "/home/alican/.local/share/openchat/runtimes/sglang/v0.5.23",
        Engine::Sglang,
        "v0.5.23"
    ));
    assert!(!valid_managed_wsl_root(
        "/home/alican/.local/share/openchat/runtimes/exllama/v0.5.23",
        Engine::Sglang,
        "v0.5.23"
    ));
    assert!(!valid_managed_wsl_root(
        "/home/alican/.local/share/openchat/runtimes/sglang/../outside",
        Engine::Sglang,
        "../outside"
    ));
}

#[cfg(windows)]
#[test]
fn accepts_only_safe_wsl_home_paths() {
    assert!(valid_wsl_home("/home/alican"));
    assert!(valid_wsl_home("/root"));
    assert!(!valid_wsl_home("/mnt/c/Users/alican"));
    assert!(!valid_wsl_home("/home/../tmp"));
}

#[test]
fn accepts_safe_release_tags_for_managed_runtime_updates() {
    assert!(valid_release_tag("v0.5.23"));
    assert!(valid_release_tag("tabbyapi-4728158-exllamav3-1.6.7"));
    assert!(!valid_release_tag("../outside"));
    assert!(!valid_release_tag("v0.5.23/other"));
    assert!(!valid_release_tag(""));
}

#[cfg(any(windows, target_os = "linux"))]
#[tokio::test]
async fn discovers_previous_native_runtime_release_as_an_update() {
    let directory = TestDirectory::new();
    let storage = AppStorage::open_at(directory.0.clone()).expect("test storage should open");
    let engine = Engine::Exllama;
    let current = runtime_paths_sync(&storage, engine)
        .expect("runtime paths should resolve")
        .expect("native ExLlama paths should exist");
    let previous_root = current
        .root
        .parent()
        .expect("runtime version should have a platform parent")
        .join("previous-release");
    fs::create_dir_all(&previous_root).expect("previous runtime should be created");
    let marker = RuntimeMarker {
        engine_id: engine.id().to_owned(),
        release_tag: "previous-release".to_owned(),
        variant_id: engine.variant(cfg!(windows)),
    };
    fs::write(
        previous_root.join(super::MARKER_NAME),
        serde_json::to_vec(&marker).expect("marker should serialize"),
    )
    .expect("previous release marker should be written");

    let installed = installed_runtime_paths(&storage, engine)
        .await
        .expect("managed runtimes should be discoverable");
    assert_eq!(installed.len(), 1);
    assert_eq!(
        installed_release_for_catalog(&storage, engine)
            .await
            .expect("installed release should be readable")
            .as_deref(),
        Some("previous-release")
    );
}

#[cfg(all(target_arch = "x86_64", any(windows, target_os = "linux")))]
#[tokio::test]
async fn install_rejects_an_unavailable_variant_without_creating_runtime_files() {
    let directory = TestDirectory::new();
    let storage = AppStorage::open_at(directory.0.clone()).expect("test storage should open");
    let (_cancel, mut cancellation) = tokio::sync::watch::channel(false);

    let error = install(
        &storage,
        "exllama",
        "unsupported-variant",
        &serde_json::json!("request-1"),
        &EventSink::new(),
        &mut cancellation,
    )
    .await
    .expect_err("an unlisted runtime package must be rejected");

    assert_eq!(error.code, "local_engine_install_unavailable");
    assert!(!storage.root().join("runtimes/exllama").exists());
}

#[cfg(all(target_arch = "x86_64", any(windows, target_os = "linux")))]
#[tokio::test]
async fn cancelled_install_stops_before_runtime_discovery_or_creation() {
    let directory = TestDirectory::new();
    let storage = AppStorage::open_at(directory.0.clone()).expect("test storage should open");
    let (cancel, mut cancellation) = tokio::sync::watch::channel(false);
    cancel
        .send(true)
        .expect("cancellation receiver should be active");

    let error = install(
        &storage,
        "exllama",
        &Engine::Exllama.variant(cfg!(windows)),
        &serde_json::json!("request-2"),
        &EventSink::new(),
        &mut cancellation,
    )
    .await
    .expect_err("a cancelled installation must stop before preflight work");

    assert_eq!(error.code, "operation_cancelled");
    assert!(!storage.root().join("runtimes/exllama").exists());
}

#[cfg(all(target_arch = "x86_64", any(windows, target_os = "linux")))]
#[tokio::test]
async fn successful_runtime_marker_registers_the_installed_engine() {
    let directory = TestDirectory::new();
    let storage = AppStorage::open_at(directory.0.clone()).expect("test storage should open");
    let engine = Engine::Exllama;
    let paths = runtime_paths_sync(&storage, engine)
        .expect("runtime paths should resolve")
        .expect("native ExLlama paths should exist");
    fs::create_dir_all(&paths.root).expect("managed runtime directory should be created");
    let (_cancel, mut cancellation) = tokio::sync::watch::channel(false);

    write_marker(
        &paths,
        engine,
        &engine.variant(cfg!(windows)),
        &mut cancellation,
    )
    .await
    .expect("successful installation should write its runtime marker");

    let installed = installed_engine(&storage, engine.id())
        .expect("installed runtime should be discoverable")
        .expect("managed runtime marker should register the engine");
    assert_eq!(installed.engine_id, engine.id());
    assert_eq!(installed.variant_id, engine.variant(cfg!(windows)));
    assert_eq!(installed.release_tag, engine.release());
    assert_eq!(installed.root, paths.root);
}

#[cfg(all(target_arch = "x86_64", any(windows, target_os = "linux")))]
#[tokio::test]
async fn catalog_marks_python_engines_incompatible_without_a_cuda_gpu() {
    let directory = TestDirectory::new();
    let storage = AppStorage::open_at(directory.0.clone()).expect("test storage should open");

    let catalog = engine_json(&storage, "exllama", "/models/exllama", "stopped", false)
        .await
        .expect("engine catalog should load without a CUDA GPU");

    assert_eq!(catalog["engineId"], "exllama");
    assert_eq!(catalog["variants"][0]["status"], "gpu_unavailable");
    assert_eq!(catalog["variants"][0]["canInstall"], false);
    assert_eq!(catalog["modelDirectory"], "/models/exllama");
}

#[cfg(all(target_arch = "x86_64", any(windows, target_os = "linux")))]
#[tokio::test]
async fn uninstall_removes_only_the_managed_runtime_and_preserves_downloaded_models() {
    let directory = TestDirectory::new();
    let storage = AppStorage::open_at(directory.0.clone()).expect("test storage should open");
    let engine = Engine::Exllama;
    let paths = runtime_paths_sync(&storage, engine)
        .expect("runtime paths should resolve")
        .expect("native ExLlama paths should exist");
    fs::create_dir_all(&paths.root).expect("managed runtime directory should be created");
    let marker = RuntimeMarker {
        engine_id: engine.id().to_owned(),
        release_tag: engine.release().to_owned(),
        variant_id: engine.variant(cfg!(windows)),
    };
    fs::write(
        &paths.marker,
        serde_json::to_vec(&marker).expect("runtime marker should serialize"),
    )
    .expect("runtime marker should be written");

    let model_directory = storage.root().join("models/exllama");
    fs::create_dir_all(&model_directory).expect("model directory should exist");
    let model_file = model_directory.join("model.safetensors");
    fs::write(&model_file, b"model data").expect("downloaded model fixture should be written");

    let result = uninstall(&storage, engine.id())
        .await
        .expect("verified managed runtime should uninstall");

    assert_eq!(result["removed"], true);
    assert!(!paths.root.exists());
    assert_eq!(
        fs::read(&model_file).expect("downloaded model should remain"),
        b"model data"
    );
}

#[test]
fn joins_native_and_wsl_paths_without_changing_separators() {
    assert_eq!(
        runtime_child_path(
            std::path::Path::new("/opt/openchat/runtime"),
            "models",
            true
        ),
        std::path::PathBuf::from("/opt/openchat/runtime/models")
    );
    assert_eq!(
        runtime_child_path(std::path::Path::new("C:/OpenChat/runtime"), "models", false),
        std::path::PathBuf::from("C:/OpenChat/runtime/models")
    );
}
