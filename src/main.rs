#[tokio::main]
async fn main() {
    env_logger::init();

    log::info!("Starting DNX server");
    let mut server = dnx_rs::server::setup_server().await.unwrap();
    log::info!("DNX server started, press Ctrl+C to exit");

    tokio::signal::ctrl_c().await.unwrap();

    log::info!("Shutting down DNX server");
    server.shutdown_gracefully().await.expect("Failed to shutdown gracefully");
    log::info!("Goodbye!");
}
