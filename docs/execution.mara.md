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
daemon-session token, an explicit allowed-origin policy and an explicit project
allowlist; authorize before accessing project files or executing configured
commands. Do not expose a remote/cloud service or use wildcard browser-origin
access.

Browser authority is limited to authorized project operations. Project
initialization requires an explicitly authorized destination; project rollback
and refresh are allowed within that authority. Daemon-wide lifecycle,
access-policy and credential management remain local-only. HTTP restrictions
do not narrow native IPC project access.

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
The asynchronous `mara-node` N-API tasks wrap that client under
[[DES-JAVASCRIPT-CLIENT-ADAPTERS]], never the CLI or daemon runtime dispatcher.
WASM wraps the portable engine. The shared TS facade
selects N-API, HTTP or worker/WASM adapters without duplicating domain
behavior.

Use separate engine and binding crates where these build/portability boundaries
require them; native client/runtime/server/protocol can remain modules.
Existing Rust exports may be replaced; compatibility wrappers are not required.
CLI/MCP owning contracts remain compatible.

The extracted source/query/conformance and rule/trace interfaces are defined by
[[DES-EXPLICIT-SOURCE-ENGINE]]; mutation planning is defined by
[[DES-SEMANTIC-CHANGE-PLANS]]. The embedded native interface is
`mara::NativeRuntime::new(selected, current_directory)`; `from_environment`
is the CLI/MCP convenience constructor. The caller directory must be absolute;
relative native selections resolve against it. `OperationContext` remains an alias
for existing adapters. `for_project` preserves absolute request selection and
bound-server override refusal. Each call reconciles current inputs through the
memory-only sessions in [[DES-NATIVE-PROJECT-SESSIONS]]. CLI/MCP use the
high-level `NativeClient` under [[DES-NATIVE-CLIENT-ROUTING]]; the daemon
continues to call `NativeRuntime` directly. The native server
foundation and reusable dispatcher are delivered under [[DES-LOCAL-DAEMON-PROTOCOL]].

The runtime exposes schema get/list/validate, item list/search/get/related,
relation get/add/remove (including occurrence removal), all item mutations,
project/item validation with continuation options, trace matrices,
initialization, MID backfill and explicit rollback. It invokes shared engine
queries/conformance/rule/trace computations and `ChangePlanner` plans; native
discovery, code indexing, MID generation and guarded publishers stay in
`mara`. Rollback resolves configuration but bypasses schema/corpus loading.
CLI/MCP keep their public parameters, invocation and results.
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

The delivered Rust boundary is `SourceEngine::planner()` and a
`ChangePlanner` with `plan_create`, `plan_backfill`, `plan_update`,
`plan_delete`, `plan_move`, `plan_rename` and `plan_relation` (add/remove,
including occurrence selectors). `SourceEngine::plan_initialize(name, template)`
also works before project configuration exists and preserves initialization's
no-overwrite prerequisites without demanding a valid pre-existing corpus.
Initialization/templates and source-span/reference checks are engine-owned.

Each operation returns a non-forgeable `ChangePlan<T>`: inspect its original
and candidate explicit sources (version-1 snapshots for portable engines),
source fingerprint, ordered source changes
with optional preimage/candidate bytes, and owning result. Item deletion removes
the item block but retains its containing file. Preconditions bind the complete
supplied inventory, exact sources, configuration/schema/rule sources,
selection/ignore context and caller base identity, including untouched inputs.
Native code plans additionally bind loaded index structure and relevant code/
asset bytes, including file-only dependencies first introduced by the candidate. Missing selected sources are not absence proof.

MID entropy/time belongs to hosts. Creation receives one host-generated MID;
backfill receives identities in document/source order. The engine checks
canonical form and uniqueness before using them. Native CLI/MCP still generate
MIDs internally; users cannot supply them through those transports.
Backfill reference correspondence treats an old ID-only item as the same target
after its new MID is inserted, including incoming/self and contained-node links.

Trusted native adapters use `SourceEngine::from_native_sources` for
host-relative UTF-8 filenames. Preserve existing native colon/literal-backslash
names on hosts where they are valid; filesystem containment, regular-file and
discovery checks still belong to the native host. Rebuild candidates with the
same path interpretation. This constructor is not a browser snapshot import:
portable `SourceEngine::new` restrictions remain unchanged, and native source
collections containing host-only names are not portable snapshot exports.

