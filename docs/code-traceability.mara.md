# Code traceability

Code associations use project-configured SCIP identities and optional Tree-sitter
comment ownership. [[DES-CODE-TRACEABILITY]] owns their shared contract.

:::mara requirement REQ-CODE-TRACEABILITY
:mid: 01M3EQAFT3DPZ2VF1GWZBX2T6Y
:title: Resolve code associations as typed relations
:status: accepted
:kind: functional
:derives_from: SCN-TRACE-IMPLEMENTATION-AND-CHECKS

A project can declare code-to-item relations, navigate the resulting source
associations, and mutate item-authored inverse assertions through relation
commands. Source code comments remain directly authored in code. Missing,
ambiguous and unsupported code targets produce source-located diagnostics. A
source association does not establish passing test evidence. Symbol links use exact
indexer identities: body edits, overload ordering, and package version bumps
must not select another symbol. Renames, moves, or deletion may break links;
Mara never falls back to a matching name or position. Language indexers own
overload distinctions; declarations sharing one implementation may share an identity.

Configured language integrations must support projects with no matching source
files without preventing documentation operations, and activate automatically
when matching sources appear. Once sources exist, indexer failures remain errors.
:::

:::mara design DES-CODE-TRACEABILITY
:mid: 01M3EQAJBYC0Q2TAVD3KM6E11H
:title: Resolve code endpoints through pluggable SCIP indexers
:status: accepted
:kind: interface
:satisfies: REQ-CODE-TRACEABILITY

Schema format 3 accepts optional `code_source: true` on a directed relation.
Such a declaration has `source: []`, a nonempty item-flavour `target`, and an
optional `inverse` for authoring from the item. It cannot be symmetric,
same-flavour, external, or code-to-code. It may declare incoming cardinality
on eligible item targets; outgoing and symmetric cardinality and acyclic
policies are invalid. A relation without `code_source` retains
its current item/external behaviour. All names remain project-declared. For
example, Mara's self-hosting project declares:

```yaml
relations:
  implements:
    description: The code implements the requirement.
    source: []
    target: [requirement]
    code_source: true
    inverse: implemented_by
  checks:
    description: The code defines a check of the requirement.
    source: []
    target: [requirement]
    code_source: true
    inverse: checked_by
```

The canonical edge is `(relation, code endpoint, item MID)`. Code is a built-in
node kind in the disposable relation graph, alongside item and external nodes.
Each kind supplies its own graph identity; only items have persisted MIDs and
schema-defined flavours. Code identity is the exact project-relative
file path plus the configured language name and exact SCIP descriptor, or just the path for a
file-only endpoint. Identical assertions from either side count as one edge
with distinct source occurrences. Code links are structural associations;
`checks` identifies a check definition and never claims a passing execution.
Incoming cardinality counts distinct canonical code-to-item edges, including
marker and item-authored assertions, at each eligible item. It does not count
multiple occurrences of one edge twice.

## Authored forms

An item may assert the inverse in metadata or a typed inline reference:

```markdown
:implemented_by: code:src/graph_constraints.rs::rust::graph_constraints/evaluate().

Implemented by [[implemented_by:code:src/graph_constraints.rs::rust::graph_constraints/evaluate().]].
```

The target grammar is `code:<project-relative-path>[::<language>::<descriptor>]`.
Paths use `/`, contain only ordinary relative components, and cannot escape the
project through `..` or a symlink. Matching is case-sensitive. A file-only target
resolves to an existing regular file without an indexer or grammar. Symbol paths
must be reported by the configured indexer. Descriptors retain SCIP's punctuation
and disambiguators, omitting scheme and all package metadata. Whitespace and
`%`, `[`, `]`, `<`, `>`, backslash and `|` are percent-escaped using
uppercase UTF-8 byte values. Other characters, including backticks, are preserved
in metadata and typed inline references. For example,
`` code:service.ts::typescript::`service.ts`/parse(). `` represents the descriptor
`` `service.ts`/parse(). ``. These spellings are canonical, not URL aliases.
No authored byte span or line number is part of the identity.

