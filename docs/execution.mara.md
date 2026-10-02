# Native execution architecture for 0.4.0

Target contracts for the [0.4.0 scope](0.4.mara.md). These draft items do not
claim shipped functionality or passing execution. Existing operation contracts
remain the source of semantic detail; this document defines their new execution
boundary. JavaScript/browser adapters are defined in
[integration](integration.mara.md).

## Workflow

:::mara scenario SCN-SHARE-NATIVE-MARA
:mid: 01M3YVYMAZP4WKC7AQ3QFETRPB
:title: Use one persistent engine from native consumers
:status: draft
:contributes_to: GOAL-BOUNDED-AGENT-CONTEXT

A user starts a local daemon. CLI, MCP and native library consumers address one
project through the shared client and reuse its loaded state. Another request
may address a different project root or Git worktree without mixing knowledge.
External file edits are reconciled before the next operation. With no
compatible daemon, native auto mode performs the same operation in process.
:::

## Requirements

:::mara requirement REQ-PORTABLE-DOCUMENT-ENGINE
:mid: 01M3YVZZHES267CPB1XENPPHYC
:title: Execute document semantics independently of the host
:status: draft
:kind: functional
:derives_from: SCN-DRAFT-MARA-BROWSER

One Rust semantic engine must provide the existing document operations for both
native and browser execution: project initialization and MID
generation/backfill; schema discovery and validation; item list, search, get
and related; item create/update/delete/move/rename; relation
inspection/add/remove; project/item validation; current-state rules and
request-local trace matrices.

Preserve each owning operation's identity, source-preservation,
reference-safety, diagnostic and bounded-continuation semantics. Inputs are
explicit project state; filesystem access, process environment, subprocess
execution and publication belong to hosts. Native-only code and recovery
capabilities are not silently emulated.
:::

:::mara requirement REQ-OPTIONAL-DAEMON-EXECUTION
:mid: 01M3YW0A9ETM85ST5VZZ4G0F5F
:title: Choose native execution without requiring a running server
:status: draft
:kind: functional
:derives_from: SCN-SHARE-NATIVE-MARA

The Rust high-level client must support direct, auto and daemon-required modes.
Auto is the native default: use a compatible running daemon, otherwise execute
in process when absence or incompatibility is established before dispatch. Do
not automatically start a daemon.

Never silently replay a dispatched operation on another backend. A lost
mutation response must identify an unknown outcome rather than imply failure or
success. Operation failures and other connection/authentication errors are not
fallback triggers. Embedded and daemon execution share the same native runtime.
:::

:::mara requirement REQ-FRESH-PROJECT-SESSIONS
:mid: 01M3YW0Y81J6D2WWX14HFEHCBT
:title: Reuse project state without hiding external changes
:status: draft
:kind: quality
:derives_from: SCN-SHARE-NATIVE-MARA

Native sessions must reconcile relevant project inputs before each operation
and rebuild affected derived state when inputs change. Detect changes to
contents, effective file inventory, configuration, schema, rules, ignore
behavior and code-index inputs. Watchers cannot be the sole freshness
mechanism. Do not return an old successful snapshot to conceal invalid current
inputs.

Unchanged state must reuse parsing/index construction and eligible code
indexes. Force refresh must invalidate derived state even when recorded files
are unchanged. Cursors and structural handles retain their owning stale-state
contracts. Concurrent external multi-file edits do not gain an atomic-snapshot
guarantee.
:::

:::mara requirement REQ-USER-DAEMON-LIFECYCLE
:mid: 01M3YW19028R716Y47YCM9AP1E
:title: Manage one local daemon per user explicitly
:status: draft
:kind: functional
:derives_from: SCN-SHARE-NATIVE-MARA

Provide one native daemon per OS user with isolated sessions for multiple
project roots; distinct Git worktrees remain distinct projects. Support
foreground run, background start, stop, status and selected-project refresh. Do
not install an OS service or start automatically.

Caches are disposable, memory-only and bounded; restart rebuilds them. Indexers
use the daemon startup environment and each project's root as working
directory. Users refresh for external indexing-input changes and restart to
adopt a changed environment. Stopping or losing the daemon must not make
canonical source depend on cache recovery.
:::