`planner_with_code` consumes existing trusted native `CodeIndex` inputs;
it does not index, discover or import browser code snapshots. Document-only
planning rejects unavailable required code before every operation, including
no-ops and backfill. Candidate code dependencies cannot bypass this gate.
Unused code-relation declarations alone remain editable. Preserve each owning
mutation's source-validation/scaffold exceptions; lifecycle/rule checks remain
separate under [[ADR-EXPLICIT-RULE-CHECK-INPUTS]], not invented planner outcomes.

The native host retains [[DES-MUTATION-RECOVERY]] and
[[DES-MUTATION-TRANSACTION]], including locks, permissions,
current-input/preimage checks, staging and journals where required. Do not
replace recoverable publication with unguarded writes. Initialization and
explicit rollback retain their existing prerequisites; a cached invalid corpus
must not prevent recovery.

`ChangePlan::check_preconditions` rejects stale input/code state before native
publication. Native adapters rebuild current inputs under the mutation lock and
use existing guarded single-file or journaled publishers; non-journaled backfill
stages its candidates before per-file replacements. Native snapshot loading
rejects unreadable supplied source/selection inputs rather than treating an
inspection failure as absent input. `load_native_source_engine` exposes trusted
host-relative input loading; portable `load_source_engine` retains snapshot
path restrictions. Neither loader turns an incomplete projection into a
complete inventory.

`SourceEngine::apply_plan` checks a document-only plan against current inputs
and replaces its memory state atomically, preserving caller base identity.
Changed sources invalidate owning cursor/structural-handle fingerprints;
no-op plans leave them unchanged. Native-code plans cannot be accepted as
document-only memory edits. Failed planning or acceptance leaves supplied state
unchanged. No engine method publishes files, locks, journals or rollback.

The browser applies accepted candidates atomically to its memory workspace.
This does not grant filesystem atomicity,
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

The delivered native API is `NativeRuntime::with_sessions(NativeSessions)`.
Runtime clones and `for_project` selections share that pool; separately
constructed runtimes have independent pools unless the caller supplies one.
`NativeSessions::new(SessionPolicy)` configures idle retention (default: at most
16 idle roots, five-minute idle timeout). Operations prune before/after calls;
`prune_idle` lets an idle host trigger maintenance without a background service.
Active and waiting operations hold leases and cannot be evicted. Calls on one
canonical root serialize; different roots can operate concurrently. Each root
retains at most 32 compiled rule variants, including parameterized checks, with
least-recently-used eviction; a later request recompiles an evicted variant.
Request-local rule sources are re-read before every content-keyed lookup and
are not retained as root dependencies; configured rule paths remain tracked.

Reconciliation scans/hashes effective native indexer inputs and records explicit
schema/rule/grammar/query, SCIP-emitted source and file-only code dependencies,
including ignored ones. Derived state is reused only on unchanged recorded inputs; an input change
conservatively drops the root's projections and rebuilds their dependency records
from current loaders, so retired explicit dependencies cannot permanently disable
reuse. Internal cache keys preserve native path bytes. A failed scan of current
inputs disables reuse rather than concealing current loading diagnostics. Completed read construction checks
inputs again before retaining state, without promising an atomic external
multi-file snapshot. Newly discovered ignored SCIP sources without emitter text
cannot verify their first indexer execution. Retain their dependency records but
discard that operation's projections; a subsequent stable operation can verify
those inputs across a fresh execution before allowing reuse.
Write attempts use uncached loaders and existing locked
publication/precondition checks, discarding projections before and after every
attempt, including errors, no-ops and explicit rollback.

`NativeRuntime::force_refresh()` waits for active selected-root work and discards
all derived state; the next operation rebuilds from source. It does not run
indexers itself. `session_activity` exposes cache-eligible read parse, code-load,
rule-compilation and discovery/relation-build counts for reuse verification,
not write-work accounting or an acceptance result. Eviction resets those
disposable counters. The portable corpus's `ProjectionCache` only memoizes
owned semantic topology with source/schema/code identity checks; it owns no
filesystem access, environment, session retention or publication policy.

Indexer executables, PATH resolution, external dependencies and tool inputs
outside the recorded boundary are not automatically tracked. Force refresh
after changing them. Indexer misses inherit the host process environment; a
daemon uses its startup environment and must restart to adopt environment
changes. Embedded callers must keep the indexing environment stable or discard/
refresh their pool after changing it. Canonical source and source-derived cursor/
handle identities do not depend on the pool, process identity or retention.
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

