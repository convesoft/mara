//! VER-SOUFFLE-SANDBOX: a child that reaches outside its evaluation by a way past the
//! admission check is stopped by the sandbox, and the evaluation is a fault; on the kernel's
//! Landlock ABI and on ABI 1. The way is planted below the check: directives appended to the
//! generated wrapper for Soufflé, and a helper that the test builds with `cc` run in Soufflé's
//! place for arbitrary code in the child.

use std::collections::BTreeMap;
use std::fs;
use std::net::{TcpListener, UdpSocket};
use std::os::fd::AsRawFd as _;
use std::os::linux::net::SocketAddrExt as _;
use std::os::unix::fs::MetadataExt as _;
use std::os::unix::net::{SocketAddr, UnixDatagram, UnixListener};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use mara_engine::evaluation::{Fault, Limits, Rows};
use mara_engine::facts::MARA_DL;
use mara_engine::program::{Files, Program, admit};
use mara_engine::work::WorkCounts;

use super::super::{Run, evaluate, prepare, run_in};
use super::ABI_CAP;

const SECRET: &str = "SECRET-MARKER-5d1e";

/// The ABIs each case runs on: the kernel's, and the first, as on an old kernel.
const ABIS: [Option<i32>; 2] = [None, Some(1)];

/// What lies outside the evaluation: a secret, a file that must stay unchanged, and a TCP, a
/// UDP, a UNIX stream, a UNIX datagram and an abstract UNIX datagram socket that must receive
/// nothing.
struct Outside {
    root: tempfile::TempDir,
    tcp: TcpListener,
    udp: UdpSocket,
    unix: UnixListener,
    datagram: UnixDatagram,
    /// The abstract socket and its name.
    abstract_socket: (UnixDatagram, String),
    /// The kept file's bytes, mode and modification time.
    kept: (Vec<u8>, u32, SystemTime),
}

impl Outside {
    fn new() -> Outside {
        let root = tempfile::Builder::new().prefix("sb").tempdir().unwrap();
        fs::write(root.path().join("secret"), format!("{SECRET}\n")).unwrap();
        let kept = root.path().join("kept");
        fs::write(&kept, "kept\n").unwrap();
        let tcp = TcpListener::bind("127.0.0.1:0").unwrap();
        tcp.set_nonblocking(true).unwrap();
        let udp = UdpSocket::bind("127.0.0.1:0").unwrap();
        udp.set_nonblocking(true).unwrap();
        let unix = UnixListener::bind(root.path().join("s")).unwrap();
        unix.set_nonblocking(true).unwrap();
        let datagram = UnixDatagram::bind(root.path().join("d")).unwrap();
        datagram.set_nonblocking(true).unwrap();
        let name = format!("mara-{}", root.path().file_name().unwrap().display());
        let address = SocketAddr::from_abstract_name(name.as_bytes()).unwrap();
        let abstract_socket = UnixDatagram::bind_addr(&address).unwrap();
        abstract_socket.set_nonblocking(true).unwrap();
        let kept = Outside::state(&kept);
        Outside {
            root,
            tcp,
            udp,
            unix,
            datagram,
            abstract_socket: (abstract_socket, name),
            kept,
        }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.root.path().join(name)
    }

    fn state(path: &Path) -> (Vec<u8>, u32, SystemTime) {
        let metadata = fs::metadata(path).unwrap();
        (
            fs::read(path).unwrap(),
            metadata.mode(),
            metadata.modified().unwrap(),
        )
    }

    /// Requires that nothing outside was touched: no file created, the kept file unchanged,
    /// no connection or datagram received.
    #[track_caller]
    fn untouched(&self, case: &str) {
        for name in ["written", "marked"] {
            assert!(!self.path(name).exists(), "{case}: {name} exists");
        }
        assert!(
            Outside::state(&self.path("kept")) == self.kept,
            "{case}: the kept file changed"
        );
        assert!(self.tcp.accept().is_err(), "{case}: a TCP connection");
        assert!(
            self.udp.recv(&mut [0; 16]).is_err(),
            "{case}: a UDP datagram"
        );
        assert!(self.unix.accept().is_err(), "{case}: a UNIX connection");
        let mut buffer = [0; 16];
        assert!(
            self.datagram.recv(&mut buffer).is_err(),
            "{case}: a UNIX datagram"
        );
        assert!(
            self.abstract_socket.0.recv(&mut buffer).is_err(),
            "{case}: an abstract UNIX datagram"
        );
    }
}