:::mara requirement REQ-LOCAL-DAEMON-HTTP
:mid: 01M3YW1KSX05E12N35SYJ97CK5
:title: Expose browser operations only through an authorized local interface
:status: draft
:kind: constraint
:derives_from: SCN-SELECT-BROWSER-MARA

Provide an opt-in loopback-only HTTP adapter for the daemon's
document-operation protocol. Native IPC remains the default. HTTP requires a
daemon-session token and an explicit allowed-origin policy; authorize before
accessing project files or executing configured commands. Do not expose a
remote/cloud service or use wildcard browser-origin access.

Native IPC and HTTP must enforce the same operation contracts. Browser HTTP
selection is explicit and reports unavailable/incompatible backends; it does
not silently replace local filesystem access with a WASM workspace.
:::

## Architecture

:::mara design DES-EXECUTION-BOUNDARIES
:mid: 01M3YW5EGZJ7A69CHXH2RA8938
:title: Separate portable semantics, native hosting and clients
:status: draft
:kind: structure
:satisfies: REQ-PORTABLE-DOCUMENT-ENGINE
:satisfies: REQ-OPTIONAL-DAEMON-EXECUTION
:satisfies: REQ-SHARED-TYPESCRIPT-CLIENT
:satisfies: REQ-SURFACE-PARITY

The dependency boundary is a portable Rust engine, a native runtime, and thin
client/transport/binding adapters. The engine takes explicit source snapshots
and produces domain results and validated candidate source changes. Native
hosting owns discovery, file loading, code indexing, environment, cache
lifetime, locks, permissions and recoverable publication. Browser hosting
applies candidates to memory.

The native high-level client routes to the embedded runtime or daemon; the
daemon invokes the runtime directly, never recursively through auto routing.
N-API wraps that client. WASM wraps the portable engine. The shared TS facade
selects N-API, HTTP or worker/WASM adapters without duplicating domain
behavior.

Use separate engine and binding crates where these build/portability boundaries
require them; native client/runtime/server/protocol can remain modules.
Existing Rust exports may be replaced; compatibility wrappers are not required.
CLI/MCP owning contracts remain compatible.
:::

:::mara design DES-SEMANTIC-CHANGE-PLANS
:mid: 01M3YW5S9SRKRYTY61H3VM02R7
:title: Validate source edits before a host publishes them
:status: draft
:kind: behavior
:satisfies: REQ-PORTABLE-DOCUMENT-ENGINE
:satisfies: REQ-BROWSER-LOCAL-WORKSPACE
:satisfies: REQ-RECOVERABLE-MUTATION

Document mutations operate on one explicit source snapshot, generate candidate
replacements/additions/deletions, and check the owning source, identity and
surviving-reference obligations before acceptance. Changes retain
original-state preconditions over all relevant inputs, not only the touched
file. Keep ordinary parsing/source-span logic in the shared engine.

The native host retains [[DES-MUTATION-RECOVERY]] and
[[DES-MUTATION-TRANSACTION]], including locks, permissions,
current-input/preimage checks, staging and journals where required. Do not
replace recoverable publication with unguarded writes. Initialization and
explicit rollback retain their existing prerequisites; a cached invalid corpus
must not prevent recovery.

The browser applies accepted candidates atomically to its memory workspace.
Failed edits preserve that workspace. This does not grant filesystem atomicity,
change the journal format or coordinate concurrent manual edits.
:::

:::mara design DES-NATIVE-PROJECT-SESSIONS
:mid: 01M3YW64174TFBYTBS6Y2QCMR4
:title: Reuse reconciled native project projections
:status: draft
:kind: behavior
:satisfies: REQ-FRESH-PROJECT-SESSIONS
:satisfies: REQ-USER-DAEMON-LIFECYCLE

Key sessions by canonical project root, not Git common directory. Retain
project/schema, parsed documents, lookup/discovery/relation projections,
compiled rules and eligible code indexes. Reconcile effective inventory and
recorded input contents before every operation; invalidation covers
input/configuration/ignore changes and completed, failed or rolled-back
mutations that may have changed files. Do not publish stale success after a
failed refresh.

