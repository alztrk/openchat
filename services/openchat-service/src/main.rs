//! Process entry point for the OpenChat local sidecar service.

#[tokio::main]
async fn main() {
    if let Err(error) = openchat_service::run().await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