/// An admitted program with one output relation, `r`, holding the row `1`.
fn program() -> Program {
    let programs = BTreeMap::from([("p.dl".to_owned(), b".decl r(x: number)\nr(1).\n".to_vec())]);
    let files = Files {
        declarations: MARA_DL.as_bytes(),
        programs: &programs,
    };
    admit("p.dl", &files).expect("admitted")
}

fn souffle() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/souffle-2.5/souffle")
}

fn limits() -> Limits {
    Limits::with_time(20)
}

/// Evaluates `program` with `souffle` on Landlock ABI `abi`, its private directory prepared
/// as Mara does, then `planted` appended to the wrapper.
fn evaluate_planted(souffle: &Path, abi: Option<i32>, planted: &str) -> Result<Rows, Fault> {
    let program = program();
    let run = Run {
        souffle,
        program: &program,
        relations: &[],
        outputs: &["r"],
        limits: limits(),
    };
    let dir = tempfile::Builder::new().prefix("p").tempdir().unwrap();
    prepare(dir.path(), &run, &[], &[]).unwrap();
    let wrapper = dir.path().join(mara_engine::evaluation::WRAPPER);
    let text = fs::read_to_string(&wrapper).unwrap();
    fs::write(&wrapper, format!("{text}{planted}")).unwrap();
    ABI_CAP.with(|cap| cap.set(abi));
    let result = run_in(&run, dir.path(), &mut WorkCounts::default());
    ABI_CAP.with(|cap| cap.set(None));
    result
}

/// Requires a runtime fault that does not quote the secret.
#[track_caller]
fn stopped(case: &str, result: &Result<Rows, Fault>) {
    match result {
        Err(Fault::Runtime { message, .. }) => {
            assert!(!message.contains(SECRET), "{case}: {message}")
        }
        other => panic!("{case}: not stopped as a runtime fault: {other:?}"),
    }
}

/// A normal evaluation in the sandbox returns its rows, on either ABI.
#[test]
fn a_normal_evaluation_returns_its_rows() {
    for abi in ABIS {
        let rows = evaluate_planted(&souffle(), abi, "").expect("rows");
        let r = &rows.relations["r"];
        assert_eq!(format!("{r:?}"), "[[Number(1)]]", "{abi:?}");
    }
}

/// Soufflé's own input and output directives, as a program past the check could write them.
/// A read it cannot make fails the evaluation. An output file it cannot open Soufflé 2.5
/// skips without a message, so the write is stopped and the outcome is the program's rows.
#[test]
fn planted_directives_cannot_read_or_write_outside() {
    let outside = Outside::new();
    let secret = outside.path("secret");
    let written = outside.path("written");
    let read = format!(
        ".decl s(x: symbol)\n.input s(IO=file, filename=\"{}\")\n\
         .output s(IO=file, filename=\"out/s.csv\")\n",
        secret.display()
    );
    let writes = [
        format!(
            ".decl w(x: symbol)\nw(\"{SECRET}\").\n.output w(IO=file, filename=\"{}\")\n",
            written.display()
        ),
        format!(
            ".decl w(x: symbol)\nw(\"{SECRET}\").\n\
             .output w(IO=jsonfile, format=list, filename=\"{}\")\n",
            written.display()
        ),
    ];
    for abi in ABIS {
        let case = format!("{abi:?} {read}");
        stopped(&case, &evaluate_planted(&souffle(), abi, &read));
        outside.untouched(&case);
        for planted in &writes {
            let case = format!("{abi:?} {planted}");
            let rows = evaluate_planted(&souffle(), abi, planted).expect(&case);
            assert_eq!(
                format!("{:?}", rows.relations["r"]),
                "[[Number(1)]]",
                "{case}"
            );
            outside.untouched(&case);
        }
    }
}

