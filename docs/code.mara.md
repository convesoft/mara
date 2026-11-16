# Code links

## Requirements

---

<a id="01M7BF6GPD85NN3TCFJE7RCKWB"></a>

### Link code to items with comment markers

| Type | Requirement |
| --- | --- |
| ID | REQ-CODE-MARKER-LINKS |
| Status | Accepted |
| Satisfies | [NEED-CODE-LINKS](product.mara.md#01M7BF6H7D3K8R07DTSRVVGX0G) |

A comment `@mara <relation> <ID>` in a code file creates an edge from that code location to the item, and `relation list` shows it from both ends. Code endpoints are recomputed on every scan and never written into documents.

---

<a id="01M7BF6GPR0B36WAK7ESQH41WM"></a>

### Only `mara code add` downloads

| Type | Requirement |
| --- | --- |
| ID | REQ-ONLY-CODE-ADD-DOWNLOADS |
| Status | Accepted |
| Satisfies | [NEED-CODE-LINKS](product.mara.md#01M7BF6H7D3K8R07DTSRVVGX0G) |

No operation other than `mara code add` connects to another machine. With a grammar missing, scanning falls back to the basic level and reports a diagnostic.

---

## Design

---

<a id="01M7BF6GQ1G370S0G7HPN4HGY1"></a>

### Marker levels

| Type | Design |
| --- | --- |
| ID | DES-CODE-MARKERS |
| Status | Accepted |
| Satisfies | [REQ-CODE-MARKER-LINKS](#01M7BF6GPD85NN3TCFJE7RCKWB) |

| Level | Needs | Endpoint |
| --- | --- | --- |
| Basic, every language | Comment prefixes and include patterns configured per language | File and line |
| Grammar | A tree-sitter WASM grammar and its upstream `queries/tags.scm` | File and owning declaration |

- `[code]` in the configuration declares the code languages: its own `format_version` (1), the version of the marker syntax, and per `[code.languages.<name>]` the `include` patterns of its files ([DES-PROJECT-CONFIGURATION](project.mara.md#01M7BF6H9RDYSET9QTAKJWTX27)) and its `comment_prefixes`, at least one, none empty or with a space, tab or line break. `[code]` is optional: without it, or without a language, the project declares no code language. A language with an invalid name, pattern list or prefix list is left out.

```toml
[code]
format_version = 1

[code.languages.rust]
include = ["src/**/*.rs"]
comment_prefixes = ["//", "///", "*"]
```

- A file that patterns of two languages match belongs to the first language in the configuration. A document, or a temporary file of a write ([DES-WRITE-PROCEDURE](writes.mara.md#01M7BF6HJA1E3SE8WJH77FGP17)), is never a code file.
- A marker is a comment whose text is the marker, on its own line or after code on the same line, as in 0.3.0: the comment start, then `@mara`, directly or after spaces or tabs, then a relation's forward name and an ID, each after spaces or tabs, and nothing after the ID but spaces, tabs and a carriage return. `@mara` later in a comment's text is no marker.
- At basic level a comment starts at the first of the language's comment prefixes on a line that stands at the line's start or after a space or tab, so `"// @mara …"` holds none. Where several prefixes start there, the longest one is the comment start, so `/// @mara …` is a marker when `///` is a prefix. A prefix that is also an operator, such as `*`, starts a comment after a space too, so `a * b; // @mara …` holds no marker. A prefix after a space inside a string literal is read as a comment start; only the grammar level tells strings apart.
- A comment start followed by `@mara` and a space, a tab or the line's end, but not by one forward name that the schema declares and one word of the form of an ID ([DES-ITEM-IDS](identity.mara.md#01M7BF6H1ZBTKWTF9ZXEJ8EBGS)), is `invalid-marker`. A marker whose ID no item has is `unresolved-marker`, and one whose relation does not lead from `code` to the target's type is `disallowed-target` ([DES-SCHEMA-CHECKS](validation.mara.md#01M7BF6HG7AQZ9FM38CMT4BTHR)). Each is an error at the marker's path and line, without an item's ID or MID. A `disallowed-target` marker still links, so programs see it too.
- Code files are read as bytes, in lines that end at a line feed. A marker is ASCII, so the rest of a file need not be UTF-8.
- At grammar level, declarations come from `tags.scm`, named by their `@name` capture, and comments are recognized by node kind (`comment`, `line_comment`, `block_comment`). A comment starts where its node starts, or on a later line of a block comment at a prefix that is the line's first text; text in a string is never a comment there, so a marker-like string is a marker at basic level only.
- The owning declaration, as in 0.3.0, is the one that the marker's comment precedes with only comments, attributes and blank space between, else the innermost one that contains the marker, else the file. A marker after code on its line belongs to that code: the innermost declaration that contains the start of its line, else the file.
- A marker names one relation and one ID, never a MID. Several items take several markers. Each marker is one code link; one whose ID several items have links to each of them.
- A code location is addressed as `<path>:<line>`, like a narrative unit ([DES-NARRATIVE-UNITS](retrieval.mara.md#01M7CMW710P9V73WXFQ15PBYBS)). Its line is the marker's line at basic level, and at grammar level the owning declaration's first line, or 1 for the file. `relation list` adds the declaration name at grammar level. A code link's source is the marker's path and line.
- `get` and `relation list` of `<path>:<line>` in a code file name the location that holds that line: at basic level that line, at grammar level the declaration that owns it. `<path>` names the whole file.
- `get` of a code location returns the owning declaration, from its first line to its last, at grammar level, and the file from that line at basic level.

---

<a id="01M7CQ57TTB40GWX8YHKB259DK"></a>

### Operations that read code

| Type | Design |
| --- | --- |
| ID | DES-CODE-READERS |
| Status | Accepted |
| Satisfies | [REQ-CODE-MARKER-LINKS](#01M7BF6GPD85NN3TCFJE7RCKWB) |
| Satisfies | [REQ-WORK-FOLLOWS-CHANGE](performance.mara.md#01M7BF6H3A6YQ43351V9YSV3SQ) |

An operation reads code only when its result holds something from code.

| Operation | Reads code |
| --- | --- |
| `relation list` | Yes: it returns code links |
| `item rename`, `item delete` | Yes: markers name the ID |
| `project validate` | Yes: it reports a marker whose ID does not resolve or whose relation the schema does not allow for code, and gives code links to invariants |
| `query` operations | Only when the program uses code links |
| `get` of a code location | That file |
| Every other operation, `search` included | No |

- Reading code rescans only the code files that changed ([DES-INPUTS-AND-KEYS](engine.mara.md#01M7BF6GYCNTZQ0VXCMGJYPZHY)). `get` of a code location lists the code files and reads only that one.
- A project that configures no code language never reads code.
- A program uses code links when it, or a file it includes, names the code-link relation of [DES-FACT-BRIDGE](rules-facts.mara.md#01M7BF6HEHHJ7XZM8HDFXK115J).
- All code is read, and Mara adds `source_complete("code")` ([DES-REQUEST-INPUTS](rules.mara.md#01M7CT60KMGA2M9WMQ8RRJREPG)), when the request read and scanned every code file that the configuration declares, at either level. A project that configures no code language has read all of its code. A code file that is an `io-error`, or a directory or `.gitignore` that cannot be read or applied where a code file could be, leaves code incomplete. `item rename` and `item delete` fail with `io-error` at such a place before they write anything, since a marker there could name the ID. `item delete` counts every marker that names one of the item's IDs, its relation declared or not and its target allowed or not.
- `item rename` rewrites each marker that names the old ID, its relation declared or not, by replacing only the bytes of that ID, at the offsets the scan found. Before the write procedure ([DES-WRITE-PROCEDURE](writes.mara.md#01M7BF6HJA1E3SE8WJH77FGP17)), it checks that those bytes are the old ID and that the new ID has the form of an ID, so each line stays a marker of its relation, naming the new ID; a failed check is `unreadable-edit` and nothing is written. A code file is not scanned again: unlike a document, it is not read back.

---

<a id="01M7BF6GQC7MW6ZSA9NMEQRQHM"></a>

### Grammar assets

| Type | Design |
| --- | --- |
| ID | DES-GRAMMAR-ASSETS |
| Status | Accepted |
| Satisfies | [REQ-ONLY-CODE-ADD-DOWNLOADS](#01M7BF6GPR0B36WAK7ESQH41WM) |

- `mara code add <language>` fetches the grammar's `.wasm` and `tags.scm` from a built-in catalog (repository, release, extensions, comment syntax) or from a given repository. Official tree-sitter grammar releases publish both files.
- It verifies a checksum, stores the files in a per-user cache, and pins version and checksum in the project configuration.
- A catalog entry carries the checksum of its release's files. A given repository is trusted on first use: `code add` pins the checksum of what it fetched, and the repository with it. Every later fetch and every load checks the files against the pin; a mismatch is refused and leaves the cache and the configuration unchanged.
- A given repository is an `https://` URL, or an `http://` URL whose host is a loopback address. Anything else is rejected before a connection.
- `code add` writes the configuration through [DES-WRITE-PROCEDURE](writes.mara.md#01M7BF6HJA1E3SE8WJH77FGP17).
- A grammar runs as WebAssembly in tree-sitter's WASM store inside Mara's process: it reaches no file, network or native code, and `tags.scm` is a query, not code.
- Grammar ABI compatibility is checked on load.
- A grammar that is not in the cache, does not match its pin or cannot be loaded is `unavailable-grammar`, an error at the configuration line that pins it naming `mara code add`; that language's files are scanned at the basic level. No operation but `code add` fetches it.

---

## Decisions

---

<a id="01M7JN6VK2WTSD2XWXCTY38KYY"></a>

### Trust a given grammar repository on first use

| Type | Decision |
| --- | --- |
| ID | ADR-GRAMMAR-TRUST-ON-FIRST-USE |
| Status | Draft |
| Justifies | [DES-GRAMMAR-ASSETS](#01M7BF6GQC7MW6ZSA9NMEQRQHM) |

- Chosen provisionally by the orchestrator on 2026-11-16, for the maintainer to confirm: catalog entries carry their checksums; a repository the user gives is trusted on first use, its fetched files' checksum and the repository pinned, and every later fetch and load checked against the pin; `https://`, or `http://` only to a loopback host ([DES-GRAMMAR-ASSETS](#01M7BF6GQC7MW6ZSA9NMEQRQHM)).
- Rejected: no pin. A later fetch or a changed cache could run other code unnoticed.
- Rejected: a required checksum argument for every repository outside the catalog. Users would copy it from the same source they fetch from.
- Rejected: plain HTTP to any host. Anyone on the path could replace the grammar on first use.
