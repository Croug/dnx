use std::{ffi::OsString, sync::{Arc, atomic::AtomicBool}, time::Duration};
use windows_service::{
    define_windows_service,
    service_dispatcher,
    service::{ServiceControl, ServiceStatus, ServiceType, ServiceState, ServiceExitCode, ServiceControlAccept},
    service_control_handler::{self, ServiceControlHandlerResult, ServiceStatusHandle},
};

define_windows_service!(ffi_service_main, service_main);

fn update_status(current_state: ServiceState, status_handle: &ServiceStatusHandle) -> windows_service::Result<()> {
    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state,
        controls_accepted: ServiceControlAccept::STOP,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })
}

fn service_main(_: Vec<OsString>) {
    let signal = Arc::new(AtomicBool::new(false));
    let signal_dup = signal.clone();
    let status_handle = service_control_handler::register("DnxHostService", move |event| {
        match event {
            ServiceControl::Stop => {
                signal_dup.store(true, std::sync::atomic::Ordering::Relaxed);
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    }).expect("Failed to register service control handler");

    tokio::spawn(async move {
        log::info!("Starting DNX server");
        let mut server = dnx_rs::server::setup_server().await.unwrap();
        log::info!("DNX server started");

        update_status(ServiceState::Running, &status_handle)
            .expect("failed to update service status");

        while !signal.load(std::sync::atomic::Ordering::Relaxed) { }
        log::info!("Shutting down DNX server");

        server.shutdown_gracefully().await.expect("Failed to shutdown gracefully");
        log::info!("Goodbye!");

        update_status(ServiceState::Stopped, &status_handle)
            .expect("failed to update service status");
    });
}

fn main() -> Result<(), windows_service::Error> {
    let status_handle = service_dispatcher::start("DnxHostService", ffi_service_main)?;

    Ok(())
}