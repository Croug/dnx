#[tokio::main]
async fn main() {
    env_logger::init();

    let mut server = dnx_rs::server::setup_server().await.unwrap();

    tokio::signal::ctrl_c().await.unwrap();

    server.shutdown_gracefully().await.expect("Failed to shutdown gracefully");

    println!("Goodbye!");
}
