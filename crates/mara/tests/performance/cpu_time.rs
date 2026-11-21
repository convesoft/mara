//! CPU time, user plus system, which the scaling test measures instead of wall-clock time
//! (DES-PERFORMANCE-HARNESS): it leaves out the time a process waited for a CPU that other work
//! held. Linux and macOS provide it through `clock_gettime` and `wait4`.

use std::cell::Cell;
use std::io;
use std::mem::MaybeUninit;
use std::os::unix::process::ExitStatusExt;
use std::process::{Child, ExitStatus};
use std::time::Duration;

thread_local! {
    /// The CPU time of the processes that [`wait`] reaped on this thread.
    static CHILDREN: Cell<Duration> = const { Cell::new(Duration::ZERO) };
}

/// The CPU time of the calling thread plus that of the processes [`wait`] has reaped on it. The
/// difference between two readings times what the thread did in between, its child processes
/// included.
pub fn now() -> Duration {
    let mut time = MaybeUninit::<libc::timespec>::uninit();
    // SAFETY: `clock_gettime` writes a `timespec` into `time` and returns 0, or returns -1.
    let result = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, time.as_mut_ptr()) };
    assert_eq!(result, 0, "thread CPU time: {}", io::Error::last_os_error());
    // SAFETY: initialized by the successful call above.
    let time = unsafe { time.assume_init() };
    let seconds = u64::try_from(time.tv_sec).expect("a CPU time after the epoch");
    let nanoseconds = u32::try_from(time.tv_nsec).expect("nanoseconds below one second");
    Duration::new(seconds, nanoseconds) + CHILDREN.get()
}

/// Adds `time`, CPU time spent for this thread elsewhere, such as by a daemon serving it, to
/// this thread's [`now`].
pub fn charge(time: Duration) {
    CHILDREN.set(CHILDREN.get() + time);
}

/// The CPU time of the process `pid` so far, user plus system, its exited threads included and
/// its child processes not: its process CPU clock (DES-PERFORMANCE-HARNESS).
#[cfg(target_os = "linux")]
pub fn of_process(pid: u32) -> Duration {
    let mut clock: libc::clockid_t = 0;
    let pid = libc::pid_t::try_from(pid).expect("a process ID");
    // SAFETY: `clock_getcpuclockid` writes a clock ID into `clock`, and returns 0 or an error.
    let result = unsafe { libc::clock_getcpuclockid(pid, &mut clock) };
    assert_eq!(result, 0, "the CPU clock of process {pid}: error {result}");
    let mut time = MaybeUninit::<libc::timespec>::uninit();
    // SAFETY: as in [`now`].
    let result = unsafe { libc::clock_gettime(clock, time.as_mut_ptr()) };
    assert_eq!(
        result,
        0,
        "CPU time of {pid}: {}",
        io::Error::last_os_error()
    );
    // SAFETY: initialized by the successful call above.
    let time = unsafe { time.assume_init() };
    let seconds = u64::try_from(time.tv_sec).expect("a CPU time after the epoch");
    let nanoseconds = u32::try_from(time.tv_nsec).expect("nanoseconds below one second");
    Duration::new(seconds, nanoseconds)
}

/// Waits for `child` to exit and reaps it, adds its CPU time to this thread's [`now`], and returns
/// its exit status.
pub fn wait(child: Child) -> ExitStatus {
    let pid = libc::pid_t::try_from(child.id()).expect("a process ID");
    let mut status = 0;
    let mut usage = MaybeUninit::<libc::rusage>::uninit();
    // SAFETY: `wait4` writes the exit status into `status` and the child's resource usage into
    // `usage`, and returns `pid` once it has reaped the child.
    while unsafe { libc::wait4(pid, &mut status, 0, usage.as_mut_ptr()) } != pid {
        let error = io::Error::last_os_error();
        assert_eq!(
            error.kind(),
            io::ErrorKind::Interrupted,
            "wait for mara: {error}"
        );
    }
    // SAFETY: initialized by the call that reaped the child.
    let usage = unsafe { usage.assume_init() };
    CHILDREN.set(CHILDREN.get() + duration(usage.ru_utime) + duration(usage.ru_stime));
    ExitStatus::from_raw(status)
}

fn duration(time: libc::timeval) -> Duration {
    let seconds = u64::try_from(time.tv_sec).expect("a CPU time after the epoch");
    let microseconds = u64::try_from(time.tv_usec).expect("microseconds below one second");
    Duration::from_secs(seconds) + Duration::from_micros(microseconds)
}