/// Soufflé writes its outputs only in `out/`: the planted output beside the wrapper is
/// skipped, while the same output in `out/` is written.
#[test]
fn planted_outputs_are_written_only_in_out() {
    let program = program();
    let souffle = souffle();
    let run = Run {
        souffle: &souffle,
        program: &program,
        relations: &[],
        outputs: &["r"],
        limits: limits(),
    };
    for abi in ABIS {
        let dir = tempfile::Builder::new().prefix("p").tempdir().unwrap();
        prepare(dir.path(), &run, &[], &[]).unwrap();
        let wrapper = dir.path().join(mara_engine::evaluation::WRAPPER);
        let text = fs::read_to_string(&wrapper).unwrap();
        let planted = ".decl w(x: symbol)\nw(\"x\").\n\
                       .output w(IO=file, filename=\"w.csv\")\n\
                       .output w(IO=file, filename=\"out/w.csv\")\n";
        fs::write(&wrapper, format!("{text}{planted}")).unwrap();
        ABI_CAP.with(|cap| cap.set(abi));
        let result = run_in(&run, dir.path(), &mut WorkCounts::default());
        ABI_CAP.with(|cap| cap.set(None));
        assert!(result.is_ok(), "{abi:?}: {result:?}");
        assert!(!dir.path().join("w.csv").exists(), "{abi:?}");
        assert!(dir.path().join("out/w.csv").exists(), "{abi:?}");
    }
}

/// The helper's C source: each attempt that succeeds makes it exit 0 silently, as a passing
/// evaluation; when every attempt fails it prints why and exits 1.
const HELPER: &str = r#"
#define _GNU_SOURCE
#include <arpa/inet.h>
#include <linux/fs.h>
#include <linux/sockios.h>
#include <stddef.h>
#include <sys/ioctl.h>
#include <sys/xattr.h>
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <sys/un.h>
#include <unistd.h>

static char report[4096];
static int attempts = 0, reached = 0;

static void attempt(const char *what, long result) {
    attempts++;
    if (result >= 0) {
        reached = 1;
    } else {
        size_t used = strlen(report);
        snprintf(report + used, sizeof report - used, "%s: %s\n", what, strerror(errno));
    }
}

static int inet(int type) {
    int s = socket(AF_INET, type, 0);
    attempt("open an internet socket", s);
    return s;
}

