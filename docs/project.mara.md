# Project initialization and schema discovery

Initialization creates project-owned configuration and schema. The optional
engineering profile supplies editable vocabulary and knowledge policies.

:::mara requirement REQ-PROJECT-INITIALIZATION
:mid: 01M1PXP2KG7SDH3FRPXK9EN06C
:title: Initialize Mara in a current or named directory
:status: accepted
:derives_from: SCN-START-STRUCTURED-PROJECT
:kind: functional

`mara project init` initializes the current directory. An optional path is
created when absent or initialized when it is an existing directory. Global
`--project <path>` selects the initialization target when the positional path
is omitted; supplying both is rejected. Existing content is not
overwritten, and an existing Mara project is rejected. The default template is
`minimal`; `--template empty` creates no project flavours. MCP `project_init`
provides the same operation with an optional `template`. An unbound server call
requires an absolute `project` path; a server started with `--project` uses its
bound target and rejects a request-level `project` override.
:::

:::mara requirement REQ-PROJECT-DISCOVERY
:mid: 01M1PXP2KG99P47TB3HH0A9635
:title: Resolve one explicit project for each operation
:status: accepted
:derives_from: SCN-START-STRUCTURED-PROJECT
:kind: functional

Commands discover the nearest parent containing `.mara/project.toml`. Global
`--project <path>` overrides discovery. `mara mcp` starts without resolving a
project. Each project-bound MCP tool accepts an optional absolute `project`
path; when absent, it discovers from the server execution directory. Starting
the server as `mara mcp --project <path>` binds it to that selection, and tools
must then omit `project`. Each operation resolves exactly one project.
:::

:::mara requirement REQ-SCHEMA-DISCOVERY
:mid: 01M1PXP2KGPMYWSV20VA6TP3R6
:title: Make the effective project schema discoverable
:status: accepted
:derives_from: SCN-START-STRUCTURED-PROJECT
:kind: functional

Users and agents can retrieve the complete effective project schema, list flavours or relations, retrieve one declaration, and validate the configured schema. Every flavour and relation has a concise description suitable for discovery.

Schema validation checks project/schema configuration and configured YAML rule definitions independently of item or code content. Report invalid definitions with stable diagnostics and bounded continuation following [[DES-SCHEMA-VALIDATION]]. Successful definition checking does not establish item conformance.
:::

:::mara requirement REQ-SURFACE-PARITY
:mid: 01M1PXP2KGA6GZQB9MCYMYVNJA
:title: Expose the same operations through CLI and MCP
:status: accepted
:derives_from: SCN-START-STRUCTURED-PROJECT
:kind: functional

CLI and MCP share operation semantics and domain results. CLI defaults to human-readable output; global `--format json` returns agent-oriented data equivalent to MCP structured results. Transport wrappers and operation-error classification follow each owning operation contract.

CLI help describes every public command and its arguments/options. MCP tool descriptions and input schemas expose equivalent invocation guidance, including exact identities, paths, defaults, omission and empty values, repeated inputs and bounded continuation. State current behavior and supported formats. Project selection follows [[DES-OPERATION-PROJECT-CONTEXT]]; reads, mutations, validation and matrices retain their own contracts.

Reject undeclared CLI options and MCP arguments before invoking an operation or changing project source. CLI argument-parse failures exit with status 2: human diagnostics use stderr, while JSON mode emits an error message object on stdout. Help and version requests remain human-readable successful responses even when JSON output was selected.

Verify rendered help and real MCP initialization, tools/list and tools/call responses. Guidance must agree with executed behavior; advertised operations require working transport paths, and parameter schemas must describe nested inputs as well as top-level fields.
:::

:::mara decision ADR-EXPLICIT-INIT-TARGET
:mid: 01M1PXP2KGRT31XDJG6KEJZEDB
:title: Use the global project option as an initialization target
:status: accepted
:justifies: REQ-PROJECT-INITIALIZATION

Initialization accepts global `--project` as an alternative to its positional
target so automation can use one explicit-root option across bootstrap and
project-bound operations. The two forms conflict so target selection never
depends on precedence.
:::

