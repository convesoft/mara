# Mara: Architecture and Performance — Design Handoff

## Goal

Keep the rewrite fast as projects grow. Each rule below prevents a failure measured in the performance audit of the previous implementation (`~/Work/local/workspace/projects/mara-performance-audit/`: report, flamegraphs, and tools). The rules engine is designed separately; its constraints from the audit are listed below.

## Execution modes

All modes are supported:

| Mode | Host | Input access |
| --- | --- | --- |
| CLI, direct | In-process | Filesystem |
| Daemon | One per OS user; serves CLI, MCP, and Node over native IPC, browsers over loopback HTTP | Filesystem |
| Node library | Embedded or through the daemon | Filesystem |
| Browser through daemon | Loopback HTTP with session token, allowed origins, and allowed projects | Daemon's filesystem |
| Browser, local | Engine compiled to WASM in a worker | Supplied file contents; edits exported as a change set |

## Engine boundary

- The core engine is a library over file contents: no filesystem, processes, threads, clocks, or network. It compiles to WASM.
- Hosts (CLI, daemon, Node, browser worker) supply contents and own I/O, watching, caching, and process management.
- A host that cannot provide a capability (for example grammar-based code ownership in the browser) reports it as unavailable; it never returns partial results as complete.

## Inputs and keys

- Inputs are only the declared files: documents matched by include patterns, schema, project config, code files per language config, and grammar assets. `.git` and undeclared files are never read or hashed.
- A file is re-hashed (BLAKE3) only when its size, mtime, ctime, or inode changes. Files modified within 2 s of the check are re-hashed anyway, because timestamp resolution can hide a same-tick edit.
- Derived values are keyed by content, never by path or time:

| Value | Key |
| --- | --- |
| Parsed document | Parser version + file hash. Schema is applied later, so a schema edit does not re-parse documents. |
| Code markers for one file | Scanner version + language config hash + grammar hash + file hash |
| Document snapshot | Sorted (path, parsed-document key) pairs + schema hash + config hash |
| Code layer | Sorted (path, code-marker key) pairs |
| Validation, search, query results | Keys of the layers actually read + request or definition hash |

- A result's key covers only the inputs it reads: a code change never invalidates document-only results.
- Every key includes the version of its derivation logic, so an upgraded Mara never reuses stale values.
- Identities are computed once per request from per-file hashes. Never hash serialized or `Debug` dumps of whole structures, and never compute an identity inside a per-item or per-edge helper.
- Results with diagnostics are cached like any other. A failed request or an unknown ID never invalidates anything.
- Paging cursors carry the snapshot key, so staleness is detected exactly.

## Snapshot

- One immutable snapshot per input generation holds items, ID and MID maps, forward and inverse edges, per-document item spans, and search tokens.
- It is assembled from per-file parse results. A one-file change re-parses that file, then rebuilds the indexes in one linear pass.
- One Markdown pass per file (`pulldown-cmark`) yields items, metadata, and links. The region being parsed owns everything found in it, so no containment scans are needed.
- No O(n) work inside per-item loops; lookups are by hash.
- Search uses a token index built with the snapshot. Text is normalized once per file change, never per request.
- Snapshots are shared by `Arc`; readers never copy them.

## Paging

- Each (snapshot, normalized request) result is computed once into a bounded result cache; pages are slices of it.
- Byte budgets use per-record sizes computed once, never by re-serializing the page after each record.
- Internal consumers call evaluate-all APIs, never the public paged API in a loop.

## Rules engine constraints

The engine is Soufflé, run as a child process natively and as WASM in the browser; see [mara-rules-engine.md](mara-rules-engine.md). Measured failures of the previous rules and query engine, to avoid in the new design:

- One engine was built per item × rule, so no outcome was shared across items. Evaluate each rule once over the snapshot, or memoize outcomes across items.
- A per-outcome helper iterated all shapes × all graph nodes: 85–95% of validation CPU, growing ×3.9 per corpus doubling.
- Node IRIs were formatted and parsed inside inner loops (~46% of validation CPU in IRI parsing). Build each node identity once.
- Query definitions were recompiled on every execution, and queries ran per root with the optimizer disabled and ordering on every variable. Compile once per definition hash and evaluate in one pass.
- A per-edge helper recomputed a whole-input fingerprint and scanned every relation. Compute identities once per request.

## Daemon

### Lifecycle

