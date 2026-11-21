//! The sandbox of the `souffle` child process on Linux (DES-SOUFFLE-SANDBOX in
//! `docs/rules/evaluation.mara.md`): the parent builds a Landlock ruleset and a seccomp filter
//! for one evaluation, and the child enters both between fork and exec, after setting
//! no-new-privileges.

use std::fs::{File, OpenOptions};
use std::io::{self, Read as _, Seek as _, SeekFrom};
use std::os::fd::{AsRawFd as _, FromRawFd as _, OwnedFd};
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::fs::OpenOptionsExt as _;
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use mara_engine::evaluation;

/// The Landlock domain and the seccomp filter of one evaluation, before the child enters them.
pub(super) struct Sandbox {
    ruleset: OwnedFd,
    filter: Filter,
}

impl Sandbox {
    // @mara implements REQ-PROGRAMS-ONLY-COMPUTE
    /// The sandbox of Soufflé at `souffle` running in the private directory `dir`. Refuses,
    /// and the evaluation does not start, where the kernel has no Landlock or no seccomp
    /// filter, or Mara has no filter for the architecture (ADR-SANDBOX-REQUIRED in
    /// `docs/rules/decisions.mara.md`).
    pub(super) fn new(souffle: &Path, dir: &Path) -> io::Result<Sandbox> {
        let filter = filter()?;
        let abi = abi()?;
        let handled = handled(abi);
        // SAFETY: the kernel reads the struct of the given size and returns a new descriptor.
        let fd = unsafe {
            libc::syscall(
                libc::SYS_landlock_create_ruleset,
                &handled as *const RulesetAttr,
                size_of::<RulesetAttr>(),
                0u32,
            )
        };
        if fd < 0 {
            return Err(refused(
                "cannot create its Landlock ruleset",
                io::Error::last_os_error(),
            ));
        }
        // SAFETY: `landlock_create_ruleset` returned a new descriptor, owned by nothing else.
        let ruleset = unsafe { OwnedFd::from_raw_fd(fd as i32) };
        let sandbox = Sandbox { ruleset, filter };
        let fs = handled.handled_access_fs;
        sandbox.allow(dir, fs & (READ_FILE | READ_DIR))?;
        let out = dir.join(evaluation::OUTPUT_DIR);
        sandbox.allow(&out, fs & (WRITE_FILE | MAKE_REG | TRUNCATE))?;
        sandbox.allow(souffle, fs & (EXECUTE | READ_FILE))?;
        if let Some(loader) = interpreter(souffle)? {
            let loader = loader.canonicalize()?;
            sandbox.allow(&loader, fs & (EXECUTE | READ_FILE))?;
            if let Some(libraries) = loader.parent() {
                sandbox.allow(libraries, fs & READ_FILE)?;
            }
            let cache = Path::new(LOADER_CACHE);
            if cache.is_file() {
                sandbox.allow(cache, fs & READ_FILE)?;
            }
        }
        Ok(sandbox)
    }

