# Engine

## Requirements

---

<a id="01M7BF6GXHZNABX9R7NKPNGJNM"></a>

### Same inputs, same result on every host

| Type | Requirement |
| --- | --- |
| ID | REQ-SAME-RESULT-ON-EVERY-HOST |
| Status | Accepted |
| Satisfies | [NEED-ONE-BEHAVIOUR-EVERYWHERE](product.mara.md#01M7BF6H7Q0YP8SPVYWYJFB1MF) |

Given the same file contents, every host returns the same result for an operation it supports.

---

<a id="01M7BF6GXSJVVASJ4C8ZGHM909"></a>

### Unavailable, never partial

| Type | Requirement |
| --- | --- |
| ID | REQ-UNAVAILABLE-NOT-PARTIAL |
| Status | Accepted |
| Satisfies | [NEED-ONE-BEHAVIOUR-EVERYWHERE](product.mara.md#01M7BF6H7Q0YP8SPVYWYJFB1MF) |

A host that cannot provide a capability, such as grammar-level code links in the browser, reports it as unavailable. It never returns partial results as complete.

---

## Design

---

<a id="01M7BF6GY13YX9PD76M0TQG8YK"></a>

### Engine boundary

| Type | Design |
| --- | --- |
| ID | DES-ENGINE-BOUNDARY |
| Status | Accepted |
| Satisfies | [REQ-SAME-RESULT-ON-EVERY-HOST](#01M7BF6GXHZNABX9R7NKPNGJNM) |

- The engine is a library over file contents, the `mara-engine` crate: no filesystem, processes, threads, clocks or network. It compiles to WASM for `wasm32-unknown-unknown`.
- Hosts (CLI, daemon, Node library, browser worker) supply contents and own I/O, watching, caching and process management. They also supply the time and the random bytes of each new MID.

---

<a id="01M7BF6GYCNTZQ0VXCMGJYPZHY"></a>

### Inputs and cache keys

| Type | Design |
| --- | --- |
| ID | DES-INPUTS-AND-KEYS |
| Status | Accepted |
| Satisfies | [REQ-WORK-FOLLOWS-CHANGE](performance.mara.md#01M7BF6H3A6YQ43351V9YSV3SQ) |

- Inputs are only the files the configuration declares ([DES-PROJECT-CONFIGURATION](project.mara.md#01M7BF6H9RDYSET9QTAKJWTX27)): documents, schema, the configuration itself, programs, code files and grammar assets. `.git` and undeclared files are never read or hashed.
- A document or code file that a `.gitignore` file in the project tree ignores is never an input. Those `.gitignore` files are inputs themselves; Git's global excludes and `.git/info/exclude` are never read.
- A file is re-hashed (BLAKE3) only when its size, mtime, ctime or inode changes. A file modified within 2 s of the check is re-hashed anyway, so a same-tick edit is detected.
- Each request also stats the directories that can hold inputs and lists a directory again only when its mtime, ctime or inode changed, or when it was modified in the last 2 s. That is how added, removed and renamed files are found without walking the tree.
- Derived values are keyed by content:

| Value | Key |
| --- | --- |
| Parsed document | Parser version + file hash. The parse depends on neither the path nor the schema, which are applied later, so a schema edit does not re-parse documents |
| Code markers of one file | Scanner version + language configuration hash + grammar hash + file hash |
| Document snapshot | Sorted (path, parsed-document key) pairs + schema hash + configuration hash |
| Code layer | Sorted (path, code-marker key) pairs |
| Validation, search and query results | Keys of the layers actually read + request or definition hash |

- Every key includes the version of its derivation logic.
- A result's key covers only the inputs the result reads.
- Identities are computed once per request from per-file hashes: never by hashing serialized or `Debug` dumps of whole structures, and never inside a per-item or per-edge helper.
- A result with diagnostics is cached like any other.

---

<a id="01M7BF6GYM8NPPGY9RZRF38EGE"></a>

### Snapshot

| Type | Design |
| --- | --- |
| ID | DES-SNAPSHOT |
| Status | Accepted |
| Satisfies | [REQ-WORK-FOLLOWS-CHANGE](performance.mara.md#01M7BF6H3A6YQ43351V9YSV3SQ) |
| Satisfies | [REQ-NEAR-LINEAR-SCALING](performance.mara.md#01M7BF6H3K1M4HMG6CW2B85VQK) |

- One immutable snapshot per input generation holds items, ID and MID maps, forward and inverse edges, per-document item spans and search tokens.
- It is assembled from per-file parse results. A one-file change re-parses that file, then rebuilds the indexes in one linear pass.
- One Markdown pass per file (`pulldown-cmark`) yields items, metadata and links. The region being parsed owns everything found in it, so nothing scans for containment.
- No O(n) work runs inside a per-item loop; lookups are by hash.
- Search uses a token index built with the snapshot. Text is normalized once per file change, never per request.
- Snapshots are shared by `Arc`; readers never copy them.
