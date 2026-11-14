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
- The `ignore` crate matches `.gitignore` patterns. Known limitation: unlike Git, it reads `{a,b}` as alternatives, reads `[[:digit:]]` as a set followed by `]` instead of a POSIX class, trims trailing tabs, and keeps a NUL byte in a pattern instead of ending the pattern there.
- The inventory lists the project root and each directory below it in which an include pattern can still match a path, except an entry named `.git` at any depth, a directory that a `.gitignore` file ignores, another project's root ([DES-PROJECT-CONFIGURATION](project.mara.md#01M7BF6H9RDYSET9QTAKJWTX27)) and a symbolic link. The `.gitignore` files of the listed directories are its inputs. A document is a listed `*.mara.md` file that a pattern matches and no `.gitignore` file ignores.
- The inventory follows no symbolic link, not even to stat its target. A link that a pattern matches as a document is `symbolic-link` at the link's path, line 1, and is not read. Any other link is skipped like an undeclared file, so nothing below a linked directory is an input. A directory whose `.mara` is a link is `symbolic-link` at that `.mara`, and nothing below it is an input.
- A directory that cannot be listed or examined for a `.mara`, an input that cannot be read or is not a regular file, and an entry whose name is not UTF-8 where an input could be are `io-error` at that path, line 1. Nothing below such a directory is an input; the rest of the inventory goes on.
- A `.gitignore` that cannot be applied is an error at its path, and nothing in its directory or below is an input: any entry there might be one it ignores. That is one that cannot be read, is not a regular file or is a symbolic link (`symbolic-link`), at line 1; or one the `ignore` crate cannot take (`unsupported-gitignore`, with the crate's reason), at the line that is not UTF-8 or whose pattern it refuses, or at line 1 when it cannot compile the file's patterns together. It is never opened unless it is a regular file, and never applied in part.
- A file is re-read and re-hashed (BLAKE3, computed by the engine, so every host has the same keys) only when its size, mtime, ctime or inode changes, or when the check that last hashed it came less than 2 s after its mtime or ctime. That second case finds an edit made in the same timestamp tick as that check, however late the next check comes. Contents that hash as before keep what was derived from them.
- A file with several roles in one request, such as a schema that a pattern also matches as a document, is read and hashed once in it.
- Each request also stats the directories that can hold inputs and lists a directory again only when its mtime, ctime or inode changed, or when the check that last listed it came less than 2 s after its mtime or ctime. That is how added, removed and renamed files are found without walking the tree.
- Derived values are keyed by content:

| Value | Key |
| --- | --- |
| Parsed document | Parser version + file hash. The parse depends on neither the path nor the schema, which are applied later, so a schema edit does not re-parse documents |
| Code markers of one file | Scanner version + language configuration hash + grammar hash + file hash |
| Document snapshot | Sorted (path, parsed-document key) pairs + schema hash + configuration hash |
| Code layer | Sorted (path, code-marker key) pairs |
| Validation, search and query results | Keys of the layers actually read + request or definition hash |

- Every key includes the version of its derivation logic. The parser's version covers the search words it derives.
- Every path whose contents have the same parsed-document key shares one parse, so a moved or copied document is not parsed again. Paths, diagnostics and spans stay per file.
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
- One Markdown pass per file (`pulldown-cmark`) yields items, metadata, links and search words. The region being parsed owns everything found in it, so nothing scans for containment.
- No O(n) work runs inside a per-item loop; lookups are by hash.
- An edge is a metadata row keyed by a relation that the item's type is a source of ([DES-SCHEMA-MODEL](project.mara.md#01M7BF6HA0KG3R3AATWM1P2X1G)) whose value is a link to a MID, or a mention ([DES-RELATION-LINKS](identity.mara.md#01M7BF6H29JGHG65SV91K6D11V)), in an item or in the narrative. A row keyed by a relation that the item's type is not a source of, or by a field, is no edge. Inverse edges are found by the target MID, so a missing or repeated target costs nothing extra.
- The MID map and the ID map, which holds an item under each of its `ID` values, keep every item of a repeated key, each once.
- Search uses a token index built with the snapshot. Text is normalized once per file change, never per request: the words are the maximal runs of letters and digits, lowercased, of an item's MID and `ID` values, title, other metadata values and body text, each as GitHub shows it: a link's or image's text without its destination, an autolink's text being its address, and no HTML. A block boundary, a table cell boundary, a line break and an HTML tag end a word; emphasis, links and code spans do not. Each word records which of these four zones holds it.
- Snapshots are shared by `Arc`; readers never copy them.
