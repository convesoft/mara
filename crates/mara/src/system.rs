//! What the host takes from the system it runs on: the filesystem, the clock, random bytes and
//! its process ID (DES-ENGINE-BOUNDARY in `docs/engine.mara.md`). Natively they are the operating
//! system's. In the WebAssembly build of the browser workspace (DES-BROWSER-WORKSPACE in
//! `docs/hosts/index.mara.md`) the filesystem is the workspace's files in memory ([`memory`]),
//! and the worker that runs the module supplies the time and the random bytes.
//!
//! The host's code that a browser workspace runs names the filesystem as `crate::system::fs`.

use std::io;
use std::path::Path;
use std::time::SystemTime;

#[cfg(any(target_arch = "wasm32", test))]
pub mod memory;

#[cfg(target_arch = "wasm32")]
pub use memory as fs;
#[cfg(not(target_arch = "wasm32"))]
pub use std::fs;

/// The current time.
#[cfg(not(target_arch = "wasm32"))]
pub fn now() -> SystemTime {
    SystemTime::now()
}

/// The current time, as the worker's clock gives it.
#[cfg(target_arch = "wasm32")]
pub fn now() -> SystemTime {
    let millis = imports::now();
    let millis = if millis.is_finite() && millis > 0.0 {
        millis as u64
    } else {
        0
    };
    SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(millis)
}

/// Fills `bytes` from the operating system's random source.
#[cfg(not(target_arch = "wasm32"))]
pub fn random(bytes: &mut [u8]) -> io::Result<()> {
    use std::io::Read as _;
    std::fs::File::open("/dev/urandom")?.read_exact(bytes)
}

/// Fills `bytes` from the worker's random source (`crypto.getRandomValues`).
#[cfg(target_arch = "wasm32")]
pub fn random(bytes: &mut [u8]) -> io::Result<()> {
    // SAFETY: the worker writes exactly `bytes.len()` bytes at the pointer, into this module's
    // memory, before it returns.
    unsafe { imports::random(bytes.as_mut_ptr(), bytes.len()) };
    Ok(())
}

/// Creates the directory `path`: natively with the mode that the request's caller would give it
/// ([`crate::caller::create_dir`]).
pub fn create_dir(path: &Path) -> io::Result<()> {
    #[cfg(not(target_arch = "wasm32"))]
    return crate::caller::create_dir(path);
    #[cfg(target_arch = "wasm32")]
    return memory::create_dir(path);
}

/// Creates the file `path`, which must not exist, for writing: natively with the mode that the
/// request's caller would give it ([`crate::caller::create_new`]).
pub fn create_new(path: &Path) -> io::Result<fs::File> {
    #[cfg(not(target_arch = "wasm32"))]
    return crate::caller::create_new(path);
    #[cfg(target_arch = "wasm32")]
    return fs::File::create_new(path);
}

/// This process's ID; 0 in a browser workspace, which is no process.
pub fn process_id() -> u32 {
    #[cfg(not(target_arch = "wasm32"))]
    return std::process::id();
    #[cfg(target_arch = "wasm32")]
    return 0;
}

/// What the worker that instantiates the module supplies, in the import module `mara`.
#[cfg(target_arch = "wasm32")]
mod imports {
    #[link(wasm_import_module = "mara")]
    unsafe extern "C" {
        /// Milliseconds since the Unix epoch (`Date.now()`).
        #[link_name = "now"]
        safe fn now_ms() -> f64;
        /// Writes `length` random bytes at `bytes`.
        #[link_name = "random"]
        pub unsafe fn random(bytes: *mut u8, length: usize);
    }

    pub fn now() -> f64 {
        now_ms()
    }
}
