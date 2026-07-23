# Project and content system

The project system turns a working directory into a deterministic Mara input
set. It establishes one project root, one schema, and an explicit set of source
documents without requiring Git or a hosted service merely to validate files.

## Project initialization and discovery

Authors need commands to find the same project from any nested directory and to
bootstrap the smallest valid configuration without silently imposing a process.

:::req m_01KY7Y9R405AY85BQ48JMXJ4Y3
:id: REQ-PROJECT-DISCOVERY
:title: Mara shall discover the nearest project configuration
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: SCN-INITIALIZE-PROJECT

Mara shall walk from the current directory toward the filesystem root and use
the nearest `.mara/project.toml` as the project configuration unless the caller
supplies an explicit project path.
:::

:::req m_01KY7Y9R41VJ5RSW3MR6Q22KH9
:id: REQ-PROJECT-ROOT
:title: Mara shall resolve project paths from one root
:status: approved
:level: system
:kind: constraint
:priority: must
:derives_from: SCN-INITIALIZE-PROJECT

The directory containing `.mara/` shall define the project root. Mara shall
resolve every relative schema, content, index, and output path from that root,
independent of the caller's current directory.
:::

:::req m_01KY7Y9R42Z0CGPE48F80DH9WE
:id: REQ-PROJECT-INIT
:title: Mara shall initialize a minimal process-neutral project
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: SCN-INITIALIZE-PROJECT

`mara init` shall create `.mara/project.toml` and a valid local
`.mara/schema.yaml` containing identity configuration and empty flavour,
relation, and rule collections. It shall not create business flavours and shall
refuse to overwrite an existing Mara project or target file.
:::

:::req m_01KY7Y9R43EYNDEGF18G6YWEME
:id: REQ-PROJECT-CONFIG-STRICT
:title: Mara shall parse project configuration strictly
:status: approved
:level: software
:kind: functional
:priority: must
:derives_from: STORY-VALIDATE-PROJECT

The project configuration shall declare an integer format version, project
name, schema path, content discovery settings, index path, validation settings,
and Git write policy. Unknown keys, duplicate assignments, invalid types, and
unsupported versions shall be errors with source locations.
:::

## Content-file discovery

Projects must be able to distinguish authored Mara documents from ordinary
Markdown while retaining explicit control over mixed repositories.

:::req m_01KY7Y9R4493V79VHJRGKF59MS
:id: REQ-CONTENT-GLOBS
:title: Mara shall discover content through configured globs
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: STORY-AUTHOR-KNOWLEDGE

Mara shall apply project-relative include and exclude globs to discover source
documents. A newly initialized project shall include `**/*.mara.md` and shall
allow projects to add explicit ordinary Markdown paths or patterns.
:::

:::req m_01KY7Y9R453PP2J5DQ2FT6R9Y5
:id: REQ-CONTENT-GITIGNORE
:title: Mara shall respect Git ignore rules by default
:status: approved
:level: system
:kind: functional
:priority: should
:derives_from: STORY-VALIDATE-PROJECT

When a Git worktree is available and the project enables ignore handling, Mara
shall exclude files ignored by the repository after applying Mara's configured
include and exclude patterns.
:::

:::req m_01KY7Y9R46P83S19JPWT1KAGAR
:id: REQ-CONTENT-UNTRACKED
:title: Mara shall validate new untracked content
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: STORY-VALIDATE-PROJECT

Mara shall include matching, non-ignored untracked files so authors can validate
new documents before committing them.
:::

:::req m_01KY7Y9R47Q6H0QHSAMABCEG9V
:id: REQ-CONTENT-SYMLINKS
:title: Mara shall constrain symbolic-link traversal
:status: approved
:level: system
:kind: security
:priority: must
:derives_from: STORY-VALIDATE-PROJECT

Mara shall not follow directory symbolic links by default. It may read a file
symbolic link only when configuration permits it and the resolved target remains
inside the project root.
:::

## Locality and path safety

Local deterministic loading protects reproducibility and prevents schemas from
becoming executable configuration.

:::req m_01KY7Y9R48TD56F0N8CQB5GQKH
:id: REQ-PATH-CONTAINMENT
:title: Mara shall contain configured paths within the project
:status: approved
:level: system
:kind: security
:priority: must
:derives_from: GOAL-AUDITABLE

Mara shall reject schema, content, index, and output paths whose normalized
targets escape the project root. Diagnostics shall report both the configured
path and the reason it is outside the allowed boundary.
:::

