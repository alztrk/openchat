//! Tauri host entry point and registration of frontend-accessible native commands.
#![deny(missing_docs)]
#![deny(clippy::undocumented_unsafe_blocks)]
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[cfg(all(feature = "desktop-profile-test-support", not(debug_assertions)))]
compile_error!("desktop profile test support is unavailable in optimized builds");

use tauri::Manager;
#[cfg(windows)]
use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings3;
#[cfg(windows)]
use windows_core::Interface;

#[cfg(feature = "startup-benchmark")]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(feature = "startup-benchmark")]
struct StartupBenchmarkState {
    started_at: std::time::Instant,
    result_path: std::path::PathBuf,
    reported: AtomicBool,
}

#[cfg(feature = "startup-benchmark")]
macro_rules! app_invoke_handler {
    ($($command:path),* $(,)?) => {
        tauri::generate_handler![$($command,)* startup_benchmark_ready]
    };
}

#[cfg(not(feature = "startup-benchmark"))]
macro_rules! app_invoke_handler {
    ($($command:path),* $(,)?) => {
        tauri::generate_handler![$($command),*]
    };
}

mod app_preferences;
mod archive_export;
mod chat_storage;
mod computer_use_overlay;
mod file_ops;
mod local_data;
mod local_service;
mod mcp_probe;
mod mcp_storage;
mod notifications;
mod project_folder;
mod provider_storage;
mod request_bounds;
mod save_dialog;
#[cfg_attr(test, allow(dead_code))]
mod window_broadcast;
mod window_state;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "startup-benchmark")]
    let startup_started_at = std::time::Instant::now();
    #[cfg(feature = "startup-benchmark")]
    let benchmark_profile_root = Some(benchmark_profile_root_from_args()?);
    #[cfg(not(feature = "startup-benchmark"))]
    let benchmark_profile_root: Option<std::path::PathBuf> = None;
    #[cfg(feature = "desktop-profile-test-support")]
    let desktop_test_profile_root = Some(desktop_test_profile_root_from_args()?);
    #[cfg(not(feature = "desktop-profile-test-support"))]
    let desktop_test_profile_root: Option<std::path::PathBuf> = None;

    #[cfg(all(
        feature = "startup-benchmark",
        feature = "desktop-profile-test-support"
    ))]
    let setup_profile_root: Option<std::path::PathBuf> = {
        return Err(std::io::Error::other(
            "Startup benchmarking and desktop profile testing must run separately",
        )
        .into());
    };
    #[cfg(not(all(
        feature = "startup-benchmark",
        feature = "desktop-profile-test-support"
    )))]
    let setup_profile_root = desktop_test_profile_root
        .as_ref()
        .or(benchmark_profile_root.as_ref())
        .cloned();
    let local_service_state = setup_profile_root
        .as_ref()
        .map(|root| local_service::LocalServiceState::with_storage_root(root.join("storage")))
        .unwrap_or_default();

    let builder = tauri::Builder::default();
    #[cfg(feature = "startup-benchmark")]
    let builder = {
        let result_path = benchmark_profile_root
            .as_ref()
            .map(|root| root.join("result.json"))
            .ok_or_else(|| std::io::Error::other("The benchmark profile is unavailable"))?;
        builder.manage(StartupBenchmarkState {
            started_at: startup_started_at,
            result_path,
            reported: AtomicBool::new(false),
        })
    };

    let app = builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .manage(local_service_state)
        .manage(app_preferences::AppPreferencesState::default())
        .invoke_handler(app_invoke_handler![
            app_preferences::set_close_window_behavior,
            app_preferences::set_windows_startup_enabled,
            archive_export::save_pending_archive_export,
            chat_storage::get_chat_schema_version,
            computer_use_overlay::set_computer_use_overlay,
            project_folder::select_project_folder,
            project_folder::discard_project_folder_selection,
            project_folder::consume_project_folder_selection_command,
            local_data::export_support_diagnostics,
            notifications::read_windows_notification_status,
            notifications::open_windows_notification_settings,
            notifications::send_test_notification,
            notifications::send_assistant_response_notification,
            notifications::send_user_question_notification,
            notifications::take_pending_notification_activations,
            notifications::set_notification_frontend_unready,
            chat_storage::save_conversation_markdown,
            chat_storage::save_conversation_export,
            chat_storage::record_chat_request_failure,
            chat_storage::record_connection_removal_diagnostic,
            chat_storage::record_context_usage_failure,
            mcp_storage::load_global_mcp_servers,
            mcp_storage::save_global_mcp_server,
            mcp_storage::set_global_mcp_enabled,
            mcp_storage::remove_global_mcp_server,
            mcp_storage::get_global_mcp_secret_status,
            mcp_storage::save_global_mcp_secrets,
            mcp_probe::probe_global_mcp_server,
            local_service::start_local_service,
            local_service::start_local_service_with_empty_storage,
            local_service::write_local_service,
            local_service::respond_local_service_permission,
            provider_storage::load_provider_credential_status,
            provider_storage::save_chatgpt_api_key,
            provider_storage::save_compatible_provider_api_key,
            provider_storage::save_opencode_api_key,
            provider_storage::delete_chatgpt_api_key,
            provider_storage::delete_compatible_provider_api_key,
            provider_storage::delete_opencode_api_key,
            provider_storage::save_custom_provider,
            provider_storage::delete_custom_provider,
            window_broadcast::broadcast_window_sync_event,
        ])
        .setup(move |app| {
            if let Err(error) = local_service::start_local_service_early(app.handle()) {
                eprintln!(
                    "OpenChat local service early startup failed; the renderer will retry: {error}"
                );
            }
            let main_window_config = app
                .config()
                .app
                .windows
                .iter()
                .find(|window| window.label == "main")
                .ok_or_else(|| std::io::Error::other("Main window configuration is unavailable"))?;
            let mut main_window_builder =
                tauri::WebviewWindowBuilder::from_config(app.handle(), main_window_config)?
                    .visible(false);
            #[cfg(feature = "desktop-profile-test-support")]
            if desktop_test_profile_root.is_some() {
                main_window_builder = main_window_builder.visible(true);
            }
            #[cfg(feature = "startup-benchmark")]
            {
                main_window_builder = main_window_builder.initialization_script(
                    r#"(() => {
                      let windowVisible = false;
                      let reportReactReady = () => {};
                      window.addEventListener('openchat-startup-window-visible', () => {
                        windowVisible = true;
                        reportReactReady();
                      }, { once: true });
                      const observeReactRoot = () => {
                        const root = document.getElementById('root');
                        if (!root) return;
                        const initialChildren = new Set(root.childNodes);
                        let reactRootReady = false;
                        let observer;
                        const reportReady = () => {
                          if (!reactRootReady || !windowVisible) return;
                          observer.disconnect();
                          const navigation = performance.getEntriesByType('navigation')[0];
                          const resources = performance.getEntriesByType('resource')
                            .filter((entry) => entry.initiatorType === 'script' || entry.initiatorType === 'css' || /\.(m?js|css)(?:$|\?)/.test(new URL(entry.name).pathname))
                            .map((entry) => ({
                              file: new URL(entry.name).pathname.split('/').pop(),
                              initiatorType: entry.initiatorType,
                              startTimeMs: Math.round(entry.startTime * 10) / 10,
                              responseEndMs: Math.round(entry.responseEnd * 10) / 10,
                              durationMs: Math.round(entry.duration * 10) / 10,
                              transferBytes: entry.transferSize,
                            }))
                            .sort((left, right) => right.responseEndMs - left.responseEndMs)
                            .slice(0, 20);
                          const performanceSnapshot = {
                            renderedRootClass: String(root.firstElementChild?.className ?? ''),
                            renderedRootText: (root.textContent ?? '').slice(0, 600),
                            navigation: navigation ? {
                              domInteractiveMs: navigation.domInteractive,
                              domContentLoadedMs: navigation.domContentLoadedEventEnd,
                              loadEventEndMs: navigation.loadEventEnd,
                            } : null,
                            resources,
                          };
                          void window.__TAURI_INTERNALS__.invoke('startup_benchmark_ready', { performanceSnapshot }).catch((error) => {
                            globalThis.reportError(error);
                          });
                        };
                        reportReactReady = reportReady;
                        const hasReactChild = () =>
                          Array.from(root.childNodes).some((node) => !initialChildren.has(node));
                        observer = new MutationObserver(() => {
                          if (hasReactChild()) {
                            reactRootReady = true;
                            reportReady();
                          }
                        });
                        observer.observe(root, { childList: true });
                        if (hasReactChild()) {
                          reactRootReady = true;
                          reportReady();
                        }
                      };
                      if (document.readyState === 'loading') {
                        document.addEventListener('DOMContentLoaded', observeReactRoot, { once: true });
                      } else {
                        observeReactRoot();
                      }
                    })();"#,
                );
            }
            if let Some(profile_root) = setup_profile_root.as_ref() {
                let webview_data_directory =
                    ensure_profile_child_directory(profile_root, "webview")?;
                main_window_builder = main_window_builder.data_directory(webview_data_directory);
            }
            let main_window = main_window_builder.build()?;
            #[cfg(windows)]
            let app_handle = app.handle().clone();
            #[cfg(windows)]
            main_window.with_webview(move |webview| {
                // SAFETY: Tauri keeps the WebView2 controller alive for this UI-thread callback.
                // The COM interfaces are obtained from that controller and used on the UI thread.
                let result = unsafe {
                    webview
                        .controller()
                        .CoreWebView2()
                        .and_then(|core| core.Settings())
                        .and_then(|settings| settings.cast::<ICoreWebView2Settings3>())
                        .and_then(|settings| {
                            settings.SetAreBrowserAcceleratorKeysEnabled(false)
                        })
                };
                if let Err(error) = result {
                    eprintln!(
                        "OpenChat could not disable WebView2 browser accelerator keys; stopping startup: {error}"
                    );
                    app_handle.exit(1);
                }
            })?;
            if setup_profile_root.is_none() {
                window_state::restore_window_state(&main_window);
                notifications::initialize(app.handle()).map_err(std::io::Error::other)?;
                main_window.hide()?;
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            #[cfg(any(
                feature = "startup-benchmark",
                feature = "desktop-profile-test-support"
            ))]
            {
                let _ = (window, event);
                return;
            }
            #[cfg(not(any(
                feature = "startup-benchmark",
                feature = "desktop-profile-test-support"
            )))]
            {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if let Some(state) = window
                    .app_handle()
                    .try_state::<app_preferences::AppPreferencesState>()
                {
                    if app_preferences::should_minimize_on_close(&state) {
                        api.prevent_close();
                        if let Err(error) = window.minimize() {
                            eprintln!(
                                "OpenChat could not minimize the window during close: {error}"
                            );
                        }
                        return;
                    }
                }
                window_state::save_window_state(window);
            } else if matches!(
                event,
                tauri::WindowEvent::Resized(_) | tauri::WindowEvent::Moved(_)
            ) {
                window_state::save_window_state(window);
            }
            }
        })
        .build(tauri::generate_context!())?;
    app.run(|handle, event| {
        if matches!(event, tauri::RunEvent::Exit) {
            if let Err(error) = handle.state::<local_service::LocalServiceState>().stop() {
                eprintln!("OpenChat local service shutdown failed: {error}");
            }
        }
    });
    Ok(())
}

