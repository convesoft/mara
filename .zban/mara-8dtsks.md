---
title: A stop at an evaluation limit still leaves a crash record where the core pattern is a socket
status: todo
type: bug
priority: deferred
tags:
  - deferred
  - backlog
related:
  - mara-ptq6xc
discovered_from: mara-2dnzjc
created_at: 2026-11-16T20:48:19Z
---

Goal: an evaluation stopped at its memory or output limit, and the `mara` process that the interrupted-write tests kill by `SIGXFSZ`, leave no crash record also where `kernel.core_pattern` names a socket.

Context: mara-2dnzjc gives the Soufflé child, and the tests' `mara` child, a soft core limit of 1 byte where the hard limit allows it (ADR-LIMIT-STOPS-NOT-CRASHES in `docs/rules/decisions.mara.md`, DES-RULE-EXECUTION in `docs/rules/evaluation.mara.md`). The kernel checks that limit only for a piped `core_pattern`. A pattern that names a socket (`@@…`, systemd's kernel socket handler on hosts that enable it) ignores it, and systemd-coredump still writes the journal entry, without the core. The two stops are Soufflé's `abort()` after `std::bad_alloc` at `RLIMIT_AS` and `SIGXFSZ` at `RLIMIT_FSIZE`. A piped handler under an inherited hard core limit of 0 also keeps the record; the options below would cover that case too. The mechanism stays inside `limit()` and `spawn_and_wait` in `crates/mara/src/rules.rs`, which mara-ptq6xc (the Linux sandbox) may replace.

Options, not chosen:
- Ignore `SIGXFSZ` in the child and read the output limit from the file sizes: an ignored signal survives `exec`, the write fails with `EFBIG`, and a file whose size reached `output_bytes` means `Limit::Output`.
- Make the Soufflé child non-dumpable from inside: a small patch that `scripts/build-souffle.sh` applies, since `PR_SET_DUMPABLE` set before `exec` is reset.
- A cgroup memory limit (`memory.max`): the kernel then ends Soufflé with SIGKILL, which is never recorded; it needs a delegated cgroup.
- For the tests: stop `mara` in the "Killed" cases of VER-INTERRUPTED-EDITS with a signal that does not dump core.

- [ ] Choose an option for each stop, record it in ADR-LIMIT-STOPS-NOT-CRASHES and DES-RULE-EXECUTION, and implement it
- [ ] Check on a host whose `core_pattern` names a socket: `cargo test` followed by `coredumpctl list --since <start of run>` shows no new `souffle` or `mara` entry