:::mara design DES-OPERATION-PROJECT-CONTEXT
:mid: 01M1PXP2KG1T44F3T2E5YR4YX2
:title: Resolve project context at the operation boundary
:status: accepted
:satisfies: REQ-PROJECT-DISCOVERY
:satisfies: REQ-SURFACE-PARITY
:kind: interface

Public operation wrappers select a project from an explicit path or the execution directory, resolve its current project context, and invoke the operation against that context. Discovery selects the nearest parent containing `.mara/project.toml`; explicit selection overrides discovery.

MCP request selection is absolute and takes effect only when the server was not started with `--project`; a bound server rejects request-level selection. Resolution occurs for each call, so source and schema changes remain visible without server-side project caches.

Each operation resolves one project. Workspace aggregation and cross-project operations are outside this boundary.
:::

:::mara requirement REQ-ENGINEERING-TEMPLATE
:mid: 01M1XSKPP0SRTDXBZE3J2PVDJ8
:title: Initialize from a bundled engineering template
:status: accepted
:derives_from: SCN-START-ENGINEERING-KNOWLEDGE
:kind: functional

Offer an optional `engineering` template with the reusable engineering vocabulary
and incremental knowledge policies defined by the bundled template files. Keep `minimal`
as the default and retain `empty`. Initialization preserves
[[REQ-PROJECT-INITIALIZATION]] and creates no starter Markdown, product items or MIDs.

All templates install `.mara/project.toml` and `.mara/schema.yaml`.
Engineering additionally installs enabled `.mara/engineering-rules.yaml` and
request-local `.mara/engineering-checks.yaml` and `.mara/engineering-execution.yaml`.
Its configuration uses format 2; minimal and empty retain format 1. All schemas
use format 3.

Embed template source files in the executable. Generate configuration from the
destination name and selected template. Preserve all existing destination files;
on initialization failure remove only files created by that attempt. Bundled
template changes never silently rewrite previously initialized projects.

The bundled realization check supports accepted requirement and design roots,
including mixed selections. Both can qualify through direct implementation;
only a requirement can also qualify through an accepted satisfying design.
Missing realization is a failed coverage result, not an invalid check request.

Lifecycle, acceptance and selected-scope interpretation follow [[DES-ENGINEERING-PROFILE]].
:::

:::mara requirement REQ-FLAVOUR-AUTHORING-GUIDANCE
:mid: 01M1XSKPP7HRP5JKTDX05EFE04
:title: Require project-defined flavour selection guidance
:status: accepted
:derives_from: SCN-CHOOSE-KNOWLEDGE-FLAVOUR
:kind: constraint

Every declared flavour must provide guidance covering its purpose,
when to use it, when to avoid it, and distinctions from other flavours. Missing
required guidance is a schema validation error, including for existing project
schemas. An empty schema has no flavours that require guidance.

The persisted keys, value types, and content-validation rules follow
[[DES-FLAVOUR-AUTHORING-GUIDANCE]]. CLI and MCP schema inspection must expose the
same project-defined guidance under [[REQ-SCHEMA-DISCOVERY]] and
[[REQ-SURFACE-PARITY]]. Bundled templates must supply complete guidance for
their declared flavours without hardcoded business flavours in the engine.

:::

:::mara design DES-FLAVOUR-AUTHORING-GUIDANCE
:mid: 01M231916PQP6XRY5PYCMMW8QE
:title: Store mandatory guidance directly in each flavour declaration
:status: accepted
:satisfies: REQ-FLAVOUR-AUTHORING-GUIDANCE
:kind: data

Schema format 3 stores guidance directly in each project-defined flavour, alongside its ID prefix, body requirement, and field declarations. Schema declarations and bundled templates require `format_version: 3`. Readers reject unsupported format versions without rewriting source. Schema inspection reads these declarations without hardcoded business flavours or a separate guidance wrapper.

| Key | Type and validation |
|---|---|
| `description` | Required nonblank string. |
| `use_when` | Required sequence with at least one nonblank string. |
| `avoid_when` | Required sequence of nonblank strings; `[]` is valid. |
| `distinguish_from` | Required mapping from another declared flavour's name to a nonblank explanation; `{}` is valid. |

