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
| Basic, every language | Comment prefixes configured per file extension | File and line |
| Grammar | A tree-sitter WASM grammar and its upstream `queries/tags.scm` | File and owning declaration |

- At grammar level, declarations come from `tags.scm`, and comments are recognized by node kind (`comment`, `line_comment`, `block_comment`).
- A marker names one relation and one ID, never a MID. Several items take several markers.
- A code location is addressed as `<path>:<line>`, like a narrative unit ([DES-NARRATIVE-UNITS](retrieval.mara.md#01M7CMW710P9V73WXFQ15PBYBS)). `relation list` adds the declaration name at grammar level.
- `get` of a code location returns the owning declaration at grammar level, and the file from that line at basic level.

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

- Reading code rescans only the code files that changed ([DES-INPUTS-AND-KEYS](engine.mara.md#01M7BF6GYCNTZQ0VXCMGJYPZHY)).
- A project that configures no code language never reads code.

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
- Grammar ABI compatibility is checked on load.