The delivered Rust interface is `NativeClient::new(selected, caller_directory,
ClientOptions)`. It is environment-free; the caller directory must be absolute,
relative selections resolve against it, and an unspecified mode means auto.
`NativeClient::from_environment(selected, options)` reads the caller cwd and
matching environment defaults. `ClientOptions.mode: Option<ExecutionMode>`
selects `Direct`, `Auto` or `DaemonRequired`;
`daemon_directory: Option<PathBuf>` selects an absolute private IPC host
directory, subject to [[DES-LOCAL-DAEMON-PROTOCOL]] checks.

CLI global `--execution-mode direct|auto|daemon-required` and
`--daemon-directory PATH` apply to operation dispatch, including stdio MCP.
Their matching environment settings are `MARA_EXECUTION_MODE` and
`MARA_DAEMON_DIRECTORY`. An explicit value overrides only its matching setting;
invalid effective mode/directory values fail without dispatch. No persisted
preference, daemon startup or endpoint repair is performed.

The client exposes the native runtime's typed operation methods and owning
results, with `ClientError` distinguishing owning operation data, invalid
arguments, unavailable/incompatible daemon, other connection/transport failures
and `UnknownOutcome { operation, project }`. `envelope()` retains owning
operation-error data (including relationship context) or returns error format 1
with a stable routing code. Unknown outcomes use `unknown_outcome`, identify
the mutation/root, prohibit retry/replay and direct source/recovery inspection.
CLI JSON and MCP structured tool errors preserve this classification. Existing
string-domain CLI/MCP errors keep their transport shape.

Each daemon operation uses one authenticated connection and negotiates before
dispatch. Compatibility requires the exact application/protocol versions,
owning result formats and advertised native operation/capability set.
Negotiation has a five-second read/write timeout; operations have no fixed
deadline because native indexers may run longer. Auto absence means a missing
runtime directory/endpoint (`NotFound`); refused occupied sockets, permission,
peer-authentication, malformed negotiation and other connection failures are
not absence. A supported negotiation incompatibility or incompatible capability
advertisement permits auto direct execution only before dispatch. Required mode
reports either unavailable or incompatible conditions. After dispatch, neither
reads nor mutations reconnect/retry or choose another backend. Lost/unusable
mutation responses, including server result-encoding failure, report unknown
outcomes; unclassifiable mutation RPC errors also remain unknown unless the
protocol establishes a pre-dispatch rejection. Over-limit requests are rejected
locally before sending their frame. Request/root JSON encoding happens only after
a compatible daemon is selected: native direct execution retains owning host-path
handling, while unencodable (non-UTF-8) daemon request paths return
`invalid_argument` before dispatch, without a backend change.
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

The delivered reusable interface is `mara::protocol::RpcSession`, constructed
with the server's shared `NativeSessions`. It accepts one JSON-RPC object at a
time; transport hosts authenticate/authorize before dispatch. Native IPC has one
compatibility session per connection. Send `mara.negotiate` with
`{protocol_version:1,application_version:"<exact Cargo workspace version>"}`.
Success returns `protocol_version`, `application_version`, `result_formats`
(discovery 2; relationship, validation, trace and operation_error 1), and
`capabilities`: backend `native`, persistence `filesystem`, supported
`operations`, code_indexing/configured_indexers/transaction_rollback/
force_refresh `true`, project_sessions `reconciled_memory`, and `local_methods`
(the local-only methods below on IPC; an empty array on a plain `RpcSession`).
These are host capabilities, not a guarantee that a project's configured indexer
is installed.
Failed renegotiation clears compatibility; reconnects must negotiate again.

Requests use `{jsonrpc:"2.0",id,method,params}`; IDs are JSON numbers, strings
or null and are echoed unchanged. Params are named objects; omission means
`{}`. Every project operation requires `project`, an absolute caller-resolved
root (initialization destination for `project_init`). No discovery uses the
daemon cwd and no project is bound globally. Remaining parameters and results
retain the existing native/MCP operation shapes and owning contracts:

