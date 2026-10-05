# JavaScript and browser integration for 0.4.0-alpha.1

Target contracts for the [0.4.0-alpha.1 scope](0.4.mara.md). The shared client uses the
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
persistence beyond the in-memory workspace is outside 0.4.0-alpha.1.
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

WASM 0.4.0-alpha.1 neither runs indexers nor imports native code-index snapshots.
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

The public generated npm artifact exports ESM-only `@convesoft/mara` and
`@convesoft/mara/node` from `node.mjs`/`node.d.mts`, and browser-safe
`@convesoft/mara/client` from `client.mjs`/`client.d.mts` and explicit browser-safe
`@convesoft/mara/http` from `http.mjs`/`http.d.mts`. Node root/node exports
`createNativeClient`, the shared `createClient`, `MaraClientError` and
`clientError`; the client entry exports only the shared facade/error helpers.
The HTTP entry exports `createHttpClient`, `HttpClientOptions` and the shared
facade/error helpers/types; it imports no Node or native modules.
Declarations export the owning parameter/result types, `OperationInputs`,
`OperationResults`, `DocumentOperation`, `ClientAdapter`, `ClientContext`,
`Capabilities` and `MaraClient`. These entries have import, not require,
conditions; the separate `bin/mara.cjs` CLI keeps its existing behavior.

`await createNativeClient(options)` requires absolute `callerDirectory`;
optional `project` is absolute or relative to it, `mode` is
`direct|auto|daemon-required` (default auto), and `daemonDirectory` is an absolute
private native IPC directory. Omitted project preserves nearest-project discovery.
Options do not read MARA environment defaults or start a daemon. The loader
selects the supported host's exact-version optional package and `bin/mara.node`.
An optional absolute `addonPath` selects a local prebuilt addon instead; both
paths check its advertised application version against the generated main
package manifest before returning a client. Missing/load-failed addons are typed
unavailable failures; package/addon mismatch is incompatible. Host packaging and script-free installed-addon gates follow
[[DES-INTEGRATION-ARTIFACTS]]; actual release/host certification remains separate.

`await createClient(adapter)` transfers adapter ownership, obtains capabilities
through `execute('capabilities', {})`, and returns one `MaraClient`. The injected
adapter supplies `{context, execute(operation, params), close()}`; it owns setup,
transport, owning results and typed errors. The injected boundary does not
define backend factories; the delivered HTTP and WASM factories are specified below.
Setup failure closes the adapter and preserves the
original error. Initialization is adapter-specific through
`MaraClient<InitParams>`, `ClientAdapter<InitParams>`, `OperationInputs<InitParams>`
and `createClient<InitParams>(adapter)`. Defaults and the Node factory use
`ProjectInitParams` (`project?`, `template?`); native initialization does not accept
`name`. The existing worker/WASM input is `MemoryProjectInitParams`
(`name` required, `template?`, no native project override). A typed injected
memory consumer selects that generic explicitly; its `projectInit` requires the
parameter object/name. Both retain the owning project-summary result. The WASM
factory returns this memory-specific initialization type without a native override.
`client.context` is an immutable `{backend, selection}` setup
snapshot, not a resolved-root claim. Native selection contains its supplied
caller/project/mode/daemon options, excluding the addon path. Immutable
`client.capabilities` exposes supported operation names, code support and
mutation persistence before document operations; native capabilities also report
backend, application version and owning result formats. Adapter-owned backend
identity is always available in context.

Public methods take named owning parameter objects and return Promises:
`projectInit`, `projectMidBackfill`, `projectTransactionRollback`, `projectRefresh`,
`schemaGet`, `schemaList`, `schemaValidate`, `projectValidate`, `itemValidate`,
`itemList`, `search`, `get`, `related`, `itemCreate`, `itemUpdate`, `itemMove`,
`itemDelete`, `itemRename`, `relationGet`, `relationAdd`, `relationRemove`, and
`traceMatrix`. Each maps to the identically named snake_case native operation
under [[DES-LOCAL-DAEMON-PROTOCOL]]; input/result fields stay snake_case. Methods
without required owning fields allow omitted params (`{}`). Unbound native calls
may supply absolute `project`; bound handles reject overrides. Continuations are
explicit `cursor`/`next_cursor` with `has_more`: repeat unchanged inputs and page
limits until complete, and restart after source/schema/configuration changes.
There is no automatic pagination, replay or backend switching.

