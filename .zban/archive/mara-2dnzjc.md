---
title: Evaluation limits and limit tests leave crash records (systemd-coredump) on Linux
status: done
type: bug
priority: high
blocked_by:
  - mara-d3ea6f
related:
  - mara-ptq6xc
  - mara-17tdb3
created_at: 2026-11-16T18:12:04Z
closed_at: 2026-11-16T20:37:52Z
---

Filed for the maintainer on 2026-11-16, to be handled right after the documentation regrouping (mara-d3ea6f).

Goal: running Mara's tests, or hitting an evaluation limit in normal use, leaves no crash record in systemd-coredump and no desktop "Process crashed" notification. A stopped evaluation is an expected outcome, not a crash.

Context: on Linux with `kernel.core_pattern` piped to systemd-coredump (Arch/Omarchy default), each test run records two Soufflé "crashes" and several `mara` core dumps; Omarchy raises a crash notification for each. 56 Soufflé records on 2026-11-16 alone, all from `~/.t3/worktrees/mara/*`.

- `crates/mara/tests/programs/faults.rs` `a_program_past_the_memory_limit_is_stopped_before_the_time_limit`: Soufflé hits `RLIMIT_AS` (1 GiB, `limit()` in `crates/mara/src/rules.rs`), throws `std::bad_alloc`, calls `abort()` → SIGABRT record.
- `faults.rs` `an_output_past_the_size_limit_is_a_fault_without_rows`: Soufflé passes `RLIMIT_FSIZE` (64 MiB) with SIGXFSZ at its default action → SIGXFSZ record.
- `tests/convert/stops.rs`, `tests/fix.rs`, `tests/relink/interrupted.rs`: run `target/debug/mara` itself under `RLIMIT_FSIZE` with SIGXFSZ at default → `mara` cores stored (~240 KB each).

A real user whose program hits either limit gets the same crash notification, so this is a product issue too, not only test noise.

Findings, checked on kernel 7.2 (Omarchy):

- `RLIMIT_CORE = 0` (what `limit()` sets) does not help: with a piped `core_pattern` the kernel still runs systemd-coredump; it only skips storing the core. `RLIMIT_CORE = 1` does not help either (tested: record still written).
- `PR_SET_DUMPABLE` is reset by exec, so it cannot be set for Soufflé from `pre_exec`.
- An ignored signal survives exec. With SIGXFSZ set to `SIG_IGN` and `ulimit -f` small, Soufflé 2.5 exits 0, no record is written, and the output file is truncated at exactly the limit. So the output limit can be detected without the signal: an output file whose size reached `output_bytes` means `Limit::Output`.
- SIGABRT from `abort()` cannot be ignored (glibc restores the default and re-raises). Avoiding it needs a different memory-limit mechanism, e.g. a cgroup v2 `memory.max` (the kernel OOM-kills with SIGKILL, which never produces a core record), or containing it within the sandbox work of mara-ptq6xc. Decide whether `RLIMIT_AS` stays as a fallback where no cgroup is available, and whether a SIGKILL with the cgroup's OOM event is reported as `Limit::Memory`.

- [x] Output limit: no crash record for the stop by `SIGXFSZ` (revised by the worker on 2026-11-16: a core limit of 1 byte in `limit()` does this, so `SIGXFSZ` keeps its default action and `output_limit()` stays; `SIG_IGN` with size detection is a rejected alternative in ADR-LIMIT-STOPS-NOT-CRASHES); DES-RULE-EXECUTION updated
- [x] Memory limit: a Decision for a limit that ends Soufflé without a crash record on Linux, with the fallback chosen (ADR-LIMIT-STOPS-NOT-CRASHES: `RLIMIT_AS` stays, its abort is not recorded; no fallback needed); `a_program_past_the_memory_limit_is_stopped_before_the_time_limit` still passing
- [x] Tests that run `mara` under `RLIMIT_FSIZE` with SIGXFSZ at its default action (`tests/convert/stops.rs`, `tests/fix.rs`, `tests/relink/interrupted.rs`, `tests/code/edits.rs`): both variants kept, the child's `RLIMIT_CORE` set to 1 byte so the kill dumps no core and leaves no record
- [x] Check: `cargo test` (whole workspace) followed by `coredumpctl list --since <start of run>` shows no new `souffle` or `mara` entries

## Summary of Changes

