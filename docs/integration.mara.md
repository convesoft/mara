# JavaScript and browser integration for 0.4.0

Target contracts for the [0.4.0 scope](0.4.mara.md). The shared client uses the
[execution architecture](execution.mara.md). These draft items specify library
artifacts and behavior, not graphical applications or passing test results.

## Workflows

:::mara scenario SCN-IMPORT-MARA-NODE
:mid: 01M3YVYZB8T1CF5CY0819DCW77
:title: Use Mara from a Node TypeScript application
:status: draft
:contributes_to: GOAL-UNIFIED-PROJECT-KNOWLEDGE

A Node application imports the installed Mara package, receives typed
asynchronous results, and reads and edits a selected project without spawning
CLI commands. The native binding uses the shared Rust client, selecting
embedded or daemon execution. Installing the package needs neither Rust nor npm
lifecycle scripts.
:::

:::mara scenario SCN-SELECT-BROWSER-MARA
:mid: 01M3YVZA35AATJQV5V42DNGQCC
:title: Select browser-local or daemon execution behind one client
:status: draft
:contributes_to: GOAL-UNIFIED-PROJECT-KNOWLEDGE

A browser application uses one typed document-operation interface.
Configuration selects either authenticated loopback HTTP against a native
project or a bundled worker/WASM engine against supplied project sources.
Application operation code does not implement transport or Mara semantics.
Backend capabilities and disk-versus-memory persistence remain explicit. A
graphical application is not required to deliver this client.
:::

:::mara scenario SCN-DRAFT-MARA-BROWSER
:mid: 01M3YVZMSF5JM22HEPCXXXHXQS
:title: Prepare repository changes entirely in the browser
:status: draft
:contributes_to: GOAL-UNIFIED-PROJECT-KNOWLEDGE

A browser client loads an explicit project snapshot, searches and navigates it,
creates and revises knowledge, and evaluates document rules and trace matrices
locally. Mutations change only its in-memory workspace. The caller exports a
final file change set against the original snapshot for later publication. No
GitHub access, server computation or commit occurs inside the engine.
:::

## Requirements

:::mara requirement REQ-SHARED-TYPESCRIPT-CLIENT
:mid: 01M3YW1YGZFKDC24E6X385V5S9
:title: Use one asynchronous typed API across JavaScript backends
:status: draft
:kind: functional
:derives_from: SCN-SELECT-BROWSER-MARA
:derives_from: SCN-IMPORT-MARA-NODE

Provide one shared TypeScript client with N-API, daemon HTTP and worker/WASM
backend adapters. Keep document-operation names, input/result types, errors and
continuation conventions consistent; backend-specific project setup and
capabilities remain explicit. Do not duplicate parsing, validation or mutation
rules in JavaScript.

The public Node and browser library entry points are ESM-only. Ship TypeScript
declarations. Native work and browser engine computation must not block their
host event loops. Selecting a browser backend is explicit; no automatic
daemon-to-WASM fallback is implied.
:::

:::mara requirement REQ-NODE-NATIVE-BINDING
:mid: 01M3YW296F1AFVJVA4HTM8WCGK
:title: Distribute a directly importable native Node integration
:status: draft
:kind: functional
:derives_from: SCN-IMPORT-MARA-NODE

Ship a small N-API binding crate using napi, napi-derive and napi-build with
crate-type = ["cdylib"], a JS loader, declarations, build/package support and a
real installed-package integration test. The binding delegates to the Rust
high-level client and supports embedded and daemon execution without CLI
subprocesses.

Extend @convesoft/mara with separate Node/browser library entry points while
preserving its CLI entry. Require Node 24 or newer. Keep the current native
targets: GNU Linux x64/arm64 and Apple Silicon macOS. Prebuilt platform
packages contain the CLI binary and .node addon; installation does not
compile/download through lifecycle scripts or need Rust.
:::

:::mara requirement REQ-BROWSER-LOCAL-WORKSPACE
:mid: 01M3YW2KYJCWH13T6ABPX4HFXX
:title: Execute the complete document API in a browser worker
:status: draft
:kind: functional
:derives_from: SCN-DRAFT-MARA-BROWSER