`MaraClientError` exposes `kind`, stable string `code` and the complete owning
`envelope` (including relationship context). `clientError(kind, code, message)`
constructs error format 1 for adapters. Native operation/routing classifications
and unknown-outcome no-replay/source-inspection guidance pass through unchanged.
Invalid validation, including incomplete evaluation, remains fulfilled owning
data; it is not a rejected Promise. Unsupported advertised operations reject
with kind `unavailable`/code `unsupported_operation`. Shared calls snapshot JSON
inputs at invocation and queue in order; non-JSON-encodable input, including
non-finite numbers, functions, symbols, cyclic data and non-JSON objects, rejects
with `invalid_argument` before dispatch. Undefined optional object properties
mean omission; arrays require JSON values and are copied by indexed entries,
not custom iterators. Snapshotting never invokes `toJSON` or silently drops an
invalid position into an omitted/null append request. `close()` drains earlier calls, preserves the same close
Promise/outcome on repetition, rejects later document calls with
`closed`/`client_closed`, and releases only its adapter. It sends no daemon stop.
Browser-safe shared modules have no Node/addon/filesystem imports; injected
adapters retain responsibility for their own resources. Publication remains
caller-owned.

The browser ESM entry `@convesoft/mara/browser` (`browser.mjs`/`browser.d.mts`)
exports `createWasmClient` and the shared facade/error helpers and types.
`await createWasmClient({snapshot})` requires an explicit v1
`SourceSnapshot` under [[DES-BROWSER-PROJECT-SNAPSHOT]], including
`{format_version:1,files:[],selection:{document_paths:[],ignores:[]}}` for an
uninitialized workspace. It snapshots options at invocation, starts one dedicated
module worker at `new URL('./browser/worker.mjs', import.meta.url)`, obtains
capabilities, and loads the supplied snapshot before returning. There is no
worker injection, native project discovery, HTTP fallback or automatic replay.
Setup failures terminate the owned worker and preserve the original typed error.

The returned `WasmClient extends MaraClient<MemoryProjectInitParams>` includes
all shared document methods plus `loadSnapshot(snapshot)`, `workspaceState()`,
`workspaceSources()`, `exportChanges()` and
`checkChangeSetPreconditions({change_set,snapshot})`. All return Promises and
share invocation-time JSON snapshots, ordered execution and the same close
boundary. The extension result types are `WorkspaceState`, `WorkspaceSources`,
`FileChangeSet` and `ChangeSetPreconditionsResult`, retaining the memory host's
owning snake_case JSON fields. Snapshot/ignore and change-set types follow their
owning designs, not a second JS data format. Snapshot replacement discards edits
only on success; export/checking never advances the base or publishes.

Context is `{backend:'wasm',selection:{snapshot_format_version,base_identity}}`
for the initial supplied selection; it does not update after replacement or
claim an external revision is current. `workspaceState()` reports the current
and original-base fingerprints/identity. Capabilities remain immutable host
capabilities: memory persistence, no native recovery/indexing/import and explicit
supported operation names. Native project overrides are not memory setup inputs;
the owning engine rejects unsupported parameters and required-code mutations.

Owning operation errors retain their envelopes, including relationship context.
`invalid_argument` uses that failure kind; unsupported operations, unavailable
Web Crypto and correlated WASM startup failure use `unavailable`. Constructor
failure is `unavailable/worker_unavailable`; WASM startup failure is
`unavailable/worker_startup_failed`. A worker error, undecodable response or
invalid correlation terminates the worker and rejects pending work with
`unknown_outcome/worker_transport_failed`: dispatched work may have completed,
so do not replay; reload an explicitly selected snapshot in a new client.
A failed postMessage is `transport/worker_dispatch_failed` without dispatch or
retry. Unexpected worker computation errors retain `worker_error` with kind
`unknown_outcome`. Invalid or incomplete validation remains fulfilled data.
`close()` drains prior document and memory calls, terminates this worker,
releases its memory, and keeps its idempotent Promise. It does not stop a daemon.

