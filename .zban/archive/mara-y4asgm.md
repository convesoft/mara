---
title: "Fact bridge: mara.dl, JSON facts and request inputs"
status: done
type: feature
parent: mara-17tdb3
created_at: 2026-11-13T22:10:06Z
closed_at: 2026-11-16T14:30:54Z
---

Context: DES-FACT-BRIDGE in docs/rules-facts.mara.md; DES-REQUEST-INPUTS in docs/rules.mara.md; the generated wrapper in DES-RULE-EXECUTION (docs/rules-evaluation.mara.md); ADR-GENERIC-FACT-BRIDGE and ADR-GENERATED-JSON-IO.

- [x] `mara.dl` format version 1, with only vocabulary-free relation declarations: items, titles, revisions, fields, folded text, edges
- [x] One JSON file per fact relation, written once per document snapshot
- [x] Inputs supplied by a request, inline or from a JSON file of rows; a missing input refuses the request
- [x] Every supplied row checked before the run: column count, string for a symbol, signed 32-bit integer for a number; a wrong row refuses the request with its row and column (DES-REQUEST-INPUTS)
- [x] `source_complete` from `--complete`
- [x] Tests: the cases of VER-FACT-ROWS and VER-REQUEST-INPUTS (docs/verification-rules.mara.md) pass, natively; their browser rows wait for mara-xsm8w6

## Summary of Changes

- Engine `facts` module: `MARA_DL` (format version 1, starts with `.once`), `Snapshot::facts()` derives the rows of `item`, `title`, `revision`, `field`, `folded` and `edge` with their JSON text once per snapshot and keeps them (`OnceLock`), `Snapshot::revision` shared with `get`, `facts::fold` (lowercase as search, then NFC; new dependency `unicode-normalization`), `file_name`.
- Engine `facts::check_inputs`: listed inputs (`InputDecl` with `Column::Symbol`/`Number`) against supplied ones (`Supplied::Inline` or `Supplied::Rows(serde_json::Value)`, serde_json now also an engine dependency): missing, unlisted, repeated, `--complete` of an unlisted input, and every row's column count, string for symbol, i32 integer for number, refused with row and column. Returns the inputs as supplied plus the `source_complete` relation.
- Host `mara::rules`: `write_facts` (mara.dl and one `<relation>.json` per fact relation, input and `source_complete`) and `supplied_input` for `--input name=value` / `name=@file`.
- Tests: engine unit tests; `crates/mara/tests/facts` reads facts and inputs back through the pinned Soufflé with a test wrapper (VER-FACT-ROWS, VER-REQUEST-INPUTS below the CLI). CLI, gate outcome, MCP and `--debug-stats` cases were added to mara-bx82k0's test item.
- Docs: DES-FACT-BRIDGE (version 1 text and decisions), DES-REQUEST-INPUTS (inline values, paths, repeated and unlisted `--complete`), AGENTS.md (run `scripts/build-souffle.sh` first).

Checks: all seven local checks of AGENTS.md exit 0.

## Decisions made

- `item` holds the first `Type`, `ID`, `Status` values, an absent one as an empty symbol; status and type as schema names.
- `field` holds rows whose key the schema declares as a field of the item's type; enum values as schema names, text as written.
- `edge` holds only relation rows linking to a MID; mentions are no edges.
- `folded` = lowercase as search does, then NFC (not full Unicode case folding: `ß` stays).
- Facts JSON is rendered once per snapshot and kept with it; each run writes it into its private directory.
- An inline value is one row of one column, a decimal integer for a number column (no `+`); `@file` is relative to the command's directory.
- An input supplied twice, or `--complete` of an input the entry does not list, is refused naming it.
- Request-input checks live in the engine so every host checks alike.