    /// Allows `access` beneath `path`, a directory, or on `path`, a file.
    fn allow(&self, path: &Path, access: u64) -> io::Result<()> {
        let object = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_PATH)
            .open(path)
            .map_err(|error| refused(&format!("cannot open {}", path.display()), error))?;
        let rule = PathBeneathAttr {
            allowed_access: access,
            parent_fd: object.as_raw_fd(),
        };
        // SAFETY: the kernel reads the rule; both descriptors are open.
        let added = unsafe {
            libc::syscall(
                libc::SYS_landlock_add_rule,
                self.ruleset.as_raw_fd(),
                RULE_PATH_BENEATH,
                &rule as *const PathBeneathAttr,
                0u32,
            )
        };
        if added != 0 {
            let error = io::Error::last_os_error();
            return Err(refused(&format!("cannot allow {}", path.display()), error));
        }
        Ok(())
    }

    // @mara implements REQ-PROGRAMS-ONLY-COMPUTE
    /// Makes `command`'s child enter the sandbox after the other steps between fork and exec:
    /// every descriptor from 3 up marked close-on-exec, no-new-privileges, the Landlock
    /// domain, then the seccomp filter. A step that fails stops the child before `exec`, with
    /// the step in its error code ([`setup_error`]). The sandbox must live until the child has
    /// started.
    pub(super) fn apply(&self, command: &mut Command) {
        let ruleset = self.ruleset.as_raw_fd();
        let filter = self.filter;
        let pre_exec = move || {
            // The error code of `step`, with the system's error number below it; it allocates
            // nothing.
            let failed = |step: i32| {
                let errno = io::Error::last_os_error().raw_os_error().unwrap_or(0);
                Err(io::Error::from_raw_os_error(
                    ((step + 1) << STEP_SHIFT) | (errno & ERRNO_MASK),
                ))
            };
            // SAFETY: each call is one system call on the calling process, which only reads
            // the arguments it is given; the filter is a copy on the stack.
            unsafe {
                // Close-on-exec, not closed: `Command` reports a failed `exec` through a
                // close-on-exec pipe.
                let flags = libc::CLOSE_RANGE_CLOEXEC;
                if libc::syscall(libc::SYS_close_range, 3u32, u32::MAX, flags) != 0 {
                    return failed(0);
                }
                if libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0 {
                    return failed(1);
                }
                if libc::syscall(libc::SYS_landlock_restrict_self, ruleset, 0u32) != 0 {
                    return failed(2);
                }
                let mut program = filter.program;
                program[filter.own_pid].k = libc::getpid() as u32;
                let mut fprog = libc::sock_fprog {
                    len: filter.len as u16,
                    filter: program.as_mut_ptr(),
                };
                let mode = libc::SECCOMP_SET_MODE_FILTER;
                if libc::syscall(libc::SYS_seccomp, mode, 0u32, &mut fprog as *mut _) != 0 {
                    return failed(3);
                }
            }
            Ok(())
        };
        // SAFETY: the closure makes only system calls, which are async-signal-safe, and
        // allocates nothing.
        unsafe { command.pre_exec(pre_exec) };
    }
}

/// The steps of entering the sandbox, in order.
const STEPS: [&str; 4] = [
    "marking the descriptors it inherited close-on-exec",
    "setting no-new-privileges",
    "entering its Landlock domain",
    "loading its seccomp filter",
];
const STEP_SHIFT: i32 = 16;
const ERRNO_MASK: i32 = (1 << STEP_SHIFT) - 1;

/// The error of a failed start of Soufflé: when the child failed to enter the sandbox, one that
/// names the step and what the caller's confinement must permit; else `error` itself.
pub(super) fn setup_error(error: io::Error) -> io::Error {
    let Some(code) = error.raw_os_error() else {
        return error;
    };
    let Some(step) = usize::try_from((code >> STEP_SHIFT) - 1)
        .ok()
        .and_then(|step| STEPS.get(step))
    else {
        return error;
    };
    let cause = io::Error::from_raw_os_error(code & ERRNO_MASK);
    io::Error::new(
        cause.kind(),
        format!(
            "confining Soufflé failed at {step} ({cause}), so Soufflé was not started; a \
             confinement that Mara itself runs in, such as a container's seccomp profile or an \
             agent's sandbox, must permit `close_range`, no-new-privileges, Landlock and \
             seccomp filters"
        ),
    )
}

/// The fault of a Soufflé that ended because its dynamic loader could not load a shared
/// library in the sandbox, from what it printed on `stderr`; `None` for any other end. The
/// sandbox lets it read libraries only in its loader's directory.
pub(super) fn library_fault(souffle: &Path, stderr: &[u8]) -> Option<String> {
    let stderr = String::from_utf8_lossy(stderr);
    let (_, rest) = stderr.split_once("error while loading shared libraries: ")?;
    let line = rest.lines().next().unwrap_or_default();
    let library = line.split(':').next().unwrap_or(line);
    let place = match interpreter(souffle)
        .ok()
        .flatten()
        .map(|loader| loader.canonicalize())
    {
        Some(Ok(loader)) => format!(
            "only in {}, the directory of its dynamic loader {}",
            loader.parent().unwrap_or(Path::new("/")).display(),
            loader.display()
        ),
        _ => "only in its dynamic loader's directory".to_owned(),
    };
    Some(format!(
        "Soufflé at {} cannot load the shared library `{library}` ({line}): its sandbox lets \
         it read libraries {place}. Use the Soufflé that Mara ships or that \
         scripts/build-souffle.sh builds, whose libraries are all there",
        souffle.display()
    ))
}

/// An error that refuses the evaluation, naming the sandbox.
fn refused(what: &str, error: io::Error) -> io::Error {
    io::Error::new(
        error.kind(),
        format!("Mara runs Soufflé only in its sandbox, and {what}: {error}"),
    )
}