A source comment marker is `@mara <canonical-relation> <item-ID-or-MID>` on
its own comment line. Tree-sitter identifies comment nodes and source spans;
one shared parser searches only their text for markers. It does not scan raw
source as text. The relation must declare `code_source: true` and allow
the target item's flavour. Rust `//`, `///`, and block comments; Python `#`;
and JavaScript/TypeScript `//` and block comments are supported. A marker
in a leading group of query-captured comments attaches to the following named
declaration through its existing modifier/wrapper boundary. All markers in the
group share that owner; ordinary and documentation comments may intervene,
separated only by whitespace, including blank lines. Attachment cannot cross
statements, unrelated declarations, unsupported constructs or lexical body
boundaries. Otherwise a marker inside a declaration body attaches to the deepest
containing named declaration; a top-level marker with no attached declaration
attaches to the file. Ambiguous or unsupported ownership remains an error.
Each marker retains its own original source span. Grouping does not extend
returned declaration content to include leading comments; existing modifier and
wrapper content coverage is unchanged. Comment recognition remains query-driven;
Mara applies the shared attachment rules.

## Indexer and grammar boundary

Mara contains no compiled language integrations. Projects configure one
`[[code.languages]]` entry per integration in `.mara/project.toml` format 3.
Each entry declares source extensions and a SCIP command, with optional
Tree-sitter WebAssembly assets.
Mara owns invocation, standard SCIP protobuf reading, marker parsing, schema/item
resolution, graph identity and navigation.
Each indexer owns language support, project discovery, compilation requirements
and descriptor generation. Adding a language does not require a Mara rebuild.

```toml
format_version = 3

[[code.languages]]
name = "rust"
command = ["rust-analyzer", "scip", ".", "--output", "{output}"]
position_encoding = "utf8"
extensions = ["rs"]

# Optional: declaration content and source comment attachment.
grammar = ".mara/code/rust.wasm"
query = ".mara/code/rust.scm"
```

Each unique snake_case language name is persisted in links; renaming it breaks
those links. `command` is an executable and argument array, with exactly one
standalone `{output}` argument. Mara replaces that argument with a temporary
output path and runs the command from the project root without an implicit shell.
The `command` and nonempty `extensions` are required for every integration.
Extensions are case-sensitive alphanumeric suffixes without dots, unique across
language entries. Configure `grammar` and `query` together, or omit both for
indexing without Tree-sitter. Install the executable and
its language dependencies separately. Mara does not download indexers, compile
grammars, or contain language-specific command defaults.
Only enable commands trusted by the project: they can run compiler/build tooling
and have the permissions and network access of the Mara process.

Mara checks each integration for matching source files whenever it loads code
for a corpus operation, including validation and reads. Matching respects the
project ignore rules and accepts regular files or file symlinks resolving within
the project. With no matches, Mara skips that indexer and treats the language as
empty; documentation operations remain available. Adding the first matching file
automatically activates indexing; removing the last skips it again. This rule
applies to every indexer independently of optional grammar assets. Configuration
and declared grammar assets must remain valid even when no source files match.

Once matching files exist, Mara invokes the configured command; users need no
pre-indexing step. The indexer still owns discovery within its project. Mara
consumes fresh output and keeps no persistent SCIP cache. A valid SCIP index
with metadata and zero documents is accepted. A failed command or missing output
is never interpreted as an empty language. Nonzero exit,
missing/malformed output, a different indexed project root, invalid source ranges,
or changed project inputs during execution produce incomplete-validation errors;
normal operations fail instead of returning a partial graph. Command diagnostics
identify the indexer without exposing its raw stdout/stderr. Input snapshots use
unignored regular project files, excluding Mara mutation bookkeeping and staging
files. Indexer-generated build artifacts should follow project ignore rules.

Document positions follow SCIP's declared encoding. For older indexers that omit
it, configure their documented `position_encoding` (`utf8`, `utf16`, or `utf32`).
Without either declaration, ASCII is unambiguous; non-ASCII source is rejected.
Only global definition occurrences are linkable. SCIP `local N` identities are
unstable and excluded. Two different full symbols that collapse to the same
file/indexer/descriptor cause an error. Multiple declarations of one full symbol
share one endpoint, including TypeScript overload signatures and implementation.
Independently implemented overloads remain distinct when the indexer supplies
different descriptors. Mara does not invent signatures or choose among candidates.
The guarantee relies on the indexer's descriptor stability: upstream identity
reuse cannot be detected without persistent history. Changing indexer semantics can break authored links.