#[cfg(feature = "desktop-profile-test-support")]
fn desktop_test_profile_root_from_args() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    desktop_test_profile_root_from(std::env::args_os().skip(1))
}

#[cfg(feature = "desktop-profile-test-support")]
fn desktop_test_profile_root_from(
    mut arguments: impl Iterator<Item = std::ffi::OsString>,
) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let mut profile_root = None;
    while let Some(argument) = arguments.next() {
        if argument == std::ffi::OsStr::new("--desktop-test-profile-root") {
            if profile_root.is_some() {
                return Err(
                    std::io::Error::other("The desktop test profile argument is invalid").into(),
                );
            }
            profile_root = arguments.next().map(std::path::PathBuf::from);
        }
    }

    let profile_root = profile_root
        .filter(|path| path.is_absolute())
        .ok_or_else(|| {
            std::io::Error::other("An absolute desktop test profile path is required")
        })?;
    let profile_name = profile_root
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .ok_or_else(|| std::io::Error::other("The desktop test profile path is invalid"))?
        .to_owned();
    let profile_id = profile_name
        .strip_prefix("openchat-desktop-test-")
        .ok_or_else(|| std::io::Error::other("The desktop test profile path is invalid"))?;
    let parsed_id = uuid::Uuid::parse_str(profile_id)
        .map_err(|_| std::io::Error::other("The desktop test profile path is invalid"))?;
    if parsed_id.to_string() != profile_id {
        return Err(std::io::Error::other("The desktop test profile path is invalid").into());
    }

    let temporary_root = std::fs::canonicalize(std::env::temp_dir())?;
    let profile_parent = profile_root
        .parent()
        .ok_or_else(|| std::io::Error::other("The desktop test profile path is invalid"))?;
    if std::fs::canonicalize(profile_parent)? != temporary_root {
        return Err(std::io::Error::other(
            "The desktop test profile must be inside the temporary directory",
        )
        .into());
    }
    match std::fs::symlink_metadata(&profile_root) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            match std::fs::create_dir(&profile_root) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.into()),
            }
        }
        Err(error) => return Err(error.into()),
    }
    let metadata = std::fs::symlink_metadata(&profile_root)?;
    if !metadata.is_dir() || is_reparse_point(&metadata) {
        return Err(std::io::Error::other("The desktop test profile path is invalid").into());
    }
    let profile_root = std::fs::canonicalize(profile_root)?;
    if profile_root.parent() != Some(temporary_root.as_path())
        || profile_root.file_name() != Some(std::ffi::OsStr::new(profile_name.as_str()))
    {
        return Err(std::io::Error::other(
            "The desktop test profile must be inside the temporary directory",
        )
        .into());
    }
    Ok(profile_root)
}

