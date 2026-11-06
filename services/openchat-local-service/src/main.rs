//! Process entry point for the OpenChat local sidecar service.

fn main() {
    #[cfg(windows)]
    if let Err(hresult) = set_process_app_user_model_id() {
        eprintln!("openchat_app_user_model_id_setup_failed hresult=0x{hresult:08X}");
    }

    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    if arguments
        .first()
        .is_some_and(|argument| argument == "--storage-broker-stdio")
    {
        if arguments.len() != 1 {
            eprintln!("openchat-storage-broker-error: invalid arguments");
            std::process::exit(1);
        }
        #[cfg(windows)]
        if let Err(error) = openchat_local_service::run_broker_stdio() {
            eprintln!("openchat-storage-broker-error: {error}");
            std::process::exit(1);
        }
        #[cfg(not(windows))]
        {
            eprintln!("openchat-storage-broker-error: unsupported platform");
            std::process::exit(1);
        }
        #[cfg(windows)]
        return;
    }
    run_local_service();
}

#[cfg(windows)]
fn set_process_app_user_model_id() -> Result<(), u32> {
    use windows_sys::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;

    let app_user_model_id = if cfg!(debug_assertions) {
        "OpenChat.Desktop.Dev"
    } else {
        "OpenChat.Desktop"
    };
    let mut app_user_model_id = app_user_model_id.encode_utf16().collect::<Vec<_>>();
    app_user_model_id.push(0);

    // SAFETY: The identity buffer is NUL-terminated and remains live through the API call.
    let result = unsafe { SetCurrentProcessExplicitAppUserModelID(app_user_model_id.as_ptr()) };
    if result >= 0 {
        Ok(())
    } else {
        Err(result as u32)
    }
}

#[tokio::main]
async fn run_local_service() {
    if let Err(error) = openchat_local_service::run().await {
        match serde_json::to_string(&error.startup_failure()) {
            Ok(failure) => eprintln!("openchat-local-service-error: {failure}"),
            Err(_) => {
                eprintln!("openchat-local-service-error: startup_failure_serialization_failed")
            }
        }
        std::process::exit(1);
    }
}
