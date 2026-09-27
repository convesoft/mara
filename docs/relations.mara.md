# Relation authoring

:::mara requirement REQ-RELATION-MUTATION
:mid: 01M1PXP2KGENM6H6CP04STA54Q
:title: Author and remove semantic relationships
:status: accepted
:derives_from: SCN-AUTHOR-TRACE-CONNECTION
:kind: functional

CLI and MCP shall add or remove schema-declared relationships between resolvable endpoints. Accept canonical names, inverse aliases and ID/MID equivalents; reject ambiguous identities, invalid endpoints and adding an already asserted semantic edge. Allow schema-valid self-edges.

Add one metadata assertion to the requested source item. Remove all item-authored occurrences of the edge, including inverse/symmetric and typed inline assertions across included files, or exactly one current occurrence selector. Refuse stale/mismatched selectors without writes. Preserve unrelated bytes, permissions and surviving resolved references; demote inline assertions without deleting their prose. Code source files remain read-only; item-authored inverse assertions may change while comment assertions keep the edge present. Report changed/remaining occurrence counts and edge existence.
:::

:::mara design DES-RELATION-MUTATION
:mid: 01M2GC38XWX7TS7GJXVWC5FE8Y
:title: Publish reference-safe semantic relation changes
:status: accepted
:kind: interface
:satisfies: REQ-RELATION-MUTATION

CLI `relation add/remove SOURCE RELATION TARGET` and MCP `relation_add/relation_remove` return relationship format 1: `action`, `scope`, canonical `edge`, `changed_occurrences`, `remaining_occurrences` and `edge_exists`. Scope is `occurrence` with a selector, `item` for code edges, otherwise `relationship`. Errors use the existing relationship envelope and semantic edge/count when available.

Hold the mutation lock, load the strict corpus and reject ambiguous IDs/MIDs. Resolve the canonical edge and collect every assertion before writing. Add rejects equivalent canonical, alias, symmetric, inline and ID/MID occurrences; insert metadata using the requested relation spelling and target's current human ID, or exact external/code address. Code source requests are unsupported; code targets require an item-authored inverse.

An occurrence selector belongs to the current whole-project/schema snapshot and requested edge. Check freshness even when its old edge is absent. Remove metadata lines including their newline; edit selected spans from the end backwards. Demote internal inline assertions to `[[authored-target]]`, external assertions to `<authored-address>`, and code assertions to plain code reference text. Code comments remain immutable and count toward remaining occurrences.

Parse projected documents and reject newly broken or retargeted surviving references, including heading anchors. Use the shared journal publisher to recheck project configuration, schema and corpus before publication. Do not impose lifecycle or graph-policy validation on relationship removal; resulting obligations remain visible to validation.
:::

:::mara decision ADR-RELATION-ASSERTION-REMOVAL
:mid: 01M2GC5TK1ABKQ0V57CJGMG1AC
:title: Remove a semantic edge or one authored occurrence
:status: accepted
:justifies: DES-RELATION-MUTATION
:justifies: DES-CANONICAL-TRACE-RELATIONS

Use semantic-edge removal by default and an explicit snapshot-bound selector for one occurrence. An edge can have several metadata/inline assertions, authored from either endpoint through inverse or symmetric spellings. Removing only the requested spelling would leave the semantic edge unexpectedly present.

Demote inline assertions to their untyped navigation form rather than deleting surrounding prose. Internal mentions still preserve navigation and deletion dependencies. Code comments are read-only; item-only removal reports any remaining code assertion instead of claiming the edge disappeared.
:::

:::mara verification VER-RELATION-MUTATION
:mid: 01M3H4HDGJD4P55YH4K3CX4T53
:title: Check semantic relationship writes and journaled recovery
:status: accepted
:method: test
:level: system
:verifies: REQ-RELATION-MUTATION
:verifies: DES-RELATION-MUTATION
:verifies: DES-MUTATION-TRANSACTION
:verifies: REQ-RECOVERABLE-MUTATION

Run `cargo test --locked --test relation_mutation` and transaction unit tests. Exercise real CLI/MCP add/remove, canonical/alias/symmetric/MID identity, self-edges, duplicate refusal, snapshot selectors, whole-edge multi-file removal and inline internal/external/code demotion. Check unchanged bytes on endpoint, identity, reference and lock failures, plus permissions and unrelated prose preservation.

Inject publisher failures before and after each replacement, reject changed preimages, preserve conflicts during incomplete rollback and terminate real subprocesses at each publication boundary. Recover after restart and verify original bytes, new-destination removal and journal cleanup. Run all prior suites, formatting and Clippy, canonical validation and selected traceability. Publisher tests establish the shared dependency, not unimplemented move/rename workflows.
:::
