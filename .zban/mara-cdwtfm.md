---
title: "The local checks run only through Nx: AGENTS.md drops the list of cargo commands"
status: in-progress
type: task
related:
  - mara-c4y5ts
created_at: 2026-11-22T11:57:43Z
---

Goal: `pnpm run checks` is the one way the local checks run. AGENTS.md no longer lists the seven cargo and `zban` commands as a second way.

Context: the Nx workspace epic (mara-1fab1q, closed) added the Nx targets beside the cargo commands, which AGENTS.md kept "as what the targets run and as the fallback". The orchestrator's gate ran the checks both ways on mara-a4ga3m and mara-jmt2hr and got the same results each time (855 passed, 0 failed, 16 ignored; `project validate` 0 errors and 1 warning). Decided by the maintainer on 2026-11-22: "we don't need them since they duplicate what nx runs". Cargo stays the build tool, and a plain cargo command still works; it is only no longer documented as a local check. The decision is recorded in ADR-NX-RUST-TASKS (`docs/decisions.mara.md`), which this issue changes.

- [ ] AGENTS.md "Local checks": `pnpm run checks` and `pnpm run checks:fast` are the local checks. The code block of seven commands, the sentence that introduces it and the words "Without Nx" are gone. What stays: `pnpm install --frozen-lockfile` once in a fresh worktree; `code add rust` once on each machine; that the checks build Soufflé when it is missing; what `rustup` installs; how to run Nx on this machine; that the `wasm32` build is an architecture check of VER-HOST-COMPARISON and the release test times a release build for VER-SCALING, each named by its target. The file stays within 120 lines, not counting the managed Nx block
- [ ] ADR-NX-RUST-TASKS records the maintainer's decision of 2026-11-22 (the local checks run only through Nx; rejected: keeping the cargo commands in AGENTS.md as a second way, which duplicates the targets), and no bullet of the item still calls the cargo commands a fallback or says AGENTS.md keeps them. The item names the command of every target that is a local check, the inferred ones included, so that what a check runs is written in one place
- [ ] No other page, README or script presents the seven commands as the local checks (`grep` for them in `README.md`, `docs/`, `scripts/`, `.zban/` open issues); a mention that is a target's own command, or the instruction to run the worktree's `mara` binary, stays
- [ ] `pnpm run checks` passes at the final tree, and `project validate` reports 0 errors
