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
- Bound direct-neighbour results with continuation; keep traversal
  caller-controlled.
- Read large item bodies in bounded consecutive portions and continue relation
  lists without silently omitting content.
- Add typo-tolerant word matching and deterministic relevance ranking.
- Create one item with its initial outgoing relations atomically through CLI/MCP;
  see [item creation](docs/alpha.mara.md).
- Investigate whether and how narrative outside items should be searchable;
  implementation scope remains undecided.

See [retrieval scope and open contracts](docs/retrieval.mara.md) for the five
implementation work areas, narrative-search investigation, verification
expectations, and unresolved defaults.

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

## 0.2.0 — Multi-project workspaces

- Support multi-project and monorepo configuration.
- Define nested project discovery and boundaries.
- Add workspace list, search, and validation with explicit mutation scoping.
- Add reusable template packs and configuration composition.
- Address global plugin installation and project discovery using observed client
  behavior.
- Improve scale only where measurements justify it.
- Do not require cross-project relations in the initial 0.2 scope.

## 0.3.0 — Change-aware knowledge

- Add Git-aware item and document diffs.
- Show move and rename history through immutable identity.
- Analyze relation impact for changed knowledge.
- Support review workflows centered on knowledge changes.
- Define schema evolution and migrations.
- Add cross-project relations or imports only if 0.2 usage demonstrates the
  need.

## Later

- Language Server Protocol and editor integration.
- Persisted indexes or a graph store when measured scale requires them.
- Semantic or hybrid search when deterministic retrieval proves insufficient.
- A graphical interface when stable workflows justify one.
