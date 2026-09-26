# Baseline review

Reference: `0b47b42e31a9b1dedb71b1af5658a067c0a739a9` (`0.3.0-alpha.1`).
This review accounts for retained, changed, and removed capabilities. Pending
means the implementation and its distinct verification obligations still need
review; it does not authorize removing behavior. Product contracts live in the
linked capability documents rather than in this inventory.

| Baseline capability | Disposition and review boundary |
|---|---|
| Product intent and scenarios | Retain the two goals and eleven scenarios with their original identities in [product intent](product.mara.md). Add flows for connected editing, interrupted-edit recovery, and code associations from the existing baseline contracts. Remove obsolete release-relative wording from document-context discovery. No behavior change. |
| Canonical source, item identity, document format and schema | Pending: `alpha`, `format`, and `taxonomy` contracts; corpus and reference tests. |
| Project discovery, initialization and profiles | Pending: initialization, project selection, flavour guidance, and engineering profile; template and initialization tests. |
| Creation, update, move, rename, deletion and recovery | Pending: mutation contracts and reference preservation; CLI and mutation-reference tests. |
| Item and narrative discovery, search, navigation and bounded reads | Pending: discovery and retrieval contracts; search, handles, pagination, and reference tests. |
| Typed relations, aliases, symmetry, inline assertions and external targets | Pending: relation contracts; semantic-edge identity, occurrence inspection, mutation, and terminal external-target tests. |
| Validation, diagnostics and structural graph policies | Pending: independent diagnostics, recovery, reporting filters, graph constraints, and CLI/MCP parity tests. |
| YAML rules and native evaluation | Pending: rule grammar, applicability, bounded paths, evaluation prerequisites, and rule tests. |
| Matrices, parameter binding and pagination | Pending: selected coverage, explanations, revision parameters, bounds, and matrix tests. |
| Code endpoints, comment markers and language adapters | Pending: endpoint resolution and navigation; real code checks and isolated multi-language fixtures. |
| CLI/MCP interfaces and project context | Pending: shared operations, help, tool schemas, transport, project selection, and parity tests. |
| Packaging, installation, migration and releases | Pending: npm packaging and launchers, skill/plugin, migration, CI/release contracts, and real packaged CLI/MCP smoke. |
| Public guidance and historical research/release documents | Pending: README, roadmap, security guidance, generated changelog, and useful historical knowledge. Retained license notices remain unchanged. |

The first checkpoint establishes intent only. Source and tests have not yet
been restored. Validation of this corpus is not evidence that the rebuilt
product works. Goals and scenarios acquire validation methods and execution
evidence with the corresponding working increments; missing coverage remains
visible in selected-scope matrices.