| Methods | Parameters after project selection | Owning semantics |
| --- | --- | --- |
| project_init | template (default minimal) | [[REQ-PROJECT-INITIALIZATION]] |
| schema_get / schema_list | optional kind+name / required kind | [[REQ-SCHEMA-DISCOVERY]] |
| schema_validate / project_validate / item_validate | limit,cursor; project adds paths; item requires id | [[DES-TRACE-DIAGNOSTIC-INTERFACE]] |
| item_list / search | existing filters,page; search requires query and supports ids | [[DES-ITEM-LIST]], [[DES-UNIFIED-KNOWLEDGE-DISCOVERY]] |
| get / related | reference,cursor; related also direction,relations,flavours,limit | [[DES-BOUNDED-NODE-READ]], [[DES-DIRECT-NAVIGATION]] |
| item_create / item_update / item_delete / item_move / item_rename | existing native operation parameters | [[DES-ITEM-CREATION]], [[DES-ITEM-UPDATE]], [[DES-ITEM-DELETION]], [[DES-ITEM-MOVEMENT]], [[DES-ITEM-RENAME]] |
| relation_get / relation_add / relation_remove | source,relation,target; get adds limit,cursor; remove adds occurrence | [[DES-RELATION-INTERFACES]], [[DES-RELATION-MUTATION]] |
| trace_matrix | existing flat root filters,rules/check,limit,cursor,render | [[DES-TRACE-VIEW-INTERFACES]] |
| project_mid_backfill / project_transaction_rollback | none | [[DES-MID-BACKFILL]], [[DES-MUTATION-RECOVERY]] |
| project_refresh | none; returns `{refreshed:true}` | [[DES-NATIVE-PROJECT-SESSIONS]] |

Native IPC also exposes `daemon_status`, `daemon_stop` and
`daemon_connection_info`, with empty named params and the same mandatory exact
version negotiation. They are not project operations and are unavailable on
plain `RpcSession`/HTTP dispatch. `daemon_status` returns `{state,pid}` with state
`running` or `stopping`; PID is informational, never process-control authority.
`daemon_stop` returns `{state:"stopping"}` and requests host shutdown. While
already-dispatched project work drains, IPC keeps servicing negotiation and
local administrative methods; new project operations fail with -32000
`operation_failed` stating that the daemon is stopping, before dispatch.
`daemon_connection_info` returns the disabled result or deliberate enabled
endpoint/token handoff defined by [[DES-DAEMON-LIFECYCLE]].

Unknown parameters fail before dispatch. Responses contain exactly `result`
or `error:{code,message,data?}`. Standard JSON-RPC errors are -32700 parse,
-32600 invalid request, -32601 unknown method, -32602 invalid parameters and
-32603 internal result-encoding failure. Mara errors use -32001
negotiation_required, -32002 incompatible_version and -32000 operation failure.
Their data retains `{format_version:1,error:{code,message}}`; relationship
failures also retain owning edge/count context. Compatibility failures include
the supported application/protocol versions. Typed validation/trace/relationship
error codes survive; native String failures use `operation_failed`. Failure
messages strip control characters and are bounded to 2,048 characters; no
indexer stdout/stderr or credentials are added. Domain validation, including
`valid:false` or incomplete evaluation, is successful RPC result data.

Native framing is newline-delimited UTF-8 JSON. The 1 MiB request limit includes
the terminating newline; oversized input receives -32600 and closes that
connection. EOF without a newline never dispatches the partial frame. Response
writes fail after one second of blocked socket backpressure; operation execution
has no such deadline. Response loss never cancels or rolls back dispatched work,
and a non-reading consumer cannot indefinitely pin shutdown. Existing
owning result budgets and continuation remain unchanged; IPC framing does not
truncate domain results. Batches return -32600. Valid notifications receive no
response and never negotiate or execute project operations. A connection processes
requests in order; separate consumers can run concurrently through the shared
root-isolated runtime. A complete dispatched mutation finishes even if its
consumer disconnects; response loss is not rollback or permission to replay.

