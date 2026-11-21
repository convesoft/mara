# Validation

## Requirements

---

<a id="01M7BF6HEVWZ2490K493GDP8KC"></a>

### Report every problem with its location

| Type | Requirement |
| --- | --- |
| ID | REQ-REPORT-EVERY-PROBLEM |
| Status | Accepted |
| Satisfies | [NEED-CONFORMANCE-FEEDBACK](product.mara.md#01M7BF6H6S3P42Y9TQ30T38WG6) |

Project validation reports every independently detectable problem in the configuration, schema and documents, each with a stable code, severity, file and line. Item validation reports the problems of one item, checked against the whole project.

---

<a id="01M7BF6HF8R0Q5BK349ESZ2PS7"></a>

### Validity covers the whole project

| Type | Requirement |
| --- | --- |
| ID | REQ-VALIDITY-COVERS-WHOLE-PROJECT |
| Status | Accepted |
| Satisfies | [NEED-CONFORMANCE-FEEDBACK](product.mara.md#01M7BF6H6S3P42Y9TQ30T38WG6) |

The validity result and the CLI exit status reflect every error in the project, even when the report is paged or filtered by path.

---

<a id="01M7BF6HFM0MQ2NH7RA7S00B8K"></a>

### Reading never rewrites source

| Type | Requirement |
| --- | --- |
| ID | REQ-READS-NEVER-WRITE |
| Status | Accepted |
| Satisfies | [NEED-SAFE-AUTHORING](product.mara.md#01M7BF6H67YSV2DQ3QYBVQCN9V) |

Parsing, validation and every read operation leave source files untouched.

---

<a id="01M7BF6HFZVTK5PC7W60FPND94"></a>

### Invalid rows are reported, never dropped

| Type | Requirement |
| --- | --- |
| ID | REQ-INVALID-ROWS-REPORTED |
| Status | Accepted |
| Satisfies | [NEED-CONFORMANCE-FEEDBACK](product.mara.md#01M7BF6H6S3P42Y9TQ30T38WG6) |

An item with invalid metadata rows still loads and can be read. Each invalid row is reported; none is silently dropped.

---

## Design

---

<a id="01M7BF6HG7AQZ9FM38CMT4BTHR"></a>

### Built-in checks

| Type | Design |
| --- | --- |
| ID | DES-SCHEMA-CHECKS |
| Status | Accepted |
| Satisfies | [REQ-REPORT-EVERY-PROBLEM](#01M7BF6HEVWZ2490K493GDP8KC) |

Implemented in Rust, always on, linear in items plus edges.

| Check | Rules |
| --- | --- |
| Document structure | [DES-REGIONS](format.mara.md#01M7BF6GZ5V5HWV00MDZN68Z06), [DES-ITEM-STRUCTURE](format.mara.md#01M7BF6GZDTNNW0Z0MX7FEG6ZF) |
| Metadata | Required fields, allowed values, undeclared keys, repeated rows ([DES-METADATA-TABLE](format.mara.md#01M7BF6GZNS6VM3T94VS9A5HTD)) |
| Identity | MID form and uniqueness, ID prefix and uniqueness ([DES-MID-ANCHOR](identity.mara.md#01M7BF6H1MB5RR3RS43B3CJHAR), [DES-ITEM-IDS](identity.mara.md#01M7BF6H1ZBTKWTF9ZXEJ8EBGS)) |
| Links | Relation and mention targets exist; paths and relation link text are current ([DES-RELATION-LINKS](identity.mara.md#01M7BF6H29JGHG65SV91K6D11V)) |
| Relations | Target type allowed for the source type, same-type relations, acyclic relations ([DES-SCHEMA-MODEL](project.mara.md#01M7BF6HA0KG3R3AATWM1P2X1G)) |
| Code markers | Form, target and relation of each marker; grammars ([DES-CODE-MARKERS](code/index.mara.md#01M7BF6GQ1G370S0G7HPN4HGY1), [DES-GRAMMAR-ASSETS](code/index.mara.md#01M7BF6GQC7MW6ZSA9NMEQRQHM)) |

Parsing reports the Document structure failures and the malformed metadata rows of each document on its own, with the codes of the pages linked above. The other checks have these codes:

| Code | Problem | Reported at |
| --- | --- | --- |
| `undeclared-type` | A `Type` value names no type of the schema | The row |
| `missing-row` | No `ID` or `Status` row, or no row of a required field | The anchor |
| `duplicate-row` | The row repeats the key and value of an earlier row | The later row |
| `repeated-key` | Another value for `Type`, `ID`, `Status` or a field that is not repeatable | The later row |
| `undeclared-key` | A key that is no field of the item's type and no relation that leads from it | The row |
| `invalid-value` | A `Status` outside the schema's statuses, or a value outside its field's values | The row |
| `invalid-id` | An `ID` value not of the form of [DES-ITEM-IDS](identity.mara.md#01M7BF6H1ZBTKWTF9ZXEJ8EBGS) | The row |
| `wrong-id-prefix` | An `ID` value without its type's prefix | The row |
| `duplicate-mid` | An earlier item has the MID | The anchor |
| `duplicate-id` | An earlier item has the ID | The `ID` row |
| `bare-id-value` | A relation row's value that is an ID, not a link | The row |
| `invalid-relation-value` | Any other relation row's value that is no link to a MID | The row |
| `broken-link` | No item has the link's MID | The link |
| `stale-link-path` | The link's path, resolved from the linking file, does not name the file that holds the MID | The link |
| `stale-link-text` | A relation link's text is not the target's ID | The link |
| `disallowed-target` | The relation does not lead from the source's type, or from `code`, to the target's | The link or marker |
| `invalid-marker` | A line that starts as a marker but is not one | The line |
| `unresolved-marker` | No item has the marker's ID | The marker |
| `unavailable-grammar` | A pinned grammar is not cached, does not match its pin, cannot be loaded, or could not parse a code file in time ([DES-GRAMMAR-ASSETS](code/index.mara.md#01M7BF6GQC7MW6ZSA9NMEQRQHM)) | The configuration line that pins it |
| `relation-cycle` | Links of an acyclic relation form a cycle | Its first link |

- Earlier and first are in path and line order. A problem that several items share, a repeated MID or ID or a cycle, is reported once to the project and to the validation of each item it concerns, there at that item's own anchor, row or link.
- The first `Type` row gives the item's type. A row's key names a field of that type, a relation that the type is a source of ([DES-SNAPSHOT](engine.mara.md#01M7BF6GYM8NPPGY9RZRF38EGE)), or neither, which is `undeclared-key`; only a relation row is an edge.
- Every row's value is checked on its own, a repeated one included. The first row of a single-valued key gives the item's value for the other checks. A shared problem stays once per item when rows repeat: an exact repeat of an `ID` row has its own form and prefix checked, not the uniqueness of its value.
- A check that needs a declaration the schema lacks does not run: without a schema, or for an item of an undeclared type. The problem that keeps the declaration out is reported instead.
- A link path is percent-decoded and resolved like a relative URL. An absolute path, or one that leaves the project root, is stale. A link to a MID that several items have is not checked further.
- A message quotes at most 100 characters of a value, and lists at most 10 names followed by `and <N> more` for the rest.

Conditional obligations, such as "an accepted Verification verifies an accepted Requirement", are project logic ([DES-RULE-CONSUMERS](rules/index.mara.md#01M7BF6HE529B61Q2ZNXFQ1JXE)).

---

<a id="01M7CMW71GXWZS8HPVVTYPENDK"></a>

### Diagnostics

| Type | Design |
| --- | --- |
| ID | DES-DIAGNOSTICS |
| Status | Accepted |
| Satisfies | [REQ-REPORT-EVERY-PROBLEM](#01M7BF6HEVWZ2490K493GDP8KC) |
| Satisfies | [REQ-VALIDITY-COVERS-WHOLE-PROJECT](#01M7BF6HF8R0Q5BK349ESZ2PS7) |

- A diagnostic has a code, a severity (`error` or `warning`), a message, a path and a line, and the ID and MID of the item it concerns.
- Codes are stable kebab-case names such as `stale-link-path`. An invariant violation carries the invariant's name.
- A diagnostic owns its strings, except the code of a built-in check, which is a constant of the program and is not allocated; a process keeps none of the others once the result that carries the diagnostic is dropped.
- Every built-in check failure is an error. A warning comes only from an invariant that declares that severity.
- A result carries the project's validity and its error and warning counts, then one page of diagnostics in path and line order.
- The CLI prints each diagnostic as `<path>:<line>: <severity>[<code>]: <message>`.
- Validity and counts cover the target. `project validate` targets the whole project. `item validate <ID or MID>` targets the diagnostics of that item, of the configuration and of the programs its invariants use ([DES-RULE-EXECUTION](rules/evaluation.mara.md#01M7BF6HD6K4B2K5Q7FJVGCDNH)), and fails with `unknown-item` or `ambiguous-item` when no item or several have the handle. `schema validate` targets the diagnostics of the schema file and those of the configuration at the line that names the schema, or every configuration diagnostic when it names no schema, and reads no document.
- `--limit` and `--cursor` page every target ([REQ-PAGE-LIMITS](retrieval/index.mara.md#01M7CMW719S9H8FQKRFWWQ6NQK), [DES-PAGING](retrieval/index.mara.md#01M7BF6HCCWQH6P13ZR2YHAPS0)). `project validate --path <path>`, repeatable and relative to the project root, keeps the diagnostics in that file or below that directory. As in 0.3.0, empty and `.` parts and a trailing `/` are dropped, and an absolute path, a `..` part or the root itself is rejected. The normalized paths, sorted and without repeats, enter the result key.
- A result's key also covers the code, path and line of each diagnostic that no input key covers, such as an input that cannot be read.
- The JSON result is `format_version` (1), `valid`, `errors`, `warnings`, `diagnostics` and `next_cursor`, `null` on the last page. A diagnostic is `code`, `severity`, `message`, `path`, `line`, `id` and `mid`; `id` and `mid` are `null` outside an item.
- The human form prints the page's diagnostics, then `valid` or `invalid` with the counts, then the next cursor.
