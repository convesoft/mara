---
title: Tests run with nextest
status: done
type: task
parent: mara-c8scn7
blocked_by:
  - mara-1avrq4
related:
  - mara-7wysb0
discovered_from: mara-7wysb0
created_at: 2026-11-22T21:55:41Z
closed_at: 2026-11-22T23:32:31Z
---

Goal: every `test` task runs its crate's tests with cargo-nextest (`testRunner = "nextest"`), which ran the full suite in 84 s against 159 s with `cargo test` on the maintainer's machine, with equal results (mara-7wysb0, `ADR-NX-RUST-TASKS`). `affectedTests` stays off: selection gained nothing on this repository.

Context: nx-rust 1.0.0-rc.2 documents the runner in `node_modules/@convesoft/nx-rust/docs/testing.mara.md` ("Options", "One test target") and `DES-TEST-RUN` there. cargo-nextest 0.9.152 is installed on the maintainer's machine; CI (`DES-CI-WORKFLOW`) and the release (`DES-RELEASE-WORKFLOW`) run `pnpm run checks` and need it installed (a pinned version, as the other tools there are).

What mara-7wysb0 found, to carry over:

- `crates/mara-engine/tests/diagnostic_memory.rs` has `harness = false` and ignores its arguments, so nextest exits with 104 ("creating test list failed ... did not end with the string `: test`"). Its `main` must answer libtest's `--list --format terse` with `diagnostic_memory: test`, print nothing when `--ignored` is also given, and run on any other arguments (https://nexte.st/docs/design/custom-test-harnesses/). With that change the run passed: 825 passed, 14 skipped.
- nextest counts 825 tests where cargo's `test result:` lines count 824: cargo prints no such line for the `harness = false` binary. The 14 ignored are the timing tests of `tests/performance.rs` that a debug build ignores.
- Output of a full run with nextest: each task of the `nx` command prints the same line for the whole run, `     Summary [  81.945s] 825 tests run: 825 passed (1 slow), 14 skipped`, then `<project>:test: nextest ran <n> tests of <project>, as its JUnit report shows.`, then the doctest step's `test result: ok. 0 passed; ...` line (no crate has doctests). Require the Nx command to exit 0. Read counts from one workspace `Summary` line and count the separate performance run once. `mara:test:performance` (a `test` option) runs `cargo nextest run --no-tests=pass -p <spec> --test performance` with the release profile (`DES-TEST-RUN`, "One test target") and prints its own `Summary` line.
- One test exceeds nextest's 60 s slow mark in debug: `mara::performance work_counters::cold_counts`; it is the critical path of the run.

- [x] `testRunner = "nextest"` for the workspace; `diagnostic_memory` answers `--list`
- [x] `pnpm run checks` passes; `mara:test:performance` still runs its one test target with the release profile, alone and never restored
- [x] `ADR-NX-RUST-TASKS` (the `test` and `mara:test:performance` rows), AGENTS.md "Local checks" and the CI and release Design items name nextest where they name the commands or tools
- [x] The pinned nextest install goes into the CI toolchain step that the release workflow already references (`DES-CI-WORKFLOW`, `DES-RELEASE-WORKFLOW`)
- [x] The orchestrator's gate before a squash merge reads nextest's `Summary` line; the closing summary gives a sample of a full run's output

Not part of this issue: `affectedTests` and the coverage baseline (`ADR-NX-RUST-TASKS`, mara-7wysb0).

## Summary of Changes

Every `test` task runs cargo-nextest, then the doctests: `testRunner = "nextest"` in `[workspace.metadata.nx]` of the root `Cargo.toml`. `affectedTests` stays off.

