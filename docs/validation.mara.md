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

Conditional obligations, such as "an accepted Verification verifies an accepted Requirement", are project logic ([DES-RULE-CONSUMERS](rules.mara.md#01M7BF6HE529B61Q2ZNXFQ1JXE)).

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
- Every built-in check failure is an error. A warning comes only from an invariant that declares that severity.
- A result carries the project's validity and its error and warning counts, then one page of diagnostics in path and line order.
- The CLI prints each diagnostic as `<path>:<line>: <severity>[<code>]: <message>`.