Omitting a key, supplying the wrong type, using blank entries, or naming an unknown or identical flavour as a distinction target is invalid. Empty collections express absence of exclusions or distinctions; authors need not add filler. CLI and MCP expose identical guidance. Verify these constraints through real schema loading, including valid empty collections and schemas with no flavours.
:::

:::mara verification VER-PROJECT-BOOTSTRAP
:mid: 01M3FVRDVRPA7DCVBECXP2W9A0
:title: Initialize and use an isolated engineering project through CLI and MCP
:status: accepted
:method: test
:level: system
:verifies: REQ-PROJECT-INITIALIZATION
:verifies: REQ-PROJECT-DISCOVERY
:verifies: REQ-SCHEMA-DISCOVERY
:verifies: REQ-ENGINEERING-TEMPLATE
:verifies: REQ-FLAVOUR-AUTHORING-GUIDANCE
:verifies: DES-OPERATION-PROJECT-CONTEXT
:verifies: DES-FLAVOUR-AUTHORING-GUIDANCE
:validates: SCN-START-STRUCTURED-PROJECT
:validates: SCN-START-ENGINEERING-KNOWLEDGE
:validates: SCN-CHOOSE-KNOWLEDGE-FLAVOUR
:verifies: DES-ENGINEERING-PROFILE

Run `cargo test --locked --test project_bootstrap --test trace_matrix` against the candidate binary. The test run must exercise the complete authoring flow below through real CLI and MCP requests. Exercise current, named, and explicit initialization targets; default, empty, and engineering templates; refusal to overwrite project-owned files; nearest and explicit project selection; bound and unbound stdio MCP; and schema guidance inspection and rejection.

Initialize an engineering project, create each supported knowledge flavour, add valid typed relationships, inspect both directions through CLI and MCP, reject invalid endpoints without changing source, and validate the resulting project. Exercise the bundled realization check on design-only and mixed requirement/design selections, first without realization and then with a direct implementation of the design. Both cases must evaluate completely, with failed then passed coverage and equivalent CLI/MCP results.

Fixtures live in disposable directories with their own configuration and Git state. Child processes use explicit working directories and isolated Git/configuration inputs. Pass only when real processes return the expected results and rejected requests preserve source. This checks project bootstrap and demonstrates the engineering authoring scenario; detailed mutation, retrieval, and rule-engine checks remain separate.
:::

:::mara verification VER-PROJECT-INSPECTION
:mid: 01M3FXVMC6V323RK02JD4RJ13Y
:title: Initialize and inspect isolated projects through CLI and MCP
:status: accepted
:method: test
:level: system
:verifies: REQ-PROJECT-INITIALIZATION
:verifies: REQ-PROJECT-DISCOVERY
:verifies: REQ-SCHEMA-DISCOVERY
:verifies: REQ-ENGINEERING-TEMPLATE
:verifies: REQ-FLAVOUR-AUTHORING-GUIDANCE
:verifies: DES-OPERATION-PROJECT-CONTEXT
:verifies: DES-FLAVOUR-AUTHORING-GUIDANCE

Run `cargo test --locked --test project_bootstrap` against the candidate CLI and real stdio MCP server in disposable projects with isolated Git/configuration state. Check current, named and explicit initialization targets; all three bundled templates; existing-file preservation and conflicting-target rejection; nearest and explicit discovery; and absolute per-call selection versus bound-server override rejection.

Compare complete schema, flavour/relation lists and named declarations through both transports. Inspect configured schema paths and authoring guidance, and reject malformed guidance without rewriting source. Inspect CLI help and MCP tools/list for the public operations and project-selection inputs under [[REQ-SURFACE-PARITY]].

This method covers initialization, template installation and schema inspection. [[VER-SCHEMA-DEFINITIONS]] checks definition validation; [[VER-PROJECT-BOOTSTRAP]] defines the broader engineering authoring workflow. Installation checks alone do not establish rule evaluation or full template behavior.
:::

