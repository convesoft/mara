---
title: Parser for the admitted program language
status: done
type: feature
parent: mara-17tdb3
created_at: 2026-11-14T12:48:21Z
closed_at: 2026-11-16T13:50:45Z
---

Goal: Mara decides which programs may run, with its own parser, before Soufflé sees them.

Context: DES-PROGRAM-PROFILE and RISK-SOUFFLE-REACHES-OUTSIDE in docs/rules-facts.mara.md; REQ-PROGRAMS-ONLY-COMPUTE in docs/rules.mara.md; ADR-PROGRAM-PROFILE in docs/decisions-rules.mara.md. Five independent reviews found ways past word lists; EVD-SOUFFLE-PROGRAM-REACH in docs/rules-evaluation.mara.md records what each left-out construct did. The reviewers' probes (about 300 cases, with a token and grammar audit of Soufflé 2.5) are on the maintainer's machine in `~/Work/local/workspace/scratch/souffle-spike/reviews/`; use them as test seeds.

The parser lives in the engine crate: it does no I/O and builds for `wasm32`.

- [x] Grammar of the admitted language: types, relations and their qualifiers, structure directives, rules, text functions, aggregates, comments and literals as Soufflé reads them
- [x] Type resolution: only `symbol`, `number`, subtypes, unions and records; boundary relations hold only symbols and numbers
- [x] A program outside the language is refused like a compile error, with file and line
- [x] The probe programs and the trial programs rewritten in the language copied into the repository as test fixtures, with the rows the probes recorded natively
- [x] Tests: the cases of VER-ADMITTED-LANGUAGE (docs/verification-programs.mara.md) pass

## Summary of Changes

- `crates/mara-engine/src/program.rs` and `program/`: Mara's parser for the admitted language. A lexer reads text as Soufflé 2.5 does with `--no-preprocessor` and refuses whatever the two could read differently; a recursive-descent grammar admits only DES-PROGRAM-PROFILE's language; the resolver loads includes (`.once`), expands components, resolves types and checks `min`/`max` aggregates, record ordering and `.plan`. `admit(path, &Files)` returns a `Program` with every relation's resolved column kinds, or a `Refusal` at path, line and column whose `diagnostic()` is a compile error's form with a caret. `Program::boundary(name)` refuses a crossing relation with a record column or `inline`; `admit` applies it to every relation of `mara.dl`. No I/O, no new dependency (`serde_json` only for tests), builds for `wasm32`.
- Fixtures in `crates/mara-engine/tests/programs/`: 281 review probes (`review4/`, `review5/`) with their program files and a `case.json` holding the wrapper's boundary relations, the inputs, the native outcome (exit, stdout, stderr, output rows; row counts for the 16x trial corpus) and the parser's reviewed verdict; the five trial programs rewritten without I/O directives (`trial/`, with the trial `mara.dl`) and the trial's compile errors (`trial-errors/`).
- Tests: unit tests for every VER-ADMITTED-LANGUAGE row and the parser's share of VER-PROGRAM-REACH-CASES; `tests/programs.rs` checks every probe's verdict, that every admitted probe compiled in its recorded native run, and the trial programs. The rows that need a Soufflé run are mara-stpjjc.
- Measured: admitting 0.85 MB and 3.5 MB programs took 46 and 179 ms (release), near-linear.

Checks: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked --config 'build.warnings="deny"'`, `cargo test --workspace --locked`, `cargo run --package mara --locked --quiet -- project validate`, `cargo test --package mara --test performance --release --locked`, `cargo build --package mara-engine --target wasm32-unknown-unknown --locked`, `zban check`: all exit 0.

## Decisions made

Recorded in DES-PROGRAM-PROFILE (docs/rules-facts.mara.md), from Soufflé behaviour observed with the spike's static binary:

- `#` only inside comments and strings (Soufflé ignores it there); comments do not nest; `///` and `/** */` only right before a declaration, fact or rule; `//!` and `/*!` refused.
- Strings on one line with the escapes `\"`, `\\`, `\'`, `\n`, `\t`, `\r`; text UTF-8, non-ASCII only in strings and comments, no control characters but tab, LF, CR.
- Integers decimal, `0x`, `0b` within signed 32 bits, 2^31 only right after a unary `-`; IP-address literals refused.
- No Soufflé keyword, built-in type name, `?` or leading `__` as a name (so `__LINE__` is refused too).
- `.include` only between items outside components, its path relative without `.`/`..`, resolved against the including file's directory (as Soufflé does first), else `mara.dl` to Mara's declarations; a file included twice starts with `.once`, which comes first in its file; a declared program at project path `mara.dl` refused.
- Aggregates: `count :` or `sum`/`min`/`max` of one value, no `;` in the body (Soufflé reports that without a location); `min`/`max` only of values the aggregate's own atoms or comparisons show to be numbers.
- `<`, `<=`, `>`, `>=` refused on records (Soufflé refuses at compile time; needed so every admitted probe compiles).
- `.plan` only after a recursive rule with one head, no `;` and no parenthesised terms, versions below its number of recursive atoms, each order a permutation of its atoms (needed so `plan_bad` is refused).
- A boundary relation is also not `inline` (Soufflé cannot read or write it).
- Components resolve names in the instance, then enclosing instances, then the program; a type parameter only in its own component's body.
- Limits: nesting 100 deep (refuses the 300-deep `deep_expression` probe); 100,000 declarations, clauses and instances after expansion.

