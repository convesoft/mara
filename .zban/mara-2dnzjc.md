---
title: Evaluation limits and limit tests leave crash records (systemd-coredump) on Linux
status: in-progress
type: bug
priority: high
blocked_by:
  - mara-d3ea6f
related:
  - mara-ptq6xc
  - mara-17tdb3
created_at: 2026-11-16T18:12:04Z
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

- [ ] Output limit: SIGXFSZ set to `SIG_IGN` in `limit()` (`crates/mara/src/rules.rs`); `Limit::Output` detected from output file sizes (and the EFBIG path) instead of `status.signal() == SIGXFSZ`; `output_limit()` and its doc updated; Design items for the limits (DES-RULE-EXECUTION) updated to match
- [ ] Memory limit: a Design/Decision for a limit that ends Soufflé without `abort()` on Linux (cgroup `memory.max` or another option), with the fallback chosen; implementation and `a_program_past_the_memory_limit_is_stopped_before_the_time_limit` still passing
- [ ] Tests that run `mara` under `RLIMIT_FSIZE` with SIGXFSZ at its default action (`tests/convert/stops.rs`, `tests/fix.rs`, `tests/relink/interrupted.rs`): keep only the `SIG_IGN`/EFBIG variants, or otherwise stop them dumping core; decide whether the default-action behaviour still needs a test
- [ ] Check: `cargo test` (whole workspace) followed by `coredumpctl list --since <start of run>` shows no new `souffle` or `mara` entries