Ship a bundled WASM engine and worker-backed browser client accepting an
explicit project snapshot. Support the document operations in
[[REQ-PORTABLE-DOCUMENT-ENGINE]], including configured document rules and trace
matrices, not merely a search/read/edit prototype.

Mutations apply to browser memory only and preserve the same document
invariants. Rejected operations preserve the workspace. No filesystem
discovery, native subprocess, GitHub fetch, commit or PR publication occurs in
the engine. Loading and final publication are caller responsibilities. Browser
persistence beyond the in-memory workspace is outside 0.4.0.
:::

:::mara requirement REQ-BROWSER-CHANGE-EXPORT
:mid: 01M3YW2YPVJFDGQSB76NQ8N6JP
:title: Export final file changes with their original-state preconditions
:status: draft
:kind: functional
:derives_from: SCN-DRAFT-MARA-BROWSER

The browser client must export a versioned file change set relative to the
loaded base snapshot. Include additions, modifications and deletions, their
final source contents, original-state preconditions and base identity.
Consolidate intermediate edits; omit files restored to their base contents.

Export is not publication and does not reset the base or claim that an external
revision is current. It contains source data and preconditions, not executable
publication instructions. Hosts must compare the preconditions before applying
changes. Git operations, semantic revision comparison and item history are not
part of this export.
:::

:::mara requirement REQ-EXPLICIT-BACKEND-CAPABILITIES
:mid: 01M3YW39EZDRDS3J3J8459808Z
:title: Report unsupported work without inventing successful coverage
:status: draft
:kind: constraint
:derives_from: SCN-SELECT-BROWSER-MARA
:derives_from: SCN-DRAFT-MARA-BROWSER

Clients must expose supported operations, code capabilities and mutation
persistence. Native mutations write the project working tree; WASM mutations
write only its memory workspace.

WASM 0.4.0 neither runs indexers nor imports native code-index snapshots.
Document-only execution must not interpret missing code coverage as an empty
valid code graph. Code-dependent validation/matrix work is explicitly
unavailable; edits requiring absent code information for source/reference
safety are refused. Independent document operations remain available. Native
locking, transaction rollback and daemon lifecycle are native-only rather than
fake browser successes.
:::

## Interfaces and artifacts

:::mara design DES-JAVASCRIPT-CLIENT-ADAPTERS
:mid: 01M3YW7F82ACNVG79WWN6AG0S7
:title: Share one JS facade without importing native code into browsers
:status: draft
:kind: interface
:satisfies: REQ-SHARED-TYPESCRIPT-CLIENT
:satisfies: REQ-EXPLICIT-BACKEND-CAPABILITIES

Bind each client to an explicitly selected project/workspace context. N-API
delegates native backend selection to the Rust client; HTTP targets a selected
native project with supplied session authorization; WASM targets a supplied
source snapshot. Expose document operations through Promise-returning typed
methods and keep existing result formats, source locations and continuation
conventions. Distinguish typed operation/backend errors from invalid validation
results.

Expose capabilities and filesystem-versus-memory mutation persistence before
operations. Unsupported work, including native lifecycle/recovery on WASM,
fails explicitly rather than pretending to succeed. Node loads the addon
without spawning the CLI. Browser ESM imports must not eagerly import Node
built-ins, N-API or filesystem loaders. The WASM adapter owns worker startup,
messages and shutdown; closing a browser/Node client must not shut down a
shared daemon.

No implicit browser backend switching or draft publication occurs. The
application supplies native project selection or browser snapshot loading and
handles external publication.
:::

:::mara design DES-BROWSER-PROJECT-SNAPSHOT
:mid: 01M3YW88YJ88ZA9277BQPX2QB2
:title: Load an explicit source snapshot into the portable engine
:status: draft
:kind: data
:satisfies: REQ-BROWSER-LOCAL-WORKSPACE
:satisfies: REQ-EXPLICIT-BACKEND-CAPABILITIES

