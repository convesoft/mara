# Changelog

All notable changes to Mara are generated from Conventional Commit history.
## [0.4.0-alpha.1]

### Added

- Extract explicit-source document engine
- Port document rules and trace evaluation to the engine
- Integrate validated semantic source-change plans
- Integrate complete embedded native runtime
- Integrate the browser WASM document workspace
- Integrate consolidated browser source export
- Reuse bounded fresh native project sessions
- Add versioned same-user daemon IPC
- Route native operations through an optional daemon
- Manage the native daemon lifecycle explicitly
- Bind native operations through asynchronous N-API
- Expose authorized loopback daemon HTTP operations
- Expose the shared typed ESM client and native adapter
- Add the typed browser worker WASM client
- Add the explicit browser HTTP client adapter
- Prove portable query and SHACL result execution
- Add impact-based Rust verification
- Add typed named factual queries
- Compose named queries with explicit projections
- Add query-backed rules and contextual gates
- Render factual query views and evaluation reports
- Integrate external facts and derived query computations
- Expose shared query consumers across native and browser adapters
- Verify the reusable query catalog across real hosts

### Build

- Establish browser WASM document dependency path
- Package native and browser libraries before publication

### Changed

- Bundle reviewer assets and launch script in the skill
- Use Bash for the Pi review launcher
- Simplify query input preflight condition

### Continuous integration

- Use standard build storage and warm Rust caches
- Normalize host temporary paths before creating fixtures
- Save native caches only after complete successful jobs

### Documentation

- Define 0.4 execution and integration scope
- Settle 0.4 browser safety and access contracts
- Require disk-backed scratch and explicit cleanup
- Move scratch policy to global Pi instructions
- Save standalone reviewer instructions for Pi
- Add reusable reviewer scope prompt
- Defer macOS execution verification to the final 0.4.0 gate
- Adopt bounded local scope delivery
- Close work after verified target integration
- Define the shared label-based backlog policy
- Rename planned scope to 0.4.0-alpha.1
- Schedule reusable query engine for 0.4.0-alpha.1
- Specify reusable query semantic contracts
- Select the typed named-query adapter boundary
- Record single-language query authoring goal
- Select standard SPARQL query authoring
- Schedule initial v1 SPARQL contract alignment
- Limit v1 alignment to query definition numbering
- Defer dedicated alpha qualification and guidance

### Fixed

- Skip project resources in Pi reviewer sessions
- Isolate test Chromium runtime sockets
- Renumber named-query definitions to v1

### Maintenance

- Switch delivery tracking to Work
- Support Pi and Codex PR workflows
- Track project MCP configuration
- Remove project MCP configuration
- Add an opt-in local Codex review loop
- Support worker-driven Codex review loops
- Remove the bounded review limit
- Plan 0.4.0 delivery in Work
- Record integrated document portability acceptance
- Run local review loops with Pi
- Record source-engine integration and completion
- Migrate source-engine evidence to session storage
- Record rule and trace integration completion
- Record semantic planner integration completion
- Record completed parallel worker cleanup
- Record native runtime integration completion
- Record WASM workspace integration completion
- Record native and WASM worker cleanup
- Record browser export integration completion
- Record browser export worker cleanup
- Record native sessions acceptance and cleanup
- Record native routing integration acceptance
- Record native routing resource cleanup
- Record daemon lifecycle integration and cleanup
- Declare HTTP and native verification delivery targets
- Record N-API scope acceptance and resource cleanup
- Declare typed ESM local delivery target
- Record HTTP acceptance and authorized worker cleanup
- Record native verification acceptance and integration cleanup
- Add reusable finding triage to review workflows
- Record finding triage verification and cleanup
- Record backlog policy verification and cleanup
- Record typed ESM local integration acceptance
- Record typed ESM resource cleanup
- Record browser WASM local integration acceptance
- Record browser WASM resource cleanup
- Record browser HTTP local integration acceptance
- Record browser HTTP resource cleanup
- Record packaging integration acceptance
- Record packaging resource cleanup
- Record installed native integration acceptance
- Record installed native resource cleanup
- Record packaged WASM integration acceptance
- Record packaged WASM resource cleanup
- Record query contract integration acceptance
- Record query contract resource cleanup
- Record verified query proof integration
- Record query proof worker cleanup
- Relax review reasoning
- Record fresh named-query worker dispatch
- Record verified named-query integration
- Record named-query resource cleanup
- Record verified query composition integration
- Record query policy integration and cleanup
- Record query view integration and cleanup
- Record accepted external-facts scope integration
- Clarify adapter worker phases and execution profile
- Record accepted adapter scope integration
- Record combined verification worker profile and handoff
- Record query catalog integration and host handoff
- Select available Luna for query format alignment
- Record bounded query format v1 dispatch
- Record verified query definition v1 integration

