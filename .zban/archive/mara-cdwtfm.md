---
title: "The local checks run only through Nx: AGENTS.md drops the list of cargo commands"
status: done
type: task
related:
  - mara-c4y5ts
created_at: 2026-11-22T11:57:43Z
closed_at: 2026-11-22T12:30:52Z
---

Goal: `pnpm run checks` is the one way the local checks run. AGENTS.md no longer lists the seven cargo and `zban` commands as a second way.

Context: the Nx workspace epic (mara-1fab1q, closed) added the Nx targets beside the cargo commands, which AGENTS.md kept "as what the targets run and as the fallback". The orchestrator's gate ran the checks both ways on mara-a4ga3m and mara-jmt2hr and got the same results each time (855 passed, 0 failed, 16 ignored; `project validate` 0 errors and 1 warning). Decided by the maintainer on 2026-11-22: "we don't need them since they duplicate what nx runs". Cargo stays the build tool, and a plain cargo command still works; it is only no longer documented as a local check. The decision is recorded in ADR-NX-RUST-TASKS (`docs/decisions.mara.md`), which this issue changes.

- [x] AGENTS.md "Local checks": `pnpm run checks` and `pnpm run checks:fast` are the local checks. The code block of seven commands, the sentence that introduces it and the words "Without Nx" are gone. What stays: `pnpm install --frozen-lockfile` once in a fresh worktree; `code add rust` once on each machine; that the checks build Soufflé when it is missing; what `rustup` installs; how to run Nx on this machine; that the `wasm32` build is an architecture check of VER-HOST-COMPARISON and the release test times a release build for VER-SCALING, each named by its target. The file stays within 120 lines, not counting the managed Nx block
- [x] ADR-NX-RUST-TASKS records the maintainer's decision of 2026-11-22 (the local checks run only through Nx; rejected: keeping the cargo commands in AGENTS.md as a second way, which duplicates the targets), and no bullet of the item still calls the cargo commands a fallback or says AGENTS.md keeps them. The item names the command of every target that is a local check, the inferred ones included, so that what a check runs is written in one place
- [x] No other page, README or script presents the seven commands as the local checks (`grep` for them in `README.md`, `docs/`, `scripts/`, `.zban/` open issues); a mention that is a target's own command, or the instruction to run the worktree's `mara` binary, stays
- [x] `pnpm run checks` passes at the final tree, and `project validate` reports 0 errors

## Summary of Changes

- AGENTS.md "Local checks": `pnpm run checks` (and `checks:fast` while working) are the local checks; the seven-command block, its introduction and "Without Nx" are gone. The setup steps (pnpm install, `code add rust`, Soufflé built when missing, rustup, Nx on this machine) and the VER-HOST-COMPARISON / VER-SCALING notes stay, the latter named by target. 99 lines without the managed Nx block.
- ADR-NX-RUST-TASKS: records the maintainer's decision of 2026-11-22 with its rejected alternative; the first bullet no longer says the cargo commands of AGENTS.md keep working; the table now also names the commands of the inferred `fmt-check`, `lint` (warnings denied by `.env.lint`) and `test` targets, as nx-rust prints them with `--verbose`; the locked-mode bullet no longer refers to the cargo commands.
- `.env.lint`: its comment no longer calls `--config 'build.warnings="deny"'` "the local check's"; the variable is unchanged.
- grep of `README.md`, `docs/`, `scripts/` and open issues: no other place presents the seven commands as the local checks.

Checks: `pnpm install --frozen-lockfile`; `berth --exclusive -- pnpm run checks` on an empty private `NX_CACHE_DIRECTORY`: exit 0, 855 passed, 0 failed, 16 ignored; `project validate` 0 errors, 1 warning.

## Decisions made

- The table lists what `checks` runs with the default Cargo configuration, as nx-rust prints it with `--verbose`: the plugin batches the work of `fmt-check`, `lint` and `test` across crates, `test` runs three cargo commands (`--no-run`, `--tests` with the plugin's test runner as the host's `runner`, `--doc`), and the `wasm32` row gives its `cargo build` with the package name in place of the long package ID the plugin prints.
- The stale comment in `.env.lint` was fixed in this branch as part of the same defect (a comment only, no target or value changed).