Code reuse records the configured command, effective project inputs and
grammar/query assets using the native indexing input boundary in
[[DES-CODE-TRACEABILITY]]. Preserve empty-language activation,
staging/bookkeeping exclusions and indexing failure semantics. External
tools/dependencies outside those recorded inputs require explicit force
refresh; changing the daemon environment requires restart. This limitation is
visible, not a claim of complete external-dependency tracking.

Bound idle cache retention without evicting in-flight state or authoritative
data. Caches and compiled projections are memory-only; no persisted index
format is introduced. Use the same session implementation for long-lived
embedded clients. Preserve owning cursor/handle fingerprints across reuse and
eviction.
:::

:::mara design DES-NATIVE-CLIENT-ROUTING
:mid: 01M3YW6ETVA0JCQTWD910ZMB4J
:title: Route native operations before dispatch
:status: draft
:kind: interface
:satisfies: REQ-OPTIONAL-DAEMON-EXECUTION
:satisfies: REQ-SURFACE-PARITY

Resolve project selection in the caller's context before dispatch, never the
daemon working directory. Keep nearest-project discovery, absolute MCP request
selection and bound-MCP override refusal under
[[DES-OPERATION-PROJECT-CONTEXT]]. Each request addresses one project;
initialization carries its explicit target.

Use direct, auto or daemon-required configuration. Explicit library options/CLI
flags override matching environment settings; otherwise default to auto. Add no
persisted client-preference or project-configuration format. Auto checks
connection compatibility before operation dispatch and falls back only for
absence or incompatibility. Required mode reports either condition. Do not fall
back for domain failures, authorization failures or uncertainty after dispatch.

A lost dispatched mutation response is an unknown outcome. Return an actionable
typed failure and direct the caller to inspect source/recovery state; do not
retry or claim rollback. Server restart, stale connection state and
compatibility negotiation cannot authorize duplicate writes.
:::

:::mara design DES-LOCAL-DAEMON-PROTOCOL
:mid: 01M3YW6STP380FYZ9F4QZKHPM4
:title: Share versioned JSON-RPC over native IPC and HTTP
:status: draft
:kind: interface
:satisfies: REQ-OPTIONAL-DAEMON-EXECUTION
:satisfies: REQ-LOCAL-DAEMON-HTTP

Use JSON-RPC 2.0 for both native IPC and HTTP, with Mara protocol version 1
negotiated before project operations. Initially require the exact same Mara
application version and protocol version; report supported result formats and
backend capabilities. Operation parameters/results follow their owning
contracts. Preserve the distinction between invalid domain validation results
and RPC/operation failures.

Native transport uses same-user Unix sockets on the supported Linux/macOS
hosts. Keep endpoint discovery and session credentials in private per-user
runtime state, never project documents; stale endpoint handling must not stop
an unrelated process. HTTP is disabled unless explicitly enabled, binds only
loopback, and checks the session token and allowed Origin before project
access. Do not put tokens in URLs or logs, or allow wildcard browser origins.

MCP remains a separate stdio adapter to the high-level client, not the daemon's
operation protocol. No remote hosting, event subscription or cross-project
aggregation is added.
:::

:::mara design DES-DAEMON-LIFECYCLE
:mid: 01M3YW74J0SYTCM32WHNFE96QN
:title: Own the daemon independently of individual consumers
:status: draft
:kind: deployment
:satisfies: REQ-USER-DAEMON-LIFECYCLE

Expose foreground run and background start plus stop/status and
selected-project force refresh. Background start detaches from the caller;
consumers disconnecting do not stop the daemon. Only one daemon instance owns
the per-user endpoint; a second start reports the existing instance rather than
replacing it. No installer, OS service or auto-start behavior is included.

The daemon's startup environment is the execution environment for configured
indexers. Commands run from each selected project root with the existing
trusted-command boundary. Explicit refresh discards selected-project derived
state even if its recorded inputs appear unchanged. Stop/restart discards
memory caches; durable mutation recovery remains in project files under
[[DES-MUTATION-RECOVERY]]. CLI/MCP and binding consumers do not need a daemon
for direct execution.
:::

