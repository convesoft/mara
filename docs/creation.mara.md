# Item creation

:::mara requirement REQ-ITEM-CREATION
:mid: 01M1PXP2KGSJHD00W32AGQ3YVT
:title: Create a complete item or scaffold in an explicit document
:status: accepted
:kind: functional
:derives_from: SCN-AUTHOR-ITEM-FLEXIBLY

`item create` and MCP `item_create` require flavour, human ID, explicit destination file, title and schema-required custom fields. Create a missing file only when its parent exists; otherwise append, or insert before one-based `line` (`line_count + 1` means EOF). The destination must be selected by content includes, Git ignores and directory-symlink discovery rules. CLI body input accepts inline text or `-` for stdin; MCP body is literal.

An omitted, empty or whitespace-only required body creates an incomplete scaffold: report `complete:false` and `missing:["body"]`. Validation continues to reject it until filled. Optional bodies may be empty. Generate a canonical MID immediately after the opener and return it; callers cannot supply a MID.

Optional initial typed relations are separate from fields. Validate the complete candidate item and requested edges before publishing item, MID and relations in one atomic source update. Refusal preserves an existing destination exactly and leaves no new file/item. Empty and omitted relations are equivalent. Later relationship editing is a separate operation.
:::

:::mara requirement REQ-ITEM-INSERTION-SAFETY
:mid: 01M1PXP2KGKT8ET242DR0YHV9S
:title: Insert items without corrupting document structure
:status: accepted
:kind: functional
:derives_from: SCN-AUTHOR-ITEM-FLEXIBLY

An explicit insertion point must be outside every existing Mara item but may split ordinary narrative. Ensure at least one blank line around the new item when adjacent content exists without accumulating separators. Validate the resulting source and publish atomically only on success. Reject bodies that close the created item or introduce another item; a required-body scaffold remains structurally safe but semantically incomplete.

New references must resolve. Surviving references must retain their resolved destinations, including Markdown links affected by shifted generated heading anchors. Refusal identifies link impacts and preserves source; unrelated existing broken links do not by themselves prevent creation.
:::

:::mara decision ADR-ATOMIC-ITEM-CREATION
:mid: 01M1S9YE52K3V1ZRMR93FB5C1K
:title: Publish an item and its initial relations together
:status: accepted
:justifies: REQ-ITEM-CREATION

Publish one item and its initial outgoing relations in one validated atomic source update. Separate creation and edge operations can leave an incomplete graph when a later relation fails. Dedicated relation input keeps typed edges distinct from custom fields. This does not introduce multi-item batches or unresolved forward references.
:::

:::mara design DES-ITEM-CREATION
:mid: 01M3H3RG8KT0XGJKCN1M16B9GZ
:title: Preflight the inserted item and surviving reference destinations
:status: accepted
:kind: behavior
:satisfies: REQ-ITEM-CREATION
:satisfies: REQ-ITEM-INSERTION-SAFETY

Hold the project mutation lock. Validate authored scalars, schema fields and identity, resolve initial targets to human IDs, and reject equivalent semantic assertions. Check the destination's confinement and discovery eligibility; render MID/title/fields/relations/body with destination separators, preserving untouched bytes and permissions.

Build a disposable candidate corpus and check the created item's conformance, allowing only its deliberately missing required body. Check new references and compare surviving references against their original resolved destinations. Establish correspondence separately for each durable item and document narrative scope using a character diff; do not equate snapshot handles or generated anchors with durable identity. Structural content, containment and source correspondence prevent duplicate headings/blocks from impersonating an old target. Rename and explicit body-update correspondence follow [[DES-ITEM-RENAME]] and [[DES-ITEM-UPDATE]].

Publish one atomic source replacement after all checks. Return `{id,mid,path,line,complete,missing}` with relative path, one-based opener line and `missing:[]` or `["body"]`. Rejected requests do not create parents, partial items or edge fragments.
:::

:::mara verification VER-ITEM-CREATION
:mid: 01M3H3RMKTQ7DKP3NJ2YP2K2JN
:title: Create, reload and navigate complete items and scaffolds safely
:status: accepted
:method: test
:level: system
:verifies: REQ-ITEM-CREATION
:verifies: REQ-ITEM-INSERTION-SAFETY
:verifies: DES-ITEM-CREATION

Run `cargo test --locked --test item_creation` through the candidate CLI and stdio MCP in fixture-owned projects. Check complete/scaffold results, field validation, literal/stdin body semantics, exact MID placement, safe line insertion, LF/CRLF and permissions, discovery/path refusal and body containment. Create initial item/MID/self/inverse/external relations; reload get/related and compare transport results. Reject invalid/duplicate edges and broken new references without changing existing files or creating new ones.

Check existing link destinations across insertion, including duplicate-heading anchor shifts, and allow unrelated existing broken references. Run the full regression suite, formatting and Clippy against unchanged sources. Use library conformance checks to verify created sources. [[VER-PROJECT-VALIDATION]] owns validation transports; each mutation has its own verification method.
:::

:::mara evidence EVD-ITEM-CREATION
:mid: 01M3H45434E7XS5ND26DFQ8N0S
:title: Item creation and existing capability suites pass
:status: retired
:result: passed
:captured_at: 2026-09-27T09:46:27Z
:subject_revision: 3f9666e008c14b5f3f889bc2274a01aca250e19c
:evidences: VER-PROJECT-INSPECTION
:evidences: VER-SCHEMA-DEFINITIONS
:evidences: VER-DOCUMENT-PARSING
:evidences: VER-CODE-DISCOVERY
:evidences: VER-ITEM-LIST
:evidences: VER-UNIFIED-SEARCH
:evidences: VER-BOUNDED-NODE-READ
:evidences: VER-DOCUMENT-NAVIGATION
:evidences: VER-RELATION-INSPECTION
:evidences: VER-CORPUS-CONFORMANCE
:evidences: VER-MID-AND-RECOVERY
:evidences: VER-ITEM-CREATION

At `3f9666e008c14b5f3f889bc2274a01aca250e19c`, `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets --all-features -- -D warnings`, and `cargo test --locked --all-targets` passed: 196 tests, none failed or ignored. The final run used unchanged repository source.

Twelve creation groups exercise real CLI/stdin and MCP authoring, complete/scaffold/optional bodies, fields and generated identities, source/newline/permission preservation, safe insertion, body containment, discovery confinement, initial item/MID/self/inverse/external relations, semantic duplicate and invalid-edge refusal, get/related transport parity, shifted-heading link protection, independent old/new reference errors and mutation blocking. Candidate library conformance verified created sources; this run did not exercise candidate project/item validation transports.

Candidate CLI schema validation and installed-baseline MCP schema/project validation returned complete validity without diagnostics. Installed-tool intent passed two selected roots; realization and verification each passed three roots, consuming all pages. CLI help and MCP tools/list expose creation. This execution does not establish relation editing, update/delete/move/rename, journal publication or rule/graph evaluation.
:::