Native transport uses same-user Unix sockets on the supported Linux/macOS
hosts. Keep endpoint discovery and session credentials in private per-user
runtime state, never project documents; stale endpoint handling must not stop
an unrelated process. The delivered `mara::ipc::IpcServer` binds
`native-v1.sock` in `mara/` below the Linux XDG_RUNTIME_DIR (fallback
`/run/user/<effective-uid>`) or macOS private user TMPDIR. Discovery is local
and deterministic; native IPC authenticates OS peers, not bearer tokens.
Both the base and runtime directory must be same-user mode 0700 directories;
the runtime leaf cannot be a symlink. The socket and persistent advisory
`native.lock` are same-user mode 0600. Bind a short nonce-named staging socket in
the private runtime directory, set its mode, then atomically hard-link its inode
to the discoverable endpoint without overwriting an existing name; remove the
staging name. Concurrent starts never observe a published socket awaiting chmod.
A crash before publication may leave an undiscovered stale staging socket; it
has no PID authority and is not reused as an endpoint. Validate endpoint type/owner/mode and
peer UID on connect; the server checks peer UID before reading any request
(Linux SO_PEERCRED; macOS getpeereid). Socket paths must be shorter than 104
bytes. Explicit host runtime directories follow the same checks and must be
outside project/data directories. Do not repair permissions silently. Refuse an
owned lock or any occupied endpoint in `bind_in` without unlinking it, signaling
a process or replacing state. Lifecycle `bind_recovering` holds the same private
ownership lock before inspecting a stale endpoint: only a validated same-user
mode-0600 socket with a refused connection and unchanged device/inode may be
removed. Live endpoints, non-sockets, symlinks and invalid permissions are refused.
No PID file or process signal is used for stale recovery or local stop.
Drop removes only this instance's socket identity; the lock file remains.

The foreground example `cargo run --locked --example native-ipc-server`
exercises the real host. `serve_until` accepts a host-owned shutdown flag;
consumer EOF never stops listening. Host shutdown closes existing connection
read sides and waits for dispatched operations (and their response writes) to
finish before releasing the endpoint/ownership lock. Local administrative
connections remain serviceable during that drain and cannot indefinitely pin
shutdown; new project requests are refused. The CLI lifecycle adapter is defined by
[[DES-DAEMON-LIFECYCLE]]. The opt-in HTTP host shares the IPC host's sessions
and shutdown control; local status remains available while dispatched HTTP work
drains. Shutdown wakes incomplete HTTP readers and waits for dispatched work
and bounded response writes before releasing ownership.

HTTP is disabled unless explicitly enabled and binds only a numeric loopback
address. Send one JSON-RPC object in `POST /rpc` with `Content-Type:
application/json`, a Content-Length body (at most 1 MiB), `Authorization: Bearer
<token>`, an exact allowed `Origin`, `X-Mara-Protocol-Version: 1` and
`X-Mara-Application-Version: <exact workspace version>`. Query strings, URL
credentials and chunked request bodies are not supported. Each request negotiates
a fresh `RpcSession` with those version headers before dispatch, independent of
HTTP connection reuse; explicit `mara.negotiate` returns normal capabilities.
Missing version headers return -32001; incompatible versions return the shared
-32002 error. No HTTP negotiation handle or cookie is introduced.

Token and origin refusal return HTTP 401 and 403 respectively; exact-root
refusal returns 403 without project diagnostics. Accepted RPC results/errors
use HTTP 200 with the same JSON-RPC/domain payloads as IPC. Notifications use
204 without dispatch. Unknown methods, including daemon administration, return
-32601; HTTP capabilities have empty `local_methods`. Connections close after
one response; response loss is not rollback or permission to replay. Request
headers are bounded to 16 KiB/32 fields; incomplete reads have a five-second
socket timeout, response backpressure a one-second timeout. Operation execution
has no transport deadline. At most 16 HTTP connections, including
unauthenticated readers, own workers at once; additional peers wait in the OS
listen backlog. Both HTTP and native IPC back off on temporary
accept/descriptor/thread resource pressure without terminating their listeners.
Admission may fail or wait during pressure; existing dispatched operations keep
running. Releasing pressure restores both services without a daemon restart.

Every POST requires both token and allowed Origin before target resolution.
`OPTIONS /rpc` permits only an explicitly allowed origin, requested method POST
and supported request headers; it carries no token and never dispatches RPC or
inspects a project. CORS echoes only the selected exact origin, never wildcard
or credential cookies. Other paths/methods and malformed/unsupported framing
fail before RPC dispatch. The daemon stores a fresh cryptographically random
256-bit bearer token only in memory for that daemon session. Never place it in
URLs, logs, project files or routine status.

Startup authority stores resolved exact identities, not caller spelling.
Resolve existing path components and real symlinks using filesystem metadata
only, before reading project configuration/source or executing indexers. Explicit
roots/destinations may have an absent suffix; reject dangling symlinks and parent
traversal under absent components rather than inventing filesystem identity.
Re-resolve each request, compare exact identity and dispatch that resolved path;
a later alias retarget cannot inherit authority. A non-UTF-8 resolved identity
returns -32602 before dispatch, even when its caller alias is UTF-8; JSON cannot
encode that resolved host path. An alias of the same authorized root is otherwise
permitted, but descendants, other projects and other Git worktrees are
not. This is root selection authority, not a sandbox for an authorized project's
trusted configuration or concurrent filesystem attackers.

