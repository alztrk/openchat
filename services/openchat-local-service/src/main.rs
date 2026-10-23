//! Process entry point for the OpenChat local sidecar service.

#[tokio::main]
async fn main() {
    if let Err(error) = openchat_local_service::run().await {
        eprintln!("openchat-local-service-error: {error}");
        std::process::exit(1);
    }
}