int main(int argc, char **argv) {
    if (argc > 1 && strcmp(argv[1], "mark") == 0) {
        close(open(ROOT "/marked", O_CREAT | O_WRONLY, 0644));
        return 0;
    }
#if defined(READ)
    attempt("read the secret", open(ROOT "/secret", O_RDONLY));
    attempt("list the directory", open(ROOT, O_RDONLY | O_DIRECTORY));
    attempt("read the session keyring", syscall(SYS_keyctl, 0, -3, 0));
#elif defined(WRITE)
    attempt("create a file", open(ROOT "/written", O_CREAT | O_WRONLY, 0644));
    attempt("write a file", open(ROOT "/kept", O_WRONLY | O_APPEND));
    attempt("truncate a file", truncate(ROOT "/kept", 0));
    attempt("change a file's mode", chmod(ROOT "/kept", 0600));
    attempt("change a file's time", utimensat(AT_FDCWD, ROOT "/kept", NULL, 0));
    // Through a descriptor opened for reading, on its own binary, which the sandbox lets it
    // read; each change, where allowed, keeps what is there.
    int self = open(argv[0], O_RDONLY);
    struct stat status;
    fstat(self, &status);
    attempt("change a mode through a descriptor", fchmod(self, status.st_mode & 07777));
    attempt("change an owner through a descriptor", fchown(self, status.st_uid, status.st_gid));
    attempt("set an attribute through a descriptor", fsetxattr(self, "user.mara", "x", 1, 0));
    attempt("remove an attribute through a descriptor", fremovexattr(self, "user.mara"));
    struct timespec omit[2] = {{0, UTIME_OMIT}, {0, UTIME_OMIT}};
    attempt("change the times through a descriptor", futimens(self, omit));
    int flags = 0;
    ioctl(self, FS_IOC_GETFLAGS, &flags);
    attempt("change the attribute flags through a descriptor", ioctl(self, FS_IOC_SETFLAGS, &flags));
#elif defined(EXECUTE)
    char *outside[] = {ROOT "/marker", "mark", NULL};
    attempt("execute a program", execv(outside[0], outside));
    char copy[1 << 16];
    int from = open(argv[0], O_RDONLY);
    int to = open("out/copy", O_CREAT | O_WRONLY, 0755);
    ssize_t n;
    while (from >= 0 && to >= 0 && (n = read(from, copy, sizeof copy)) > 0) write(to, copy, n);
    close(to);
    char *written[] = {"out/copy", "mark", NULL};
    attempt("execute a program it wrote", execv(written[0], written));
#elif defined(NETWORK)
    struct sockaddr_in tcp = {.sin_family = AF_INET, .sin_port = htons(TCP_PORT)};
    tcp.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    int s = inet(SOCK_STREAM);
    if (s >= 0) attempt("connect over TCP", connect(s, (struct sockaddr *)&tcp, sizeof tcp));
    struct sockaddr_in udp = tcp;
    udp.sin_port = htons(UDP_PORT);
    int d = inet(SOCK_DGRAM);
    if (d >= 0) attempt("send over UDP", sendto(d, "x", 1, 0, (struct sockaddr *)&udp, sizeof udp));
    struct sockaddr_un unix_address = {.sun_family = AF_UNIX, .sun_path = ROOT "/s"};
    int u = socket(AF_UNIX, SOCK_STREAM, 0);
    attempt("open a UNIX socket", u);
    if (u >= 0) {
        int connected = connect(u, (struct sockaddr *)&unix_address, sizeof unix_address);
        attempt("connect to a UNIX socket", connected);
    }
    struct sockaddr_un datagram = {.sun_family = AF_UNIX, .sun_path = ROOT "/d"};
    struct sockaddr_un abstract_address = {.sun_family = AF_UNIX};
    memcpy(abstract_address.sun_path + 1, ABSTRACT, strlen(ABSTRACT));
    socklen_t abstract_length = offsetof(struct sockaddr_un, sun_path) + 1 + strlen(ABSTRACT);
    int pair[2] = {-1, -1};
    attempt("open a socket pair", socketpair(AF_UNIX, SOCK_DGRAM, 0, pair));
    if (pair[0] > 0) {
        attempt("send to a UNIX socket", sendto(pair[0], "x", 1, 0, (struct sockaddr *)&datagram, sizeof datagram));
        attempt("send to an abstract socket", sendto(pair[0], "x", 1, 0, (struct sockaddr *)&abstract_address, abstract_length));
    }
#elif defined(SIGNAL)
    attempt("signal the parent", kill(getppid(), 0));
    attempt("signal every process", kill(-1, 0));
    // Signals that the kernel sends when I/O becomes possible on a descriptor.
    int pipe_ends[2];
    pid_t parent = getppid();
    if (pipe(pipe_ends) == 0) {
        int end = pipe_ends[0];
        attempt("direct a descriptor's signal to the parent", fcntl(end, F_SETOWN, parent));
        struct f_owner_ex owner = {F_OWNER_PID, parent};
        attempt("direct it with F_SETOWN_EX", fcntl(end, F_SETOWN_EX, &owner));
        attempt("choose its signal", fcntl(end, F_SETSIG, SIGWINCH));
        attempt("direct it with FIOSETOWN", ioctl(end, FIOSETOWN, &parent));
        attempt("direct it with SIOCSPGRP", ioctl(end, SIOCSPGRP, &parent));
        int on = 1;
        attempt("make the descriptor asynchronous", ioctl(end, FIOASYNC, &on));
    }
#elif defined(DESCRIPTORS)
    int inherited = 0;
    for (int fd = 3; fd < 1024; fd++) {
        if (fcntl(fd, F_GETFD) >= 0) {
            inherited++;
            char byte;
            attempt("read a descriptor Mara inherited", read(fd, &byte, 1));
        }
    }
    if (inherited == 0) {
        errno = EBADF;
        attempt("find a descriptor Mara inherited", -1);
    }
#elif defined(LIBRARY)
    int tiny(void);
    return tiny();
#endif
    if (attempts == 0 || reached) return 0;
    fputs(report, stderr);
    return 1;
}
"#;

/// Builds the helper for `mode` into the root as `helper`, and the program that the helper
/// tries to execute as `marker`, and returns the helper's path.
fn helper(outside: &Outside, mode: &str) -> PathBuf {
    helper_linked(outside, mode, &[])
}

/// [`helper`], with `link` after the source on the compiler's command line.
fn helper_linked(outside: &Outside, mode: &str, link: &[String]) -> PathBuf {
    let source = outside.path("helper.c");
    fs::write(&source, HELPER).unwrap();
    let tcp = outside.tcp.local_addr().unwrap().port();
    let udp = outside.udp.local_addr().unwrap().port();
    for (mode, name) in [(mode, "helper"), ("NOTHING", "marker")] {
        let status = Command::new("cc")
            .arg(format!("-D{mode}"))
            .arg(format!("-DROOT=\"{}\"", outside.root.path().display()))
            .arg(format!("-DTCP_PORT={tcp}"))
            .arg(format!("-DUDP_PORT={udp}"))
            .arg(format!("-DABSTRACT=\"{}\"", outside.abstract_socket.1))
            .arg("-o")
            .arg(outside.path(name))
            .arg(&source)
            .args(if name == "helper" { link } else { &[] })
            .status()
            .expect("run cc");
        assert!(status.success(), "cc failed for {mode}");
    }
    outside.path("helper")
}