/// The highest Landlock ABI of the kernel, at most the test's cap.
fn abi() -> io::Result<i32> {
    // SAFETY: with this flag and no attribute the call only returns the version.
    let abi = unsafe {
        libc::syscall(
            libc::SYS_landlock_create_ruleset,
            std::ptr::null::<RulesetAttr>(),
            0usize,
            CREATE_RULESET_VERSION,
        )
    } as i32;
    let error = io::Error::last_os_error();
    #[cfg(test)]
    let abi = ABI_CAP
        .with(|cap| cap.get())
        .map_or(abi, |cap| abi.min(cap));
    if abi < 1 {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!(
                "Mara runs Soufflé only in a Landlock sandbox, and this kernel offers no \
                 Landlock ({error}): use Linux 5.13 or later with `landlock` among the \
                 security modules that the `lsm=` boot parameter or the kernel configuration \
                 enables"
            ),
        ));
    }
    Ok(abi)
}

/// Every access right that Landlock ABI `abi` knows: the ruleset handles them all, so each is
/// denied unless a rule allows it.
fn handled(abi: i32) -> RulesetAttr {
    let mut fs = (1 << 13) - 1;
    for (since, right) in [(2, REFER), (3, TRUNCATE), (5, IOCTL_DEV), (9, RESOLVE_UNIX)] {
        if abi >= since {
            fs |= right;
        }
    }
    let net = match abi {
        ..4 => 0,
        4..10 => NET_TCP,
        _ => NET_TCP | NET_UDP,
    };
    let scoped = if abi >= 6 { SCOPE_ALL } else { 0 };
    RulesetAttr {
        handled_access_fs: fs,
        handled_access_net: net,
        scoped,
    }
}

/// The ELF interpreter that `binary` names (`PT_INTERP`), the dynamic loader the kernel starts
/// with it; none for a static binary. Reads 64-bit little-endian ELF, the platforms Mara ships
/// for.
fn interpreter(binary: &Path) -> io::Result<Option<PathBuf>> {
    let bad = || {
        let name = binary.display();
        refused(
            "cannot read its ELF headers",
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{name} is not 64-bit ELF"),
            ),
        )
    };
    let mut file = File::open(binary)?;
    let mut header = [0u8; 64];
    file.read_exact(&mut header).map_err(|_| bad())?;
    if header[..4] != *b"\x7fELF" || header[4] != 2 || header[5] != 1 {
        return Err(bad());
    }
    let u16_at = |bytes: &[u8], at: usize| u16::from_le_bytes([bytes[at], bytes[at + 1]]);
    let u64_at = |bytes: &[u8], at: usize| {
        u64::from_le_bytes(bytes[at..at + 8].try_into().expect("8 bytes"))
    };
    let (offset, size, count) = (
        u64_at(&header, 0x20),
        usize::from(u16_at(&header, 0x36)),
        usize::from(u16_at(&header, 0x38)),
    );
    if size < 56 {
        return Err(bad());
    }
    let mut headers = vec![0u8; size * count];
    file.seek(SeekFrom::Start(offset))?;
    file.read_exact(&mut headers).map_err(|_| bad())?;
    let Some(interp) = headers
        .chunks_exact(size)
        .find(|entry| entry[..4] == PT_INTERP.to_le_bytes())
    else {
        return Ok(None);
    };
    let length = u64_at(interp, 32);
    if length > 4096 {
        return Err(bad());
    }
    let mut path = vec![0u8; length as usize];
    file.seek(SeekFrom::Start(u64_at(interp, 8)))?;
    file.read_exact(&mut path).map_err(|_| bad())?;
    let end = path
        .iter()
        .position(|&byte| byte == 0)
        .unwrap_or(path.len());
    let path = Path::new(std::ffi::OsStr::from_bytes(&path[..end]));
    Ok(Some(path.to_owned()))
}

const PT_INTERP: u32 = 3;
/// The cache of library paths that the GNU dynamic loader reads.
const LOADER_CACHE: &str = "/etc/ld.so.cache";

