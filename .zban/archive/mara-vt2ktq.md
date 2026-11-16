---
title: Build the pinned Soufflé with a script
status: done
type: task
parent: mara-17tdb3
created_at: 2026-11-13T22:10:06Z
closed_at: 2026-11-16T12:54:02Z
---

Context: DES-SOUFFLE-BUILD in docs/distribution.mara.md; build findings in EVD-SOUFFLE-TRIAL. The CI builds and Linux arm64 wait for mara-4dr2p2; macOS waits for mara-ag4ees.

- [x] Build script for the pinned Soufflé: native, with a static C++ runtime and without OpenMP, for the local platform
- [x] The binary is available to tests and packaging

## Summary of Changes

- `scripts/build-souffle.sh` downloads Soufflé 2.5 from its GitHub tag archive and verifies its SHA-256 before it builds anything. It builds natively with CMake (Ninja when present), `-static-libstdc++ -static-libgcc -s`, OpenMP, libffi, SQLite, zlib and curses off, and `SOUFFLE_GIT=OFF` with `PACKAGE_VERSION=2.5`. These are the trial's settings; the Emscripten patch to `Index.h` is not applied, because the native build does not need it.
- The script fails when `readelf -d` shows a shared C++ runtime or OpenMP. It also fails when a smoke run is wrong: `souffle --no-preprocessor -l "" wrapper.dl` with `env -i PATH=/nonexistent`, JSON input and output, and a tab in a value, compared byte for byte.
- Output: `target/souffle-2.5/souffle`, with `LICENSE` beside it. A rerun only re-checks the existing binary. The work tree in `target/souffle-2.5/work/` is removed after a successful build.
- DES-SOUFFLE-BUILD records the script, the path, and the rule that a test that needs the binary and does not find it fails with a message that names the script.

Checks: the build through berth succeeded (15 MB stripped, NEEDED only libc, libm and ld-linux, `--version` prints 2.5) and a rerun passes in 0.01 s. The linkage check rejects the trial's dynamic binary (libgomp, libstdc++, libgcc_s). All seven local checks of AGENTS.md exit 0.

## Decisions made

- The binary lives at `target/souffle-<version>/souffle` in the workspace, with a fixed path and no environment override. A new version gets a new directory, so a stale binary is never reused. Rejected: `$TMPDIR`, which is cleaned after 7 days and is specific to one machine; an override variable, which no consumer needs yet.
- The source is GitHub's tag archive `archive/refs/tags/2.5.tar.gz`, pinned by SHA-256 `5d009ad6…385067`. Its `src/` is identical to the trial's tree apart from the Emscripten patch.
- `PACKAGE_VERSION=2.5` is passed so that `souffle --version` names the release. The trial's binary printed an empty version.
- The smoke run is part of the script, not a cargo test, so the local checks keep working where the script has not run.
