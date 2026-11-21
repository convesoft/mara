//! The cases of ADR-DEBUG-PAUSE-POINT (`docs/authoring/writes.mara.md`) that find no trace of the
//! test hooks in a release build: the binary holds no `MARA_TEST_`, the prefix of every hook's
//! variable, and a write of DES-WRITE-PROCEDURE with `MARA_TEST_PAUSE` set completes without
//! pausing.

use std::process::{Command, Stdio};

use crate::corpus::Corpus;
use crate::{SEED, cli};

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "checks a release build: cargo test --release --package mara --test performance"
)]
fn a_release_build_has_no_pause_points() {
    if cfg!(debug_assertions) {
        panic!("the pause points are absent from a release build only");
    }
    let corpus = Corpus::generate(1, SEED);
    let pauses = tempfile::Builder::new()
        .prefix("mara-pauses-")
        .tempdir()
        .expect("create a temporary directory");
    let case = cli::cases(&corpus)
        .into_iter()
        .find(|case| case.args[..2] == ["item", "create"])
        .expect("the case of `item create`");
    let status = Command::new(env!("CARGO_BIN_EXE_mara"))
        .args(&case.args)
        .current_dir(corpus.root())
        .env_clear()
        .env("MARA_NO_DAEMON", "1")
        .env("MARA_TEST_PAUSE", pauses.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run mara");
    assert!(
        status.success(),
        "item create with MARA_TEST_PAUSE set: {status}"
    );
    assert!(
        !pauses.path().join("paused").exists(),
        "the release build paused"
    );
}

/// The one check of the hooks' absence: a release binary that held the prefix could read a
/// hook's variable, present or future.
#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "checks a release build: cargo test --release --package mara --test performance"
)]
fn a_release_binary_holds_no_test_hook() {
    if cfg!(debug_assertions) {
        panic!("the test hooks are absent from a release build only");
    }
    let binary = std::fs::read(env!("CARGO_BIN_EXE_mara")).expect("read the mara binary");
    let prefix = b"MARA_TEST_";
    let found: Vec<String> = (0..binary.len().saturating_sub(prefix.len() - 1))
        .filter(|&at| binary[at..].starts_with(prefix))
        .map(|at| {
            let name = binary[at..]
                .iter()
                .take_while(|byte| byte.is_ascii_alphanumeric() || **byte == b'_')
                .copied()
                .collect::<Vec<_>>();
            String::from_utf8_lossy(&name).into_owned()
        })
        .collect();
    assert!(
        found.is_empty(),
        "the release binary holds `MARA_TEST_`, as {found:?}"
    );
}
