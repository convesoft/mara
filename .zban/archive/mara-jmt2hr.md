---
title: Every local check is an Nx target, and package.json scripts combine them
status: done
type: feature
parent: mara-1fab1q
blocked_by:
  - mara-a4ga3m
created_at: 2026-11-21T23:23:32Z
closed_at: 2026-11-22T10:31:28Z
---

Goal: the checks that nx-rust does not infer are custom Nx targets, and `package.json` scripts combine the usual checks, so that one command runs everything AGENTS.md asks before a branch is handed over.

Context: the epic mara-1fab1q; mara-a4ga3m set up the workspace and the inferred targets. The maintainer, on 2026-11-21: "all the things plugin don't handle could be still done through custom nx targets which call zban for example, and we can then combine usual checks we need in package.json scripts". The local checks of AGENTS.md and what they become:

| Local check | Under Nx |
| --- | --- |
| `cargo fmt --all --check`, clippy with warnings denied, `cargo test --workspace --locked` | the inferred targets (mara-a4ga3m) |
| `scripts/build-souffle.sh`, which the tests need | a custom target the tests depend on |
| `cargo run --package mara … project validate` | a custom target that depends on the `mara` build; it reads `docs/`, `.mara/`, every Rust file of the crates and the pinned grammar in the per-user cache |
| `cargo test --package mara --test performance --release --locked` | a custom target: the plugin has no option to choose one test binary; it times a release build, so it runs alone (no other task beside it) |
| `cargo build --package mara-engine --target wasm32-unknown-unknown --locked` | the inferred `build` with a target triple, or a custom target |
| `zban check` | a custom target |

A custom target is cached only if its inputs are declared completely; one that cannot name its inputs is not cached. A timing test is no candidate for a result restored from another machine state: say in the summary what was decided for it and why.

- [x] Each row above is an Nx target with its inputs and its dependencies on other targets, and each gives the result of the cargo or `zban` command it replaces
- [x] `package.json` has scripts that combine them: at least one that runs every local check, and the fast subset a developer runs while working
- [x] AGENTS.md "Local checks" names the script as the way to run them, and keeps the cargo commands as what the targets run and as the fallback (they stay until the maintainer says otherwise); the file stays within 120 lines, not counting the managed Nx block
- [x] ADR-NX-RUST-TASKS: the custom targets and their inputs are recorded there, and its Warnings bullet lists each rejected alternative on its own line (left over from the re-check of mara-a4ga3m)
- [x] The summary shows: a full run in a fresh worktree; a second run at the same tree and what it restored; and for one change each to a crate's source, a page under `docs/` and an issue file under `.zban/`, which targets ran again

## Summary of Changes

Every local check of AGENTS.md is an Nx target. ADR-NX-RUST-TASKS lists the targets with their inputs.

