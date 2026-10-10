# Roadmap

Mara's supported behavior is defined in the [product documentation](docs/index.mara.md).
The following development intentions are provisional, not implementation promises.

## 0.4.0-alpha.1 — Execution, integration and reusable query engine

The [confirmed scope](docs/0.4.mara.md) establishes one portable document engine,
native embedded/daemon execution, N-API and browser WASM bindings, and a shared
TypeScript client. It also includes the [reusable query/policy engine](docs/query-engine.mara.md):
factual queries, contextual gates and separate views/evaluation reports, without
engineering-template refinement. It includes the library/transport paths, not
graphical applications or GitHub publication. Detailed contracts and release acceptance
live in that scope and its linked Mara items. The implementation is locally
verified; publication and corresponding-host acceptance must be established by
actual release results, not this roadmap. Dedicated manual Apple Silicon
certification and comprehensive final guidance are deferred for this alpha;
existing automated host CI/release checks remain required.

## Later

### Knowledge change review

- Compare items and documents across Git revisions, distinguishing content and
  relation changes from moves and human-ID renames.
- Follow item history through immutable identity.
- Evaluate project-defined lifecycle transitions against previous and current
  states once revision comparison is available.
- Identify connected knowledge that may need review, explaining why it is a
  candidate without claiming a proven inconsistency.
- Combine differences, history and relation context in a bounded review workflow.

This former 0.4.0 intention is deferred without a new release commitment.
Detailed contracts will be established when it is selected.

### Other intentions

- Multi-project aggregation, nested boundaries and cross-project relations.
- Remote template packs and configuration composition.
- Delivery synchronization and richer graph provenance for concrete consumers.
- Measured scale improvements, including persisted indexes if needed.
- Language Server Protocol and editor integration.
- Semantic or hybrid search when deterministic retrieval is insufficient.
- A graphical interface when stable workflows justify one.

See [release preparation](docs/release.mara.md) for verification and publication
gates. Published changes are recorded in the generated [changelog](CHANGELOG.md).