The lower-level private Node adapter remains `crates/mara-node/loader.mjs`:
`loadAddon(absoluteAddonPath)(options)` creates a handle with
`execute(operation, params = {}) -> Promise<owning result>` and
`close() -> Promise<void>`. Its small declarations describe this addon boundary,
not the downstream public per-operation TS facade. The raw addon exports
`NativeHandle(options)`, with Promise-returning `execute(operation, params)` and
`close()` producing `{ok:true,result}` or `{ok:false,kind,error:owning envelope}`;
the loader unwraps these responses and supplies ordering. Options require absolute
`callerDirectory`; optional `project` resolves relative to it, optional `mode`
is direct/auto/daemon-required (default auto), and optional `daemonDirectory`
selects the private IPC directory. These explicit library options do not read
MARA environment defaults. Malformed native setup data, including N-API argument
conversion failures, rejects with `invalid_argument`; addon loading failures remain
`unavailable`. Unbound calls may supply absolute `params.project`;
bound handles refuse overrides. Initialization follows the same selection rules.

Operation names and parameters match the native protocol's document/setup
operation set, plus local `capabilities`. Results keep their owning objects,
formats, source locations and opaque cursors. JSON numeric results remain JS
numbers, not BigInts, with ordinary JavaScript number precision (including
exact safe integers), so owning results remain JSON-serializable. Capabilities report native code
indexing, filesystem `mutation_persistence`, recovery, refresh, operation names
and result formats. Invalid validation remains a fulfilled result.
`NativeClientError` retains `kind`, stable `code`, and the complete owning
`envelope`; kinds distinguish operation, invalid_argument, unavailable,
incompatible, connection, transport, unknown_outcome and closed. Unknown outcomes
retain the native no-replay/source-inspection guidance.

Each handle snapshots JSON-compatible parameters at invocation and queues calls,
including close, in order. Rust N-API tasks run host work on libuv workers;
only argument/result conversion occurs on the JS thread. Close drains preceding
calls, drops the native client/session ownership, rejects later calls with
`client_closed`, and is idempotent. GC also releases an unused handle; neither
path sends daemon shutdown. Independent handles remain independently usable.

`await createHttpClient({endpoint, token, project})` explicitly selects daemon
HTTP. All three string fields are required; extra setup fields are rejected.
`endpoint` is the deliberately handed-off numeric loopback `http://127.x.x.x[:port]/rpc`
or `http://[::1][:port]/rpc` address, without credentials, query or fragment.
`token` is the daemon-session bearer credential; `project` is an absolute native
root or authorized initialization destination. No filesystem discovery, pairing,
policy/credential administration or daemon startup/stop occurs. Bind every call
to that project and reject per-call `project` fields, including null. To use a
different project, construct another client. Public context is
`{backend:"http",selection:{endpoint,project}}`; it excludes the token.

The generated browser-safe `version.mjs` inherits the Cargo workspace version,
not the planned milestone label. Setup sends `mara.negotiate` with exact artifact
application/protocol versions, and every POST sends both version headers under
[[ADR-HTTP-REQUEST-NEGOTIATION]]. Refuse mismatched versions, owning result formats
or native capability/operation advertisements before returning the client.
Expose native filesystem persistence, code indexing/configured-indexer support,
rollback and refresh through the same shared `Capabilities`; context identifies
HTTP transport while capabilities identify the native execution backend.