- `crates/mara-engine/tests/diagnostic_memory.rs` answers `--list --format terse` with `diagnostic_memory: test`, lists nothing with `--ignored`, and runs on any other arguments. With a temporary `std::mem::forget` of the diagnostics, `cargo nextest run -p mara-engine --test diagnostic_memory` failed (`FAIL`, "326264 bytes stay allocated ...", exit 100); reverted. Without the change, `mara-engine:test` fails with nextest's status 104 ("creating test list failed ... did not end with the string \": test\"").
- `.github/workflows/ci.yml`: a step "Install cargo-nextest" after Node: the prebuilt `x86_64` or `aarch64` `unknown-linux-gnu` archive of cargo-nextest 0.9.152 for `RUNNER_ARCH`, checked against a pinned SHA-256 (the x86_64 one equals nx-rust's CI; both equal the digests of the GitHub release), put on `PATH`. About 12 MB per job, no compilation. Both archives need only glibc 2.27, so the release's `ubuntu-22.04` images run them.
- `ADR-NX-RUST-TASKS`: the runner decision, the `test` and `mara:test:performance` commands, the runner's cache inputs, and why nextest counts one test more than cargo. `ADR-CI-CHECKS`: how CI installs nextest. `DES-CI-WORKFLOW` (Toolchains step) and `DES-RELEASE-WORKFLOW` (Checks job) name it. AGENTS.md "Local checks": one line.
- CI: one green run on `ubuntu-24.04` and `ubuntu-24.04-arm` of the commit that differs from this one only by the branch in the push trigger. Its run ID is in the squash commit's message.

### Cache probes (private `NX_CACHE_DIRECTORY`, `nx run-many -t test`)

1. At the tip with `cargo test`: 6 tasks run, results stored (828 passed, 14 ignored by `test result:` lines).
2. With nextest: all three `test` tasks ran, 0 of 6 restored. Under `cargo`, the `test` target has neither the `cargo nextest --version` runtime input nor `.config/nextest.toml` (`nx show project mara-engine --json`), so the runner changes the key also apart from the edit of `Cargo.toml`.
3. Again: the three `test` tasks restored.
4. `docs-checks:test` with a `cargo-nextest` on `PATH` that reports 0.9.999: ran; with the real one again: restored.
5. A comment appended to `crates/mara-engine/src/lib.rs`: `docs-checks:test`, `mara-engine:test` and `mara:test` ran; reverted: all three restored.
6. A comment appended to `docs/identity.mara.md`: only `docs-checks:test` ran.

### Checks

- `berth --exclusive -- pnpm run checks`: exit 0. First `nx` command: 15 tasks, `valid: 0 errors, 0 warnings`, `Summary [  79.824s] 829 tests run: 829 passed, 14 skipped`. `mara:test:performance`: `cargo nextest run --no-tests=pass -p 'path+file://.../crates/mara#0.4.0-alpha.5' --test performance --cargo-profile release --locked`, alone after `souffle`, `lockfile` and `mara:build`, `Cache: Skipped (--skip-nx-cache)`, `Summary [  51.763s] 32 tests run: 32 passed, 2 skipped`.
- Totals: 861 passed, 0 failed, 16 skipped. Under `cargo test` the same suite counts 860 passed and 16 ignored: cargo prints no `test result:` line for the `harness = false` binary `diagnostic_memory`.
- A second `pnpm run checks:fast`: 13 of 15 restored.
- The flaky `client::after_a_daemon_is_killed_...` (mara-g663ez) passed in all 4 full nextest runs.

## Decisions made

- `testRunner` is set in `[workspace.metadata.nx]` of the root `Cargo.toml`, not in the plugin's options in `nx.json` (`ADR-NX-RUST-TASKS`).
- CI installs nextest from its prebuilt release with a pinned SHA-256 per architecture, as nx-rust's CI does, not with `cargo install` or a third-party action (`ADR-CI-CHECKS`, `DES-CI-WORKFLOW`).
- `diagnostic_memory` follows the issue's rule for its arguments: `--list` lists, `--list --ignored` lists nothing, everything else runs.
- No `.config/nextest.toml`: the defaults ran the suite, and the plugin keys the file if one is added.
