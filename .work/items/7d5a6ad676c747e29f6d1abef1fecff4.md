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
- [x] Implement the selected named-query composition, joins/unions and structured supporting bindings.
- [x] Support explicit transitive traversal with the designed direction, duplicate/counting, cycle termination and evaluation-limit semantics.
- [x] Demonstrate fixture basis alternatives, decomposition/supersession and workload-preserving ancestry that excludes sibling workload.
- [x] Demonstrate deterministic case-normalized title/alias collisions without fuzzy matching and direct-or-design-mediated coverage retaining its support.
- [x] Verify complete empty results versus unavailable computation, deterministic results and input-sensitive continuation; update canonical semantics and evidence.

## Verified implementation handoff (not integrated)
Implementation commits: `58b34a711d5e757562003094641d5efa6998242e` and `02d9d29863446c5f4f88e90f47bc2691ed14780b`. Tested source SHA: `02d9d29863446c5f4f88e90f47bc2691ed14780b`; tree: `0b8f73816761717d2e4cc29eb6107e2ce8fe1f94`. Branch `w-7d5a6ad6-query-composition`, based on `plan-execution-architecture` at `244c689ab8be432952ee3cb7c575738d6245d595`.

Local acceptance evidence:
- Format-4 fixtures exercise explicit child parameters/projections, typed joins, unions, optional absence, support and contributor retention; a forwarded-only parent parameter filters its child without a dummy parent SELECT use.
- Fixture cases cover distinct basis/support alternatives with lifecycle values, direct/design-mediated coverage with both canonical support edges, transitive decomposition/supersession, upward ancestry excluding the root and sibling workload, NFC/full-casefold title/alias collisions without fuzzy matches, complete empty roots versus required-code unavailability, observed-row limits and input-bound continuation. Portable engine and real disposable CLI/MCP paths agree.
- `cargo test -p mara-engine --test named_query -- --nocapture`: 14 passed. `scripts/verify.sh selected 'binary(=named_query)'`: 29 passed, 0 failed, 0 skipped across the engine and native/portable/CLI/MCP named-query test binaries. `cargo fmt --all -- --check` and `cargo clippy --locked -p mara-engine --all-targets -- -D warnings` passed.
- Mara `project_validate` and `item_validate DES-QUERY-COMPOSITION-CONTRACT`: valid, evaluation complete, zero diagnostics.
- Independent review against base `244c689ab8be432952ee3cb7c575738d6245d595`: first review `pi-review.MnKNSl` found two required fixes (dependency row identity collision; unused loaded dependency validation), both fixed in the first implementation commit. Second review `pi-review.TMe1tU` found forwarded parent parameters were rejected as unused; fixed in the second commit. Final review `pi-review.FHyH4o` at the tested SHA is clean: `findings: []`, `overall_correctness: patch is correct`, exit 0, settled, final stop, unchanged checkout. Full records are in `/home/araketski/.local/state/pi/verification/01a11fcb-75d5-7713-8673-df91494fe687/`.
- Known nonblocking deferral remains Work `e71fd9e53a954a56b44b025bee8377dc` (whole-corpus path-support cost); it is unchanged and not fixed here.

User authorization explicitly excludes integration, publication and Work closure. No integrated SHA is claimed; do not close this item. The full integration verification gate remains pending. Other limitations: no final daemon/Node/HTTP/TypeScript/WASM rollout; the observed solution-row limit is not a graph-work/time/memory bound.

## Delivery and evidence
Follow docs/delivery.mara.md: branch from the current scope tip, perform required local review and acceptance verification, squash-integrate locally and verify the integrated result before closing. Record actual tested and integrated SHAs, commands/results and remaining limitations here; check boxes only with evidence. Keep durable product decisions in canonical Mara documents. This task does not authorize publication or engineering-template refinement.
