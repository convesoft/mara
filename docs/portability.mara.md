# Document-engine dependency gate

Build findings for [[ADR-PORTABLE-ENGINE-HOSTS]] and
[[REQ-PORTABLE-DOCUMENT-ENGINE]]. This gate establishes a dependency path before
extraction, not the packaged browser workflow in [[VER-BROWSER-DOCUMENT-ENGINE]].
The current Mara package remains native and still contains host I/O.

## Rule-engine path

Use unmodified crates.io `shacl`, `rudof_rdf` and `rudof_iri` **0.3.24**;
SHACL/RDF default features remain disabled. `shacl` 0.3.21 compiles schema/IR on
`wasm32-unknown-unknown` but excludes its entire validator module. A schema-only
build is not evidence of rule execution. Upstream 0.3.24 removes that exclusion;
no vendored patch, new rule language or custom constraint interpreter is needed.

Keep Mara's existing synchronous `RuleEngine` adapter and nested-error ledger.
Upstream calls this implementation `NativeEngine`; the name does not require a
native host. Do not invoke its Rayon parallel processor in browser WASM.
Definitions, applicability, literal binding and deterministic diagnostics remain
Mara-owned; constraint/path/logical/qualification semantics remain upstream-owned
under [[ADR-NATIVE-SHACL-ADAPTER]]. No SPARQL/network evaluator is enabled.

On WASM, `rudof_rdf::OxigraphInMemory` uses its in-memory RDF graph implementation;
on native it uses Oxigraph. Both parse the same JSON-LD projection and expose
the same RDF/SHACL APIs. The dependency probe must execute both, not infer parity
from shared type names. Native transitive HTTP/Tokio dependencies are not a
browser subprocess or required browser service.

## Complete-profile boundary audit

These are extraction boundaries, not newly delivered operation APIs. Every row
retains its owning source, identity, reference, diagnostic and continuation
contract; no document operation is dropped.

| Profile area and current source | Portable computation/dependencies | Host boundary to remove from the engine |
| --- | --- | --- |
| Initialization/schema discovery/validation (`src/lib.rs`, `src/operations.rs`) | Included template bytes; TOML/YAML parsing; schema declarations/recovery; serde/schemars results | Nearest-project selection, environment, canonical paths, reads, directories, exclusive creation and cleanup. Explicit browser snapshots supply sources; an empty snapshot permits initialization. |
| Markdown/identity validation (`src/corpus.rs`, `src/corpus/markdown.rs`) | rushdown parsing/extensions, byte spans, metadata/reference recovery, ULID parsing | Filesystem inventory/read failures and code discovery must be explicit snapshot/capability inputs, not an assumed empty valid code index. |
| MID creation/backfill (`src/mutation.rs`) | ULID encoding, collision checks, placement/newline-preserving insertion | `Ulid::new()` reads native system time/randomness. Browser host supplies milliseconds and entropy for `Ulid::from_parts`; collision checks stay shared. Publication remains host-owned. |
| Selection/ignore membership (`src/corpus.rs`) | globset include matching; ignore GitignoreBuilder/add_line/matching over supplied ignore text and logical paths | WalkBuilder, parent `.gitignore` discovery, ancestor/symlink checks and reads. Preserve current settings: hidden included; `.ignore`, Git global/exclude disabled; parent Git ignores enabled; no followed symlinks. Snapshot producer supplies effective selection/ignore inputs, including parent context needed for new-path membership. |
| List/search/get/related (`src/query*`, `src/discovery*`) | Schema filters, lookup, ordering, fuzzy/Unicode matching, source ranges, bounded serialization and fingerprints | Project loading and code-source/grammar reads. Retrieval fingerprints must use explicit snapshot bytes instead of reading native assets. |
| All item create/update/delete/move/rename and relation inspection/add/remove (`src/mutation*`, `src/relations.rs`) | Source-span edits, similar text mapping, canonical edge/occurrence identity, surviving-link checks and candidate-corpus conformance | Locks, permissions, destination canonicalization, filesystem membership, preimages, staging, writes, journals/rollback. Preserve whole-input preconditions and atomic browser-memory candidate acceptance under [[DES-SEMANTIC-CHANGE-PLANS]]. |
| Project/item validation and graph policies (`src/operations/validation.rs`, `src/graph_constraints.rs`) | Recovering diagnostics, petgraph cardinality/cycles, stable ordering, severity, summary and output paging | Source loading, code-index activation and byte reads for validation fingerprints. Incomplete prerequisites remain unavailable, including item-targeted requests. |
| Current-state rules (`src/rules*`) | YAML spans, schema-generated JSON-LD, shape/type/depth checks, exact parameter binding, unchanged SHACL adapter | Rule file canonicalization/reads become supplied source inputs. Logical evaluation stays synchronous in a worker, with errors classified unavailable. |
| Request-local trace matrices (`src/trace.rs`) | Same rule outcomes/cache/counts, canonical edges, explanations, Markdown, summaries and bounded continuation | Configuration/rule/excluded-source/code-asset reads in snapshot hashing become explicit input bytes. Parameter values remain part of continuation identity. |