- **Extra targets** in the manifests' Nx metadata, each running the command of its local check:
  - `cargo-workspace:souffle` runs `scripts/build-souffle.sh`, with output `target/souffle-2.5`. It is not cached. `mara:test` depends on it.
  - `mara:validate` runs `cargo run --package mara --locked --quiet -- --no-daemon project validate`, cached. It depends on `mara:build` and `souffle`. Its key holds:
    - their outputs (`dependentTasksOutputFiles`);
    - `docs/`, `.mara/`, every file under `crates/` (Rust files, `.gitignore` files and any nested project's `.mara/config.toml`), and the root `.gitignore`;
    - what `cargo run` reads besides the binary: the root `.cargo/config.toml` and `.cargo/config`, `rust-toolchain.toml`, and every `CARGO_TARGET_*` variable, such as the host's runner;
    - the mode and SHA-256 of Soufflé;
    - the files of the per-user grammar cache, links followed, in the XDG folder (`XDG_CACHE_HOME`, else `~/.cache`) and in macOS's `~/Library/Caches`;
    - `TMPDIR` and the `MARA_TEST_*` hooks that the debug build reads.
  - `mara:test-performance` runs `cargo test --package mara --test performance --release --locked`. It is not cached, has `parallelism = false`, and depends on `souffle`.
  - `cargo-workspace:zban-check` runs `zban check`, cached. Its key holds `.zban/**` and the SHA-256 of the `zban` on `PATH`.
  - `mara-engine:build:wasm32` is a configuration of the inferred `build` with `targetTriple = "wasm32-unknown-unknown"`. Its cache entry is separate from the host build's.
- **Scripts** in `package.json`:
  - `checks` runs `nx run-many -t fmt-check lint test validate zban-check test-performance`, then `nx run mara-engine:build:wasm32`.
  - `checks:fast` runs `nx run-many -t fmt-check lint test validate zban-check`.
  - Both pass `--output-style=static`. Without it, Nx hides the task logs, and the test counts with them, when stdout is not a terminal.
- **AGENTS.md** "Local checks" names the scripts and keeps the cargo commands as what the targets run and as the fallback. The file has 107 lines without the managed Nx block.
- **ADR-NX-RUST-TASKS** has the table of targets and the decision on the output style. Every rejected alternative is on its own line, in the Warnings, cache and keys bullets too. Its first bullet says that the inferred targets run cargo. It also says that `validate` reads `docs/`, `.mara/` and Soufflé too, and that the limit on configuration outside the repository covers `validate`.

### Review round (b9d67e7)

The key of `mara:validate` gained the run-stage inputs of `cargo run` and every file under `crates/`. Its grammar input now follows links and also hashes the macOS folder. Checked in this worktree with `berth -- pnpm nx run mara:validate --output-style=static`:

- With `CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUNNER='/usr/bin/env --verbose'`, the task ran again and failed with `/usr/bin/env: unrecognized option '--verbose'` (exit 1), as the direct `cargo run` does. With the variable unset, it was restored.
- With `crates/mara/.mara/config.toml` created, the task ran again. After removing it, the task was restored.
- The grammar input, run with a throwaway `HOME` that holds a link `~/.cache/mara/grammars/rust` and a `~/Library/Caches/mara/grammars` folder, printed the hashes of the files of both.
- **The timing test gets the machine to itself** in two ways, with no `berth` in the repository. Within one Nx run, `parallelism = false` keeps every other task away from it (Nx's `canBeScheduled` in `tasks-schedule.js`). On the maintainer's machine, AGENTS.md says to run `pnpm run checks` under `berth --exclusive --`.

### Evidence

All runs used `NX_DAEMON=false NX_USE_LOCAL=true`, with `MARA_NO_DAEMON` and `CI` unset.

- **Full run in a fresh copy.** A `git clone` of this worktree under `$TMPDIR` with the change applied: no `target/`, empty `.nx/cache`. After `pnpm install --frozen-lockfile`, `berth --exclusive -- pnpm run checks` exited 0 in 7m28s.
  - `run-many`: "12 tasks: 12 succeeded", 0/12 hits. `souffle` downloaded and built Soufflé in 1m57s, and `mara:test` passed against it in the same run.
  - `build:wasm32`: 2/2 succeeded.
- **Second run at the same tree** (the same command, 2m9s, exit 0): 9/12 hits, then 1/2.
  - Restored: `zban-check`, both `fmt-check`, both `lint`, `mara:build`, both `test`, and `mara:validate`. `mara:test` is among them, so its key did not change when Soufflé appeared. Nx keys a task that has a `dependentTasksOutputFiles` input only after its dependencies have run (`hashTasksThatDoNotDependOnOutputsOfOtherTasks` in Nx 24.0.6), and the plugin gives `test` one.
  - Ran: `souffle`, `lockfile` and `test-performance`, which are never cached.
  - The restored results replay the counts cargo gives: 823 passed, 0 failed, 14 ignored, in 26 binaries (`mara` 599/14 in 23, `mara-engine` 224/0 in 3). Performance: 32 passed, 2 ignored.
- **Probes** with `berth -- pnpm run checks:fast`. Besides the listed targets, `souffle` and `lockfile` run every time. After each revert, 9/11 hits.

  | Change | Ran again | Revert |
  | --- | --- | --- |
  | `// probe` appended to `crates/mara/src/main.rs` | `mara:fmt-check`, `mara:lint`, `mara:build`, `mara:test`, `mara:validate` (4/11 hits); `mara-engine` and `zban-check` restored | all restored |
  | newline appended to `docs/index.mara.md` | `mara:test`, `mara:validate` (7/11) | all restored |
  | line appended to `.zban/mara-1fab1q.md` | `zban-check` (8/11) | all restored |
  | byte appended to `target/souffle-2.5/souffle` | `mara:test`, `mara:validate` (7/11) | all restored |

  Afterwards `git status` showed only this change, and `sha256sum -c` confirmed the Soufflé binary.
- **The gate, in this worktree:**
  - The seven commands of AGENTS.md each exited 0. `cargo test --workspace --locked`: 823 passed, 0 failed, 14 ignored, in 26 binaries. Performance: 32 passed, 2 ignored. Validate: 0 errors, 1 warning.
  - `pnpm install --frozen-lockfile` exited 0.
  - `berth --exclusive -- pnpm run checks` on an empty `NX_CACHE_DIRECTORY` exited 0, with 0/12 and 0/2 hits and the same counts.
  - `/run/user/1000/mara` was not created, and no Nx process was left running.

## Decisions made

- The extra targets live in Nx metadata of the manifests. `validate` and `test-performance` are on `mara`. `souffle` and `zban-check` are on the workspace project, in `[workspace.metadata.nx.project.targets]` of the root `Cargo.toml`. Rejected: a root `project.json`, which Nx would merge into that project and whose name would rename it.
- `souffle` is not cached. What it builds depends on the host's C++ toolchain and a download, and with the binary present it only checks it, in about a second. Rejected: caching it keyed on the script and the tool versions.
- `test-performance` is not cached, because a restored timing measures nothing, and it has `parallelism = false`.
- The `wasm32` build is a configuration of the inferred `build`, so it keeps the plugin's keys. `checks` runs it as a second `nx` command, because `run-many -c` applies a configuration to every project it selects. Rejected: an extra target with its own command, which would have to repeat the plugin's inputs.
- `validate` keeps the local check's `cargo run` command and has the outputs of `mara:build` and `souffle` in its key.
- The fast subset leaves out the performance test and the `wasm32` build.
- The scripts pass `--output-style=static`.
- `validate`'s key holds the run-stage inputs of `cargo run`, declared one by one. Nx has no input that names another target's inputs, and the plugin's named input `default` for `mara` holds only the crate's files, so neither covers the toolchain or the `CARGO_*` variables. Configuration outside the repository stays the documented limit.
- `validate` keys every file under `crates/`. Files that git ignores are left out of the key, as Mara itself leaves out ignored directories before it looks for a nested project.

Rejected along the way: the default output style, which printed no task logs when stdout was not a terminal.