- Started on demand. A CLI, MCP, or Node client that finds no daemon starts one in the background and runs its current request in-process, so the first call is never slower than direct mode and never depends on daemon startup. Later calls use the daemon.
- A lock in the daemon's runtime directory (`$XDG_RUNTIME_DIR/mara/`, mode 0700) ensures a single instance when several clients start it at once. The socket path stays under the 108-byte Unix limit.
- Clients fall back to direct mode only before dispatching a request; a dispatched request is never replayed on another backend.
- A client of a different Mara version asks the running daemon to finish in-flight work and exit, then starts its own.
- The daemon exits after an idle period with no requests and no connected clients. Restarting is cheap because documents and code markers rebuild in milliseconds to seconds, not the old indexer's tens of seconds.
- No automatic start in CI (`CI` set) or when disabled (`MARA_NO_DAEMON=1` or a direct-mode flag), so one-shot environments leave no background processes.
- `mara daemon status | start | stop` remain available for explicit control.

### Behaviour

- One per OS user, with one session per canonical project root. Git worktrees are separate projects.
- Watch mode is on by default. The daemon watches only directories containing declared inputs, honoring ignore rules (inotify watch limits are per user). It debounces bursts such as a branch checkout and re-derives in the background, so the next request is already warm. A watcher overflow or error triggers a full rescan.
- Every request still runs the stat check, because watchers can miss events. A detected change waits for the rebuild; stale state is never returned as current.
- Readers take the current snapshot without locks. Writers queue per project; there is no global lock.
- Writes:
  1. Check that target files still have the hashes from the snapshot the edit was computed against.
  2. Write a temporary file and rename it into place.
  3. Re-derive only the changed files.
- Long work runs on worker threads with cancellation. Protocol handlers never block each other.
- Caches live in memory. An on-disk store is added only when a measured cost requires it.

## Code scanning

- No SCIP.
- Code-to-item edges come from comment markers: `@mara <relation> <ID>`. Code endpoints are recomputed on every scan and never written into documents.
- Basic level, every language: comment prefixes configured per extension; the endpoint is file + line.
- Grammar level: tree-sitter WASM grammars. Declarations come from the grammar's upstream `queries/tags.scm`; comments are recognized by node kind (`comment`, `line_comment`, `block_comment`). The endpoint is file + owning declaration.
- `mara code add <language>` fetches a grammar's `.wasm` and `tags.scm` from a built-in catalog (repository, release, extensions, comment syntax) or a given repository, verifies a checksum, stores it in a per-user cache, and pins version and checksum in project config. Official tree-sitter grammar releases publish both files.
- Normal operations never download. A missing grammar falls back to the basic level with a diagnostic. Grammar ABI compatibility is checked on load.
- Code is scanned per file and keyed by file content. Document operations never scan code.

## Performance test suite

Exists from the first commit. Every new operation ships with its performance tests.

### Corpus generator

- Deterministic and seeded, written in the new format.
- Produces items across documents with realistic relation counts, body sizes, and code files carrying markers.
- Sizes: 1× (~200 items), 4×, 16× on every PR; 64× nightly.
- Generated into disposable temporary directories, never into the checkout.

### Work counters (deterministic, every PR)

Engine and hosts count file reads, hashes, parses, snapshot builds, code scans, subprocess spawns, and rule evaluations. A debug stats option exposes them, so tests check the real CLI and daemon. Tests assert exact expectations:

| Situation | Expectation |
| --- | --- |
| Unchanged warm request | 0 hashes, 0 parses, 0 snapshot builds |
| One document edited | 1 parse, 1 snapshot build, 0 code scans |
| One code file edited | 1 code scan, 0 document parses |
| Any document operation | 0 subprocesses, 0 code scans |
| Validation or gate, unchanged snapshot and rules | 0 `souffle` runs |
| Validation or gate after a change | At most 1 `souffle` run per program and parameter set |
| Unrelated file, `.git` change, failed request, idle period | No invalidation |
| Validation | Rule evaluations ≤ c × (items + edges) |

### Timing (release builds)

- Scaling, every PR: time each operation at 1×, 4×, and 16× (median of runs). Fail when growth exceeds 2.3× per doubling.
- Budgets, nightly on a reference machine. Starting targets at 16×, to adjust after first measurements: cold CLI read ≤ 200 ms, warm daemon read ≤ 20 ms, validation ≤ 1 s.
- Concurrency: a read issued during a long validation of the same project completes within its warm budget.
- Modes covered: CLI direct, daemon cold, daemon warm, daemon after a watched edit; browser-local WASM once it exists.
- Standard tools only: ordinary Rust integration tests for counters and scaling, `hyperfine` for end-to-end CLI timings.
- Profiling: `samply` for CPU profiles (Firefox Profiler UI), `perf` + `inferno` for flamegraphs, `valgrind --tool=callgrind` for exact instruction counts, `heaptrack` for allocations. Profile a release build with line tables (`[profile.profiling]` inheriting `release` with `debug = "line-tables-only"`). Unprivileged profiling needs `kernel.perf_event_paranoid` ≤ 1, and samply on many-core machines needs `kernel.perf_event_mlock_kb` ≥ 2048.