// Landlock's interface, from `linux/landlock.h`.
const CREATE_RULESET_VERSION: u32 = 1;
const RULE_PATH_BENEATH: libc::c_int = 1;
const EXECUTE: u64 = 1 << 0;
const WRITE_FILE: u64 = 1 << 1;
const READ_FILE: u64 = 1 << 2;
const READ_DIR: u64 = 1 << 3;
const MAKE_REG: u64 = 1 << 8;
const REFER: u64 = 1 << 13;
const TRUNCATE: u64 = 1 << 14;
const IOCTL_DEV: u64 = 1 << 15;
const RESOLVE_UNIX: u64 = 1 << 16;
const NET_TCP: u64 = 0b11;
const NET_UDP: u64 = 0b1100;
const SCOPE_ALL: u64 = 0b11;

#[repr(C)]
struct RulesetAttr {
    handled_access_fs: u64,
    handled_access_net: u64,
    scoped: u64,
}

#[repr(C, packed)]
struct PathBeneathAttr {
    allowed_access: u64,
    parent_fd: libc::c_int,
}

// The seccomp filter: a classic BPF program over `struct seccomp_data`.

/// The filter: its program, the program's length, and the instruction that the child sets to
/// its process ID.
#[derive(Clone, Copy)]
struct Filter {
    program: [libc::sock_filter; FILTER_CAPACITY],
    len: usize,
    own_pid: usize,
}

const FILTER_CAPACITY: usize = 64;

#[cfg(target_arch = "x86_64")]
mod arch {
    pub const AUDIT_ARCH: u32 = 0xC000_003E;
    /// System call numbers of the x32 ABI have this bit; the filter refuses them all.
    pub const X32_BIT: Option<u32> = Some(0x4000_0000);
    pub const LEGACY: [libc::c_long; 6] = [
        libc::SYS_chmod,
        libc::SYS_chown,
        libc::SYS_lchown,
        libc::SYS_utime,
        libc::SYS_utimes,
        libc::SYS_futimesat,
    ];
}

#[cfg(target_arch = "aarch64")]
mod arch {
    pub const AUDIT_ARCH: u32 = 0xC000_00B7;
    pub const X32_BIT: Option<u32> = None;
    pub const LEGACY: [libc::c_long; 0] = [];
}

/// System calls that reach outside the evaluation and that Landlock does not cover on every
/// ABI: sockets and socket pairs, and `io_uring`, which opens sockets past this filter; signals
/// through a pidfd; changes to the mode, owner, times or extended attributes of a file, by path
/// or by descriptor, and to its size by path; and the keyrings.
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
const DENIED: [libc::c_long; 22] = [
    libc::SYS_socket,
    libc::SYS_socketpair,
    libc::SYS_io_uring_setup,
    libc::SYS_pidfd_send_signal,
    libc::SYS_fchmod,
    libc::SYS_fchmodat,
    452, // fchmodat2
    libc::SYS_fchown,
    libc::SYS_fchownat,
    libc::SYS_utimensat,
    libc::SYS_setxattr,
    libc::SYS_lsetxattr,
    libc::SYS_fsetxattr,
    libc::SYS_removexattr,
    libc::SYS_lremovexattr,
    libc::SYS_fremovexattr,
    463, // setxattrat
    466, // removexattrat
    libc::SYS_truncate,
    libc::SYS_keyctl,
    libc::SYS_add_key,
    libc::SYS_request_key,
];

/// System calls that send a signal, allowed only to the child itself (`abort` signals itself).
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
const SELF_ONLY: [libc::c_long; 5] = [
    libc::SYS_kill,
    libc::SYS_tkill,
    libc::SYS_tgkill,
    libc::SYS_rt_sigqueueinfo,
    libc::SYS_rt_tgsigqueueinfo,
];

/// The `fcntl` commands that make the kernel signal a chosen process when I/O is possible.
const FCNTL_DENIED: [u32; 3] = [
    8,  // F_SETOWN
    10, // F_SETSIG
    15, // F_SETOWN_EX
];

/// The one `ioctl` request allowed, by which the C library asks whether a stream is a
/// terminal. Every other one fails: among them `FIOSETOWN`, `SIOCSPGRP` and `FIOASYNC`, which
/// direct signals, and `FS_IOC_SETFLAGS` and `FS_IOC_FSSETXATTR`, which change a file's
/// attributes.
const IOCTL_ALLOWED: u32 = libc::TCGETS as u32;

/// Where a jump of the filter goes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum To {
    Next,
    Fcntl,
    Ioctl,
    OwnPid,
    Deny,
    Allow,
}