Define browser source-snapshot format version 1: project-relative UTF-8 file
contents plus an optional caller-supplied base identity. Reject duplicate
normalized paths and paths escaping the logical root. The producer supplies the
complete effective document set, required configuration/schema/rule sources and
the selection/ignore inputs needed to evaluate candidate document membership.
Filesystem/Git discovery belongs to that producer, not a hidden browser
filesystem walk. The engine must preserve configured selection and ignore
behavior for creation/movement; it cannot assume every new path is included.
Missing required inputs remain diagnostics/errors rather than being fetched or
invented.

The engine assigns a source fingerprint from the supplied inputs and uses a
logical workspace root, not a host absolute path. Caller base identity is
opaque provenance, not proof that a Git revision is current. An empty workspace
supports explicit initialization; replacing/loading a snapshot establishes a
new base. Mutations retain the original base for export and update workspace
fingerprints so stale cursors/handles are rejected.

Native code-index inputs are not supported in 0.4.0, and commands declared in
configuration are never executed. A supplied document set does not prove
repository-wide code completeness; declare unavailable code capabilities under
[[REQ-EXPLICIT-BACKEND-CAPABILITIES]].
:::

:::mara design DES-BROWSER-FILE-CHANGESET
:mid: 01M3YW8KMKMHM7PNHXV2XAXB6Q
:title: Export a consolidated source delta, not a publication command
:status: draft
:kind: data
:satisfies: REQ-BROWSER-CHANGE-EXPORT

Define file-change-set format version 1 with the base source fingerprint,
optional caller base identity and ordered file changes. Each change has a
normalized project-relative path, before (exact original UTF-8 content, or null
meaning the path must be absent) and after (final UTF-8 content, or null
meaning deletion). Reject duplicate paths and omit unchanged files; additions
later deleted and edits restored to base produce no change.

The export is computed from base versus current workspace, not an operation
log. Exporting does not publish or advance the base. A host compares
original-state preconditions and its external revision before publication;
native filesystem permissions, journals and GitHub credentials are not part of
this format. All exported source remains ordinary canonical files under
[[REQ-CANONICAL-SOURCE]].
:::

:::mara design DES-INTEGRATION-ARTIFACTS
:mid: 01M3YW8YB1YQJVE6QKQSJK92ST
:title: Ship version-aligned native and browser library artifacts
:status: draft
:kind: interface
:satisfies: REQ-NODE-NATIVE-BINDING
:satisfies: REQ-SHARED-TYPESCRIPT-CLIENT
:satisfies: REQ-BROWSER-LOCAL-WORKSPACE

Extend @convesoft/mara with a Node ESM library entry and a separate browser ESM
entry, TypeScript declarations and bundled worker/WASM assets. Keep the CLI
dispatcher separate; its internal CJS format does not establish public CJS
library support. Browser assets are platform-independent and usable without
native optional dependencies.

A small cdylib binding crate uses napi/napi-derive and napi-build setup;
bindings call the Rust client asynchronously. Existing platform
optional-dependency packages add the .node addon alongside the executable,
retaining exact workspace-version selection and current host/glibc boundaries.
Build/package scripts produce these artifacts before publication; packages
contain no install-time downloader or compiler.

All Rust crates and generated npm manifests inherit the workspace version. Node
24+ replaces the 0.3 Node 18 floor for 0.4 npm artifacts. Browser packaging
must verify worker/WASM asset resolution from an installed package, not only
source-tree imports.
:::

## Decision

:::mara decision ADR-ESM-NODE-LTS-INTEGRATION
:mid: 01M3YW9YKMJQ1KG4YAPBP3J3TK
:title: Use ESM and a fixed modern Node baseline
:status: draft
:justifies: DES-INTEGRATION-ARTIFACTS
:justifies: REQ-NODE-NATIVE-BINDING

Choose ESM-only library entry points rather than maintain dual CJS/ESM builds
and declarations without existing external library consumers. Retaining a CJS
CLI launcher is independent of the library interface.

