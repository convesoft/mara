# Roadmap

This roadmap states Mara's current development intention. It is updated as
evidence and priorities change; detailed behavior becomes canonical in
`docs/*.mara.md` when a milestone begins.

## Toward 0.1.0

### 0.1.0-alpha.0 — Distributable core (current)

- Publish the existing single-project CLI and MCP workflow.
- Add essential public documentation, licensing, and the roadmap.
- Ship script-free npm dispatch and native platform packages.
- Generate the changelog and automate CI, packaging, and protected releases.
- Verify clean CLI and stdio MCP use without a local Rust installation.

### 0.1.0-alpha.1 — Agent-ready onboarding

- Add an Agent Plugin manifest, Mara skill, and MCP configuration.
- Manage project `AGENTS.md` guidance and reusable project templates.
- Add project-level configuration needed by the onboarding workflow.
- Leave client-wide discovery unspecified until observed client behavior gives
  it a reliable contract.

### 0.1.0-alpha.2 — Durable identity and editing

- Introduce immutable machine identities with deliberate backfill.
- Add safe structured update, move, rename, and delete operations.
- Preserve relation and validation integrity throughout lifecycle changes.

### 0.1.0-alpha.3 — Enhanced deterministic retrieval

- Improve deterministic CLI and MCP search, retrieval, and relation traversal.
- Keep results bounded and explicit without adding context profiles.

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

Context profiles are not planned through 0.3.0.

## Later

- Language Server Protocol and editor integration.
- Persisted indexes or a graph store when measured scale requires them.
- Semantic or hybrid search when deterministic retrieval proves insufficient.
- A graphical interface when stable workflows justify one.
