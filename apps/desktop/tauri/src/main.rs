//! Tauri host entry point and registration of frontend-accessible native commands.
#![deny(missing_docs)]
#![deny(clippy::undocumented_unsafe_blocks)]
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[cfg(all(feature = "desktop-profile-test-support", not(debug_assertions)))]
compile_error!("desktop profile test support is unavailable in optimized builds");

use tauri::Manager;
#[cfg(windows)]
use webview2_com::{
    CoTaskMemPWSTR,
    Microsoft::Web::WebView2::Win32::{
        ICoreWebView2Settings3, COREWEBVIEW2_PROCESS_FAILED_KIND,
        COREWEBVIEW2_PROCESS_FAILED_KIND_FRAME_RENDER_PROCESS_EXITED,
        COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED,
    },
    ProcessFailedEventHandler,
};
#[cfg(windows)]
use windows_core::{Interface, PWSTR};

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
mod renderer_recovery;
mod request_bounds;
mod save_dialog;
#[cfg_attr(test, allow(dead_code))]
mod window_broadcast;
#[cfg_attr(
    any(
        feature = "startup-benchmark",
        feature = "desktop-profile-test-support"
    ),
    allow(dead_code)
)]
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
    #[cfg(feature = "desktop-profile-test-support")]
    let desktop_test_run_id = desktop_test_run_id_from_args()?;

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
    #[cfg(feature = "desktop-profile-test-support")]
    let desktop_test_normal_close_marker = desktop_test_profile_root
        .as_ref()
        .map(|root| desktop_test_normal_close_marker_path(root, desktop_test_run_id));

    let builder =
        tauri::Builder::default().manage(renderer_recovery::RendererRecoveryTracker::default());
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
            chat_storage::record_chat_message_persistence_failure,
            chat_storage::record_file_changes_load_failure,
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
            report_webview_renderer_ready,
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
            let renderer_recovery = app
                .state::<renderer_recovery::RendererRecoveryTracker>()
                .inner()
                .clone();
            #[cfg(windows)]
            let recovery_window = main_window.clone();
            #[cfg(windows)]
            main_window.with_webview(move |webview| {
                // SAFETY: Tauri keeps the WebView2 controller alive for this UI-thread callback.
                // The COM interfaces are obtained from that controller and used on the UI thread.
                let result = unsafe {
                    webview
                        .controller()
                        .CoreWebView2()
                        .and_then(|core| {
                            let settings = core
                                .Settings()?
                                .cast::<ICoreWebView2Settings3>()?;
                            settings.SetAreBrowserAcceleratorKeysEnabled(false)?;

                            let recovery_for_callback = renderer_recovery.clone();
                            let recovery_window_for_callback = recovery_window.clone();
                            let handler = ProcessFailedEventHandler::create(Box::new(
                                move |sender, args| {
                                    let Some(core) = sender else {
                                        return Ok(());
                                    };
                                    let Some(args) = args else {
                                        return Ok(());
                                    };
                                    let mut kind = COREWEBVIEW2_PROCESS_FAILED_KIND(0);
                                    if let Err(error) = args.ProcessFailedKind(&mut kind) {
                                        eprintln!("OpenChat could not read the WebView2 process failure kind: {error}");
                                        return Ok(());
                                    }
                                    if kind != COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED
                                        && kind != COREWEBVIEW2_PROCESS_FAILED_KIND_FRAME_RENDER_PROCESS_EXITED
                                    {
                                        return Ok(());
                                    }

                                    match recovery_for_callback.record_failure() {
                                        renderer_recovery::RendererRecoveryAction::Reload { attempt } => {
                                            eprintln!("OpenChat WebView2 renderer exited; reloading the application view (attempt {attempt}).");
                                            if let Err(error) = core.Reload() {
                                                eprintln!("OpenChat could not reload the WebView2 renderer: {error}");
                                            }
                                        }
                                        renderer_recovery::RendererRecoveryAction::ShowRecovery => {
                                            eprintln!("OpenChat WebView2 renderer exited repeatedly; showing recovery screen.");
                                            if let Err(error) = recovery_window_for_callback.set_decorations(true) {
                                                eprintln!("OpenChat could not enable the recovery window title bar: {error}");
                                            }
                                            let current_source = if recovery_for_callback.has_app_url() {
                                                None
                                            } else {
                                                let mut source = PWSTR::null();
                                                match core.Source(&mut source) {
                                                    Ok(()) => Some(CoTaskMemPWSTR::from(source).to_string()),
                                                    Err(error) => {
                                                        eprintln!("OpenChat could not read the failed renderer URL: {error}");
                                                        None
                                                    }
                                                }
                                            };
                                            let recovery_page = recovery_for_callback
                                                .recovery_page(current_source.as_deref());
                                            let recovery_page = CoTaskMemPWSTR::from(recovery_page.as_str());
                                            if let Err(error) = core.NavigateToString(*recovery_page.as_ref().as_pcwstr()) {
                                                eprintln!("OpenChat could not show the WebView2 recovery screen: {error}");
                                            }
                                            if let Err(error) = recovery_window_for_callback.show() {
                                                eprintln!("OpenChat could not show the renderer recovery window: {error}");
                                            }
                                        }
                                    }
                                    Ok(())
                                },
                            ));
                            let mut token = 0;
                            core.add_ProcessFailed(&handler, &mut token)?;
                            Ok(())
                        })
                };
                if let Err(error) = result {
                    eprintln!(
                        "OpenChat could not disable WebView2 browser accelerator keys; stopping startup: {error}"
                    );
                    app_handle.exit(1);
                }
            })?;
            if setup_profile_root.is_none() || cfg!(feature = "desktop-profile-test-support") {
                let writer_result = match setup_profile_root.as_ref() {
                    Some(profile_root) => window_state::WindowStateWriter::start_at_path(
                        profile_root.join("window-state.json"),
                    ),
                    None => window_state::WindowStateWriter::start(app.handle()),
                };
                match writer_result {
                    Ok(writer) => {
                        if !app.manage(writer) {
                            eprintln!("OpenChat could not register the window geometry writer.");
                        }
                    }
                    Err(error) => {
                        eprintln!(
                            "openchat_window_state_writer_start_failed kind={:?}",
                            error.kind()
                        );
                    }
                }
                main_window.unminimize()?;
                if let Some(profile_root) = setup_profile_root.as_ref() {
                    window_state::restore_window_state_at_path(
                        &main_window,
                        &profile_root.join("window-state.json"),
                    )?;
                } else {
                    window_state::restore_window_state(&main_window)?;
                    notifications::initialize(app.handle()).map_err(std::io::Error::other)?;
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                #[cfg(any(
                    feature = "startup-benchmark",
                    feature = "desktop-profile-test-support"
                ))]
                let _ = api;
                #[cfg(not(any(
                    feature = "startup-benchmark",
                    feature = "desktop-profile-test-support"
                )))]
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
                window_state::flush_window_state_save(window);
            } else if matches!(
                event,
                tauri::WindowEvent::Resized(_) | tauri::WindowEvent::Moved(_)
            ) {
                window_state::schedule_window_state_save(window);
            }
        })
        .build(tauri::generate_context!())?;
    #[cfg(feature = "desktop-profile-test-support")]
    let mut desktop_test_window_close_requested = false;
    app.run(move |handle, event| {
        #[cfg(feature = "desktop-profile-test-support")]
        if let tauri::RunEvent::ExitRequested { code, .. } = &event {
            desktop_test_window_close_requested = desktop_test_user_exit_requested(*code);
        }

        if matches!(&event, tauri::RunEvent::Exit) {
            window_state::shutdown_window_state_writer(handle);
            let shutdown_result = handle.state::<local_service::LocalServiceState>().stop();
            if let Err(error) = &shutdown_result {
                eprintln!("OpenChat local service shutdown failed: {error}");
            }

            #[cfg(feature = "desktop-profile-test-support")]
            if shutdown_result.is_ok() && desktop_test_window_close_requested {
                if let Some(marker_path) = &desktop_test_normal_close_marker {
                    if let Err(error) =
                        write_desktop_test_normal_close_marker(marker_path, desktop_test_run_id)
                    {
                        eprintln!(
                            "OpenChat could not record the normal test-window close: {error}"
                        );
                    }
                }
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
fn desktop_test_run_id_from_args() -> Result<uuid::Uuid, Box<dyn std::error::Error>> {
    desktop_test_run_id_from(std::env::args_os().skip(1))
}

#[cfg(feature = "desktop-profile-test-support")]
fn desktop_test_run_id_from(
    mut arguments: impl Iterator<Item = std::ffi::OsString>,
) -> Result<uuid::Uuid, Box<dyn std::error::Error>> {
    let mut run_id = None;
    while let Some(argument) = arguments.next() {
        if argument == std::ffi::OsStr::new("--desktop-test-run-id") {
            if run_id.is_some() {
                return Err(
                    std::io::Error::other("The desktop test run identifier is invalid").into(),
                );
            }
            run_id = arguments.next();
        }
    }

    let run_id = run_id
        .and_then(|value| value.into_string().ok())
        .ok_or_else(|| std::io::Error::other("A desktop test run identifier is required"))?;
    let parsed_id = uuid::Uuid::parse_str(&run_id)
        .map_err(|_| std::io::Error::other("The desktop test run identifier is invalid"))?;
    if parsed_id.to_string() != run_id {
        return Err(std::io::Error::other("The desktop test run identifier is invalid").into());
    }
    Ok(parsed_id)
}

#[cfg(feature = "desktop-profile-test-support")]
fn desktop_test_normal_close_marker_path(
    profile_root: &std::path::Path,
    run_id: uuid::Uuid,
) -> std::path::PathBuf {
    profile_root.join(format!("normal-close-{run_id}.marker"))
}

#[cfg(feature = "desktop-profile-test-support")]
fn desktop_test_user_exit_requested(code: Option<i32>) -> bool {
    code.is_none()
}

#[cfg(feature = "desktop-profile-test-support")]
fn write_desktop_test_normal_close_marker(
    marker_path: &std::path::Path,
    run_id: uuid::Uuid,
) -> std::io::Result<()> {
    use std::io::Write;

    let mut marker = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(marker_path)?;
    marker.write_all(run_id.to_string().as_bytes())?;
    marker.sync_all()
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

    use super::{
        desktop_test_normal_close_marker_path, desktop_test_profile_root_from,
        desktop_test_run_id_from, desktop_test_user_exit_requested, ensure_profile_child_directory,
        write_desktop_test_normal_close_marker,
    };

    #[test]
    fn only_user_requested_exit_is_classified_as_a_normal_window_close() {
        assert!(desktop_test_user_exit_requested(None));
        assert!(!desktop_test_user_exit_requested(Some(0)));
        assert!(!desktop_test_user_exit_requested(Some(1)));
    }

    #[test]
    fn run_identifier_must_be_a_single_canonical_uuid() {
        let run_id = Uuid::new_v4();
        let valid = [
            OsString::from("--desktop-test-run-id"),
            OsString::from(run_id.to_string()),
        ];
        assert_eq!(
            desktop_test_run_id_from(valid.into_iter()).expect("parse test run identifier"),
            run_id,
        );

        let duplicate = [
            OsString::from("--desktop-test-run-id"),
            OsString::from(run_id.to_string()),
            OsString::from("--desktop-test-run-id"),
            OsString::from(run_id.to_string()),
        ];
        assert!(desktop_test_run_id_from(duplicate.into_iter()).is_err());

        let invalid = [
            OsString::from("--desktop-test-run-id"),
            OsString::from("not-a-uuid"),
        ];
        assert!(desktop_test_run_id_from(invalid.into_iter()).is_err());
    }

    #[test]
    fn normal_close_marker_is_unique_to_the_test_run_and_contains_its_identifier() {
        let profile = TestProfile::new();
        let profile_args = [
            OsString::from("--desktop-test-profile-root"),
            profile.0.as_os_str().to_owned(),
        ];
        let profile_root =
            desktop_test_profile_root_from(profile_args.into_iter()).expect("create test profile");
        let run_id = Uuid::new_v4();
        let marker_path = desktop_test_normal_close_marker_path(&profile_root, run_id);

        write_desktop_test_normal_close_marker(&marker_path, run_id)
            .expect("write normal-close marker");
        assert_eq!(
            fs::read_to_string(&marker_path).expect("read normal-close marker"),
            run_id.to_string(),
        );
        assert!(write_desktop_test_normal_close_marker(&marker_path, run_id).is_err());
    }

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

#[tauri::command]
fn report_webview_renderer_ready(
    window: tauri::WebviewWindow,
    renderer_recovery: tauri::State<'_, renderer_recovery::RendererRecoveryTracker>,
    language: request_bounds::BoundedText<32>,
    app_url: request_bounds::BoundedText<4096>,
) -> Result<(), &'static str> {
    if window.label() != "main" {
        return Err("Renderer readiness can only be reported by the main window.");
    }
    let decorations_result = window.set_decorations(false);
    let app_url = match url::Url::parse(&app_url.into_inner()) {
        Ok(url) => Some(url),
        Err(_) => {
            eprintln!("OpenChat ignored an invalid renderer recovery URL.");
            None
        }
    };
    renderer_recovery.renderer_ready(app_url, &language.into_inner());
    decorations_result.map_err(|_| "The custom application title bar could not be restored.")?;
    Ok(())
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