Tree-sitter supplies declaration ranges and comment ownership, never symbol
identity. Its query captures declarations with `@symbol` and `@name`, lexical
containers with `@scope` and `@name`, and comments with `@comment`. Optional
`@modifier` and `@wrapper` captures support attributes, decorators and exports.
The owning declaration, determined by the attachment rules above, must have a
name span matching exactly one global SCIP identity. Otherwise the marker is
unsupported. File-owned markers need no symbol. Invalid packs and
duplicate extension assignments are diagnosed. Asset paths stay in the project.
With no grammar, SCIP symbol links still work, using the indexer's enclosing
range for content, or its definition token if no enclosing range is supplied.
Multiple declaration ranges of one identity are read as their enclosing source
interval.

The repository contains example runtime grammar/query assets built from
tree-sitter-rust 0.24.2, tree-sitter-python 0.25.0,
tree-sitter-javascript 0.25.0 and tree-sitter-typescript 0.23.2, with MIT notices.
They do not supply semantic identities without a configured indexer. Build other
grammars with `tree-sitter build --wasm` and supply a matching capture query.

## Validation and navigation

Missing file or symbol, ambiguous selector, and unsupported file/selector or
comment attachment produce distinct stable error codes at the authored
occurrence. Invalid relation permission and missing/ambiguous item markers
remain relation/reference errors. Validation invokes the configured indexer commands as described above. Moving, renaming, or deleting a symbol re-derives marker-owned
backlinks from current source; an old item-authored selector fails validation
until edited. Equivalent marker and item-authored assertions deduplicate as
one edge, while retaining both locations.

`related` accepts an item reference or `code:` reference and returns direct
code/item neighbours, canonical edges, direction, occurrence count, and
navigable source locations. `get` accepts the returned code reference and
reads UTF-8 file or symbol source in bounded pages; a binary file-only target
still resolves as a relation endpoint but `get` reports that its content is
not readable as text. Relation inspection shows both marker and item-authored
occurrences. CLI and MCP expose the same
results and diagnostics. Item detail summaries expose an item-authored inverse
target as `code_reference`; this is the exact `code:` reference and has no
item MID or item summary. Code files are discovered locally under the project
root using the repository's ignore rules; references to ignored or unsupported
files still resolve as file-only targets when explicitly authored. With an item
as `source` and a declared inverse alias, `relation add` writes the inverse
metadata to that item, rejecting an edge already asserted by either an item
or code marker. `relation remove` removes only item-authored inverse
occurrences of that edge. A remaining code marker keeps the canonical edge
present; mutation results report its remaining occurrence count and
`edge_exists: true`. An item-side remove with no item-authored occurrence fails
without changing code. A code endpoint as mutation `source` is rejected with
an explicit diagnostic: relation commands never modify code source files.
Removing an item-authored inline code relation leaves its `code:` target as
plain text in the item body.
Code markers are added or removed by directly editing their source comments.
:::

:::mara verification VER-CODE-DISCOVERY
:mid: 01M3FZXJ1PRPJPV1CE5YT1W304
:title: Check SCIP identities and code associations
:status: accepted
:method: test
:level: integration
:verifies: REQ-CODE-TRACEABILITY
:verifies: DES-CODE-TRACEABILITY

Run `cargo test --locked --test code_discovery` and the affected CLI/MCP, retrieval, relation, mutation and validation suites in disposable projects with fixture-owned Git/configuration state. Use real supplied Wasm grammars where comment ownership is under test. Preserve provenance of recorded SCIP outputs; configured fixture-copy commands verify the protocol boundary, not language indexing.

Check exact language-scoped descriptors, canonical escaping and literal backticks, package-version and body-edit stability, supported overload distinctions and shared declarations, local-symbol exclusion, collisions, missing/removed/ambiguous identities and valid file-only references. Check declared/fallback UTF-8, UTF-16 and UTF-32 positions and invalid ranges. Check grouped comments, original marker spans, modifier/wrapper content, lexical boundaries, deepest owner and valid file fallback; assert the exact expected symbol endpoint.