/// The filter: system calls of another architecture or ABI and those of [`DENIED`] fail with
/// `EPERM`; so do those of [`SELF_ONLY`] unless their first argument is the child's process ID,
/// `fcntl` with a command of [`FCNTL_DENIED`] and `ioctl` with any request but
/// [`IOCTL_ALLOWED`].
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
fn filter() -> io::Result<Filter> {
    const NR: u32 = 0;
    const ARCH: u32 = 4;
    const ARG0: u32 = 16;
    const ARG1: u32 = 24;
    let mut code: Vec<(libc::sock_filter, To, To)> = Vec::new();
    let mut labels: Vec<(To, usize)> = Vec::new();
    let op = |code: u32, k: u32| libc::sock_filter {
        code: code as u16,
        jt: 0,
        jf: 0,
        k,
    };
    let load = libc::BPF_LD | libc::BPF_W | libc::BPF_ABS;
    let jeq = libc::BPF_JMP | libc::BPF_JEQ | libc::BPF_K;
    let ret = libc::BPF_RET | libc::BPF_K;
    let deny = libc::SECCOMP_RET_ERRNO | libc::EPERM as u32;
    code.push((op(load, ARCH), To::Next, To::Next));
    code.push((op(jeq, arch::AUDIT_ARCH), To::Next, To::Deny));
    code.push((op(load, NR), To::Next, To::Next));
    if let Some(bit) = arch::X32_BIT {
        let jge = libc::BPF_JMP | libc::BPF_JGE | libc::BPF_K;
        code.push((op(jge, bit), To::Deny, To::Next));
    }
    for &number in DENIED.iter().chain(&arch::LEGACY) {
        code.push((op(jeq, number as u32), To::Deny, To::Next));
    }
    for &number in &SELF_ONLY {
        code.push((op(jeq, number as u32), To::OwnPid, To::Next));
    }
    code.push((op(jeq, libc::SYS_fcntl as u32), To::Fcntl, To::Next));
    code.push((op(jeq, libc::SYS_ioctl as u32), To::Ioctl, To::Next));
    code.push((op(ret, libc::SECCOMP_RET_ALLOW), To::Next, To::Next));
    labels.push((To::Fcntl, code.len()));
    code.push((op(load, ARG1), To::Next, To::Next));
    for command in FCNTL_DENIED {
        code.push((op(jeq, command), To::Deny, To::Next));
    }
    code.push((op(ret, libc::SECCOMP_RET_ALLOW), To::Next, To::Next));
    labels.push((To::Ioctl, code.len()));
    code.push((op(load, ARG1), To::Next, To::Next));
    code.push((op(jeq, IOCTL_ALLOWED), To::Allow, To::Deny));
    labels.push((To::OwnPid, code.len()));
    code.push((op(load, ARG0), To::Next, To::Next));
    let own_pid = code.len();
    // The child's process ID replaces the 0.
    code.push((op(jeq, 0), To::Allow, To::Deny));
    labels.push((To::Deny, code.len()));
    code.push((op(ret, deny), To::Next, To::Next));
    labels.push((To::Allow, code.len()));
    code.push((op(ret, libc::SECCOMP_RET_ALLOW), To::Next, To::Next));
    let offset = |from: usize, to: To| -> u8 {
        let target = match to {
            To::Next => from + 1,
            to => {
                labels
                    .iter()
                    .find(|(label, _)| *label == to)
                    .expect("a label")
                    .1
            }
        };
        u8::try_from(target - from - 1).expect("a forward jump within 255 instructions")
    };
    let mut filter = Filter {
        program: [op(0, 0); FILTER_CAPACITY],
        len: code.len(),
        own_pid,
    };
    for (at, (mut instruction, jt, jf)) in code.into_iter().enumerate() {
        instruction.jt = offset(at, jt);
        instruction.jf = offset(at, jf);
        filter.program[at] = instruction;
    }
    Ok(filter)
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
fn filter() -> io::Result<Filter> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Mara runs Soufflé only in its sandbox, and has no seccomp filter for this architecture",
    ))
}

#[cfg(test)]
thread_local! {
    /// The highest Landlock ABI that the sandbox uses on this thread, as on an older kernel; 0
    /// as on a kernel without Landlock.
    static ABI_CAP: std::cell::Cell<Option<i32>> = const { std::cell::Cell::new(None) };
}

#[cfg(test)]
mod tests;
