---
title: "Question: Soufflé's native and WebAssembly builds number records differently in a proof"
status: todo
type: task
tags:
  - maintainer
  - question
discovered_from: mara-xsm8w6
created_at: 2026-11-25T13:04:26Z
---

Context: VER-EXPLANATIONS (docs/rules/verification.mara.md) requires, for its browser row (mara-xsm8w6), "the test cases in the browser: the same proofs". The case whose proof passes a record (`at([m, l]) :- item(m, _, _, _, _, l).`, `placed(m) :- at([m, _]), item(m, "need", _, _, _, _).`, row `placed` of NEED-ONE) gives the same proof in both builds but for the step `at(record #3)` natively and `at(record #2)` from the WebAssembly module.

Evidence (mara-xsm8w6): the same private directory and the same console input (`format json`, `explain __mara_ready()`, `explain placed("01M7D8DZ2663WP3A0300000003")`, `exit`) run by `target/souffle-2.5/souffle -t explain` and by the Emscripten module under Node print identical replies but for that number. Symbol numbers agree (`ord` of every MID is the same in both builds); a reduced program with only `at` and `placed` prints `record #3` in both. So the difference is in the order in which Soufflé 2.5 packs records in provenance mode, which differs between the builds (libstdc++ on 64 bits against libc++ on 32 bits); Mara prints Soufflé's atom as it is. No other case of VER-EXPLANATIONS, VER-FACT-ROWS, VER-PROGRAM-REACH-CASES or of the `engineering` template on the 1× corpus differs.

The browser test of mara-xsm8w6 compares that one case with the number after `record #` masked, and every other case exactly, until this is decided.

Options:
- [ ] A (recommended): accept the difference. VER-EXPLANATIONS's browser row reads "the same proofs, but for the internal number that Soufflé prints for a record in a step's atom", and EVD-SOUFFLE-PROGRAM-REACH gets the row. No output changes.
- [ ] B: Mara prints a record in a proof's atoms without its number (`at(record)`) on every host. The proofs are then the same; native output changes, and two records in one atom read alike.
- [ ] C: the browser explains no row whose proof passes a record ("cannot explain"). Loses explanations the CLI gives.

Adopted provisionally by the orchestrator on 2026-11-25 in the review of mara-xsm8w6: Option A. VER-EXPLANATIONS's browser row reads "The same proofs, except for the internal record number that Soufflé prints for a record in a step's atom", EVD-SOUFFLE-PROGRAM-REACH has the row, and the browser test masks only that number. It stays open for the maintainer; switching to Option B means printing a record without its number in `crates/mara/src/rules/explain.rs` (the proof's atoms) and dropping the mask. mara-xsm8w6 does not wait for it.