`project_init` checks the separate destination allowlist; other project operations
check the project-root allowlist. To initialize and then read/edit without restart,
list the destination in both. Destination permission alone grants no subsequent
project authority; project permission alone grants no initialization authority.
HTTP permits all authorized project operations, including rollback and force
refresh, but no daemon-wide lifecycle, access-policy or credential management.
Native IPC authority is unchanged. Connection information is handed off locally
under [[DES-DAEMON-LIFECYCLE]]; there is no browser pairing protocol.

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
:satisfies: REQ-LOCAL-DAEMON-HTTP

Expose foreground run and background start plus stop/status and
selected-project force refresh. Background start detaches from the caller;
consumers disconnecting do not stop the daemon. Only one daemon instance owns
the per-user endpoint; a second start reports the existing instance rather than
replacing it. No installer, OS service or auto-start behavior is included.

Delivered Linux/macOS CLI: `mara daemon run|start|stop|status|refresh|connection-info`.
Lifecycle and project routing select the same private IPC directory:
`--daemon-directory PATH` overrides `MARA_DAEMON_DIRECTORY`; omission uses the
platform's per-user runtime default. Detached start passes that exact directory
to its child, so every lifecycle command controls the selected endpoint, never
an unrelated default. Existing ownership/permission checks still apply.
`--format human|json` applies normally (human lifecycle results are compact JSON).
`run` blocks until authenticated local stop, SIGINT or SIGTERM, and emits
`{state:"stopped"}` after graceful shutdown. `start` launches the same executable's
`daemon run` with a new OS session and null standard streams; it inherits the
caller's indexing environment. The child explicitly selects direct mode because
the server calls the runtime directly; inherited client preferences cannot
invalidate an accepted parent invocation. It waits up to five seconds for compatible IPC readiness,
then returns `{state:"running",pid,already_running:false}`; a compatible existing
owner returns its state/PID with `already_running:true`. If a concurrent owner
wins after this invocation launches a child, start terminates and reaps only its
own losing child before reporting that owner, preventing a delayed replacement
after explicit stop. A losing child's exit does not end the five-second readiness
wait: a concurrent owner may hold the lock before publishing its endpoint.
A second foreground run fails with the ownership diagnostic. Background startup failures direct the user
to foreground `run` for diagnostics; there is no persistent log or PID file.

`status` returns `{state:"stopped"}` when the endpoint is absent,
`{state:"stale"}` for a validated socket whose connection is refused, or the
live `{state:"running"|"stopping",pid}`. Invalid ownership/permissions, incompatible
versions and other connection errors fail rather than imply absence.
`stop` retains one authenticated IPC connection, records its endpoint identity,
negotiates and requests shutdown on that same connection, then waits up to 30
seconds for that owner's endpoint identity to disappear/change; already absent
or stale endpoints return their status without modification. A timeout reports
that shutdown was requested and pending work remains; no process is forcibly
signaled. Startup timeout cleanup targets only the child just launched.

Only `refresh` accepts `--project PATH`, resolving relative selections against
the caller cwd; omission uses nearest-project discovery. It sends the absolute
selection through existing `project_refresh`, waits for active selected-root work,
and returns `{refreshed:true}`. Non-UTF-8 selected or discovered roots return
`invalid_argument` before IPC dispatch rather than panic. After negotiation, refresh has no response
health-check timeout: legitimate selected-root work may take longer than 30
seconds. A disconnect reports possible completion without automatic replay.
Refresh discards projections without running indexers; the next operation rebuilds them. Other lifecycle commands reject
`--project` because ownership is per user, not per project. These commands use
only narrow IPC lifecycle/refresh exchanges; they do not route existing CLI/MCP
project operations or automatically replay requests.

Provide an explicit local connection-info command for HTTP callers. Keep
connection information in private per-user runtime state; return the HTTP
endpoint and daemon-session token only on deliberate local request. Routine
status output and logs omit tokens. With a running daemon and HTTP disabled,
`connection-info` returns `{available:false,reason:"http_disabled"}` successfully;
absence/incompatibility is a connection error, not an enabled HTTP handoff. The
enabled result is `{available:true,endpoint:"http://<loopback-address>:<port>/rpc",
token:"<daemon-session credential>"}` through this local-only boundary, never
through status or project files. Credentials remain memory-only, not a persisted
runtime file. The caller supplies the connection
information to the browser client. A dedicated UI launcher and browser pairing
are outside 0.4.0; a later launcher can automate this handoff without expanding
browser authority.

