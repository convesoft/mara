//! The case of DES-WRITE-PROCEDURE that finds no trace of the pause points in a release build
//! (ADR-DEBUG-PAUSE-POINT in `docs/writes.mara.md`). Its run of a write with the variable set
//! comes with the first write operation (mara-w0xasr).

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "checks a release build: cargo test --release --package mara --test performance"
)]
fn a_release_build_has_no_pause_points() {
    if cfg!(debug_assertions) {
        panic!("the pause points are absent from a release build only");
    }
    let binary = std::fs::read(env!("CARGO_BIN_EXE_mara")).expect("read the mara binary");
    let variable = b"MARA_TEST_PAUSE";
    assert!(
        !binary
            .windows(variable.len())
            .any(|bytes| bytes == variable),
        "the release binary holds the name of the pause variable"
    );
}