/// Runs `helper` in Soufflé's place through [`evaluate`] on Landlock ABI `abi`, with no
/// output relation, and returns the outcome and the subprocess spawns.
fn evaluate_helper(helper: &Path, abi: Option<i32>) -> (Result<Rows, Fault>, u64) {
    let program = program();
    let run = Run {
        souffle: helper,
        program: &program,
        relations: &[],
        outputs: &[],
        limits: limits(),
    };
    ABI_CAP.with(|cap| cap.set(abi));
    let mut work = WorkCounts::default();
    let result = evaluate(&run, &mut work);
    ABI_CAP.with(|cap| cap.set(None));
    (result, work.subprocess_spawns)
}

/// Runs the helper for `mode` on each ABI and requires it stopped.
#[track_caller]
fn reach(mode: &str) {
    let outside = Outside::new();
    let helper = helper(&outside, mode);
    for abi in ABIS {
        let case = format!("{mode} {abi:?}");
        let (result, spawns) = evaluate_helper(&helper, abi);
        stopped(&case, &result);
        assert_eq!(spawns, 1, "{case}");
        outside.untouched(&case);
    }
}

#[test]
fn the_child_cannot_read_outside() {
    reach("READ");
}

#[test]
fn the_child_cannot_write_outside() {
    reach("WRITE");
}

#[test]
fn the_child_cannot_execute_another_program() {
    reach("EXECUTE");
}

#[test]
fn the_child_cannot_open_a_network_connection() {
    reach("NETWORK");
}

#[test]
fn the_child_cannot_signal_another_process() {
    reach("SIGNAL");
}

/// Without any attempt the helper passes, so the cases above fail only for what the sandbox
/// stops; and the program it would execute runs outside the sandbox.
#[test]
fn the_helper_without_an_attempt_passes() {
    let outside = Outside::new();
    let helper = helper(&outside, "NOTHING");
    for abi in ABIS {
        let (result, spawns) = evaluate_helper(&helper, abi);
        assert_eq!(result, Ok(Rows::default()), "{abi:?}");
        assert_eq!(spawns, 1);
    }
    let status = Command::new(outside.path("marker")).arg("mark").status();
    assert!(status.unwrap().success());
    assert!(outside.path("marked").exists());
}

/// Without Landlock the evaluation is refused before Soufflé starts.
#[test]
fn without_landlock_nothing_runs() {
    let program = program();
    let souffle = souffle();
    let run = Run {
        souffle: &souffle,
        program: &program,
        relations: &[],
        outputs: &["r"],
        limits: limits(),
    };
    ABI_CAP.with(|cap| cap.set(Some(0)));
    let mut work = WorkCounts::default();
    let result = evaluate(&run, &mut work);
    ABI_CAP.with(|cap| cap.set(None));
    match result {
        Err(Fault::Host(message)) => assert!(message.contains("no Landlock"), "{message}"),
        other => panic!("not refused: {other:?}"),
    }
    assert_eq!((work.subprocess_spawns, work.program_runs), (0, 0));
}

/// A descriptor that `mara` inherited, here one open on the secret without close-on-exec, is
/// not open in the child after `exec`.
#[test]
fn the_child_holds_no_inherited_descriptor() {
    let outside = Outside::new();
    let helper = helper(&outside, "DESCRIPTORS");
    let secret = fs::File::open(outside.path("secret")).unwrap();
    // SAFETY: clears the close-on-exec flag of a descriptor this test owns.
    assert_eq!(
        unsafe { libc::fcntl(secret.as_raw_fd(), libc::F_SETFD, 0) },
        0
    );
    for abi in ABIS {
        let case = format!("DESCRIPTORS {abi:?}");
        let (result, spawns) = evaluate_helper(&helper, abi);
        stopped(&case, &result);
        assert_eq!(spawns, 1, "{case}");
    }
    drop(secret);
}