`run` and `start` accept startup-only `--http-bind <numeric-loopback-IP:port>`
(port 0 selects an available port), repeatable `--http-origin <serialized-origin>`,
`--http-project <absolute-root>` and `--http-init-destination <absolute-target>`.
Omitting `--http-bind` disables HTTP; policy flags without it fail. Enabling HTTP
requires at least one exact serialized HTTP(S) origin (scheme, host and optional
non-default port; no path, wildcard, credentials or opaque origin). Empty root
and destination lists grant no project authority. Origins need not be loopback;
only the daemon listener must be. Hosted-browser certification remains downstream.
`start` forwards policy to its child; an existing compatible owner is reported
unchanged, never reconfigured. Policy changes require local stop/restart.

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

:::mara decision ADR-PROJECT-SCOPED-HTTP-ACCESS
:mid: 01M40DS7B7RNYE5JDVYHF6C052
:title: Limit browser authority to explicitly authorized projects
:status: draft
:justifies: REQ-LOCAL-DAEMON-HTTP
:justifies: DES-LOCAL-DAEMON-PROTOCOL

Choose explicit project-scoped browser authority rather than grant access to
every project available to the daemon's OS user. A session token authenticates
the caller and the origin policy identifies the browser application; neither
substitutes for project authorization.

Project reads can execute configured indexers, so access also grants command
execution within that project's trusted-command boundary. Keep unrelated
projects and daemon-wide administration outside browser authority. Local
connection-info handoff supports the library workflow without adding a pairing
service or the deferred UI launcher.
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

For the delivered embedded session boundary, run `cargo test --locked --test
native_sessions` plus `cargo test --locked --test native_sessions
real_indexer_activity -- --ignored --exact --nocapture` with an installed real
`scip-typescript`. The explicit probe invokes that indexer against a disposable
TypeScript project; captured SCIP bytes are not a substitute indexer. Require
observable unchanged parse/projection/compilation counts and subprocess activity,
current invalid-input errors, isolated real Git worktrees, ignored explicit
assets, forced rebuilding and idle/in-flight retention checks.

Use existing Rust tests/tempfile and standard process tools; keep fixture
Git/configuration state isolated. Capture results at the tested revision.
:::

:::mara design DES-EXPLICIT-SOURCE-ENGINE
:mid: 01M410H76N0V58ATK409CD5FV4
:title: Query and check source conformance from explicit inputs
:status: draft
:kind: interface
:satisfies: REQ-PORTABLE-DOCUMENT-ENGINE
:satisfies: REQ-EXPLICIT-BACKEND-CAPABILITIES

The `mara-engine` workspace crate owns project/schema parsing and recovery,
Markdown projection, identity/field/reference conformance, discovery and semantic
relations, bounded document queries, structural graph policies, supported-profile
rule compilation/evaluation and request-local trace computation. It contains
no filesystem discovery/loading, cwd/environment access, configured subprocess
execution, identity entropy/time generation or publication. All packages inherit
the workspace version. Native `mara` owns host effects and uses the shared models
and computations; CLI/MCP invocation and result contracts remain unchanged.

`SourceEngine::new(SourceSnapshot)` takes snapshot version 1 under
[[DES-BROWSER-PROJECT-SNAPSHOT]]. Its public methods are schema get/list/validate,
item list/search/get/related, relation inspection, project/item validation,
`rule_checks(target)` and `trace_matrix(params)`.
Reuse existing query parameter/result types and validation options; validation
uses a logical project root. Errors retain owning classifications and locations.
Snapshot construction or strict reads reject invalid inputs; recovering
validation reports incomplete prerequisites. Native snapshot construction
refuses incomplete document projections; retained nodes alone cannot establish
a complete source inventory. When include configuration is unusable, source
validation retains its configuration diagnostics and independently supplied
source recovery instead of failing membership checks before reporting them.