### Tests

- Verify native execution workflows through real lifecycle daemons
- Verify installed native and HTTP clients
- Verify packaged browser parity and safe export
- Fix macOS path fixtures and shutdown backpressure
- Retain macOS fixture declarations for code indexing
## [0.3.0]

### Breaking changes and migration

Schema format 3 is required. Review existing project vocabulary and authored
relations before editing the schema; existing projects do not acquire the new
engineering profile or its rules automatically. The Agent Plugin distribution
has been removed; configure MCP explicitly and install the matching skill
separately. Follow the [0.3 migration guide](https://github.com/convesoft/mara/blob/v0.3.0/docs/migration-0.3.mara.md)
for source edits, validation, and client interface changes.


### Added

- Rebuild Mara around engineering traceability

### Build

- Drop Intel macOS and speed verification
## [0.3.0-alpha.0]

### Added

- Support inverse authoring and symmetric relationships
- Support typed inline relationships
- Introduce validation diagnostic codes and severity
- Validate conditional lifecycle and coverage rules
- Support typed external targets
- Validate relationship cardinality and cycles
- Generate traceability matrices
- Make code traceability language pluggable

### Documentation

- Define relationship authoring and mutation contracts
- Define SHACL rules and trace-view contracts
- Retire specification export contract
- Document manual 0.3 migration workflow

### Fixed

- Locate bounded trace chain limit failures

### Maintenance

- Evaluate code traceability pilot
## [0.2.0]

### Breaking changes and migration

Schema format 2 requires flavour selection guidance. Migrate existing schemas
in place, preserving custom vocabulary and item identities; do not reinitialize.
CLI and MCP discovery now use `search`, `get`, and `related` for items and
narrative, replacing their item-prefixed forms and changing response shapes.
Use the matching executable and skill, refresh MCP tools, and discard old cursors.
Follow the [0.2 migration guide](https://github.com/convesoft/mara/blob/v0.2.0/docs/migration-0.2.mara.md)
for schema examples, command/response mapping, and reference validation changes.

### Added

- Parse Mara items as Markdown containers
- Derive document structure and direct containment
- Resolve Markdown links, anchors, and mentions
- Add discovery handles and shared node summaries
- Search items and narrative through unified discovery
- Read every discovery node through unified get
- Explore direct connections through unified related
- Protect references across item mutations

### Fixed

- Align 0.2 CLI and MCP onboarding

### Tests

- Verify the packaged 0.2 workflow
## [0.2.0-alpha.0]

### Added

- Require schema format 2 and flavour guidance
- Add bundled engineering templates and prepare 0.2.0-alpha.0
## [0.1.0-alpha.3]

### Added

- Paginate search results and add optional excerpts
- Paginate direct relation results
- Paginate item reads with consecutive fragments
- Combine exact and typo-tolerant item search
- Rank search results by relevance
- Create items with initial relations atomically
- Filter item search and list by directory
- Filter project validation diagnostics by path

### Documentation

- Clarify Mara skill workflows and JSON CLI fallback
- Investigate narrative retrieval boundaries

### Fixed

- Clarify CLI help and MCP input guidance
## [0.1.0-alpha.2]

### Added

- Add durable item identities
- Move items between documents
- Update items structurally
- Delete unreferenced items safely
- Rename item IDs across the corpus

### Continuous integration

- Remove optional plugin release automation

### Documentation

- Clarify alpha 3 retrieval scope

### Fixed

- Make plugin release validation deterministic
- Make Codex compatibility dispatchable
## [0.1.0-alpha.1]

### Added

- Add portable agent onboarding
## [0.1.0-alpha.0]

### Added

- Initialize and discover Mara projects
- Load and inspect project schemas
- Parse Mara documents into memory
- Validate Mara projects and items
- Create items and mutate relations
- Query project knowledge
- Expose alpha operations through MCP
- Improve deterministic item search
- Prepare first distributable alpha

### Continuous integration

- Expose npm OIDC exchange diagnostics

### Documentation

- Establish repository and document format
- Define self-hosting taxonomy
- Define first alpha contract
- Define delivery and review workflow
- Align Linear delivery instructions
- Add pull request template
- Document pull request workflow

### Fixed

- Make release changelog squash-stable
- Run protected release from main context
- Publish missing npm package versions
- Publish npm tarballs from local paths
- Enable npm OIDC authentication
- Run releases from main push identity

### Maintenance

- Dogfood Mara repository