:::req m_01KY7Y9R4984S0899C19T67EYH
:id: REQ-LOCAL-LOADING
:title: Mara shall load projects without external side effects
:status: approved
:level: system
:kind: constraint
:priority: must
:derives_from: GOAL-AUDITABLE

Project and schema loading shall perform no network requests, environment-
variable expansion, shell execution, plugin execution, or custom validation
scripts. External URIs shall be treated as data and shall not be fetched in
v0.1.
:::

## Design, rationale, and risk

:::design m_01KY7Y9R4CW9YVMPVFNSK0MDQ3
:id: DES-PROJECT-LOADER
:title: Rooted project loader pipeline
:status: accepted
:kind: component
:satisfies: REQ-PROJECT-DISCOVERY
:satisfies: REQ-PROJECT-ROOT
:satisfies: REQ-PROJECT-CONFIG-STRICT
:satisfies: REQ-CONTENT-GLOBS
:satisfies: REQ-CONTENT-GITIGNORE
:satisfies: REQ-CONTENT-UNTRACKED
:satisfies: REQ-CONTENT-SYMLINKS
:satisfies: REQ-PATH-CONTAINMENT
:satisfies: REQ-LOCAL-LOADING

The engine shall expose one loader that resolves the project root, validates
configuration, normalizes contained paths, discovers content, and returns a
deterministically ordered project input set. Git contributes ignore and
provenance information when present but is not required for read-only loading.
:::

:::decision m_01KY7Y9R4FTJA3DPB9GHNC2ME1
:id: ADR-0001
:title: Discover projects through the .mara directory
:status: accepted
:kind: architecture
:justifies: DES-PROJECT-LOADER
:justifies: REQ-PROJECT-DISCOVERY

Mara uses a dedicated `.mara/project.toml` marker rather than overloading the
repository root manifest. This permits nested projects and makes discovery
independent of programming language or build system.
:::

:::risk m_01KY7Y9R4D4SN9CTDBSFAX8138
:id: RISK-PATH-ESCAPE
:title: Configured paths may escape the reviewed project
:status: open
:severity: high
:likelihood: medium
:affects: REQ-PATH-CONTAINMENT
:affects: DES-PROJECT-LOADER

Symlinks, parent components, and platform-specific path forms could cause Mara
to read or write files outside the repository content that a reviewer expects.
Containment checks must operate on normalized resolved paths.
:::

:::artifact m_01KY7Y9R4ECF8K0F1HBZN4TY25
:id: ART-PROJECT-CONFIG
:title: Mara project configuration
:status: proposed
:kind: schema
:uri: .mara/project.toml

The versioned TOML configuration locates the project schema and controls
operational discovery, indexing, validation, and Git write policy.
:::

## Planned verification

:::test m_01KY7Y9R4AZFZPZ9FPF5DNZY60
:id: TEST-PROJECT-DISCOVERY
:title: Project initialization and root discovery acceptance test
:status: approved
:kind: verification
:method: automated
:level: integration
:verifies: REQ-PROJECT-DISCOVERY
:verifies: REQ-PROJECT-ROOT
:verifies: REQ-PROJECT-INIT
:verifies: REQ-PROJECT-CONFIG-STRICT

Fixtures shall cover initialization, nested working directories, nearest-root
selection, strict configuration failures, and refusal to overwrite existing
files.
:::

:::test m_01KY7Y9R4BY1FDYHZVMMKVNWRD
:id: TEST-CONTENT-DISCOVERY
:title: Content discovery acceptance test
:status: approved
:kind: verification
:method: automated
:level: integration
:verifies: REQ-CONTENT-GLOBS
:verifies: REQ-CONTENT-GITIGNORE
:verifies: REQ-CONTENT-UNTRACKED
:verifies: REQ-CONTENT-SYMLINKS

Fixtures shall demonstrate deterministic include/exclude precedence, ignored
files, new untracked documents, directory symlinks, and permitted internal file
symlinks.
:::

:::test m_01KY7Y9R4GQ49M36FDYNRXJB17
:id: TEST-PROJECT-LOCALITY
:title: Project locality and containment security test
:status: approved
:kind: verification
:method: automated
:level: system
:verifies: REQ-PATH-CONTAINMENT
:verifies: REQ-LOCAL-LOADING

Fixtures shall attempt traversal through parent components, absolute paths,
symlinks, environment expressions, and external URIs. Mara shall reject escaped
paths and shall perform no external action.
:::