Portable dependency versions are pinned in the probe manifest; transitive
versions are recorded by its generated lockfile. This covers serde/JSON,
schemars, serde-saphyr, TOML, rushdown, ULID, globset/ignore, SHA-256, petgraph,
similar, strsim, Unicode casefold/normalization/segmentation and URL parsing.

Native CLI/MCP/process hosting (`clap`, `rmcp`, Tokio), temporary publication
(`tempfile`) and code indexing (`tree-sitter` with Wasmtime, SCIP/protobuf,
configured subprocesses) stay outside the portable engine dependency graph.
Native-only recovery and code-dependent operations follow
[[REQ-EXPLICIT-BACKEND-CAPABILITIES]]; they are not browser document successes.
Unused code-relation declarations alone do not disable document-only editing.

## Disposable standard-tool probe

`tests/fixtures/document-portability` is test source, not a workspace package or
installed artifact. Copy it outside the checkout and copy the production
adapter/bindings unchanged. The only production code relocation puts the existing
literal-binding functions with their JSON-LD bindings so the probe exercises
that actual code, not a replacement implementation.

From the intended checkout, with Chromium and a matching ChromeDriver installed:

```bash
checkout=$PWD
probe=$(mktemp -d "$HOME/mara-document-probe.XXXXXX")
cp -R tests/fixtures/document-portability/. "$probe/"
cp src/rules/{engine,bindings}.rs "$probe/src/"
cp Cargo.lock "$probe/Cargo.lock"
rustup target add --toolchain 1.97.1 wasm32-unknown-unknown
cargo +1.97.1 install wasm-bindgen-cli --version 0.2.127 --locked \
  --root "$probe/tooling" -j 2
cd "$probe"
# Add only probe/test dependencies while retaining the supplied lock versions.
cargo +1.97.1 test -j 2
cargo +1.97.1 tree --locked --target wasm32-unknown-unknown -e normal
CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER="$probe/tooling/bin/wasm-bindgen-test-runner" \
  CHROMEDRIVER="$(command -v chromedriver)" CHROME="$(command -v chromium)" \
  WASM_BINDGEN_USE_BROWSER=1 \
  cargo +1.97.1 test --locked --target wasm32-unknown-unknown -j 2
cd "$checkout"
```

The browser runner uses a fixture-owned loopback page and real headless Chromium,
not Node. `wasm-bindgen-test` 0.3.77 / wasm-bindgen CLI 0.2.127 match the probe's
WASM binding versions. Keep its source copies, generated lockfile, compiled WASM,
commands, browser versions, revision/source hashes and full results outside the
repository. Use disk-backed disposable storage if `/tmp` is memory/quota-limited.

Require four browser tests and the same four native tests (plus the existing
native adapter regression):

- Nested two-hop qualification, class, forward/inverse paths, property/node,
  and/or/not, string/integer/double/boolean literals, regex, hasValue/in,
  min/max/qualified counts; real failed obligations retain shape/component and
  completed native counts. A false applicability shape is nonconformance, not
  an engine error.
- Exact request-literal bindings through actual binding code into evaluation:
  matching, nonmatching, empty and IRI-looking text; missing binding rejection.
- Deliberately unsupported nested constraints: upstream can return conformance,
  but the unchanged adapter ledger still records an error. These inputs bypass
  definition validation only to test unavailable classification.
- Parsing, schema serialization, text mapping, hashes, include/ignore matching,
  Unicode search dependencies, graph policy primitive, URL parsing and real
  browser clock/entropy-backed ULIDs.

Native full operation suites retain public CLI/MCP coverage for definition
validation, rule diagnostics, request-local matrices and every document operation.
The probe does **not** execute those complete operations in a browser, test Mara's
Markdown extension end to end, package a worker/client, or prove exported source
changes. Those checks remain [[VER-BROWSER-DOCUMENT-ENGINE]]. No host certification,
registry install, remote CI or power-loss guarantee follows from this local gate.
