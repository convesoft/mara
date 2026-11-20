---
title: Let the Soufflé sandbox load libraries outside the loader's directory
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-ptq6xc
created_at: 2026-11-20T14:24:23Z
---

Goal: a Soufflé whose shared libraries lie in more than one directory (RPATH or RUNPATH, `/opt`, Nix with nix-ld, Homebrew on Linux) runs in the Linux sandbox.

Context: DES-SOUFFLE-SANDBOX (`docs/rules/evaluation.mara.md`) lets Soufflé read files only beneath its ELF interpreter's directory, which holds every library of the Soufflé that Mara ships or that `scripts/build-souffle.sh` builds (libc, libm and the loader; DES-SOUFFLE-BUILD). Mara runs no other Soufflé, so this is needed only if a user's own Soufflé becomes a supported setup. Today such a Soufflé fails with a fault that names the library and the directory rule (test `a_library_outside_the_loader_directory_is_named` in `crates/mara/src/rules/sandbox/tests.rs`); ADR-SANDBOX-REQUIRED leaves no way around it. One way: resolve once per process, in the parent, the directories the loader really loads the binary's libraries from, and allow reading them.

- [ ] Decide whether another Soufflé is supported, and if so allow its library directories
