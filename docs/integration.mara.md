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
publication instructions. Before applying changes, hosts must verify the whole
effective base snapshot and each changed file's original-state precondition.
Divergence in unchanged documents, configuration, schema, rules, selection or
ignore inputs also rejects publication; checking changed files alone is
insufficient. Git operations, semantic revision comparison and item history are
not part of this export.
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
unavailable.

WASM document mutations retain their owning source-conformance prerequisites;
there is no weaker partial-conformance editing profile. When unavailable code
information prevents complete source conformance, refuse all mutations against
that workspace without changing it. Document-only workspaces remain editable;
unused schema code-relation declarations alone do not require code information.
Independent document reads and schema checks remain available. Later code
support must satisfy the same gate rather than change the safety contract.

Native locking, transaction rollback and daemon-wide lifecycle are native-only
rather than fake browser successes. HTTP authority follows
[[REQ-LOCAL-DAEMON-HTTP]].
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

The engine assigns a source fingerprint over the normalized source inventory,
exact file contents and supplied selection/ignore inputs. It uses a logical
workspace root, not a host absolute path. Caller base identity is
opaque provenance, not proof that a Git revision is current. An empty workspace
supports explicit initialization; replacing/loading a snapshot establishes a
new base. Mutations retain the original base for export and update workspace
fingerprints so stale cursors/handles are rejected.

Native code-index inputs are not supported in 0.4.0, and commands declared in
configuration are never executed. A supplied document set does not prove
repository-wide code completeness; declare unavailable code capabilities under
[[REQ-EXPLICIT-BACKEND-CAPABILITIES]].

The Rust source boundary represents v1 as `SourceSnapshot {format_version,
files, selection, base_identity}`. `files` contains `{path, content}` entries.
Normalize current-directory components and repeated separators; reject empty,
absolute, parent-containing, backslash/drive-prefixed and duplicate normalized
paths. Preserve exact UTF-8 contents and Unicode paths.

`selection.document_paths` declares the complete selected document inventory.
A listed source without contents makes conformance incomplete; a supplied
included document omitted from that inventory is an invalid selection input.
`selection.ignores` supplies each effective Git ignore text with its
root-relative directory (empty for root). An inherited ignore instead carries
`root_prefix`: exact relative path components from its ancestor origin to the
logical root. These preserve anchored ancestor patterns without absolute host
paths. Ignore matching strips leading UTF-8 BOM characters as native Git ignore
loading does, while fingerprints retain the exact supplied text. Origins must be distinct and inherited prefixes consistent. Apply
ancestor-to-descendant precedence and directory exclusion; a child cannot be
re-included while its parent remains ignored. Membership uses configured
include patterns with literal separators. No `.ignore`, Git global/exclude,
filesystem-parent discovery or followed symlinks are inferred by the engine.

The fingerprint hashes normalized, ordered file inventory, exact contents and
effective selection/ignore inputs with SHA-256. Opaque base identity is not
hashed as source state. Native producers and document-only callers use this
same source identity; code-capability inputs remain separately represented.
The delivered memory host and worker message/lifecycle boundary are defined by
[[DES-BROWSER-WORKER-BINDING]].
:::

:::mara design DES-BROWSER-WORKER-BINDING
:mid: 01M41JH7X4W2NN5ZBH4QERM0MD
:title: Host the complete document engine in an isolated memory worker
:status: draft
:kind: interface
:satisfies: REQ-BROWSER-LOCAL-WORKSPACE
:satisfies: REQ-EXPLICIT-BACKEND-CAPABILITIES

`mara-wasm` is an `rlib`/`cdylib` memory host depending on `mara-engine`, not
native Mara. `MemoryWorkspace` retains normalized base sources separately from
current `SourceEngine` state. `execute(Request, generate_mid)` delegates document
queries, source checks, configured rules, trace matrices and all semantic planners
to that engine. It serializes an accepted result before applying its non-forgeable
candidate with `apply_plan`; failure preserves current and base state. Rules are
computed for the exact validation target/source, not omitted or synthesized.

The wasm-bindgen `Workspace` starts with an empty v1 snapshot. `request(json)`
accepts `{operation, params}` and returns JSON `{ok:true,result}` or
`{ok:false,error}`; `error` is an operation envelope, while an invalid validation
result is still a successful operation result. Existing document parameters,
result versions, locations and continuation conventions are retained with
logical project root `.`. Creation/backfill identities come from browser Web
Crypto and host time; callers cannot supply MIDs in operation parameters.

