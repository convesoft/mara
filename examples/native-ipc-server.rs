//! Minimal foreground entry for the IPC foundation; no lifecycle or auto routing.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn main() -> std::io::Result<()> {
    let server = mara::ipc::IpcServer::bind_default(mara::NativeSessions::default())?;
    server.serve()
}
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn main() {
    eprintln!("Native Mara IPC is supported only on the documented Linux/macOS hosts");
    std::process::exit(1);
}
