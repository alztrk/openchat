//! Process entry point for the OpenChat local sidecar service.

fn main() {
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
        return;
    }
    run_local_service();
}

#[tokio::main]
async fn run_local_service() {
    if let Err(error) = openchat_local_service::run().await {
        eprintln!("openchat-local-service-error: {error}");
        std::process::exit(1);
    }
}