`npm/browser/worker.mjs` is a dedicated module worker. Requests are
`{format_version:1,id,operation,params?}` with safe-integer correlation IDs;
omitted/null params mean `{}`. Responses retain `format_version` and `id` plus
the binding response. A serialized queue includes WASM startup and prevents
interleaved replacement/editing. One worker owns one isolated workspace;
startup failures return correlated errors. The caller creates the worker using
its bundled asset URL and terminates it with `Worker.terminate()`. Termination
loses memory; this host has no daemon shutdown, disk persistence or recovery.
The shared typed client and installed-package asset resolution are separate.

Host setup/read operations are `capabilities`, `load_snapshot`, `workspace_state`
and `workspace_sources`; export/checking operations are defined by
[[DES-BROWSER-FILE-CHANGESET]]. Capabilities list the complete supported operation names,
`code:{indexing:false,native_index_import:false}`, memory mutation persistence
and no native recovery. Native lifecycle/rollback and other unsupported names
fail explicitly. Workspace state returns current/base source fingerprints and
opaque base identity; workspace sources returns exact normalized `{base,current}`
snapshots for caller inspection, not a file change set or publication.

`load_snapshot` accepts [[DES-BROWSER-PROJECT-SNAPSHOT]] directly as params.
It rejects malformed paths/selection and absent selected documents, project
configuration, configured schema or configured rule sources before replacement.
An empty inventory is the explicit uninitialized workspace. Supplied invalid
source/configuration remains diagnosable by recovering validation; loading does
not certify conformance. A schema path is required only when decoded from
configuration; a parser's recovery fallback does not invent a required source.
Included documents cannot be omitted from selection.
Only successful replacement establishes a new base and discards previous edits;
ordinary mutations, including initialization, retain the loaded base. Owning
source fingerprints invalidate stale continuation and affected-document handles;
no-op edits retain them. Missing code required for source conformance blocks every
mutation through the document-only planner, not just code-related edits.

`scripts/build-browser.sh OUTPUT_DIRECTORY` uses locked Cargo and matching
wasm-bindgen CLI 0.2.127 (`--target web`) to produce `mara_wasm.js`,
`mara_wasm_bg.wasm`, their declarations and `worker.mjs`. Supply disposable
`TMPDIR`/`CARGO_TARGET_DIR`; generated assets are build outputs, not tracked
canonical source. For `tests/browser/playwright.config.mjs`, also set
`BROWSER_RUNTIME_TMPDIR` to an existing private, session-owned directory under
`XDG_RUNTIME_DIR`. Keep it short enough for Chromium's generated Unix socket
path (at most 107 bytes on Linux). Only Chromium's process tree receives this
`TMPDIR` override; Playwright-created profiles, native fixtures, assets, results
and retained reports stay on disk-backed storage. Record runtime-directory
ownership and remove it after the browser stops. Serve these files together so
the worker's relative imports and WASM URL resolve. No application sources or
configured commands are fetched or executed. `tests/browser/workspace.spec.mjs` is a real Playwright/Chromium
worker smoke, not final installed-client/full-profile certification under
[[VER-BROWSER-DOCUMENT-ENGINE]].
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
log. Exporting does not publish or advance the base. The base source
fingerprint represents the full effective inputs under
[[DES-BROWSER-PROJECT-SNAPSHOT]], not only changed paths. Before applying any
change, the host must match that full base, check every changed-file preimage
and verify its external revision context. Reject divergence without applying
changes; loading a replacement snapshot establishes a new base, not an
automatic rebase of prior edits.

The delivered `mara-wasm::FileChangeSet` JSON is `{format_version:1,
base_source_fingerprint, base_identity, changes}`. `base_identity` is an opaque
string or null. `changes` contains `{path, before, after}` entries ordered by
normalized path (Rust string order); before/after are strings or null, with
empty strings distinct from absence. `MemoryWorkspace::export_changes()` and
worker operation `export_changes` (empty params) compute this final delta without
mutating workspace state. Initialization against an empty base exports its new
sources; item deletion retains its containing file, so an empty retained file
is not a file deletion.

