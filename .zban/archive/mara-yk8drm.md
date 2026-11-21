---
title: Fill the shipped grammar catalog with Rust and Python
status: done
type: task
parent: mara-779tms
created_at: 2026-11-21T13:47:07Z
closed_at: 2026-11-21T17:36:20Z
---

Goal: `mara code add <language>` works without `--repository` for the languages the maintainer chooses, because the built-in catalog names where each grammar is fetched from and its checksum.

Context: mara-h0hxa3 (integrated as `10f36ad`) built `mara code add` and the catalog mechanism and tested them with a catalog the tests supply. The shipped catalog, `crates/mara/src/cli/code/catalog.toml`, is empty: nothing is published for it, and what it names is the maintainer's to decide. Until then `mara code add <language>` without `--repository` is refused with `unknown-grammar`, naming the option to use. docs/code/index.mara.md has the matter under `## Open questions`; DES-GRAMMAR-ASSETS says what an entry holds (a version, the URL of the `.wasm`, the URL of its `queries/tags.scm`, the checksum, the file extensions and the comment prefixes); ADR-GRAMMAR-TRUST-ON-FIRST-USE says catalog entries carry their checksums.

Recommended by the worker and the orchestrator: rust 0.24.2 and python 0.25.0, the releases the test fixtures already use (the checksum of rust 0.24.2 is the one in the docs' example), with the `.wasm` from the tree-sitter release assets and `tags.scm` from the same tag of the repository, once the maintainer confirms the URLs and that the `tags.scm` at the tag equals the fixture's copy. An ignored test in `crates/mara/tests/code/add.rs` adds each catalog language from its real repository; run it once by hand when the entries are in.

- [x] The maintainer names the languages, versions and URLs
- [x] The entries are added with their checksums, the ignored test passes once against the real repositories, and the bullet under `## Open questions` in docs/code/index.mara.md is removed

## The maintainer's answer (2026-11-21)

Asked by the orchestrator to choose between Rust and Python, an empty catalog for 0.4.0, and naming the entries themselves, the maintainer chose: rust 0.24.2 and python 0.25.0, the releases the test fixtures already use, with the `.wasm` from the tree-sitter GitHub release assets and `tags.scm` from the same tag of the repository. A worker adds the entries with their checksums, checks that the `tags.scm` at each tag equals the fixture's copy, and runs the ignored real-repository test once by hand; that one run is the only time a worker of this project fetches from the internet, and only those two releases. If a release asset or a `tags.scm` differs from the fixture, the worker stops and reports instead of pinning what it found. The issue is no longer parked on the maintainer and belongs to the alpha.5 slice.

## Summary of Changes

- `crates/mara/src/cli/code/catalog.toml` holds `python` 0.25.0 (`blake3:1d81988af0eb6a514956528292f42d2bb46655ac839f76c589a46360962abbd5`, extension `py`, prefix `#`) and `rust` 0.24.2 (`blake3:f03b54d5fcef88042d4645c2820c4a4bedbd4edaed2e01ad85e67b325c2d7149`, the docs' example; extension `rs`, prefixes `//` and `///`). Each `.wasm` URL is `https://github.com/tree-sitter/tree-sitter-<language>/releases/download/v<version>/tree-sitter-<language>.wasm`; each `tags` URL is `https://raw.githubusercontent.com/tree-sitter/tree-sitter-<language>/v<version>/queries/tags.scm`. Both checksums were computed from the fixture files in `crates/mara/tests/fixtures/grammars` (whose SHA-256 digests match their README), not from anything fetched.
- `crates/mara/tests/code/add.rs`: new test `the_built_in_catalog_pins_the_fixture_grammars` holds each entry's version and checksum equal to the fixture's and checks that both URLs name the `v<version>` tag. The refusal of a language the catalog lacks, without a repository, now uses `go` instead of `rust`, which the catalog now names.
- docs/code/index.mara.md: DES-GRAMMAR-ASSETS says what the built-in catalog holds; the bullet under `## Open questions` and the then-empty section are removed. docs/code/verification.mara.md: VER-CODE-ADD has a row for the new test.

The real run confirms the published files equal the fixtures: `code add` fails with `checksum-mismatch` when the fetched `.wasm` or `tags.scm` does not hash to the pinned checksum, which hashes both, so the `tags.scm` at each tag equals the fixture's copy.

### The one real-repository run (2026-11-21T17:28:44Z, at 94e116d plus this change)

Run once, from the worktree root, with `TMPDIR=/home/araketski/.tmp/claude`; the test gives the binary an empty environment with `HOME` in a temporary directory under it, so the cache was `$HOME/.cache/mara/grammars` there and no proxy was set.

```
$ cargo test --package mara --test code --locked -- --ignored --exact add::adds_each_catalog_language_from_its_real_repository
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.06s
     Running tests/code/main.rs (target/debug/build/mara/77890e1de859c674/out/code-77890e1de859c674)

running 1 test
test add::adds_each_catalog_language_from_its_real_repository ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 47 filtered out; finished in 2.54s
```

### Checks

All exit 0: `cargo fmt --all --check`; `cargo clippy --workspace --all-targets --locked --config 'build.warnings="deny"'`; `cargo test --workspace --locked`; `mara --no-daemon project validate` (0 errors, 1 warning `requirement-verified` in docs/distribution.mara.md, which this change does not touch); `cargo test --package mara --test performance --release --locked` under `berth --exclusive` (28 passed, 2 ignored); `cargo build --package mara-engine --target wasm32-unknown-unknown --locked`; `zban check`.

## Decisions made

- The `tags.scm` URL is `raw.githubusercontent.com` at the release's tag: the maintainer named "the same tag of the repository", and DES-GRAMMAR-ASSETS needs a URL of the file alone. Written into DES-GRAMMAR-ASSETS.
- Extensions and comment prefixes, which the maintainer left open: rust `rs` with `//` and `///` (as the tests' own catalog does; `*` from the docs' configuration example left out, since it is also an operator), python `py` with `#`. Written into DES-GRAMMAR-ASSETS.

Follow-up: mara-gjn0mr, the module comment of `catalog.rs` (owned by mara-7kfxn1 during this issue) still calls the shipped catalog empty.
