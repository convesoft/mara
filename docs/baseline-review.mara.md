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
| Project discovery, initialization and profiles | Retain initialization, project selection, schema inspection and flavour guidance under [project contracts](project.mara.md). Seventeen baseline tests retain distinct target-selection, transport, template, guidance, inspection and source-preservation obligations in `tests/project_bootstrap.rs`; add CLI/MCP parity for declaration inspection. Fixtures now own their Git/configuration state; guidance assertions use diagnostic codes/severity. Correct obsolete format-2 guidance prose to the implemented schema format 3. Fix the realization check with a requirement-class guard so design and mixed selections evaluate; a new real CLI/MCP regression first reproduced the baseline failure. Engineering policy, classification and execution semantics remain pending separate review. |
| Creation, update, move, rename, deletion and recovery | Pending: mutation contracts and reference preservation; CLI and mutation-reference tests. |
| Item and narrative discovery, search, navigation and bounded reads | Pending: discovery and retrieval contracts; search, handles, pagination, and reference tests. |
| Typed relations, aliases, symmetry, inline assertions and external targets | Pending: relation contracts; semantic-edge identity, occurrence inspection, mutation, and terminal external-target tests. |
| Validation, diagnostics and structural graph policies | Pending: independent diagnostics, recovery, reporting filters, graph constraints, and CLI/MCP parity tests. |
| YAML rules and native evaluation | Pending: rule grammar, applicability, bounded paths, evaluation prerequisites, and rule tests. |
| Matrices, parameter binding and pagination | Pending: selected coverage, explanations, revision parameters, bounds, and matrix tests. |
| Code endpoints, comment markers and language adapters | Pending: endpoint resolution and navigation; real code checks and isolated multi-language fixtures. |
| CLI/MCP interfaces and project context | Retain project context and bootstrap transport/parity behavior. Full command help, tool schemas, and remaining operation parity tests are pending. |
| Packaging, installation, migration and releases | Pending: npm packaging and launchers, skill/plugin, migration, CI/release contracts, and real packaged CLI/MCP smoke. |
| Public guidance and historical research/release documents | Pending: README, roadmap, security guidance, generated changelog, and useful historical knowledge. Retained license notices remain unchanged. |

## Runtime and remaining test obligations

The bootstrap slice restores the shared baseline production runtime unchanged,
apart from code associations. Initialization, authoring, retrieval and validation
share this dependency base. Its presence does not mark other capability rows as
reviewed. The bootstrap check runs real CLI and stdio MCP processes against the
candidate; the installed baseline executable only assists knowledge authoring.

Baseline unit-test modules and other integration tests remain pending restoration
with their owning capabilities, including isolated code-adapter fixtures. None
of their distinct obligations has been dismissed as redundant or obsolete.
The old cardinality code association is pending its contract review; new links
identify the reviewed initialization and schema paths. Missing coverage remains
visible in selected-scope matrices.
