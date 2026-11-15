//! The case of DES-WRITE-PROCEDURE that finds no trace of the pause points in a release build
//! (ADR-DEBUG-PAUSE-POINT in `docs/writes.mara.md`): a write with the variable set completes
//! without pausing, and the binary does not hold the variable's name.

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
    let binary = std::fs::read(env!("CARGO_BIN_EXE_mara")).expect("read the mara binary");
    let variable = b"MARA_TEST_PAUSE";
    assert!(
        !binary
            .windows(variable.len())
            .any(|bytes| bytes == variable),
        "the release binary holds the name of the pause variable"
    );
}