fn is_reparse_point(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes()
            & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT
            != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

fn ensure_profile_child_directory(
    profile_root: &std::path::Path,
    child_name: &str,
) -> std::io::Result<std::path::PathBuf> {
    let directory = profile_root.join(child_name);
    match std::fs::symlink_metadata(&directory) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            match std::fs::create_dir(&directory) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        Err(error) => return Err(error),
    }
    let metadata = std::fs::symlink_metadata(&directory)?;
    if !metadata.is_dir() || is_reparse_point(&metadata) {
        return Err(std::io::Error::other(
            "The desktop profile directory is invalid",
        ));
    }
    let canonical_directory = std::fs::canonicalize(directory)?;
    if canonical_directory.parent() != Some(profile_root)
        || canonical_directory.file_name() != Some(std::ffi::OsStr::new(child_name))
    {
        return Err(std::io::Error::other(
            "The desktop profile directory is invalid",
        ));
    }
    Ok(canonical_directory)
}

#[cfg(all(test, feature = "desktop-profile-test-support"))]
mod desktop_test_profile_tests {
    use std::{ffi::OsString, fs, path::PathBuf};

    use uuid::Uuid;

    use super::{desktop_test_profile_root_from, ensure_profile_child_directory};

    #[test]
    fn profile_argument_creates_and_reuses_only_a_direct_uuid_temporary_child() {
        let profile = TestProfile::new();
        let args = || {
            [
                OsString::from("--desktop-test-profile-root"),
                profile.0.as_os_str().to_owned(),
            ]
            .into_iter()
        };

        let first = desktop_test_profile_root_from(args()).expect("create test profile root");
        let webview = ensure_profile_child_directory(&first, "webview")
            .expect("create webview profile directory");
        let second = desktop_test_profile_root_from(args()).expect("reuse test profile root");

        let canonical_profile = fs::canonicalize(&profile.0).expect("canonicalize test profile");
        assert_eq!(first, canonical_profile);
        assert_eq!(webview.parent(), Some(canonical_profile.as_path()));
        assert_eq!(second, canonical_profile);
    }

