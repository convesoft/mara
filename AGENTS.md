# AGENTS.md

This file defines repository-wide working rules for human and software agents.
Mara product semantics remain in the Mara corpus; this file explains how to
implement and review changes without creating a second specification.

## Authority and entry points

- Start with [`docs/index.mara.md`](docs/index.mara.md). It is the entry point to
  the canonical engineering-knowledge corpus.
- Git-tracked Mara documents own durable product meaning: goals, scenarios,
  requirements, designs, decisions, risks, tests, and other project semantics.
- Linear owns temporary delivery state: priority, assignment, sequencing,
  blockers, and issue status. A Linear issue must link to Mara items instead of
  copying or redefining their semantics.
- GitHub owns implementation review, CI, merge, and release evidence.
- Generated indexes, graphs, reports, backlinks, and UI views are derived state.
  They must be rebuildable and must never become authoring authorities.

For project-wide boundaries, read
[`docs/product/charter.mara.md`](docs/product/charter.mara.md),
[`docs/product/workflows.mara.md`](docs/product/workflows.mara.md), and
[`docs/architecture/system.mara.md`](docs/architecture/system.mara.md). Read the
relevant capability document and normative reference before changing behavior.
Use [`docs/verification/strategy.mara.md`](docs/verification/strategy.mara.md)
to choose evidence appropriate to the changed contract.

## Working agreement

- Begin from one bounded Linear issue. Identify its referenced Mara item IDs and
  preserve that scope through implementation, tests, and the pull request.
- If a needed behavior is missing, contradictory, or still proposed, update or
  resolve the Mara contract before treating an implementation choice as settled.
- Keep business semantics schema-driven. Do not hardcode project flavours,
  fields, relations, lifecycle states, or traceability rules. Mara defines no
  default process; each project configures its own rules.
- Preserve the source/derived boundary. Normalize graph identity to MID-to-MID
  edges, store canonical authored relations once, and derive inverse or symmetric
  views without creating a second editable copy.
- Preserve document structure, narrative blocks, ordering, and exact source
  spans. Editing operations must produce the smallest safe source change.
- Contract-bearing information must be represented by extracted Mara items.
  Narrative may orient readers but must not hide requirements, decisions, or
  other independently traceable semantics.
- Do not place a real Mara item block inside another item's body. Keep syntax
  examples visibly non-authoritative, and never rely on an unextracted nested
  block as project data.
- Do not create an `artifact` item merely to link one Mara document to another.
  Use ordinary document navigation for internal corpus structure; reserve
  artifacts for independently meaningful implementation or external assets.
- Record newly discovered work in Linear Backlog instead of expanding the active
  issue. Move an issue to Done only after its implementation and verification
  evidence have merged.
- Follow `REQ-VERIFICATION-LAYERS`: choose the lowest sufficient authoritative
  verification mechanism for each claim.
- Do not invent a custom parser, analyzer, lint, test framework, generator, or
  comparable verification infrastructure unless issue readiness records the
  contract gap, bounded tool and maintenance scope, accepted Mara contract, and
  explicit human approval. If the need emerges during implementation or review,
  stop and return for clarification; review findings cannot add it incidentally.
- Treat test and helper complexity as implementation scope. If it rivals the
  production change, stop and reassess the method or split or clarify the
  contract unless that complexity was explicitly approved.

## Repository workflow

- Create branches with the `codex/` prefix when Codex creates them.
- Use Conventional Commits and keep each commit to one logical change.
- Stage only intended files; never discard unrelated user changes.
- Cite the Linear issue and affected Mara item IDs in the pull request.
- Open pull requests early when useful, but request final review only after the
  stated acceptance criteria and local validation pass.

Run the same mandatory checks as CI before requesting final review:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

Add focused tests for changed behavior. Tests should identify the Mara contract
they verify when the relationship is not already obvious from the test fixture
or enclosing module.

## Code Review Rules

- Review the Linear scope and linked Mara items before judging implementation
  details. Report behavior that violates an approved contract even if tests pass.
- Treat duplicated authority as a correctness defect: graph/index state must not
  become writable truth, and derived backlinks must not be serialized as a second
  authored relation.
- Flag hardcoded business flavours, relations, process states, or traceability
  policies unless an approved platform contract explicitly makes them built in.
- Check identity and provenance carefully: resolved edges use MIDs, authored and
  derived origins remain distinguishable, and diagnostics retain useful source
  locations.
- Check parsing and editing changes for source-span accuracy, preservation of
  surrounding Markdown, and minimal diffs. Reject regeneration that needlessly
  rewrites whole documents.
- Check documentation changes for hidden semantics in narration, real item blocks
  nested inside other items, and internal Mara documents modeled as pointless
  artifact items.
- Require tests for normal behavior, malformed input, boundary cases, and stable
  diagnostics appropriate to the change. Run all mandatory repository checks.
- Keep review findings actionable and scoped. Distinguish blocking correctness or
  contract violations from optional follow-up improvements.