`FileChangeSet::check_preconditions(snapshot)` and worker operation
`check_change_set_preconditions` with `{change_set, snapshot}` provide the
read-only host checking path. The snapshot must be freshly produced from the
host's complete effective inputs under [[DES-BROWSER-PROJECT-SNAPSHOT]], not
copied from the export's original base as a substitute for inspecting current
inputs. The checker normalizes and checks required inventory through the memory
snapshot loader, matches the whole-base fingerprint and checks every changed
file's exact preimage. Unsupported versions, non-normalized/duplicate/out-of-order
paths and unchanged entries are rejected. Worker failures return an operation
error with code `precondition_failed`; malformed request decoding uses
`invalid_argument`. Success returns `{format_version:1,
source_preconditions_match:true, external_context_verified:false}`. Neither
checking nor exporting writes or advances memory.

Caller base identity is not compared as source state. Even a successful source
check does not prove external revision currency or authorize application. The
host must separately verify its external context, establish complete effective
inputs and prevent divergence between checking and application using its own
concurrency/publication boundary. Complete all source/preimage checks before
any write; reject a failed check without partial application. This interface
provides no application method, filesystem atomicity or semantic rebase.

Native filesystem permissions, journals and GitHub credentials are not part of
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

:::mara decision ADR-BROWSER-WHOLE-BASE-PRECONDITIONS
:mid: 01M40DSJ7QKKDFEAYJM1W96DEW
:title: Require the complete browser base before applying exported changes
:status: draft
:justifies: REQ-BROWSER-CHANGE-EXPORT
:justifies: DES-BROWSER-FILE-CHANGESET

Choose whole-base preconditions rather than checking only touched files or
implicitly rebasing an export. Unchanged documents, schema, rules, inventory
and selection/ignore inputs can change whether an edit preserves references or
belongs to the corpus; matching changed-file preimages alone cannot preserve
the engine's accepted source-safety assumptions.

Reject a divergent base before application. Loading a new snapshot permits new
validated edits without claiming semantic merge or revision-comparison support.
Publication remains the caller's responsibility.
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
:verifies: DES-DAEMON-LIFECYCLE

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

In real Chromium, serve the installed browser client from a loopback origin and
exercise authenticated loopback HTTP against a real daemon. Obtain connection
information through the explicit local command; verify routine status/logs do
not disclose tokens. Verify equivalent operation results, disk persistence,
disabled-by-default HTTP and rejection of missing/incorrect tokens, disallowed
origins and unauthorized project targets before file access or command
execution. Authorizing one root must not authorize descendants or another Git
worktree; native IPC selection remains unchanged.

Exercise initialization at an explicitly authorized destination and project
rollback/refresh. Refuse daemon-wide administration over HTTP. Check mismatched
version refusal and that browser imports need no Node built-ins/addon.
Chromium with a loopback-served application is the HTTP browser release gate.
Origin configuration remains explicit; hosted HTTPS-to-daemon access and
Firefox/WebKit compatibility are not certified by that run.
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
document-only source snapshots. Exercise initialization/MID backfill, schema
operations, bounded list/search/get/related, every item and relation mutation,
validation, graph policies, configured document rules and request-local trace
matrices.
Compare native and browser outcomes for the same effective inputs, accounting
only for generated identities and host-root context. Include representative
nested/qualified/literal/parameterized rules from the supported profile; a
source-only validator does not establish rule parity.

Verify worker-backed asynchronous execution, memory-only edits, rejected-edit
preservation, Unicode source/metadata, source-preserving reference safety and
stale handles/cursors. Export a file change set, check exact original-state
preconditions and consolidated additions/modifications/deletions, then
materialize its sources in a disposable native project and validate them
through the real CLI. Verify whole-base checks reject changes to an untouched
document, configuration/schema/rule source, effective inventory or
selection/ignore input even when every changed-file preimage still matches.
No divergent-base application or GitHub publication is claimed.

Reject incomplete required source inputs and unsafe/duplicate paths. Verify
native-only/code-dependent work is explicitly unavailable and no missing code
coverage appears as a pass. In a workspace whose source conformance requires
unavailable code information, refuse every mutation, including otherwise
document-local updates, without changing memory or its export. Retain
independent reads/schema checks and full editing of document-only workspaces,
including an engineering schema with unused code-relation declarations. A mock
worker or Node-only WASM run is not browser execution evidence. Record the
candidate revision and actual artifact/browser results.
:::
