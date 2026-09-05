# Changelog

All notable changes to Mara are generated from Conventional Commit history.
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