Check per-language empty/populated transitions, strict command/output failures, source and asset confinement, ignore behavior, document-filter independence, changed-input rejection and mutation-stage exclusions. Configuration/assets must validate even with no matching source. Normal operations reject partial indexes; validation reports incomplete evaluation. Check source preservation and agreement between marker and item-authored assertions through CLI/MCP get, related, occurrence inspection, mutation and graph policy. Matrix verification also checks these identities when the matrix capability is available.

Run a genuine end-to-end workflow with configured rust-analyzer 1.97.1 against a real Rust project, including exact symbol navigation and source-marker ownership. Run formatting, Clippy and the full relevant regression suite. Record evidence at the actual tested candidate revision; installed authoring checks and recorded fixtures alone do not establish candidate acceptance.
:::

:::mara evidence EVD-CODE-DISCOVERY
:mid: 01M3G02YPBQPFD6G44N5FMVY1K
:title: Code discovery dependency passes real adapter fixtures
:status: retired
:result: passed
:captured_at: 2026-09-26T23:16:08Z
:subject_revision: 44f0f6c74e035dee8b6fa2b535ba2e7aab368ece
:evidences: VER-CODE-DISCOVERY
:evidences: VER-DOCUMENT-PARSING
:evidences: VER-PROJECT-INSPECTION
:evidences: VER-SCHEMA-DEFINITIONS

The tested implementation was committed as `44f0f6c74e035dee8b6fa2b535ba2e7aab368ece`; Git reported a clean tree before this evidence was added. Linux x86_64, Rust 1.97.1, candidate `/tmp/mara72-target/debug/mara`; build settings `CARGO_TARGET_DIR=/tmp/mara72-target CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0`.

Passed `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets --all-features -- -D warnings`, and `cargo test --locked --all-targets`: 82 tests (1 parser unit, 13 code discovery, 31 corpus, 17 bootstrap, 20 schema), none failed or ignored. Seven retained parser groups exercise real Rust/Python/JavaScript/TypeScript Wasm adapters through the loader in isolated projects. Six discovery groups check ordering/ignore/filter independence, bad packs/captures, extension collisions, unreadable-source and marker problems, symlink confinement and walk errors. Source preservation assertions pass. Existing CLI and stdio MCP suites pass as regressions.

Installed full-baseline MCP schema/project validation returned complete and valid with zero diagnostics. Selected intent passed one requirement; realization and verification each passed the requirement and design, consuming every page. These authoring checks use the installed full tool, not candidate graph operations. All dependency versions match the preserved lockfile.

The result covers the independently callable scanner and previously restored dependencies/surfaces. Exact endpoint resolution, semantic code relations, navigation, corpus composition and item listing remain pending; passing coverage matrices do not establish those behaviors.
:::

:::mara design DES-CODE-READ
:mid: 01M3H16WQK7734362HFZET92AT
:title: Resolve local code references for bounded source reading
:status: accepted
:kind: interface
:satisfies: REQ-CODE-TRACEABILITY

Resolve `code:<project-relative-path>[::<language>::<descriptor>]` under [[DES-CODE-TRACEABILITY]]. Require an existing regular file canonically inside the project. File-only references need no integration and may name ignored files; binary bytes are not readable text. Symbol references match the exact indexed identity. Preserve distinct missing-file, missing-symbol, ambiguous and unsupported failures.

Code summaries carry the exact reference, source location and path or language/descriptor title, with no item identity or structural context. Symbol content uses Tree-sitter declaration ranges including captured modifiers/wrappers, or SCIP enclosing ranges with definition-token fallback. Multiple declarations of one identity return their enclosing source interval. Location remains the indexed definition span. Feed UTF-8 content and empty metadata to [[DES-BOUNDED-NODE-READ]]. Explicit file-only content invalidates cursors even outside the discovered language index. Corpus loading invokes configured indexers; Mara does not modify source. Code relation evaluation and mutation remain separate.
:::

:::mara decision ADR-SCIP-CODE-IDENTITY
:mid: 01M3H5VS7R5JY96P7FH2JQBX2E
:title: Delegate symbol identity to pluggable SCIP indexers
:status: accepted
:justifies: DES-CODE-TRACEABILITY

Use project-configured external SCIP indexers for semantic identity and optional runtime Tree-sitter assets for source comment attachment. Language maintainers own project discovery and overload distinctions; Mara keeps integrations outside its binary. Scope descriptors by project-relative file and configured language name, omitting package metadata so version bumps preserve links. Reject collisions and unresolved descriptors without name or position fallback. Renames and moves may break links.

