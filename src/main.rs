mod args;

#[cfg(windows)]
fn main() -> windows_service::Result<()> {
    munin_service::run()
}

#[cfg(not(windows))]
fn main() {
    panic!("This program is only intended to run on Windows.");
}

#[cfg(windows)]
mod munin_service {
    use crate::args::Subcommand;
    use clap::Parser;
    use std::{
        ffi::OsString,
        net::{IpAddr, SocketAddr, UdpSocket},
        sync::mpsc,
        time::Duration,
    };
    use windows_service::{
        define_windows_service,
        service::{
            ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
            ServiceType,
        },
        service_control_handler::{self, ServiceControlHandlerResult},
        service_dispatcher, Result,
    };

    use super::args::Args;

    const SERVICE_NAME: &str = "munin_service";
    const SERVICE_DESCRIPTION: &str = "Munin monitoring and control service";
    const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

    const LOOPBACK_ADDR: [u8; 4] = [127, 0, 0, 1];
    const RECEIVER_PORT: u16 = 1234;
    const PING_MESSAGE: &str = "ping\n";

    pub fn run() -> Result<()> {
        // Register generated `ffi_service_main` with the system and start the service, blocking
        // this thread until the service is stopped.
        let res = service_dispatcher::start(SERVICE_NAME, ffi_service_main);
        if let Err(_err) = res {
            let args = Args::parse();
            match args.subcommand {
                Subcommand::Install(_install) => {
                    install_service()?;
                }
                Subcommand::Uninstall(_uninstall) => {
                    uninstall_service()?;
                }
                Subcommand::QueryConfig(_query_config) => {
                    query_config()?;
                }
            }
        }
        Ok(())
    }

    // Generate the windows service boilerplate.
    // The boilerplate contains the low-level service entry function (ffi_service_main) that parses
    // incoming service arguments into Vec<OsString> and passes them to user defined service
    // entry (my_service_main).
    define_windows_service!(ffi_service_main, my_service_main);

    // Service entry function which is called on background thread by the system with service
    // parameters. There is no stdout or stderr at this point so make sure to configure the log
    // output to file if needed.
    pub fn my_service_main(_arguments: Vec<OsString>) {
        if let Err(_e) = run_service() {
            // Handle the error, by logging or something.
        }
    }

    /// Installs yourself as a service
    fn install_service() -> windows_service::Result<()> {
        use std::ffi::OsString;
        use windows_service::{
            service::{
                ServiceAccess, ServiceErrorControl, ServiceInfo, ServiceStartType, ServiceType,
            },
            service_manager::{ServiceManager, ServiceManagerAccess},
        };

        let manager_access = ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE;
        let service_manager = ServiceManager::local_computer(None::<&str>, manager_access)?;

        let service_binary_path = ::std::env::current_exe().unwrap();

        let service_info = ServiceInfo {
            name: OsString::from(SERVICE_NAME),
            display_name: OsString::from(SERVICE_DESCRIPTION),
            service_type: ServiceType::OWN_PROCESS,
            start_type: ServiceStartType::OnDemand,
            error_control: ServiceErrorControl::Normal,
            executable_path: service_binary_path,
            launch_arguments: vec![],
            dependencies: vec![],
            account_name: None, // run as System
            account_password: None,
        };
        let service =
            service_manager.create_service(&service_info, ServiceAccess::CHANGE_CONFIG)?;
        service.set_description("Windows service example from windows-service-rs")?;
        Ok(())
    }