/// A Soufflé whose shared library lies outside its loader's directory cannot load it in the
/// sandbox; the fault names the library and the directory rule.
#[test]
fn a_library_outside_the_loader_directory_is_named() {
    let outside = Outside::new();
    let lib = outside.path("lib");
    fs::create_dir(&lib).unwrap();
    fs::write(outside.path("tiny.c"), "int tiny(void) { return 0; }\n").unwrap();
    let status = Command::new("cc")
        .args(["-shared", "-fPIC", "-o"])
        .arg(lib.join("libtiny.so"))
        .arg(outside.path("tiny.c"))
        .status()
        .expect("run cc");
    assert!(status.success());
    let link = [
        format!("-L{}", lib.display()),
        "-ltiny".to_owned(),
        format!("-Wl,-rpath,{}", lib.display()),
    ];
    let helper = helper_linked(&outside, "LIBRARY", &link);
    assert!(Command::new(&helper).status().unwrap().success());
    let loader = super::interpreter(&helper).unwrap().unwrap();
    let loader = loader.canonicalize().unwrap();
    let directory = loader.parent().unwrap().display().to_string();
    for abi in ABIS {
        match evaluate_helper(&helper, abi) {
            (Err(Fault::Host(message)), 1) => {
                assert!(message.contains("`libtiny.so`"), "{message}");
                assert!(
                    message.contains(&format!("only in {directory},")),
                    "{message}"
                );
                assert!(message.contains("scripts/build-souffle.sh"), "{message}");
            }
            other => panic!("{abi:?}: not a fault that names the library: {other:?}"),
        }
    }
}

/// Installs on the calling thread, as a caller's own confinement might, a seccomp filter that
/// refuses system call `number` with `EPERM`, only when its first argument is `first` if given.
fn refuse_on_this_thread(number: libc::c_long, first: Option<u32>) {
    let op = |code: u32, k: u32, jt: u8, jf: u8| libc::sock_filter {
        code: code as u16,
        jt,
        jf,
        k,
    };
    let load = libc::BPF_LD | libc::BPF_W | libc::BPF_ABS;
    let jeq = libc::BPF_JMP | libc::BPF_JEQ | libc::BPF_K;
    let ret = libc::BPF_RET | libc::BPF_K;
    let deny = op(ret, libc::SECCOMP_RET_ERRNO | libc::EPERM as u32, 0, 0);
    let allow = op(ret, libc::SECCOMP_RET_ALLOW, 0, 0);
    let mut program = match first {
        None => vec![op(load, 0, 0, 0), op(jeq, number as u32, 0, 1), deny, allow],
        Some(value) => vec![
            op(load, 0, 0, 0),
            op(jeq, number as u32, 0, 3),
            op(load, 16, 0, 0),
            op(jeq, value, 0, 1),
            deny,
            allow,
        ],
    };
    let mut fprog = libc::sock_fprog {
        len: program.len() as u16,
        filter: program.as_mut_ptr(),
    };
    // SAFETY: both calls only read their arguments and change only the calling thread.
    unsafe {
        assert_eq!(libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0), 0);
        let mode = libc::SECCOMP_SET_MODE_FILTER;
        assert_eq!(libc::syscall(libc::SYS_seccomp, mode, 0u32, &mut fprog), 0);
    }
}

/// Where a caller's confinement refuses a step of entering the sandbox, Soufflé is not
/// started and the fault names the step and what that confinement must permit.
#[test]
fn a_refused_setup_step_is_named() {
    let steps = [
        (libc::SYS_close_range, None, "marking the descriptors"),
        (
            libc::SYS_prctl,
            Some(libc::PR_SET_NO_NEW_PRIVS as u32),
            "setting no-new-privileges",
        ),
        (
            libc::SYS_landlock_restrict_self,
            None,
            "entering its Landlock domain",
        ),
        (libc::SYS_seccomp, None, "loading its seccomp filter"),
    ];
    for (number, first, step) in steps {
        // A thread of its own: the filter stays on the thread that installs it.
        let (result, spawns) = std::thread::spawn(move || {
            refuse_on_this_thread(number, first);
            let program = program();
            let souffle = souffle();
            let run = Run {
                souffle: &souffle,
                program: &program,
                relations: &[],
                outputs: &["r"],
                limits: limits(),
            };
            let mut work = WorkCounts::default();
            (evaluate(&run, &mut work), work.subprocess_spawns)
        })
        .join()
        .unwrap();
        match result {
            Err(Fault::Host(message)) => {
                assert!(
                    message.contains(&format!("confining Soufflé failed at {step}")),
                    "{message}"
                );
                assert!(message.contains("Operation not permitted"), "{message}");
                assert!(message.contains("Soufflé was not started"), "{message}");
                assert!(message.contains("must permit"), "{message}");
            }
            other => panic!("{step}: {other:?}"),
        }
        assert_eq!(spawns, 0, "{step}");
    }
}
