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
N-API wraps that client. WASM wraps the portable engine. The shared TS facade
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
bound-server override refusal. Each call loads current inputs; sessions, caches,
client routing and daemon transports remain separate deliverables.

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
loopback, and checks the session token, allowed Origin and project allowlist
before project access. Configure exact authorized project roots and explicit
initialization destinations locally when starting the daemon; authorizing one
root does not authorize descendants, another project or another Git worktree.
Resolve request targets against that authority rather than trusting path
spelling. Do not put tokens in URLs or logs, or allow wildcard browser origins.

HTTP exposes authorized project operations, including explicit rollback and
force refresh, but no daemon-wide start/stop, access-policy or credential
management. Connection information is handed off locally under
[[DES-DAEMON-LIFECYCLE]]; there is no browser pairing protocol.

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

Provide an explicit local connection-info command for HTTP callers. Keep
connection information in private per-user runtime state; return the HTTP
endpoint and daemon-session token only on deliberate local request. Routine
status output and logs omit tokens. The caller supplies the connection
information to the browser client. A dedicated UI launcher and browser pairing
are outside 0.4.0; a later launcher can automate this handoff without expanding
browser authority.

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
canonical duplicate checks remain host-owned.

Initialization and validated mutations are delivered separately under
[[DES-SEMANTIC-CHANGE-PLANS]], using this snapshot boundary without redesigning
rule outcomes. Native sessions/routing, worker/client packaging and complete
browser operation execution remain separate deliverables. Native mutations
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