Set Node 24 as the fixed minimum rather than a moving current-LTS condition or
the 0.3 Node 18 floor. Reuse @convesoft/mara and its native optional packages
instead of adding a separate integration package family. Publish browser assets
without requiring a native addon.
:::

## Verification

:::mara verification VER-INSTALLED-JAVASCRIPT-CLIENT
:mid: 01M3YWAM1S59307YC4HYNPVGNQ
:title: Verify installed Node and browser transport artifacts
:status: draft
:method: test
:level: system
:verifies: REQ-NODE-NATIVE-BINDING
:verifies: REQ-SHARED-TYPESCRIPT-CLIENT
:verifies: REQ-LOCAL-DAEMON-HTTP
:verifies: DES-JAVASCRIPT-CLIENT-ADAPTERS
:verifies: DES-INTEGRATION-ARTIFACTS
:verifies: DES-LOCAL-DAEMON-PROTOCOL
:validates: SCN-IMPORT-MARA-NODE
:validates: SCN-SELECT-BROWSER-MARA

Build and pack the candidate main/native packages, install them with lifecycle
scripts disabled in disposable storage, and import the Node ESM API on Node 24.
Load the real addon and run project initialization, search/read, mutation,
validation and continuation through embedded and daemon execution without
spawning the CLI. Verify declarations against real usage with the standard
TypeScript compiler. Retain installed CLI/MCP smoke.

Require artifact/runtime smoke on each supported native host for release.
Verify exact workspace versions, addon/loader/types inventories, browser
assets, Node 24 minimum and absence of install-time downloads/builds or public
CJS-library promises.

In real Chromium, import the installed browser client and exercise
authenticated loopback HTTP against a real daemon. Verify equivalent operation
results, disk persistence, disabled-by-default HTTP and rejection of
missing/incorrect tokens and disallowed origins before side effects. Check
mismatched version refusal and that browser imports need no Node
built-ins/addon. Chromium is the browser release gate; Firefox/WebKit
compatibility is not certified by that run.
:::

:::mara verification VER-BROWSER-DOCUMENT-ENGINE
:mid: 01M3YWAYS67D7NTZ3YZ8RFGJRE
:title: Verify the complete document profile in real browser WASM
:status: draft
:method: test
:level: system
:verifies: REQ-PORTABLE-DOCUMENT-ENGINE
:verifies: REQ-BROWSER-LOCAL-WORKSPACE
:verifies: REQ-BROWSER-CHANGE-EXPORT
:verifies: REQ-EXPLICIT-BACKEND-CAPABILITIES
:verifies: DES-SEMANTIC-CHANGE-PLANS
:verifies: DES-BROWSER-PROJECT-SNAPSHOT
:verifies: DES-BROWSER-FILE-CHANGESET
:verifies: DES-JAVASCRIPT-CLIENT-ADAPTERS
:validates: SCN-DRAFT-MARA-BROWSER

Use the packaged worker/WASM client in real Chromium against fixture-owned
source snapshots. Exercise initialization/MID backfill, schema operations,
bounded list/search/get/related, every item and relation mutation, validation,
graph policies, configured document rules and request-local trace matrices.
Compare native and browser outcomes for the same effective inputs, accounting
only for generated identities and host-root context. Include representative
nested/qualified/literal/parameterized rules from the supported profile; a
source-only validator does not establish rule parity.

Verify worker-backed asynchronous execution, memory-only edits, rejected-edit
preservation, Unicode source/metadata, source-preserving reference safety and
stale handles/cursors. Export a file change set, check exact original-state
preconditions and consolidated additions/modifications/deletions, then
materialize its sources in a disposable native project and validate them
through the real CLI. No GitHub publication is claimed.

Reject incomplete required source inputs and unsafe/duplicate paths. Verify
native-only/code-dependent work is explicitly unavailable, no missing code
coverage appears as a pass, and unsafe edits are refused without changing the
workspace. A mock worker or Node-only WASM run is not browser execution
evidence. Record the candidate revision and actual artifact/browser results.
:::
