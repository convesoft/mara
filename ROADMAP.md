# Roadmap

This roadmap states Mara's current development intention. It is updated as
evidence and priorities change; detailed behavior becomes canonical in
`docs/*.mara.md` when a milestone begins.

## Toward 0.1.0

### 0.1.0-alpha.0 — Distributable core

- Publish the existing single-project CLI and MCP workflow.
- Add essential public documentation, licensing, and the roadmap.
- Ship script-free npm dispatch and native platform packages.
- Generate the changelog and automate CI, packaging, and protected releases.
- Verify clean CLI and stdio MCP use without a local Rust installation.

### 0.1.0-alpha.1 — Agent-ready onboarding

- Ship a Mara skill and clear MCP onboarding guidance with the existing npm
  package.
- Let one MCP connection initialize or select one project per operation while
  retaining execution-directory discovery.
- Verify the primary Codex workflow through manual MCP registration and
  separate skill installation. Keep complete-plugin installation optional.

### 0.1.0-alpha.2 — Durable identity and editing

- Introduce immutable machine identities with deliberate backfill.
- Add safe structured update, move, rename, and delete operations.
- Preserve relation and validation integrity throughout lifecycle changes.
- Clarify the alpha.3 retrieval scope in canonical documents before release.

### 0.1.0-alpha.3 — Enhanced deterministic retrieval (current)

- Paginate search/list results and bound summaries; offer opt-in search excerpts
  and selected-item filtering.
- Filter search/list to exact documents or package directory subtrees.
- Filter validation diagnostics by directory while retaining whole-project
  context and status; see [validation reporting](docs/alpha.mara.md).
- Bound direct-neighbour results with continuation; keep traversal
  caller-controlled.
- Read large item bodies in bounded consecutive portions and continue relation
  lists without silently omitting content.
- Add typo-tolerant word matching and deterministic relevance ranking.
- Create one item with its initial outgoing relations atomically through CLI/MCP;
  see [item creation](docs/alpha.mara.md).
- Clarify CLI help, MCP parameter guidance, and the Mara skill's authoring and
  retrieval workflows, including a JSON CLI fallback.
- Retain file-based narrative access and item-only Mara search; see the reviewed
  [narrative retrieval findings](docs/retrieval.mara.md#narrative-retrieval-investigation).

See [retrieval scope and contracts](docs/retrieval.mara.md) for implementation
boundaries, verification expectations, and the accepted narrative boundary.

### 0.1.0-beta.0

- Complete the intended 0.1 feature set.
- Validate format and compatibility behavior with real projects and clients.

### 0.1.0-beta.N

- Stabilize demonstrated workflows and compatibility.
- Add no planned new feature areas.

### 0.1.0-rc.0

- Release the exact candidate artifacts.
- Accept only release blockers and documentation corrections.

### 0.1.0

- Declare the stable single-project workflow.

## 0.2.0 — Guided authoring

- Prioritize bundled template files, an optional engineering template, and
  project-defined flavour selection guidance together.
- Require flavour guidance for existing and new schemas as a documented
  breaking change, with a migration guide for 0.2.
- Supply useful engineering traceability relations through the template.
- Add bounded discovery and reading of canonical document context outside items.
- Evaluate diagnostic codes and severity as a candidate addition.

See [planned outcomes and open decisions](docs/guided-authoring.mara.md) before
ticket planning. This scope retains one project and schema for package-local
documents in a monorepo and does not expand the 0.1 stabilization sequence.

## 0.3.0 — Knowledge change review (provisional)

- Compare items and documents across Git revisions, distinguishing content and
  relation changes from moves and human-ID renames.
- Follow item history through immutable identity.
- Identify related knowledge that may need review and explain its connection
  to a change; these are review candidates, not proven inconsistencies.
- Combine diffs, history, and relation context in a bounded review workflow.

## 0.4.0 — Traceability and evolution (provisional)

- Add project-configured traceability rules driven by demonstrated checks.
- Support deliberate schema migrations for observed vocabulary changes.
- Generate useful specification and traceability-matrix views from the corpus.
- Add typed external links when a concrete workflow needs them.
- Evaluate a code-traceability pilot for one demonstrated language and workflow.

The 0.3 and 0.4 groupings are planning directions; define their detailed
contracts from usage before scheduling implementation.

## Later

- Multi-project aggregation, nested project boundaries, and cross-project
  relations when independent corpora need a shared operation.
- Remote template packs and configuration composition when bundled seeds and
  project-owned schemas are insufficient.
- Delivery synchronization and richer graph provenance for concrete consumers.
- Global plugin installation and discovery improvements for observed client
  problems.
- Scale targets and optimization based on measurements.
- Language Server Protocol and editor integration.
- Persisted indexes or a graph store when measured scale requires them.
- Semantic or hybrid search when deterministic retrieval proves insufficient.
- A graphical interface when stable workflows justify one.
