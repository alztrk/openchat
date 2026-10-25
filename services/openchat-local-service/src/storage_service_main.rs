//! Windows SCM entry point for the protected OpenChat storage broker.

#[cfg(windows)]
windows_service::define_windows_service!(ffi_service_main, service_main);

#[cfg(windows)]
fn main() -> windows_service::Result<()> {
    windows_service::service_dispatcher::start("OpenChatStorage", ffi_service_main)
}

#[cfg(windows)]
fn service_main(_arguments: Vec<std::ffi::OsString>) {
    if let Err(error) = openchat_local_service::run_storage_service() {
        eprintln!("openchat-storage-service-error: {error}");
        std::process::exit(1);
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("openchat-storage-service is supported only on Windows.");
    std::process::exit(1);
}