Send one JSON-RPC request per call with browser `fetch`, bearer authorization,
JSON Content-Type, no credential cookies, redirects or referrer, and no automatic
pagination, retries or backend fallback. The browser supplies Origin; applications
must serve from an explicitly allowed origin. Reject bodies exceeding the owning
1 MiB request budget before sending. Params/results remain owning JSON, including
ordinary JS numeric precision, locations, cursors and validation-invalid data.
Preserve complete owning error envelopes, including relationship context.
HTTP 401 and 403 reject with `authentication`/`authentication_failed` and
`authorization`/`authorization_failed`; version failures remain `incompatible`.
Known pre-dispatch framing refusals reject as `invalid_argument`, including
validated null-ID parse/invalid-request RPC errors that cannot echo an undecoded
request ID; other mismatched IDs do not establish an outcome. Stopping-daemon
503 rejects as `unavailable`/`daemon_unavailable`. Network/CORS failure during
setup or reads is `connection`/`http_connection_failed`; unreadable/malformed read
responses are `transport`/`http_transport_error`. Browser CORS cannot distinguish
a blocked origin from a network failure. Do not attach raw fetch exceptions,
URLs containing credentials or bearer tokens to diagnostics.

After an attempted mutation dispatch, response loss, malformed/mismatched responses,
result-encoding failure or unclassified RPC/HTTP errors reject with
`unknown_outcome`, operation and selected project. Prohibit retry/replay and direct
canonical-source/recovery inspection; never imply rollback. Established owning
operation errors and explicit pre-dispatch rejections retain their classifications.
There is no operation deadline or TCP-local session assumption. Shared ordered close
drains earlier work and clears this adapter's connection/credential references;
it sends no HTTP request and cannot administer or stop the shared daemon.

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

Native code-index inputs are not supported in 0.4.0-alpha.1, and commands declared in
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
startup failures return correlated errors. The public adapter in
[[DES-JAVASCRIPT-CLIENT-ADAPTERS]] owns worker construction, safe-integer
correlation, response/error decoding and termination; raw messages remain the
binding boundary, not the public client API. Termination loses memory; this host
has no daemon shutdown, disk persistence or recovery.

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
wasm-bindgen CLI 0.2.127 (`--target web`) to produce `browser/mara_wasm.js`,
`browser/mara_wasm_bg.wasm`, their declarations and `browser/worker.mjs`, plus
root-level shared/browser modules/declarations and `internal/json.mjs`. The
worker resolves its generated module and WASM within `browser/`. Supply disposable
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
public-factory smoke, with `client.spec.mjs` comparing the real native/shared
facade and `typed.spec.mjs` executing a strict compiled browser package consumer.
These local Chromium checks are not final release/registry certification under
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

The delivered `mara-node` cdylib inherits the workspace version and depends on
native `mara`, napi 3 (N-API 8 and serde-json), napi-derive 3, napi-build 2,
serde and serde_json. It directly calls `NativeClient`, not CLI processes.
`scripts/build-node.sh OUTPUT_DIRECTORY [--release]` builds the locked corresponding-host addon (debug by default, release for CI/release),
copies the Linux .so or macOS .dylib to `mara.node`, and includes the private
loader/declarations. Require caller-owned disk-backed TMPDIR/CARGO_TARGET_DIR.
Corresponding-host certification requires actual installed-package execution, not merely this build. `scripts/package-npm.mjs main OUTPUT [BROWSER_ASSETS]` includes the public
Node/shared ESM modules, declarations, HTTP adapter/generated version module and
private native loader, with ESM import-only export metadata and Node >=24.
Supplying the output of `scripts/build-browser.sh` adds the browser export,
browser declarations and bundled worker/WASM in that package-relative layout.
Omitting it preserves the native/shared/HTTP-only packaging path. A disposable
script-free local tarball install exercises both browser imports/assets and
strict types; final release packaging must supply those assets. `scripts/package-npm.mjs platform TARGET CLI ADDON OUTPUT` requires both prebuilt native artifacts. Existing platform
optional-dependency packages include the .node addon alongside the executable,
retaining exact workspace-version selection and current host/glibc boundaries.
Build/package scripts produce these artifacts before publication; packages
contain no install-time downloader, compiler or lifecycle scripts.

