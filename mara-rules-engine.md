# Mara: Rules Engine — Design Handoff

## Decision

- Project logic (invariants, named queries, gates, views) is written in Soufflé Datalog.
- Mara integrates Soufflé through its command-line contract, never its C++ interfaces:
  - Native: a bundled `souffle` binary runs as a child process.
  - Browser: Soufflé's Emscripten build runs in the worker, on an in-memory filesystem.
- Nemo is the fallback if Soufflé becomes unworkable.

Reasons, from the comparison and the trial below:

- Soufflé declares typed input and output relations in the program itself and rejects mistakes before evaluation. Nemo is dynamically typed and silently skips mismatching rows.
- Soufflé has native modules (`.include`, `.once`, `.comp`).
- In-process use does not fit project-authored rules:
  - Soufflé's public C++ API, [`souffle/SouffleInterface.h`](https://souffle-lang.github.io/interface), inserts tuples, calls `run()`, and iterates outputs in memory, but only for programs translated to C++ with `souffle -g` and then compiled. The interpreter's `ProgInterface` leaves `run()` and `loadAll()` empty (`src/interpreter/ProgInterface.h:258`). [SWIG](https://souffle-lang.github.io/swig) wraps the same compiled programs, for Java and Python only.
  - Projects write their own rules, so every rule change would need a C++ compiler and Soufflé's headers on the user's machine, and this cannot work in the browser. The trial gate program became 3,261 lines of C++: `g++ -O2` took 10.3 s, `clang++ -O2` 6.4 s, `g++ -O0` 4.6 s.
  - Setting `SOUFFLE_ALLOW_SIGNALS` stops Soufflé from replacing the process's signal handlers (`src/include/souffle/SignalHandler.h:96`), but two fatal paths remain: integer division by zero raises SIGFPE, which kills the process; and any C++ exception during evaluation reaches `SignalHandler::error()`, which calls `exit(1)` (`SignalHandler.h:148–154`). The generated code wraps rules in it. A child process contains both.
- Rejected alternatives: GraphQL needs custom resolvers or Mara-specific directives for computed fields; SPARQL has no modules or typed input/output declarations, so it needed a JSON wrapper plus separate SHACL constraints; Rego (`regorus`) forbids recursive rules.

## Two layers

1. **Built-in schema checks, in Rust.** Required fields, allowed values, relation endpoints per source type, same-type links, acyclic relations, and link, anchor, and MID integrity. Always on and linear. Per-source endpoints absorb the old `requirement-endpoints` and `design-endpoints` rules.
2. **Project logic, in Soufflé programs.** Conditional obligations (for example, an accepted Verification verifies an accepted Requirement), named queries, gates, and computed values such as risk scores.

## Consumers

One program's output relations serve several consumers; presentation never defines different facts.

| Consumer | Registered by | Effect |
| --- | --- | --- |
| Raw query | Request | Returns the output rows |
| View | Request | Renders the same rows |
| Invariant | Project configuration | Evaluated by validation; error-severity failures make documentation invalid |
| Gate | Explicit request for a scope and context | Fails the gate; never changes documentation validity |
| Report | Request | Explains an invariant or gate outcome |

- An empty result has no pass/fail meaning on its own; the consumer defines it.
- Gates report conditions, not permission to release, deploy, or act.

## Fact bridge

Mara's stable contract is a versioned declarations file that Mara ships and programs include (`.include "mara.dl"`; the file starts with `.once`). Trial version:

```souffle
.type Mid <: symbol
.type Id <: symbol
.type Type <: symbol
.type Status <: symbol
.type Rel <: symbol
.type Path <: symbol

.decl item(mid: Mid, type: Type, id: Id, status: Status, path: Path, line: number)
.decl field(mid: Mid, key: symbol, value: symbol)
.decl edge(rel: Rel, src: Mid, dst: Mid, path: Path, line: number)
.decl verification_result(ver: Mid, result: symbol, revision: symbol)
.decl source_complete(source: symbol)
```

- Each relation is `.input`. Mara writes one TSV file per relation, once per document snapshot.
- Distinct subtypes (`Mid`, `Id`, `Path`) make Soufflé reject rules that mix identities.
- `path` and `line` on `item` and `edge` let explanations point at document locations.
- Externally supplied records come with explicit completeness markers. "Missing" requires a complete source; otherwise the result is "unavailable". Mara evaluates records as supplied: it never runs tests or vouches for their truth.
- `edge` rows carry the occurrence location, so a relation authored twice yields two rows. Programs counting distinct relationships project away `path` and `line`.
- `mara.dl` starts at format version 1. Internal iterations never bump it; only released contract changes do.
- Gate parameters are one-row input relations (for example, `threshold(value: number)`).
- Code links from comment markers are added as a further input relation when code scanning lands.

## Execution

- Each evaluation is keyed by document snapshot key, program hash, and parameter set, so `souffle` runs only when documents or rules change.
- Native invocation: `souffle --no-preprocessor -F <facts> -D <out> <program>.dl`, single-threaded. In the trial `-j8` was slower than `-j1`, so the binary is built without OpenMP.
- Browser invocation: compile `souffle.wasm` once per worker, create a fresh instance per evaluation, write programs and facts into its in-memory filesystem, and read the outputs back. Build with `-sEXIT_RUNTIME=1` or export `callMain`, so the host receives `main`'s exit code.
- Mara reads only the output relations a consumer names. An `.output` in an included file is also written, so it must not be treated as a result.
- Explanations run on demand in provenance mode (`-t explain`), driven by commands on stdin: `format json`, then `explain <relation>(<args>)`. Leaves are input facts carrying `path` and `line`. Use JSON; the text proof format garbles negated atoms.
- Compile errors are reported with file, line, and caret and passed through as diagnostics. A runtime fault (integer division by zero) ends the child process with the rule's location natively, or with a `divide by zero` trap and no location in WASM.

## Trial results (2026-11-13)

Soufflé 2.5. Corpora: 1× = 200 items / 266 edges, 16× = 3,200 / 4,256, 64× = 12,800 / 17,024. Times include process start and program compilation.

| Check | Result |
| --- | --- |
| Risk score with a threshold parameter; missing inputs reported as "unavailable" | Pass |
| Coverage: at least one accepted verification | Pass |
| Ancestry across a cycle; cycle members found | Pass |
| Missing vs unavailable external results, depending on completeness | Pass |
| JSON explanation of a failed gate condition, with document locations | Pass |
| `implementation_ready` gate (5 conditions plus an unavailable outcome) | Pass |
| Compile errors: undefined relation, arity, `Mid` vs `Id`, symbol vs number, unstratifiable negation | All caught, with file, line, and caret |
| Native gate time: empty / 1× / 16× / 64× | 6.1 / 6.0 / 11.3 / 29.5 ms; 18 MB peak memory at 64× |
| Native growth from 16× to 64× (4× data) | Time ×2.6–2.8, instructions ×2.9–3.5 |
| Provenance mode overhead at 16× | 1.5× (21.0 vs 14.2 ms) |
| WASM gate time: empty / 1× / 16× / 64× | 11.9 / 13.4 / 21.7 / 44.8 ms |
| Native vs WASM outputs, all programs at 16× | Identical (6,696 rows) |
| Portable native binary (static C++ runtime, no OpenMP) | 15 MB stripped, 2.6 MB xz; depends only on libc and libm |
| WASM module | 6.1 MB, 1.2 MB gzip, 0.7 MB Brotli |
| Build time, 10 jobs | Native 107–120 s; Emscripten 66 s |

## Distribution

- Pin a Soufflé release and build it in CI for each platform. Ship the native binary with Mara and the WASM module with the browser package.
- Soufflé 2.5 needs a one-line patch to build with Emscripten's Clang: `src/interpreter/Index.h:423`, `data = src.data;` becomes `data = src.data.load();`. Upstream `master` no longer has that line.
- Licence: UPL-1.0 (permissive).

## Open items

- macOS and Windows builds were not tried. Soufflé's own CI builds with MSVC, but its explain console throws on Windows (`No explain/explore provenance on Windows`), so explanations are unavailable there unless patched.
- Editor support for `.dl` programs is community-only and inactive. Soufflé's documented [IDE/editor setup](https://souffle-lang.github.io/build#ideeditor) (clangd through `CMAKE_EXPORT_COMPILE_COMMANDS`) is for developing Soufflé's own C++ source. VS Code has `W4RH4WK.souffle-syntax` (highlighting, last release 2018) and `i-daridis.souffle-lang-server` (language server, last release Sep 2023). A `tree-sitter-souffle` grammar exists. `mara` can offer a compile-only check (a run against empty facts costs 2–6 ms).
- Program location, consumer registration (which outputs are invariants, gates, or views), and the diagnostic format for invariant violations are still to be designed.
- Admitted program profile: records, ADTs, and `autoinc()` can make evaluation unbounded. Decide whether to forbid them or bound evaluation time.
- Trial gate semantics approximate the previous template's gates. Port the real template logic when the engineering template is rewritten.

Trial artifacts (Soufflé source and builds, programs, fact generator, WASM runner, benchmarks): `~/Work/local/workspace/scratch/souffle-spike/`.