    #[test]
    fn profile_argument_rejects_duplicate_and_non_uuid_roots() {
        let profile = TestProfile::new();
        let duplicate = [
            OsString::from("--desktop-test-profile-root"),
            profile.0.as_os_str().to_owned(),
            OsString::from("--desktop-test-profile-root"),
            profile.0.as_os_str().to_owned(),
        ];
        assert!(desktop_test_profile_root_from(duplicate.into_iter()).is_err());

        let invalid = std::env::temp_dir().join("openchat-desktop-test-not-a-uuid");
        let args = [
            OsString::from("--desktop-test-profile-root"),
            invalid.into_os_string(),
        ];
        assert!(desktop_test_profile_root_from(args.into_iter()).is_err());
    }

    struct TestProfile(PathBuf);

    impl TestProfile {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("openchat-desktop-test-{}", Uuid::new_v4()));
            Self(path)
        }
    }

    impl Drop for TestProfile {
        fn drop(&mut self) {
            if self.0.exists() {
                fs::remove_dir_all(&self.0).expect("remove isolated test profile");
            }
        }
    }
}

#[cfg(feature = "startup-benchmark")]
#[tauri::command]
fn startup_benchmark_ready(
    app: tauri::AppHandle,
    performance_snapshot: serde_json::Value,
) -> Result<(), String> {
    let benchmark = app.state::<StartupBenchmarkState>();
    if benchmark.reported.swap(true, Ordering::AcqRel) {
        return Ok(());
    }
    let elapsed_ms = benchmark.started_at.elapsed().as_secs_f64() * 1000.0;
    let result = serde_json::json!({
        "processToReactReadyMs": elapsed_ms,
        "performanceSnapshot": performance_snapshot,
    });
    std::fs::write(&benchmark.result_path, result.to_string())
        .map_err(|_| "The startup benchmark result could not be written.")?;
    app.exit(0);
    Ok(())
}

