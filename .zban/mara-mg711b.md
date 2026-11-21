---
title: Diagnostic codes and paths are not leaked for the life of the process
status: in-progress
type: bug
parent: mara-0mxw21
created_at: 2026-11-20T20:22:51Z
---

Goal: a long-lived process (the daemon, `mara mcp`) gives back the memory of the strings its diagnostics name when the session that produced them is dropped.

Context: Found by the maintainer's code audit of 2026-11-20 (at `a5f8ec0`, then `a5f8ec0..c6a9706`), filed at the maintainer's request. Locations were checked at `c6a9706`. `intern` in `crates/mara-engine/src/diagnostic.rs` (line 105) turns every distinct string it is given into a `&'static str` with `Box::leak` and keeps it in a process-wide table. In a one-shot CLI call that is harmless. Since daemon sessions (mara-xqcnqw, `2af3de7`) one process serves many projects for a long time, and every path or label it ever reported stays allocated until it exits. Items: DES-DAEMON-SESSIONS and DES-IDLE-SESSIONS (docs/daemon/index.mara.md), DES-DIAGNOSTICS (docs/validation.mara.md); the engine does no I/O and builds for `wasm32` (docs/engine.mara.md).

Done when `diagnostic.rs` has no `Box::leak`, a dropped session frees the strings of its diagnostics, and the local checks of AGENTS.md pass, the `wasm32` build included.

- [ ] Diagnostics hold their strings without leaking them (`Arc<str>`, an owned `String` or `Cow<'static, str>`), with no change to any result or work count
- [ ] A test shows that the memory of a dropped session's diagnostic strings is released, or that no interning table grows with the number of distinct paths
