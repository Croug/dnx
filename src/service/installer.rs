use std::error::Error;
use std::ffi::OsString;
use std::fs::File;
use std::io::{Write, self};
use std::env;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use windows_service::service::{ServiceType, ServiceInfo, ServiceStartType, ServiceErrorControl, ServiceAccess, ServiceState};
use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};
const BINARY: &[u8] = include_bytes!("../../target/release/service.exe");

fn get_path() -> PathBuf {
    let program_data = env::var("ProgramData").expect("Failed to get ProgramData environment variable");

    let mut path = PathBuf::from(program_data);
    path.push("dnx");
    path.push("DnxHostService.exe");
    path
}

fn create_directory_if_not_exists() -> io::Result<()> {
    let binding = get_path();
    let path = binding.parent().expect("Failed to get parent directory");
    if !path.exists() {
        std::fs::create_dir_all(path)?; // Create the directory and any necessary parent directories
        println!("Created directory: {:?}", path); // Log the creation of the directory
    }
    Ok(())
}

fn write_binary() -> io::Result<()> {
    create_directory_if_not_exists()?; // Ensure the directory exists
    let path = get_path();
    println!("Writing service binary to: {:?}", path);
    let mut file = File::create(&path)?; // Use &path for clarity
    file.write_all(BINARY)?;
    println!("Binary write complete.");

    Ok(())
}

fn stop_service_if_exists() -> windows_service::Result<()> {
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
    println!("Checking if service 'DnxHostService' already exists...");

    if let Ok(service) = manager.open_service(
        "DnxHostService",
        ServiceAccess::QUERY_STATUS | ServiceAccess::STOP,
    ) {
        println!("Service found. Checking current state...");
        let status = service.query_status()?;
        if status.current_state != ServiceState::Stopped {
            println!("Service is running. Stopping service...");
            let _ = service.stop();

            for i in 0..30 {
                let status = service.query_status()?;
                if status.current_state == ServiceState::Stopped {
                    println!("Service stopped.");
                    break;
                }
                println!("Waiting for stop... ({}s)", i + 1);
                thread::sleep(Duration::from_secs(1));
            }
        } else {
            println!("Service is already stopped.");
        }
    } else {
        println!("Service not found. A new service will be created.");
    }

    Ok(())
}

fn register_service() -> windows_service::Result<()> {
    let manager = ServiceManager::local_computer(
        None::<&str>,
        ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE,
    )?;

    let service_info = ServiceInfo {
        name: OsString::from("DnxHostService"),
        display_name: OsString::from("DNX The DNS Multiplexer"),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: get_path(),
        launch_arguments: vec![],
        dependencies: vec![],
        account_name: None,
        account_password: None
    };

    if let Ok(service) = manager.open_service("DnxHostService", ServiceAccess::CHANGE_CONFIG) {
        println!("Updating existing service configuration...");
        service.change_config(&service_info)?;
        service.set_description("DNS server host process for DNX The DNS Multiplexer")?;
        println!("Service configuration updated.");
        return Ok(());
    }

    println!("Creating new service...");
    let service = manager.create_service(&service_info, ServiceAccess::CHANGE_CONFIG)?;
    service.set_description("DNS server host process for DNX The DNS Multiplexer")?;
    println!("Service created.");
    Ok(())
}

fn do_install() -> Result<(), Box<dyn Error>> {
    println!("Starting DNX Host Service installation...");
    stop_service_if_exists()?;
    write_binary()?;
    register_service()?;
    println!("Installation steps completed.");
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    match do_install() {
        Ok(_) => println!("Successfully installed/updated DNX Host Service"),
        Err(e) => {
            eprintln!("Failed to install/update DNX Host Service: {}", e);
            // Intentionally no cleanup: keep existing install intact on partial failures.
        }
    }

    println!("Press ENTER to dismiss...");
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    Ok(())
}