    fn uninstall_service() -> windows_service::Result<()> {
        use std::{
            thread::sleep,
            time::{Duration, Instant},
        };
    
        use windows_service::{
            service::{ServiceAccess, ServiceState},
            service_manager::{ServiceManager, ServiceManagerAccess},
        };
        use windows_sys::Win32::Foundation::ERROR_SERVICE_DOES_NOT_EXIST;
    
        let manager_access = ServiceManagerAccess::CONNECT;
        let service_manager = ServiceManager::local_computer(None::<&str>, manager_access)?;
    
        let service_access = ServiceAccess::QUERY_STATUS | ServiceAccess::STOP | ServiceAccess::DELETE;
        let service = service_manager.open_service(SERVICE_NAME, service_access)?;
    
        // The service will be marked for deletion as long as this function call succeeds.
        // However, it will not be deleted from the database until it is stopped and all open handles to it are closed.
        service.delete()?;
        // Our handle to it is not closed yet. So we can still query it.
        if service.query_status()?.current_state != ServiceState::Stopped {
            // If the service cannot be stopped, it will be deleted when the system restarts.
            service.stop()?;
        }
        // Explicitly close our open handle to the service. This is automatically called when `service` goes out of scope.
        drop(service);
    
        // Win32 API does not give us a way to wait for service deletion.
        // To check if the service is deleted from the database, we have to poll it ourselves.
        let start = Instant::now();
        let timeout = Duration::from_secs(5);
        while start.elapsed() < timeout {
            if let Err(windows_service::Error::Winapi(e)) =
                service_manager.open_service(SERVICE_NAME, ServiceAccess::QUERY_STATUS)
            {
                if e.raw_os_error() == Some(ERROR_SERVICE_DOES_NOT_EXIST as i32) {
                    println!("{SERVICE_NAME} is deleted.");
                    return Ok(());
                }
            }
            sleep(Duration::from_secs(1));
        }
        println!("{SERVICE_NAME} is marked for deletion.");
    
        Ok(())
    }

    fn query_config() -> windows_service::Result<()> {
        use windows_service::{
            service::ServiceAccess,
            service_manager::{ServiceManager, ServiceManagerAccess},
        };
    
        let manager_access = ServiceManagerAccess::CONNECT;
        let service_manager = ServiceManager::local_computer(None::<&str>, manager_access)?;
    
        let service = service_manager.open_service(SERVICE_NAME, ServiceAccess::QUERY_CONFIG)?;
    
        let config = service.query_config()?;
        println!("{:#?}", config);
        Ok(())
    }

    pub fn run_service() -> Result<()> {
        // Create a channel to be able to poll a stop event from the service worker loop.
        let (shutdown_tx, shutdown_rx) = mpsc::channel();

        // Define system service event handler that will be receiving service events.
        let event_handler = move |control_event| -> ServiceControlHandlerResult {
            match control_event {
                // Notifies a service to report its current status information to the service
                // control manager. Always return NoError even if not implemented.
                ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,

                // Handle stop
                ServiceControl::Stop => {
                    shutdown_tx.send(()).unwrap();
                    ServiceControlHandlerResult::NoError
                }

                // treat the UserEvent as a stop request
                ServiceControl::UserEvent(code) => {
                    if code.to_raw() == 130 {
                        shutdown_tx.send(()).unwrap();
                    }
                    ServiceControlHandlerResult::NoError
                }

                _ => ServiceControlHandlerResult::NotImplemented,
            }
        };

        // Register system service event handler.
        // The returned status handle should be used to report service status changes to the system.
        let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)?;

        // Tell the system that service is running
        status_handle.set_service_status(ServiceStatus {
            service_type: SERVICE_TYPE,
            current_state: ServiceState::Running,
            controls_accepted: ServiceControlAccept::STOP,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        })?;

        // For demo purposes this service sends a UDP packet once a second.
        let loopback_ip = IpAddr::from(LOOPBACK_ADDR);
        let sender_addr = SocketAddr::new(loopback_ip, 0);
        let receiver_addr = SocketAddr::new(loopback_ip, RECEIVER_PORT);
        let msg = PING_MESSAGE.as_bytes();
        let socket = UdpSocket::bind(sender_addr).unwrap();

        loop {
            let _ = socket.send_to(msg, receiver_addr);

            // Poll shutdown event.
            match shutdown_rx.recv_timeout(Duration::from_secs(1)) {
                // Break the loop either upon stop or channel disconnect
                Ok(_) | Err(mpsc::RecvTimeoutError::Disconnected) => break,

                // Continue work if no events were received within the timeout
                Err(mpsc::RecvTimeoutError::Timeout) => (),
            };
        }

        // Tell the system that service has stopped.
        status_handle.set_service_status(ServiceStatus {
            service_type: SERVICE_TYPE,
            current_state: ServiceState::Stopped,
            controls_accepted: ServiceControlAccept::empty(),
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        })?;

        Ok(())
    }
}
