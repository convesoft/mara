# Changelog

All notable changes to Mara are generated from Conventional Commit history.
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

### Maintenance

- Release 0.2.0

### Tests

- Verify the packaged 0.2 workflow
## [0.2.0-alpha.0]

### Added

- Require schema format 2 and flavour guidance
- Add bundled engineering templates and prepare 0.2.0-alpha.0
## [0.1.0]

### Maintenance

- Release 0.1.0
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
