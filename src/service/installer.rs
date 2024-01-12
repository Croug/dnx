use std::error::Error;
use std::ffi::OsString;
use std::fs::File;
use std::io::{Write, self};
use std::env;
use std::path::PathBuf;

use windows_service::service::{ServiceType, ServiceInfo, ServiceStartType, ServiceErrorControl, ServiceAccess};
use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};
const BINARY: &[u8] = include_bytes!("../../target/release/service.exe");

fn get_path() -> PathBuf {
    let program_data = env::var("ProgramData").expect("Failed to get ProgramData environment variable");

    let mut path = PathBuf::from(program_data);
    path.push("dnx");
    path.push("DnxHostService.exe");

    path
}

fn write_binary() -> io::Result<()> {
    let path = get_path();
    let mut file = File::create(path)?;
    file.write_all(BINARY)?;

    Ok(())
}

fn register_service() -> windows_service::Result<()> {
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CREATE_SERVICE)?;

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

    let service = manager.create_service(&service_info, ServiceAccess::CHANGE_CONFIG)?;
    service.set_description("DNS server host process for DNX The DNS Multiplexer")?;

    Ok(())
}

fn do_install() -> Result<(), Box<dyn Error>> {
    write_binary()?;

    register_service()?;

    Ok(())
}

fn cleanup() {
    let path = get_path();
    if path.exists() {
        std::fs::remove_file(path).expect("Failed to remove service binary");
    }

}

fn main() -> Result<(), Box<dyn Error>> {
    match do_install() {
        Ok(_) => println!("Successfully installed DNX Host Service"),
        Err(e) => {
            println!("Failed to install DNX Host Service: {}", e);
            cleanup();
        }
    }

    println!("Press ENTER to dismiss...");
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    Ok(())
}