`scripts/smoke-npm.sh CLI ADDON BROWSER_ASSETS [NEW_OUTPUT_DIRECTORY]` packs
and inspects complete main/host packages, installs their local tarballs offline
with scripts disabled, and compares installed CLI/addon bytes to the candidate
inputs before importing every public export and loading the default native addon.
It also installs a browser-only consumer with optional dependencies omitted.
The optional new output directory retains inventories, exact tarballs and
consumers for subsequent standard TypeScript and Chromium gates; the caller
owns its cleanup. CI/release run the real source and installed facade regressions
and Chromium HTTP/WASM paths on the corresponding native hosts. Release builds
and packs main/browser inputs before protected publication; host jobs verify
the identical main tarball. Registry and final supported-host execution remain
release gates, not claims inferred from workflow YAML.

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
:status: accepted
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
:verifies: REQ-SCRIPT-FREE-NPM-DISTRIBUTION

Build and pack the candidate main/native packages, install them with lifecycle
scripts disabled in disposable storage, and import the Node ESM API on Node 24.
Load the real addon and run project initialization, search/read, mutation,
validation and continuation through embedded and daemon execution without
spawning the CLI. Verify declarations against real usage with the standard
TypeScript compiler. Retain installed CLI/MCP smoke.

The binding-local gate builds with `scripts/build-node.sh`, then runs
`cargo test --locked --test node_binding real_node_addon -- --ignored --exact
--nocapture` with absolute MARA_NODE_ADDON, NODE24 and private MARA_NODE_RUNTIME.
The Rust fixture hosts the actual Unix-socket server; Node's standard test runner
executes the real addon in disposable projects through direct, default-auto and
required daemon routing. Cover all document/setup operations, configured and
request-local rules, known rule failures versus operation errors, continuation,
caller selection, event-loop progress and closing without daemon shutdown.
On Linux, optional MARA_NODE_PROCESS_TRACE captures process/socket syscalls via
strace, distinguishing the Node test runner's own process from addon operations.
These binding-local checks do not certify a package installed with scripts disabled.

The complete installed gate is `cargo test --locked --test node_binding
installed_node_client -- --ignored --exact --nocapture`. Supply absolute NODE24,
private MARA_NODE_RUNTIME, MARA_NODE_CLIENT pointing to the smoke install's
`node.mjs`, MARA_NODE_ADDON for the supplemental loader checks, and
MARA_NODE_TYPED_CONSUMER pointing to the standard TypeScript compiler's installed
`tests/node/usage.mts` output. It imports bare root/node exports and loads the
installed exact-version optional addon without an addonPath override. The
fixture's Node-only PATH and temporarily non-executable installed CLI require
the full JS and compiled TypeScript workflows to work without CLI execution.
The real Rust Unix-socket host stays independently owned through direct,
default-auto and daemon-required execution, complete trace/read continuation
and close/drain. Restore the CLI's permissions before later CLI/MCP smoke.
CI/release compile against the installed declarations and run this same
corresponding-host gate, not a copied source-loader substitute.

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
`tests/browser/http.spec.mjs` serves the script-free installed, optional-native-free
main package against its installed CLI's fixture-owned foreground daemon.
Use BROWSER_ASSETS_DIR, MARA_CLI, owned TMPDIR/CARGO_TARGET_DIR, private
MARA_TEST_RUNTIME_DIR/BROWSER_RUNTIME_TMPDIR and the exact serialized loopback
origin. Select it with `BROWSER_TEST_MATCH=http.spec.mjs`;
BROWSER_TEST_PORT selects its isolated server (default 18364).

The two real Chromium tests cover the full shared document API, owning
rules/errors/continuations, disk persistence and exact native CLI parity.
Recovery restores API-created source from a published-format pending-journal
fixture; this is not an interrupted-publisher claim. Separate root/destination
permissions, missing/wrong credentials, protocol/application mismatch,
disallowed/missing origins, descendants and an actual other Git worktree are
refused before FIFO configuration reads. Live configured-indexer sentinels stay
untouched on HTTP refusal and run through independently authorized native IPC
positive controls. Check disabled HTTP, local-only administration, token-free
routine status/foreground output, response-loss unknown outcome without replay
and close without daemon shutdown. Chromium imports HTTP/shared modules from
the installed browser-only consumer with no native optional package.
CI/release run these same tests on each corresponding host; local execution
certifies only its recorded host/browser/revision, not registry publication.
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

