---
title: Parser for the admitted program language
status: todo
type: feature
parent: mara-17tdb3
created_at: 2026-11-14T12:48:21Z
---

Goal: Mara decides which programs may run, with its own parser, before Soufflé sees them.

Context: DES-PROGRAM-PROFILE and RISK-SOUFFLE-REACHES-OUTSIDE in docs/rules-facts.mara.md; REQ-PROGRAMS-ONLY-COMPUTE in docs/rules.mara.md; ADR-PROGRAM-PROFILE in docs/decisions-rules.mara.md. Five independent reviews found ways past word lists; EVD-SOUFFLE-PROGRAM-REACH in docs/rules-evaluation.mara.md records what each left-out construct did. The reviewers' probes (about 300 cases, with a token and grammar audit of Soufflé 2.5) are on the maintainer's machine in `~/Work/local/workspace/scratch/souffle-spike/reviews/`; use them as test seeds.

The parser lives in the engine crate: it does no I/O and builds for `wasm32`.

- [ ] Grammar of the admitted language: types, relations and their qualifiers, structure directives, rules, text functions, aggregates, comments and literals as Soufflé reads them
- [ ] Type resolution: only `symbol`, `number`, subtypes, unions and records; boundary relations hold only symbols and numbers
- [ ] A program outside the language is refused like a compile error, with file and line
- [ ] Tests: every left-out construct of DES-PROGRAM-PROFILE is refused, in each spelling the reviews found (`#line` at the start of a line, after other text and after a block comment; the `output` qualifier; float and unsigned literals; `as`; `magic`; subsumption; `match`; `/`; `%`; `^`), and so is an integer literal outside 32 bits, on which the WASM build aborts
- [ ] Tests, typing: `min` and `max` aggregates over symbols are refused, as Soufflé refuses them; the variadic `min(...)` and `max(...)` functions and `count` over symbol tuples are admitted
- [ ] Tests: every program the parser admits from the probe set compiles in Soufflé, and the trial programs rewritten in the language are admitted
