# 0.2.0-alpha.0 release

Preparation is owned by [MARA-44](https://linear.app/convesoft/issue/MARA-44/add-the-bundled-engineering-template).

:::mara design DES-RELEASE-020-ALPHA0
:mid: 01M23Q4ZAFHEH6EHBF689GEGXH
:title: Release schema guidance and bundled engineering templates
:satisfies: REQ-REPRODUCIBLE-PUBLIC-RELEASE

Release the schema-guidance and engineering-template slice of 0.2. The candidate
combines [[REQ-FLAVOUR-AUTHORING-GUIDANCE]], [[REQ-ENGINEERING-TEMPLATE]], and
[[REQ-ENGINEERING-TRACEABILITY]] with the existing item authoring and retrieval
workflow. All project initialization templates are embedded source files; the
installed executable needs no template directory. Initialization writes only
project configuration and schema, with `minimal` still the default.

**Breaking schema change:** existing format-1 schemas require the explicit
[schema guidance migration](migration-0.2.mara.md#schema-guidance). Preserve
custom declarations and item identities. Project configuration remains format 1
and document syntax is unchanged. Unified discovery and its command/response
migration are not included; retain `item search`, `item get`, and `item related`.
Mara's own adoption of the additional engineering relations remains separate work.

Prepare one release-labelled PR for MARA-44 against main using
[[DES-PROTECTED-RELEASE-WORKFLOW]]. Generate `CHANGELOG.md` with git-cliff against
the intended squash history and verify identical regeneration. Keep the Cargo,
CLI, MCP, native npm packages, dispatcher, and packaged plugin at
`0.2.0-alpha.0`. Publish prerelease packages on `next`, preserving stable `latest`.

Verify formatting, Clippy, all-target tests, corpus validation, and a release
build. Clean-install the packaged CLI/MCP without Rust on PATH; demonstrate
engineering initialization, schema inspection, connected items, and validation.
The protected workflow must verify all four supported target artifacts from the
merged main commit before publication approval. Local host checks do not prove
other-platform or public-registry behavior.

After publication, verify the annotated tag, exact package versions and digests,
`next` tags, clean public-registry CLI/MCP workflow, and GitHub prerelease. Install
the exact `@convesoft/mara@0.2.0-alpha.0` version for the next task only after that
verification. Release preparation alone does not install or publish the MCP.
:::