- Correction to the finding above: `RLIMIT_CORE` of exactly 1 byte does stop the record. The kernel treats 1 as "start no core handler" and logs `coredump: <pid>(<comm>): RLIMIT_CORE is set to 1, aborting core`. `ulimit -c 1` in bash sets 1024 bytes, not 1, which is probably why the earlier test failed. Measured 2026-11-16 21:30 with `python3 -c` and `os.abort()`: with a limit of 1 byte there was no `coredumpctl` entry; with 0 there was one (`SIGABRT`, corefile none).
- `crates/mara/src/rules.rs`: `limit()` sets the child's `RLIMIT_CORE` to 1 byte (`CORE_LIMIT_BYTES`) instead of 0. `RLIMIT_AS`, `RLIMIT_FSIZE`, `SIGXFSZ` at default, `PR_SET_PDEATHSIG` and the interrupt handling of `spawn_and_wait` are unchanged. New unit test `the_child_gets_a_core_limit_of_one_byte` reads `/proc/self/limits` of a child set up by `limit()`.
- `tests/fix.rs`, `tests/convert/stops.rs`, `tests/relink/interrupted.rs`, `tests/code/edits.rs`: the child also gets `RLIMIT_CORE` of 1 byte. Both stops ("Killed" by `SIGXFSZ` and "Temporary write fails" with `EFBIG`) are kept as they were.
- Docs: new ADR-LIMIT-STOPS-NOT-CRASHES (`docs/rules/decisions.mara.md`, Justifies DES-RULE-EXECUTION). DES-RULE-EXECUTION now names the core limit and the kernel line. VER-INTERRUPTED-EDITS has a bullet for the core limit of its first two stops; the signal is unchanged, so its table is unchanged.

Checks, all exit 0: `cargo fmt --all --check`, clippy with warnings denied, `cargo test --workspace --locked`, `project validate` (0 errors, 0 warnings), the release performance test (19 passed), the `wasm32` engine build, `zban check`. Crash check: the whole-workspace `cargo test` started at 2026-11-16 21:33:08, and `coredumpctl list --since "2026-11-16 21:33:08"` then printed `No coredumps found.`.

## Decisions made

- The mechanism for both limits is a core limit of 1 byte (ADR-LIMIT-STOPS-NOT-CRASHES). `RLIMIT_AS` and `RLIMIT_FSIZE` stay as the limits, so no limit was loosened and no fallback is needed: the kernel has this check on every Linux, containers included. Rejected: a cgroup `memory.max` through `systemd-run --user --scope`, a cgroup that Mara creates itself, polling the child's memory, patching Soufflé, and `SIGXFSZ` ignored with size detection.
- The interrupted-write tests keep the default-action ("Killed") case and only add the core limit.

Rejected approaches: `systemd-run --user --scope -p MemoryMax=1G -p MemorySwapMax=0` works in this session (OOM kill by SIGKILL, about 4 ms each), but it needs `XDG_RUNTIME_DIR` and adds `INVOCATION_ID` to Soufflé's environment. The programs test harness also clears the environment, and the tests would use the user's systemd manager.

## Fixes after review

- Fixed: an inherited hard core limit of 0 made every evaluation fail with "cannot run Soufflé: Operation not permitted". Raising a hard limit of 0 needs privilege. `limit()` now sets only the soft limit to 1 byte, and only where the hard limit allows it. It never changes the hard limit and ignores any failure. The four test helpers that stop `mara` by `SIGXFSZ` follow the same rule. New test `faults::a_hard_core_limit_of_0_changes_no_result` runs `project validate` under `RLIMIT_CORE` 0:0 and expects the normal result. Against the previous `limit()` it fails with exactly that message. The unit test is now `the_child_gets_a_soft_core_limit_of_one_byte` and checks that the hard limit is unchanged.
- Corrected: the claim above that the 1-byte limit stops the record on every Linux and that no fallback is needed is wrong. The kernel checks the limit only for a piped `core_pattern`, and a file pattern writes nothing at 1 byte. A record remains where the pattern names a socket (`@@…`, systemd's socket handler), and with a piped handler when the inherited hard limit is 0. ADR-LIMIT-STOPS-NOT-CRASHES, the comments in `rules.rs` and DES-RULE-EXECUTION now say exactly this; DES-RULE-EXECUTION also says what a user on such a host sees. The rejected cgroup alternative now says that systemd or an administrator can delegate a cgroup. The rejected `SIGXFSZ`-ignored alternative now says it is one way to cover socket handlers.
- Filed mara-8dtsks (backlog, related mara-ptq6xc) for those remaining cases. It lists the options without choosing: ignore `SIGXFSZ` and read the output limit from file sizes; a Soufflé patch that makes it non-dumpable; a cgroup memory limit; and for the tests, a signal that does not dump core.
