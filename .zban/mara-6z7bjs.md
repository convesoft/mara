---
title: When CARGO_TARGET_DIR is not `target`, the Soufflé that build-souffle.sh writes is not where the binary and the Nx keys look
status: todo
type: bug
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-jmt2hr
created_at: 2026-11-22T10:31:40Z
---

Found while working on mara-jmt2hr. This is pre-existing. It matters whenever `CARGO_TARGET_DIR` names a folder other than `target`: one outside the checkout, or a relative one inside it such as `build-local`.

The places that write or read Soufflé disagree:
- `scripts/build-souffle.sh` always writes `<repo>/target/souffle-2.5/souffle`.
- `mara` looks for `../souffle-2.5/souffle` beside its own executable (`souffle()` in `crates/mara/src/rules.rs`), so it looks in `$CARGO_TARGET_DIR`.
- Some tests use the fixed path `CARGO_MANIFEST_DIR/../../target/souffle-2.5/souffle`: `rules/sandbox/tests.rs`, `tests/facts/main.rs` and `tests/programs/fixtures.rs`.
- The Nx inputs of `mara:test` and `mara:validate` hash `${CARGO_TARGET_DIR:-target}/souffle-2.5/souffle`.
- The `souffle` target's output is `target/souffle-2.5`.

As a result, with such a `CARGO_TARGET_DIR`, the CLI tests and `project validate` cannot find Soufflé after the script has run.

The binary also looks first for `souffle` beside its own executable (`crates/mara/src/rules.rs:77-89`), for example `target/debug/souffle`. Neither `mara:test` nor `mara:validate` keys that file, so a Soufflé put there changes what the checks run while Nx restores their earlier results.

- [ ] Decide one place: the script follows `CARGO_TARGET_DIR`, or the binary also looks in the checkout's `target/`. Then align the script, the tests, the Nx inputs and the `souffle` target's output with it.