:::mara design DES-SCHEMA-RULE-DEFINITIONS
:mid: 01M3FY8VS6DBKJ78K359ZETS59
:title: Check the supported YAML rule definition profile
:status: accepted
:kind: data
:satisfies: REQ-SCHEMA-DISCOVERY

Project formats 2/3 may enable `[rules]` with `format_version = 1` and explicit project-relative YAML/YML files. Omitted or empty files means no enabled rules. Sources must resolve to distinct regular files inside the project; no globs, imports or network reads. Read fresh UTF-8 source on each request.

Each file contains one mapping or sequence of shape mappings with string keys and JSON-compatible values. Reject duplicate keys, merge keys, custom tags and invalid aliases. Keep authored YAML locations. Top-level shapes require `id`: `rule:name` expands to `urn:mara:rule:name`; other IDs must be absolute IRIs. Nested anonymous identities are tied to source snapshot and location. Merge compatible repeated named definitions; conflicting single-valued parameters fail, while property/class/targetClass lists combine distinct values. File order is not override precedence.

Generate JSON-LD bindings in memory from declared fields, flavours and canonical relations. Reject authored contexts, raw JSON-LD keys, unsupported keys or parameter types before conversion. Compile with unmodified `shacl`, `rudof_rdf` and `rudof_iri` 0.3.21. Mara validates its vocabulary before compilation; the native engine owns constraint semantics under [[ADR-NATIVE-SHACL-ADAPTER]].

Supported shapes are NodeShape and PropertyShape (the latter requires path). Bind `targetClass`/`class` to declared flavours; property paths are declared field/canonical relation names or `{inversePath: relation}`. Use `field:`/`schema:` qualification for collisions; inverse paths require canonical relations. Support property, node, not, and, or, qualifiedValueShape, min/max and qualified min/max counts, datatype, pattern, hasValue, in, and name/description/message annotations. Datatypes are string/integer/double/boolean. Nonnegative counts must fit the supported integer range; qualified counts require qualifiedValueShape. Validate regular expressions before native compilation.

Literal constraints accept scalars or exactly `{value,datatype}` with matching scalar type; null/containers are invalid. Explicit double literals accept finite numeric values; numeric strings are not converted. Persisted enabled rules reject request-parameter placeholders. Plain scalar RDF typing remains native JSON-LD typing, not coercion from the selected field's schema.

Only enabled NodeShape roots carry targetClass, optional Violation/Warning severity, whenShape or paths. Referenced shapes have no root selection metadata; whenShape names a targetless NodeShape. Validate reusable class-scoped definitions even when unused. Reject unknown references, cycles, shape-reference depth above 32, and relationship depth above eight. Field paths preserve declared datatypes through nested/logical/reused shapes. Reject item-class/path constraints on literals and literal/datatype constraints on relation endpoints. Class constraints narrow internal endpoints; external endpoints are terminal unless narrowed to an internal flavour. Same-flavour relations narrow both traversal directions. No unbounded paths, SPARQL or external code.

These are definition checks only. Successful compilation does not establish applicability, conformance, trace coverage or execution results.
:::

:::mara design DES-SCHEMA-VALIDATION
:mid: 01M3FY9S8B1JRX642ASSDKE3SD
:title: Validate schema and configured rule definitions without reading items
:status: accepted
:kind: interface
:satisfies: REQ-SCHEMA-DISCOVERY
:satisfies: REQ-FLAVOUR-AUTHORING-GUIDANCE
:satisfies: REQ-SURFACE-PARITY

`schema validate` and MCP `schema_validate` resolve the selected project, recover independent configuration/declaration errors, and check [[DES-SCHEMA-RULE-DEFINITIONS]]. They do not discover or evaluate item/code content. Reads preserve source. Schema read I/O failures are operation errors; malformed or unsupported configuration returns an invalid, incomplete domain result. Schema get/list retain their separate operation-error behavior.