## Rejected approaches

- Pre-scanning all bytes before parsing: a later bad character masked an earlier refusal; the lexer now stops with an error token and the parser reports whichever comes first.
- Reading dotted names anywhere a name may stand: `S >= T.` swallowed the clause's final `.`; dotted names are read only at the start of a term, as in Soufflé's grammar.

## Review

The review of `9745ce9` found no way past the parser for an excluded construct; all 126 admitted probes and the five rewritten trial programs compiled with the pinned Soufflé. Fixed on top, each with a test that fails on the reviewed code:

- A `;` nested anywhere in an aggregate body, such as `count : { (a(x); a(x)) }`, is refused at the `;`. The two `agg_disjunction` probes are now refused at their `;` instead of the aggregate.
- `min (x + 1) : a(x)` and `max (x * 2) : a(x)` are admitted: after `min (` or `max (` a `,` makes the variadic function, a `)` an aggregate value that starts with a parenthesis.
- Numeric evidence for `min` and `max` counts through groups without `;` and through record fields, also of a record variable destructured in a comparison (`b(p), p = [x, _]`).
- A qualified component argument (`B<a.T>`) is refused at its `.`, as Soufflé refuses it.
- Limits that small programs got past: inherited bodies, empty ones included, count against the 100,000 expansion limit before they are expanded; instance nesting and inheritance chains are at most 100 deep; each binary operator and each atom's argument list counts one nesting level. On the reviewed code a 21-line doubling inheritance was admitted after expanding 2^20 bodies, and 40,000-deep inheritance or instances, 200,000 operands and atoms nested 200,000 deep aborted with a stack overflow; each is now refused with a diagnostic naming its limit.
- A directive other than `.once` is read only before a space or a tab, as Soufflé reads it (`.decl` before a line break or a comment is refused).

Checks at the new head: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked --config 'build.warnings="deny"'`, `cargo test --workspace --locked`, `cargo run --package mara --locked --quiet -- project validate`, `cargo build --package mara-engine --target wasm32-unknown-unknown --locked`, `zban check`. The release performance test is run by the orchestrator.

### Decisions made in the review round

- VER-ADMITTED-LANGUAGE now asks that every admitted probe compiles (exit 0, no compile diagnostic) instead of an empty stderr: decided by the orchestrator on the maintainer's behalf, because the evidence shows the criterion mixed compiling with evaluating. Four admitted fixtures print runtime warnings when run (`review4/substr-bad`, `review5/substr_beyond`, `review5/substr_neg_start`, `review5/empty_aggs`); their recorded outcomes stay as they are, and those warnings are VER-PROGRAM-FAULTS's (mara-bx82k0).
- Directives other than `.once` need a following space or tab; `<`, `<=`, `>`, `>=` are refused on records; a component's type argument is one name; the limits above. All recorded in DES-PROGRAM-PROFILE.
- mara-stpjjc and mara-x0qc0q are folded into mara-bx82k0's checklist and cancelled. The quadratic union resolution and repeated inheritance colons are backlog issue mara-gz2qp5.

### Re-check

The re-check of `26246d5` found a regression: an aggregate's value was parsed before the depth guard, so 150 nested `sum` targets were admitted and 4,000 `sum` prefixes aborted with a stack overflow. Fixed for the class, not the path:

- The parser now counts depth at the entry of every function it can reach again before returning: `term`, `operand`, `unary` and `component`, through one wrapper (`Parser::nested`); every cycle of calls passes through one of them. Binary operators, which a loop chains, still count one level each. The guards at individual call sites are gone.
- Component expansion has one depth counter (`Resolver::descend`) for its two recursive calls, `expand` for an instance in an instance and `component_bodies` for an inherited component; instances and inheritance share it.
- Type resolution, include loading, the type kinds and the recursion components do not recurse; the clause checks recurse only over the syntax tree, which is no deeper than the parser counted.
- Tests: `refuses_150_nested_aggregate_targets` and `refuses_4000_aggregate_target_prefixes` (both fail on `26246d5`, the second by aborting); `every_nesting_construct_is_bounded` refuses each nesting construct 150 and 100,000 deep with the depth diagnostic (parentheses, groups, negation, aggregate values and bodies, `min (…)` values, records, unary minus and `bnot`, binary operators, call and atom arguments, component definitions, instances, inheritance) and resolves 50,000-type chains of subtypes, aliases and records; `programs_at_the_limits_fit_a_small_stack` runs every construct up to 150 deep on a 1 MiB thread stack in a debug build.
- The `deep_expression` probe is now refused at column 36, earlier in its nesting. DES-PROGRAM-PROFILE states the limits as levels the parser descends.
