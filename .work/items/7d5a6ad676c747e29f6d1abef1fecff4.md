---
format_version: 1
id: "7d5a6ad676c747e29f6d1abef1fecff4"
title: "Implement reusable query composition and explicit transitive projections"
completion: manual
state: open
priority: 2
parent: "fa44a398bfb74533bf4bce9a20088312"
depends_on: ["1eb5b9b54b944d8eb8d25a27e7c78b3b"]
labels: ["release:0.4.0-alpha.1", "implementation"]
---
Repository: convesoft/mara
Release: 0.4.0-alpha.1 (scheduled)
Scope: docs/query-engine.mara.md; docs/0.4.mara.md
Mara: REQ-COMPOSABLE-QUERY-PROJECTIONS, DES-QUERY-POLICY-ENGINE.
Aggregate: fa44a398bfb74533bf4bce9a20088312
Declared integration target: plan-execution-architecture (existing bounded scope branch).

## Outcome
Express multi-query projections, ancestry and deterministic cross-item comparisons without hardcoded engineering vocabulary.

## Acceptance
- [ ] Implement the selected named-query composition, joins/unions and structured supporting bindings.
- [ ] Support explicit transitive traversal with the designed direction, duplicate/counting, cycle termination and evaluation-limit semantics.
- [ ] Demonstrate fixture basis alternatives, decomposition/supersession and workload-preserving ancestry that excludes sibling workload.
- [ ] Demonstrate deterministic case-normalized title/alias collisions without fuzzy matching and direct-or-design-mediated coverage retaining its support.
- [ ] Verify complete empty results versus unavailable computation, deterministic results and input-sensitive continuation; update canonical semantics and evidence.

## Delivery and evidence
Follow docs/delivery.mara.md: branch from the current scope tip, perform required local review and acceptance verification, squash-integrate locally and verify the integrated result before closing. Record actual tested and integrated SHAs, commands/results and remaining limitations here; check boxes only with evidence. Keep durable product decisions in canonical Mara documents. This task does not authorize publication or engineering-template refinement.