Return validation `format_version:1`, `project`, `target:{kind:schema}`, `path`, `flavours`, `relations`, `valid`, `evaluation_complete`, `diagnostics`, `summary`, `selection:null`, `has_more` and `next_cursor`. Declaration counts are null when the schema cannot load. Summary contains whole-target errors, warnings and counts_exact; incomplete prerequisites make counts lower bounds. Validity requires complete evaluation and zero errors.

Configuration diagnostics use project_invalid, schema_invalid, format_unsupported or rule_invalid, always error severity. Each carries scope, actionable message and location with available project-relative path, one-based line, UTF-8 byte span and JSON Pointer. External configured schema paths stay absolute. The path/line aliases match location. Preserve authored rule locations through conversion; include the expanded root rule IRI when unambiguous. Classify by code, never message parsing.

Sort by scope, path, byte/line, item MID, rule, obligation source/shape/component, code and message. Limits are 1–100 records (default 20) and 65,536 serialized bytes including envelope/cursor. Compute summary and validity before pagination. Continue with unchanged project, options and accepted configuration/rule bytes; changes or malformed cursors return stale_cursor. Rejected external rule files are not read or hashed. Never silently skip a diagnostic; an indivisible oversized diagnostic returns output_limit.

CLI returns JSON or text diagnostics and exits 0 only for valid results. MCP returns invalid domain results with isError:false. Invalid options, stale continuation, I/O preventing a result and output limits return `{format_version:1,error:{code,message}}`, nonzero CLI status and MCP isError:true. Operation codes are invalid_argument, stale_cursor, io_error and output_limit.

Project/item validation shares this response family under [[DES-TRACE-DIAGNOSTIC-INTERFACE]]; native rule evaluation follows [[DES-CURRENT-STATE-EVALUATION]].
:::

:::mara verification VER-SCHEMA-DEFINITIONS
:mid: 01M3FY9WNH3D1NC2F7VKFMBR93
:title: Check schema definitions and continuation through real CLI and MCP
:status: accepted
:method: test
:level: system
:verifies: REQ-SCHEMA-DISCOVERY
:verifies: REQ-FLAVOUR-AUTHORING-GUIDANCE
:verifies: DES-SCHEMA-VALIDATION
:verifies: DES-SCHEMA-RULE-DEFINITIONS

Run `cargo test --locked --test schema_validation` against the candidate CLI and real stdio MCP server in isolated temporary projects. Verify all bundled schemas/rule definitions; invalid schema vocabulary, structural names and guidance; configuration recovery, stable codes and authored locations; reusable and nested definition type checks; unsupported grammar, recursion and depth boundaries; and valid counterparts.

Require identical domain envelopes and operation-error classifications across transports. Check full counts across pages, deterministic continuation, stale source/options, rejected outside rule sources, invalid limits/cursors, schema read errors and oversized diagnostics. Verify schema validation preserves source and succeeds despite unreadable corpus content. Review CLI help and tools/list for schema validation. These checks do not evaluate item conformance or establish graph/matrix behavior.
:::

:::mara verification VER-INTERFACE-PARITY
:mid: 01M3HGMVWFXSA5XYAEWK8TVWP5
:title: Verify CLI and MCP invocation contracts
:status: accepted
:method: test
:verifies: REQ-SURFACE-PARITY

Run `cargo test --locked --test interface --test project_bootstrap` against the candidate CLI and real stdio MCP server in disposable projects with isolated Git/configuration state. Traverse every command's rendered help and every MCP tool's input schema, including nested properties. Require actionable parameter guidance and the same identity, path, omission, empty-value, default and continuation conventions on both transports. Check current operation names and project-selection behavior; operation-specific suites own detailed domain semantics.

Check CLI argument-parse errors with JSON selection before and after the command, human error output, and help/version output. Require undeclared MCP arguments to fail before side effects, including MID backfill in a document with a missing MID; source must remain unchanged. Inspect all advertised tool schemas for rejection of undeclared top-level inputs.

Review the actual help and tool descriptions for truthfulness and current scope. Run formatting, Clippy and the full relevant regression suite. Record the exact tested revision and concrete CLI/MCP results; inspecting schemas alone does not establish successful operation execution.
:::