#[cfg(feature = "startup-benchmark")]
fn benchmark_profile_root_from_args() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    benchmark_profile_root_from(std::env::args_os().skip(1))
}

#[cfg(feature = "startup-benchmark")]
fn benchmark_profile_root_from(
    mut arguments: impl Iterator<Item = std::ffi::OsString>,
) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let mut profile_root = None;
    while let Some(argument) = arguments.next() {
        if argument == std::ffi::OsStr::new("--startup-benchmark-root") {
            if profile_root.is_some() {
                return Err(
                    std::io::Error::other("The benchmark profile argument is invalid").into(),
                );
            }
            profile_root = arguments.next().map(std::path::PathBuf::from);
        }
    }

    let profile_root = profile_root
        .filter(|path| path.is_absolute())
        .ok_or_else(|| std::io::Error::other("An absolute benchmark profile path is required"))?;
    let profile_name = profile_root
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .ok_or_else(|| std::io::Error::other("The benchmark profile path is invalid"))?
        .to_owned();
    let profile_id = profile_name
        .strip_prefix("openchat-desktop-test-")
        .ok_or_else(|| std::io::Error::other("The benchmark profile path is invalid"))?;
    let parsed_id = uuid::Uuid::parse_str(profile_id)
        .map_err(|_| std::io::Error::other("The benchmark profile path is invalid"))?;
    if parsed_id.to_string() != profile_id {
        return Err(std::io::Error::other("The benchmark profile path is invalid").into());
    }

    let temporary_root = std::fs::canonicalize(std::env::temp_dir())?;
    let profile_parent = profile_root
        .parent()
        .ok_or_else(|| std::io::Error::other("The benchmark profile path is invalid"))?;
    if std::fs::canonicalize(profile_parent)? != temporary_root {
        return Err(std::io::Error::other(
            "The benchmark profile must be inside the temporary directory",
        )
        .into());
    }
    std::fs::create_dir(&profile_root)?;
    let metadata = std::fs::symlink_metadata(&profile_root)?;
    if !metadata.is_dir() || is_reparse_point(&metadata) {
        return Err(std::io::Error::other("The benchmark profile path is invalid").into());
    }
    let profile_root = std::fs::canonicalize(profile_root)?;
    if profile_root.parent() != Some(temporary_root.as_path())
        || profile_root.file_name() != Some(std::ffi::OsStr::new(profile_name.as_str()))
    {
        return Err(std::io::Error::other(
            "The benchmark profile must be inside the temporary directory",
        )
        .into());
    }
    Ok(profile_root)
}

