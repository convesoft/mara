# SCIP regression fixtures

The JSON files are decoded output from real indexers run on the adjacent
`.source` files on 2026-09-27: scip-typescript 0.4.0, scip-clang 0.4.0 and
rust-analyzer 1.97.1 (8bab26f). The stock SCIP CLI 0.10.0 produced the JSON.
The TypeScript and Rust package was `mara-scip-probe` version `1.0.0`.
C++ used a compilation database for `clang++ -std=c++20 -c service.cpp`.

CLI/MCP tests rewrite `metadata.project_root` and encode these records with
the standard SCIP protobuf types. Their configured `cp` process checks Mara's
invocation and consumption boundary deterministically; it is not a language
indexer. Separate live probes exercised Mara with the actual indexers, including
TypeScript package version changes and deletion of one C++ overload.
The owning contract and migration are [[DES-CODE-TRACEABILITY]].
