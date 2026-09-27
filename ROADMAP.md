# Roadmap

Mara's supported behavior is defined in the [product documentation](docs/index.mara.md).
The following development intentions are provisional, not implementation promises.

## 0.4.0 — Knowledge change review

- Compare items and documents across Git revisions, distinguishing content and
  relation changes from moves and human-ID renames.
- Follow item history through immutable identity.
- Evaluate project-defined lifecycle transitions against previous and current
  states once revision comparison is available.
- Identify connected knowledge that may need review, explaining why it is a
  candidate without claiming a proven inconsistency.
- Combine differences, history and relation context in a bounded review workflow.

Detailed contracts will be established when this work is selected.

## Later

- Multi-project aggregation, nested boundaries and cross-project relations.
- Remote template packs and configuration composition.
- Delivery synchronization and richer graph provenance for concrete consumers.
- Measured scale improvements, including persisted indexes if needed.
- Language Server Protocol and editor integration.
- Semantic or hybrid search when deterministic retrieval is insufficient.
- A graphical interface when stable workflows justify one.

See [release preparation](docs/release.mara.md) for verification and publication
gates. Published changes are recorded in the generated [changelog](CHANGELOG.md).
