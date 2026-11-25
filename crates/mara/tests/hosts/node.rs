//! The setup of the Node and browser cases (VER-HOST-COMPARISON in `docs/verification.mara.md`):
//! a temporary directory for the test, the fixtures that a module compares against, and one run
//! of a module by `node --test`, after which no process that names the directory may run.
//!
//! The module gets, in its environment:
//!
//! | Variable | Value |
//! | --- | --- |
//! | `MARA_HOSTS_DIR` | The test's temporary directory |
//! | `MARA_HOSTS_FIXTURES` | Its `fixtures/`, one directory per fixture project, with `<name>.hashes.json` beside each that [`Host::hash`] wrote |
//! | `MARA_BIN` | The `mara` binary of the build, for CLI direct mode |
//! | `MARA_BROWSER_PACKAGE` | The browser package's directory, `packages/mara-browser` |
//! | `TMPDIR` | Its `tmp/`, where Playwright puts the browser's new profile |

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use tempfile::TempDir;

/// The oldest Node that the packages support (DES-PACKAGES).
const NODE_MAJOR: u32 = 24;

/// The temporary directory of one test.
pub struct Host {
    dir: TempDir,
}

impl Host {
    pub fn new() -> Host {
        let dir = tempfile::Builder::new()
            .prefix("mara-hosts-")
            .tempdir()
            .expect("create the test's temporary directory");
        for sub in ["fixtures", "tmp"] {
            fs::create_dir(dir.path().join(sub)).expect("create a directory of the test");
        }
        Host { dir }
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// The directory of the fixture project `name`.
    pub fn fixture(&self, name: &str) -> PathBuf {
        self.path().join("fixtures").join(name)
    }

    /// Writes `<name>.hashes.json` beside the fixture `name`: the BLAKE3 hash, in lowercase
    /// hexadecimal, of every file of it outside `.git`, by its path relative to its root.
    pub fn hash(&self, name: &str) {
        let root = self.fixture(name);
        let mut hashes = BTreeMap::new();
        let mut directories = vec![root.clone()];
        while let Some(directory) = directories.pop() {
            for entry in fs::read_dir(&directory).expect("list a fixture directory") {
                let path = entry.expect("read a fixture entry").path();
                if path.file_name() == Some(".git".as_ref()) {
                    continue;
                }
                if path.is_dir() {
                    directories.push(path);
                    continue;
                }
                let relative = path.strip_prefix(&root).expect("below the root");
                let bytes = fs::read(&path).expect("read a fixture file");
                hashes.insert(
                    relative.to_string_lossy().into_owned(),
                    blake3::hash(&bytes).to_hex().to_string(),
                );
            }
        }
        let text = serde_json::to_string_pretty(&hashes).expect("hashes are JSON");
        fs::write(
            self.path()
                .join("fixtures")
                .join(format!("{name}.hashes.json")),
            text,
        )
        .expect("write the hashes");
    }

    /// Runs the module `module` of `node/` with `node --test`, which must pass. Then no process
    /// whose command line or environment names the test's directory may run: any that does is
    /// sent `SIGTERM` and fails the test.
    pub fn run(&self, module: &str) {
        let tests = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/hosts/node");
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let package = fs::canonicalize(workspace.join("packages/mara-browser"))
            .expect("the browser package's directory");
        node_version();
        let output = Command::new("node")
            .arg("--test")
            .arg("--test-reporter=spec")
            .arg(tests.join(module))
            .env("MARA_HOSTS_DIR", self.path())
            .env("MARA_HOSTS_FIXTURES", self.path().join("fixtures"))
            .env("MARA_BIN", env!("CARGO_BIN_EXE_mara"))
            .env("MARA_BROWSER_PACKAGE", &package)
            .env("TMPDIR", self.path().join("tmp"))
            .stdin(Stdio::null())
            .output()
            .expect("start node");
        let printed = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        println!("{printed}");
        let left = naming(self.path());
        // They name the test's directory, so the test started them: ended, then reported.
        for (pid, _) in &left {
            // SAFETY: `kill` takes no pointer.
            unsafe { libc::kill(*pid as libc::pid_t, libc::SIGTERM) };
        }
        assert!(
            left.is_empty(),
            "processes that name {} still run after `node --test {module}`: {left:#?}",
            self.path().display()
        );
        assert!(output.status.success(), "`node --test {module}` failed");
    }
}

/// Checks that `node` is Node 24 or later, which every case needs.
fn node_version() {
    let output = Command::new("node").arg("--version").output();
    let version = match output {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).into_owned()
        }
        Ok(output) => panic!(
            "the Node and browser cases need Node {NODE_MAJOR} or later: `node --version` failed: {output:?}"
        ),
        Err(error) => {
            panic!("the Node and browser cases need Node {NODE_MAJOR} or later on PATH: {error}")
        }
    };
    let major = version.trim().trim_start_matches('v').split('.').next();
    let major: Option<u32> = major.and_then(|major| major.parse().ok());
    assert!(
        major.is_some_and(|major| major >= NODE_MAJOR),
        "the Node and browser cases need Node {NODE_MAJOR} or later, not {}",
        version.trim()
    );
}

/// The processes, other than this one, whose command line or environment holds `path`, each
/// with its command line.
#[cfg(target_os = "linux")]
fn naming(path: &Path) -> Vec<(u32, String)> {
    let needle = path.as_os_str().as_encoded_bytes();
    let own = std::process::id();
    let mut found = Vec::new();
    for entry in fs::read_dir("/proc").expect("list /proc").flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
        else {
            continue;
        };
        if pid == own {
            continue;
        }
        // A process that ended meanwhile, or another user's, holds nothing to read.
        let read = |file: &str| fs::read(entry.path().join(file)).unwrap_or_default();
        let (command, environment) = (read("cmdline"), read("environ"));
        let holds = |bytes: &[u8]| bytes.windows(needle.len()).any(|window| window == needle);
        if holds(&command) || holds(&environment) {
            let shown = String::from_utf8_lossy(&command).replace('\0', " ");
            found.push((pid, shown));
        }
    }
    found
}

/// [`naming`] where there is no `/proc`: `ps` with each process's environment.
#[cfg(not(target_os = "linux"))]
fn naming(path: &Path) -> Vec<(u32, String)> {
    let output = Command::new("ps")
        .args(["-A", "-E", "-ww", "-o", "pid=,command="])
        .output()
        .expect("run ps");
    let own = std::process::id();
    let needle = path.to_string_lossy();
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| line.contains(needle.as_ref()))
        .filter_map(|line| {
            let (pid, command) = line.trim().split_once(' ')?;
            let pid: u32 = pid.parse().ok()?;
            (pid != own).then(|| (pid, command.to_owned()))
        })
        .collect()
}