:::mara evidence EVD-INSTALLED-NATIVE-HTTP-LINUX
:mid: 01M456QGG88SMSDWW3B5WJSQDB
:title: Installed Node and Chromium HTTP gate on Linux
:status: accepted
:result: passed
:captured_at: 2026-10-05T04:55:24Z
:subject_revision: 94bdf0dcbf4f807aa292253cc12a7d950d4ca640
:evidences: VER-INSTALLED-JAVASCRIPT-CLIENT
:evidences: VER-NPM-DISTRIBUTION

Linux x86_64 installed-artifact Node/Chromium HTTP gates passed at commit
`94bdf0dcbf4f807aa292253cc12a7d950d4ca640`, tree `f9201e27d4976cf7afbbfa0a529a3d6308e5af26`.
This is local issue-checkout execution, not integration or final release certification.

Fresh optimized CLI/addon and worker/WASM builds were made in this session.
Native compiler inputs are byte-identical between the creation base and tested
revision; the final standard builds reused that owned Cargo cache. Candidate
main and matching Linux x64 native tarballs were freshly packed and installed
offline with scripts disabled; installed CLI/addon bytes and default exact-version
loader resolution matched those candidate inputs. All-three-target native
manifests were inspected; foreign-host payloads were local metadata fixtures,
not executable host evidence.

- Actual Node 24.19.0/npm 11.17.0: installed bare ESM root/node exports and
  strict TypeScript 5.9.3 compiled consumers passed direct, default-auto and
  daemon-required full document workflows. A real Rust Unix-socket fixture
  hosted daemon work; Node-only PATH and non-executable installed CLI excluded
  CLI execution. Complete trace/large Unicode-read continuation, typed failures,
  persistence/capabilities, event-loop progress and close/drain passed.
- Actual Chromium 152.0.7977.82 / Playwright 1.58.2, application origin
  `http://127.0.0.1:18428`: two installed browser-only HTTP tests passed against
  the installed fixture daemon, with deliberate local connection-info handoff,
  exact native read/list/search/validation parity and persisted mutations.
  Disabled HTTP, version/credential/origin/root/destination refusals preceded
  FIFO config reads. Descendants and a real other Git worktree remained outside
  authority; three configured indexer sentinels ran only through native IPC
  positive controls. Root/destination authority stayed separate; routine
  foreground output/status omitted tokens; daemon administration was refused.
- Authorized init, real journal-fixture restoration and refresh passed.
  The recovery fixture used persisted API-created source and the published
  journal format; it does not claim a deliberately interrupted daemon publisher.
  Deliberate loss of a completed mutation response yielded unknown outcome
  without replay; close left the independently owned daemon usable.
- Script-free installed CLI/stdio MCP/skill smoke, private addon/public facade
  regressions, all six existing Chromium tests, WASM build with matching
  wasm-bindgen 0.2.127, formatting, locked workspace all-target build, strict
  all-feature Clippy and 470 workspace tests passed. Eight ignored probes/
  helpers are not counted as passes; Node gates ran explicitly.
  Schema/project validation was valid and evaluation-complete with zero
  diagnostics and no continuation; actionlint 1.7.12/ShellCheck 0.11.0 passed.

Retained inventories, exact candidate tarballs/CLI/addon/WASM hashes, commands,
results and test/reproduction inputs:
`/home/araketski/.local/state/pi/verification/01a10a52-517e-73f4-9dd4-58fbd7c5f393/`.
Main tarball SHA-256:
`c52d8b2e33457f750e963b5593131963e9d6ddd390cb936276a410b3bbaf19d9`;
Linux x64 native tarball:
`1bd6eeea3aff969de52984009c397cbdba41ed7c8441703a1e4fd83182158219`.
Local glibc was 2.44, Rust/Cargo 1.97.1. No actual Linux arm64, Apple Silicon,
Ubuntu 22.04 baseline, cloud/protected/public-registry or release execution is
claimed. Hosted HTTPS-to-daemon origins and Firefox/WebKit remain uncertified.
The existing browser WASM smoke is not the independently owned comprehensive
packaged-WASM acceptance lane.
:::