## Decisions

:::mara decision ADR-PORTABLE-ENGINE-HOSTS
:mid: 01M3YW993V3R07PFTYYMWRPB57
:title: Share engine semantics rather than port the daemon into the browser
:status: draft
:justifies: DES-EXECUTION-BOUNDARIES
:justifies: REQ-PORTABLE-DOCUMENT-ENGINE

A client/server-only split would leave semantic operations coupled to native
I/O and require another extraction for browser execution. Separate a portable
engine from native hosting now: embedded execution, the daemon and browser WASM
reuse it through thin adapters. The browser does not run a daemon or use N-API.

Preserve supported document-rule semantics and diagnostics across hosts.
Portability may require replacing the current native rule adapter/dependencies
with equivalent existing/standard engines. Do not silently introduce a custom
rule language/interpreter or ship reduced rule coverage as completion.
Dependency portability is checked before broad extraction; a blocker requires
an explicit plan change.
:::

:::mara decision ADR-PER-USER-MEMORY-DAEMON
:mid: 01M3YW9KWMYAWZJ46T390ES1ZE
:title: Share disposable project sessions in one per-user process
:status: draft
:justifies: DES-NATIVE-PROJECT-SESSIONS
:justifies: DES-DAEMON-LIFECYCLE

Choose one local daemon per user rather than one process per project. This
gives CLI, MCP, Node and later local browser consumers one execution endpoint
while canonical roots isolate project/worktree sessions. It does not introduce
workspace aggregation or remote tenants.

Keep derived caches in memory rather than introduce persistent indexes.
Reconcile recorded inputs before reuse; cache code as well as documents to
avoid repeated external indexing. External indexing inputs outside the recorded
project boundary require force refresh, and environment changes require
restart. This trades complete external-dependency tracking for an explicit
bounded 0.4 limitation.
:::

## Verification

:::mara verification VER-NATIVE-EXECUTION-BACKENDS
:mid: 01M3YWA9AXQW6565YJVE94JSAZ
:title: Verify native routing, freshness and guarded publication
:status: draft
:method: test
:level: system
:verifies: REQ-PORTABLE-DOCUMENT-ENGINE
:verifies: REQ-OPTIONAL-DAEMON-EXECUTION
:verifies: REQ-FRESH-PROJECT-SESSIONS
:verifies: REQ-USER-DAEMON-LIFECYCLE
:verifies: DES-EXECUTION-BOUNDARIES
:verifies: DES-SEMANTIC-CHANGE-PLANS
:verifies: DES-NATIVE-PROJECT-SESSIONS
:verifies: DES-NATIVE-CLIENT-ROUTING
:verifies: DES-LOCAL-DAEMON-PROTOCOL
:verifies: DES-DAEMON-LIFECYCLE
:validates: SCN-SHARE-NATIVE-MARA

Run the real Rust client, CLI and stdio MCP against disposable projects through
direct and daemon modes. Exercise the existing document-operation suites,
current invocation/result formats, discovery/bound selection, native indexing,
locks, journaled edits and explicit recovery. Require equivalent domain
outcomes and preserved source/permissions; a server wrapper or isolated mock is
not end-to-end evidence.

Verify auto behavior with no daemon and an incompatible daemon before dispatch,
required-mode errors, shared sessions for multiple roots/worktrees,
foreground/background lifecycle, forced refresh and restart. Disconnect after
dispatch of a real mutation and verify an unknown outcome without automatic
replay.

Change document/configuration/schema/rule/inventory/ignore/code inputs
externally and verify reconciliation and stale continuation. Verify invalid
inputs do not yield stale success. Show unchanged inputs avoid repeated parsing
and configured indexer invocations. Record representative cold/warm timings
without a fixed speedup claim. Test cache eviction/restart without source loss
and environment adoption on restart.

Use existing Rust tests/tempfile and standard process tools; keep fixture
Git/configuration state isolated. Capture results at the tested revision.
:::