The document-only constructor installs no code capability. Active configured
languages, authored code endpoints or an applicable code-source graph policy
require code information; source validation reports `evaluation_unavailable`
and incomplete evaluation rather than an empty successful graph. Unused code
relation declarations alone do not require code. Independent schema checks and
document list/search/get remain available; code-dependent navigation fails.
For `related`, capability requirements follow the selected node, direction,
relations and neighbour-flavour filters, not whole-corpus validation needs.
Native `CodeIndex`/`CodeSource` inputs are supplied by the host; only native
hosting indexes or loads code/asset bytes. This is not a serialized browser
code-index import capability.

Rule adapters supply `RuleChecks`: definition-check outcomes and, for project
or item checks, outcomes bound to the exact target and source fingerprint.
Reject stale outcomes. A corpus derived with source replacements invalidates
its loaded-source identity; rebuild the full snapshot before binding rule checks.
Missing required rule checks report unavailability;
supplied diagnostics and completeness are composed with source and graph checks
before full-target summaries, reporting filters and pagination.

`rule_checks(target)` compiles configured YAML from supplied snapshot sources
and returns actual definition/evaluation outcomes using the synchronous SHACL
adapter under [[ADR-NATIVE-SHACL-ADAPTER]]. Callers pass these outcomes into
validation; omitting them still reports unavailable configured checks. Project
and schema prerequisites must be usable; invalid rule definitions remain
source-linked diagnostics. The native `load_rule_checks` adapter preserves its
source/target binding and native corpus/code checks using the same evaluator.
Native runtime validation composes loaded configuration diagnostics, recovering
corpus/code inputs and `RuleChecks` through the engine's
`validation::evaluate_loaded` before host fingerprinting, selection and paging.
The engine rejects mismatched source/target bindings and marks missing required
rule outcomes unavailable; filesystem loading failures remain explicit host
inputs, not fabricated successful snapshots.

`trace_matrix(params)` loads configured or request-local YAML only from the
snapshot. Literal binding follows [[DES-TRACE-CHECK-BINDING]]; explanation,
applicability, canonical edges, summaries and continuation follow
[[DES-TRACE-VIEW-INTERFACES]]. Source identity includes every supplied source
and selection/ignore input. Missing rules or required code never produce a
pass. Native hosting supplies rule source text or sanitized loading failures,
plus its loaded corpus, prerequisites and complete code/asset/excluded-source
identity to the shared trace computation; native filesystem containment and
canonical duplicate checks remain host-owned. `trace::matrix_with_rule_loader`
allows a host to reuse source/schema/parameter-bound rule compilation while
the shared computation still validates parameters and request checks.

Initialization and validated mutations are delivered separately under
[[DES-SEMANTIC-CHANGE-PLANS]], using this snapshot boundary without redesigning
rule outcomes. Native session ownership follows [[DES-NATIVE-PROJECT-SESSIONS]];
client routing and packaging retain their separate boundaries. Native mutations
retain owning source prerequisites, available required code and guarded
publication. No partial-conformance edit profile is introduced.
:::

:::mara decision ADR-EXPLICIT-RULE-CHECK-INPUTS
:mid: 01M410HJ84ZT36DGH6Q5N4H7MS
:title: Compose explicit rule checks without reducing validation coverage
:status: draft
:justifies: DES-EXPLICIT-SOURCE-ENGINE

Keep the source-conformance boundary independently usable. Accept explicit,
source- and target-bound rule outcomes rather than duplicating rule semantics
or treating absent checks as successful validation. Outcomes come from the
shared synchronous SHACL adapter: document snapshots use `rule_checks(target)`;
native hosting supplies its loaded corpus and code capabilities through
`load_rule_checks`.

Without required outcomes, report incomplete evaluation and
`evaluation_unavailable`; independent document reads and schema discovery
remain usable. This is an input boundary, not passing execution evidence,
authorization or a reduced rule profile. Complete native rule/trace behavior
and the complete portable document profile remain required.
:::

:::mara decision ADR-HTTP-REQUEST-NEGOTIATION
:mid: 01M43G39EGNC015P94Y5R2GBEA
:title: Negotiate HTTP compatibility on every request
:status: draft
:justifies: DES-LOCAL-DAEMON-PROTOCOL

Choose exact application/protocol version headers on each HTTP request rather than an opaque negotiated-session handle. Browser connection pooling cannot preserve a TCP-local RpcSession; request-scoped negotiation reuses the shared negotiator without an additional credential, session store or expiry contract. Explicit mara.negotiate still returns the same capabilities. Incompatibility fails before project dispatch; no backend switching or automatic replay is authorized.
:::