Keep the command, source extensions and optional grammar assets in one language entry because they describe one integration. Extensions permit uniform empty-language detection without interpreting indexer errors. Failure remains explicit when source exists.

Preserve literal SCIP backticks for readable, exact targets. Escape characters that conflict with Mara reference syntax, without shortening or normalizing semantic descriptors and risking collapsed identities.
:::

:::mara evidence EVD-SCIP-CODE-HANDLING
:mid: 01M3HBDTXA12PPZTTJAJFZ3F8Z
:title: SCIP code associations pass candidate and real-indexer workflows
:status: accepted
:result: passed
:captured_at: 2026-09-27T11:53:18Z
:subject_revision: 3071796e99f0da2319496d7f3d4e986b75e13d1b
:evidences: VER-PROJECT-INSPECTION
:evidences: VER-SCHEMA-DEFINITIONS
:evidences: VER-DOCUMENT-PARSING
:evidences: VER-CODE-DISCOVERY
:evidences: VER-ITEM-LIST
:evidences: VER-UNIFIED-SEARCH
:evidences: VER-BOUNDED-NODE-READ
:evidences: VER-DOCUMENT-NAVIGATION
:evidences: VER-RELATION-INSPECTION
:evidences: VER-CORPUS-CONFORMANCE
:evidences: VER-MID-AND-RECOVERY
:evidences: VER-ITEM-CREATION
:evidences: VER-RELATION-MUTATION
:evidences: VER-ITEM-UPDATE
:evidences: VER-ITEM-DELETION
:evidences: VER-ITEM-MOVEMENT
:evidences: VER-ITEM-RENAME
:evidences: VER-PROJECT-VALIDATION
:evidences: VER-POLICY-VALIDATION

At subject revision 3071796e99f0da2319496d7f3d4e986b75e13d1b, `cargo test --locked --all-targets` passed 349 tests across 25 suites with zero failures. Two ignored subprocess helpers were each exercised at three interruption boundaries by passing parent tests. `cargo fmt --all -- --check` and `cargo clippy --locked --all-targets -- -D warnings` passed. The tree was clean after committing the tested source and before adding this evidence.

Fourteen discovery groups and fourteen SCIP integration groups check exact descriptors, escaped punctuation and literal backticks, declared UTF-8/UTF-16/UTF-32 positions, absent non-ASCII encoding, local-symbol exclusion, shared declarations and identity collisions, grouped comment spans/content/lexical boundaries, confinement, empty/populated transitions and strict sanitized command/output failures. Focused regressions first reproduced false-valid results for incomplete owner-name spans and invalid enclosing ranges, then passed after correction. Existing retrieval, relation, mutation, recovery and validation suites pass with explicit isolated SCIP fixtures and unchanged source assertions.

A genuine configured rust-analyzer 1.97.1 workflow, without recorded-index substitution, passed through the candidate CLI and stdio MCP in a disposable Rust package. Its exact run() endpoint remained stable after a body edit and package-version bump. Recorded scip-typescript 0.4.0 and scip-clang 0.4.0 fixtures separately cover shared callable identity, package metadata independence and distinct/deleted overloads; these protocol fixtures do not establish a fresh live TypeScript or C++ indexer run.

The candidate CLI validated the self-hosted repository with valid:true, evaluation_complete:true, zero errors/warnings and no remaining page. REQ-CODE-TRACEABILITY exposed five exact Rust symbol neighbours and DES-CODE-TRACEABILITY exposed twenty-one; none fell back to a file endpoint. Reading code:src/code.rs::rust::code/impl#%5BCodeIndex%5Dload(). returned the actual loader declaration.

Environment: Linux x86_64, Rust 1.97.1, candidate /tmp/mara72-target/debug/mara; CARGO_TARGET_DIR=/tmp/mara72-target, CARGO_PROFILE_DEV_DEBUG=0, CARGO_PROFILE_TEST_DEBUG=0, CARGO_INCREMENTAL=0. The matrix contracts remain draft and matrix runtime is not part of this checkpoint. Installed-tool trace checks are authoring checks, separate from candidate execution; matrix implementation and its code-endpoint rendering verification remain the next increment.
:::