#[cfg(all(test, feature = "startup-benchmark"))]
mod startup_benchmark_profile_tests {
    use std::{ffi::OsString, fs, path::PathBuf};

    use uuid::Uuid;

    use super::benchmark_profile_root_from;

    #[test]
    fn profile_argument_creates_only_a_new_direct_uuid_temporary_child() {
        let profile = TestProfile::new();
        let arguments = || {
            [
                OsString::from("--startup-benchmark-root"),
                profile.0.as_os_str().to_owned(),
            ]
            .into_iter()
        };

        let root = benchmark_profile_root_from(arguments()).expect("create benchmark profile");
        let temporary_root =
            fs::canonicalize(std::env::temp_dir()).expect("canonicalize temporary directory");

        assert_eq!(root.parent(), Some(temporary_root.as_path()));
        assert_eq!(root.file_name(), profile.0.file_name());
        assert!(root.is_dir());
        assert!(benchmark_profile_root_from(arguments()).is_err());
    }

    #[test]
    fn profile_argument_rejects_duplicate_malformed_existing_and_outside_roots() {
        let profile = TestProfile::new();
        let duplicate = [
            OsString::from("--startup-benchmark-root"),
            profile.0.as_os_str().to_owned(),
            OsString::from("--startup-benchmark-root"),
            profile.0.as_os_str().to_owned(),
        ];
        assert!(benchmark_profile_root_from(duplicate.into_iter()).is_err());

        let malformed = std::env::temp_dir().join("openchat-desktop-test-not-a-uuid");
        let malformed_arguments = [
            OsString::from("--startup-benchmark-root"),
            malformed.into_os_string(),
        ];
        assert!(benchmark_profile_root_from(malformed_arguments.into_iter()).is_err());

        let outside = std::env::current_dir()
            .expect("get workspace directory")
            .join(format!("openchat-desktop-test-{}", Uuid::new_v4()));
        let outside_arguments = [
            OsString::from("--startup-benchmark-root"),
            outside.into_os_string(),
        ];
        assert!(benchmark_profile_root_from(outside_arguments.into_iter()).is_err());

        let existing = TestProfile::new();
        fs::create_dir(&existing.0).expect("create pre-existing profile root");
        let existing_arguments = [
            OsString::from("--startup-benchmark-root"),
            existing.0.as_os_str().to_owned(),
        ];
        assert!(benchmark_profile_root_from(existing_arguments.into_iter()).is_err());
    }

    struct TestProfile(PathBuf);

    impl TestProfile {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!("openchat-desktop-test-{}", Uuid::new_v4())))
        }
    }

    impl Drop for TestProfile {
        fn drop(&mut self) {
            if self.0.exists() {
                fs::remove_dir_all(&self.0).expect("remove isolated benchmark profile");
            }
        }
    }
}
