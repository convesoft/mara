---
format_version: 1
id: "7d5a6ad676c747e29f6d1abef1fecff4"
title: "Implement reusable query composition and explicit transitive projections"
completion: manual
state: done
priority: 2
parent: "fa44a398bfb74533bf4bce9a20088312"
depends_on: ["1eb5b9b54b944d8eb8d25a27e7c78b3b"]
labels: ["release:0.4.0-alpha.1", "implementation"]
close_reason: "Acceptance, clean sol/low/fast independent worker review, exact-tree local squash integration and full combined verification passed on plan-execution-architecture at c4b1ada2960568dad88987c6fa36e5667aac650e (510 tests; complete schema/project validation). Evidence: /home/araketski/.local/state/pi/verification/01a11fc1-65d6-74d3-8548-77e795d5c029/query-composition-integration/."
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

## Verified local integration
The subsequent user request authorized composition integration and its gated cleanup. Worker head `4fde0b240ee27705ac091b619d2baf56cf085cf5` was locally squash-integrated into `plan-execution-architecture` at `c4b1ada2960568dad88987c6fa36e5667aac650e`. The integrated tree is exactly equal to the worker final tree (`91863d98e2bc8441d78a8d91c6e49ed138500a3f`); no conflicts or product edits were introduced. Final reviewed implementation remains `02d9d29863446c5f4f88e90f47bc2691ed14780b`; the worker's later commit changes only this Work handoff record.

The full existing integration gate passed at the exact integrated SHA: formatting, locked workspace/all-target build, strict all-feature Clippy, all 510 non-ignored workspace tests (0 failed; 8 intentionally ignored helpers), and doctest checks (0 doctests). Complete candidate schema/project validation returned valid/evaluation-complete with zero diagnostics and no remaining pages. The full run includes real portable/native Rust/CLI/MCP composition parity and the ancestry, normalization and forwarded-parameter checks. A separate disposable direct CLI fixture retained two selected roots, one distinct typed binding with canonical authored support and an explicit empty root.

Three raw independent reviews and their frozen-checkout/profile/completion records were audited. The final review is clean under `openai/gpt-6.1-sol low --fast --no-approve`, against supplied logical base `plan-execution-architecture` and supplied/actual SHA `244c689ab8be432952ee3cb7c575738d6245d595`. Both earlier reviews' three required findings were fixed; no unresolved required finding or clarification remains. The final selected-test receipt records HEAD `58b34a711d5e757562003094641d5efa6998242e` with three dirty files; its complete 296-file snapshot equals subsequently committed reviewed `02d9d298...`. Keep that original execution identity, not the worker summary's shorthand relabeling.

Combined receipt: `/home/araketski/.local/state/pi/verification/01a11fc1-65d6-74d3-8548-77e795d5c029/query-composition-integration/full-gate/full.SrJXj1/`.
Integration audit, input/result, candidate checksum, exact-tree comparison and preserved worker-history bundle: `/home/araketski/.local/state/pi/verification/01a11fc1-65d6-74d3-8548-77e795d5c029/query-composition-integration/`.
Original raw worker reviews/receipts remain in the previously cited worker evidence root.

The item passes its declared local scope-target gates. Final adapters, external facts, views, integrated engine catalog, supported-host/main/CI/release gates remain separate; path-performance deferral `e71fd9e53a954a56b44b025bee8377dc` is unchanged. No push, PR, publication, other-worker integration or successor dispatch occurred. Resource cleanup is recorded separately after its ownership gates.

## Verified resource cleanup
After acceptance and combined verification, the composition worker's sole editor buffer was confirmed unmodified and closed normally. Only the verified idle composition Pi process was stopped; its MCP children exited. Herdr removed workspace `w32` and checkout `/home/araketski/.herdr/worktrees/mara/w-7d5a6ad6-query-composition` with `forced:false`; its 611 MiB ignored Cargo target contained generated build data. The frozen branch at `4fde0b240ee27705ac091b619d2baf56cf085cf5` was compare-and-deleted after preserving and verifying `worker.bundle` and exact integration-tree equality.

Removed the control session's task scratch `/home/araketski/.cache/pi/scratch/01a11fc1-65d6-74d3-8548-77e795d5c029/query-composition-integration` (about 28 GiB) and private IPC `/run/user/1000/mc-int.QND2wj` after resolved-path, ownership, no-process and evidence checks. The worker's own scratch/runtime were already absent. Required history, source/reproduction inputs, raw worker reviews, full receipt, command/revision/checksum/acceptance/cleanup records remain in their cited retained evidence roots. Rules/gates workspace `w33`, its branch and resources, the developer target and shared storage were untouched. Final product/tool inputs remain identical to tested `c4b1ada2960568dad88987c6fa36e5667aac650e`; subsequent edits are Work delivery metadata only. No successor dispatch or publication.
