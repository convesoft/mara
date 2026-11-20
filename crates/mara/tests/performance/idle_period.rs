//! The case of DES-PERFORMANCE-HARNESS and ADR-TEST-IDLE-PERIOD (`docs/daemon/index.mara.md`)
//! that finds no trace of the test idle period in a release build: a daemon started with the
//! variable set keeps the session it opened past that period, and the binary does not hold the
//! variable's name.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use crate::daemons::Daemons;

/// `mara` with `args` in a runtime directory of its own, with nothing else in its environment.
fn mara(runtime: &Path, args: &[&str], env: &[(&str, &str)]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_mara"))
        .args(args)
        .current_dir(runtime)
        .env_clear()
        .env("XDG_RUNTIME_DIR", runtime)
        .env("TMPDIR", runtime)
        .envs(env.iter().copied())
        .stdin(Stdio::null())
        .output()
        .expect("run mara");
    assert!(output.status.success(), "{args:?}: {output:?}");
    String::from_utf8(output.stdout).unwrap()
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "checks a release build: cargo test --release --package mara --test performance"
)]
fn a_release_build_has_no_test_idle_period() {
    if cfg!(debug_assertions) {
        panic!("the test idle period is absent from a release build only");
    }
    // A short path, mode 0700: a runtime directory as DES-DAEMON-LIFECYCLE requires.
    let runtime = tempfile::Builder::new()
        .prefix("mara-idle-")
        .permissions(std::fs::Permissions::from_mode(0o700))
        .tempdir()
        .expect("create a temporary directory");
    // Kills the daemon of the test, also when the test fails.
    let daemons = Daemons::new(runtime.path(), None);
    let runtime = runtime.path();
    // A project, whose session the daemon opens as it starts.
    let project = runtime.join("p");
    std::fs::create_dir_all(project.join(".mara")).unwrap();
    std::fs::write(
        project.join(".mara/config.toml"),
        "format_version = 1\n\n[documents]\nformat_version = 1\ninclude = [\"**/*.mara.md\"]\n",
    )
    .unwrap();
    let project = std::fs::canonicalize(project).unwrap();
    let started = mara(
        runtime,
        &["--project", "p", "daemon", "start"],
        &[("MARA_TEST_IDLE_SECONDS", "1")],
    );
    assert!(started.starts_with("Started the daemon"), "{started}");
    std::thread::sleep(Duration::from_secs(3));
    let status = mara(runtime, &["daemon", "status"], &[]);
    assert!(
        status.starts_with("The daemon of this build runs")
            && status.contains(&format!("  {}\n", project.display())),
        "the release daemon took the test idle period: {status}"
    );
    assert_eq!(daemons.running().len(), 1);
    mara(runtime, &["daemon", "stop"], &[]);
    assert!(daemons.running().is_empty(), "the daemon outlived `stop`");
    let binary = std::fs::read(env!("CARGO_BIN_EXE_mara")).expect("read the mara binary");
    let variable = b"MARA_TEST_IDLE_SECONDS";
    assert!(
        !binary
            .windows(variable.len())
            .any(|bytes| bytes == variable),
        "the release binary holds the name of the idle variable"
    );
}
