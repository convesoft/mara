//! The daemons of a test, as VER-DAEMON-START (`docs/daemon/verification.mara.md`) identifies
//! them: processes of a binary under test whose environment holds the test's runtime directory
//! as `XDG_RUNTIME_DIR`. Dropping [`Daemons`] kills those that still run, and only those, also
//! when the test panics. Linux only: it reads `/proc`.

use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

/// The daemons of one runtime directory, killed when dropped.
pub struct Daemons {
    /// The runtime directory, as the test sets `XDG_RUNTIME_DIR`.
    base: PathBuf,
    /// A directory whose executables are copies of the binary under test, if any.
    copies: Option<PathBuf>,
}

impl Daemons {
    pub fn new(base: &Path, copies: Option<&Path>) -> Daemons {
        Daemons {
            base: base.to_owned(),
            copies: copies.map(Path::to_owned),
        }
    }

    /// The process IDs of those that run.
    pub fn running(&self) -> Vec<u32> {
        let entry = format!("XDG_RUNTIME_DIR={}", self.base.display()).into_bytes();
        let binary = Path::new(env!("CARGO_BIN_EXE_mara"));
        let Ok(processes) = fs::read_dir("/proc") else {
            return Vec::new();
        };
        let mut found = Vec::new();
        for process in processes.flatten() {
            let Some(pid) = process.file_name().to_str().and_then(|n| n.parse().ok()) else {
                continue;
            };
            let Ok(executable) = fs::read_link(process.path().join("exe")) else {
                continue;
            };
            let executable = executable.to_string_lossy();
            let executable = Path::new(executable.trim_end_matches(" (deleted)"));
            let under_test = executable == binary
                || (self.copies.as_deref()).is_some_and(|copies| executable.starts_with(copies));
            if !under_test {
                continue;
            }
            // Empty once the process has exited.
            let Ok(environment) = fs::read(process.path().join("environ")) else {
                continue;
            };
            if environment.split(|&byte| byte == 0).any(|e| e == entry) {
                found.push(pid);
            }
        }
        found
    }

    /// Kills those that run with SIGKILL, until none has run for 500 ms, at most for 10 s: a
    /// `daemon start` that a client of the test launched may show only after a first look.
    pub fn kill(&self) {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut quiet_since: Option<Instant> = None;
        loop {
            let running = self.running();
            let now = Instant::now();
            if now >= deadline {
                return;
            }
            if running.is_empty() {
                let since = *quiet_since.get_or_insert(now);
                if now - since >= Duration::from_millis(500) {
                    return;
                }
            } else {
                quiet_since = None;
            }
            for pid in running {
                // SAFETY: kill takes no pointer.
                unsafe { libc::kill(pid.cast_signed(), libc::SIGKILL) };
            }
            thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for Daemons {
    fn drop(&mut self) {
        self.kill();
    }
}
