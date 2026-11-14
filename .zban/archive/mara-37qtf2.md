---
title: Self-host this repository's documents
status: done
type: task
parent: mara-t1rffa
blocked_by:
  - mara-000f9q
created_at: 2026-11-13T22:10:06Z
closed_at: 2026-11-14T19:23:34Z
---

Goal: the built `mara` validates `docs/`, replacing hand checks.

- [x] Project configuration and the engineering schema added to this repository
- [x] `mara project validate` passes on `docs/` and joins the local checks (CI waits for mara-4dr2p2)
- [x] README status updated

## Summary of Changes

- `.mara/config.toml`: the configuration example of DES-PROJECT-CONFIGURATION as written: schema `.mara/schema.toml`, documents `docs/**/*.mara.md`. README.md, AGENTS.md, CLAUDE.md and `.zban/` are not matched; validation reads exactly the 29 documents in `docs/` (29 parses).
- `.mara/schema.toml`: DES-ENGINEERING-VOCABULARY. Statuses `draft`, `accepted`, `retired`; 11 types with their prefixes; `likelihood` and `consequence` (enum, single-valued) on risk, `aliases` (text, repeatable) on term, `methods` (enum, repeatable) on verification and validation, none required; 13 metadata relations with inverse names, per-source targets, `same_type` on `decomposes` and `supersedes`, `acyclic` on `supersedes`; `implements` from `code`. A script compared it with both tables of the vocabulary page (no difference). Every type, status, key and value `docs/` uses is declared; Scenario, Term, Validation, `retired`, `aliases`, `derives_from`, `supersedes`, `accepts`, `validates` and `exercises` are declared and unused. It equals the performance corpus schema except for descriptions.
- `docs/vocabulary.mara.md` (DES-ENGINEERING-VOCABULARY): a `Means` column gives each relation's description; `Implements` gets its own; schema descriptions are the `Use for` and `Means` texts ending with a full stop.
- AGENTS.md: a Mara documents bullet on `.mara/`; `cargo run --package mara --locked --quiet -- project validate` joins the local checks after `cargo test`.
- `crates/mara/tests/documents.rs`: no longer requires no diagnostics or an `ID` row, which the new check establishes through the real CLI; keeps the parser properties on the real documents (every anchor line outside code is an item; the lone-CR form parses alike).
- README Status: what the binary does today (validation only) and that the configuration and schema are written by hand.
- mara-sj9jfq: where the single schema source should live when the template is embedded (text only, status unchanged).

Result on `docs/`: valid, 0 errors, 0 warnings; 29 documents, 168 items, 311 edges (`--debug-stats`: 32 file reads, 29 parses, 647 check steps = 2 × 168 items + 311 edges, no `supersedes` edge).

Gating, in a copy of the working tree under `$TMPDIR` built from scratch (2.5 s to build and validate), each case run with the local-check command and restored afterwards:

- Link path `product.mara.md` → `products.mara.md` at docs/project.mara.md:15: `stale-link-path` at :15, exit 1.
- Link text `NEED-STRUCTURED-KNOWLEDGE` → `NEED-STRUCTURED` at :15: `stale-link-text` at :15, exit 1.
- Status `Accepted` → `Approved` at :14: `invalid-value` at :14, exit 1.
- One character of the MID anchor at :7 changed: `broken-link` at each of its three links (docs/agents.mara.md:39, docs/project.mara.md:75, docs/verification-documents.mara.md:59), exit 1.
- The anchor at :21 given the MID of :7: `duplicate-mid` at :21, and `broken-link` at the two links to the MID it lost, exit 1.
- A 25-character MID at :7: `incomplete-item` at :7 and the three `broken-link`s, exit 1.
- `.gitignore`: with `include = ["**/*.mara.md"]` and an invalid `target/stray.mara.md`, validation stays valid with 29 parses; without `.gitignore` it reads the stray file (30 parses, its errors reported). `git check-ignore` ignores nothing in `docs/` or `.mara/`.

Checks: the seven local checks of AGENTS.md exit 0 on the clean final commit.

## Decisions made

- The local check is `cargo run --package mara --locked --quiet -- project validate` from the repository root, placed after `cargo test`, whose debug build it reuses (0.05 s). Rejected: a test that runs the built binary on the checkout, because the AGENTS.md command would then repeat a test that `cargo test --workspace` already runs; calling `target/debug/mara` directly, which fails on a clean checkout. (AGENTS.md, Local checks)
- `crates/mara/tests/documents.rs` is folded, not deleted: its validity assertions go, and the parser properties that validation does not check stay.
- Relation descriptions live on DES-ENGINEERING-VOCABULARY (`Means` column), and type descriptions are its `Use for` text. Rejected: descriptions only in `.mara/schema.toml`, which would hold text the vocabulary page does not define.
- The include pattern is the documented `docs/**/*.mara.md`, so a docs subfolder needs no configuration change. Rejected: `docs/*.mara.md`.
- Only what the vocabulary states is a constraint: `derives_from` has no `same_type` flag (the page writes "Requirement → Requirement", not "the same type"), `decomposes` is not acyclic (only "Child → parent", as in 0.3.0), and no field is required ("exactly one Likelihood and one Consequence" is an acceptance rule of DES-TEMPLATE-ACCEPTANCE, a template program for alpha.3).
- The schema file carries no comment, so it can equal the template's file byte for byte (mara-sj9jfq).
- Nothing is configured for the acceptance invariant as a warning: it is a template program (mara-tx0fcf), and the docs record no setting for it.
