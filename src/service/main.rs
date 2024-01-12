use std::{ffi::OsString, sync::{Arc, atomic::AtomicBool}};
use windows_service::{
    define_windows_service,
    service_dispatcher,
    service::ServiceControl,
    service_control_handler::{self, ServiceControlHandlerResult},
};

define_windows_service!(ffi_service_main, service_main);

fn service_main(_: Vec<OsString>) {
    let signal = Arc::new(AtomicBool::new(false));
    let signal_dup = signal.clone();
    _ = service_control_handler::register("DnxHostService", move |event| {
        match event {
            ServiceControl::Stop => {
                signal_dup.store(true, std::sync::atomic::Ordering::Relaxed);
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    });

    tokio::spawn(async move {
        log::info!("Starting DNX server");
        let mut server = dnx_rs::server::setup_server().await.unwrap();
        log::info!("DNX server started");

        while !signal.load(std::sync::atomic::Ordering::Relaxed) { }
        log::info!("Shutting down DNX server");

        server.shutdown_gracefully().await.expect("Failed to shutdown gracefully");
        log::info!("Goodbye!");
    });
}

fn main() -> Result<(), windows_service::Error> {
    service_dispatcher::start("DnxHostService", ffi_service_main)?;

    Ok(())
}