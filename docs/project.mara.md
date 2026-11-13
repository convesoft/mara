# Project and schema

## Requirements

---

<a id="01M7BF6H8ND7YSXFMWCM0KXAAY"></a>

### Each operation runs against one project

| Type | Requirement |
| --- | --- |
| ID | REQ-ONE-PROJECT-PER-OPERATION |
| Status | Accepted |
| Satisfies | [NEED-STRUCTURED-KNOWLEDGE](product.mara.md#01M7BF6H61ST5R4855YZKCNJHC) |

An operation uses the nearest ancestor directory that holds a project configuration, or the directory given by `--project`. It never combines projects.

---

<a id="01M7BF6H8Z02SNKXD2XPGVD87M"></a>

### Initialize a project from a template

| Type | Requirement |
| --- | --- |
| ID | REQ-PROJECT-INIT |
| Status | Accepted |
| Satisfies | [NEED-STRUCTURED-KNOWLEDGE](product.mara.md#01M7BF6H61ST5R4855YZKCNJHC) |

`project init` writes a project configuration and the schema of the `engineering` or `empty` template into the current or a named directory. It refuses to overwrite an existing file and reports every file it created.

---

<a id="01M7BF6H97RDY6XY07F5SC4777"></a>

### The project's schema defines the vocabulary

| Type | Requirement |
| --- | --- |
| ID | REQ-SCHEMA-DEFINES-VOCABULARY |
| Status | Accepted |
| Satisfies | [NEED-STRUCTURED-KNOWLEDGE](product.mara.md#01M7BF6H61ST5R4855YZKCNJHC) |

Types, fields, allowed values and relations come only from the project's schema. Mara builds in only the `Type`, `ID` and `Status` keys.

---

<a id="01M7BF6H9F8E7RB3XBSE0EKAR3"></a>

### Inspect the schema

| Type | Requirement |
| --- | --- |
| ID | REQ-SCHEMA-INSPECTION |
| Status | Accepted |
| Satisfies | [NEED-SAFE-AUTHORING](product.mara.md#01M7BF6H67YSV2DQ3QYBVQCN9V) |

Schema inspection lists the types and relations and returns each one's description, ID prefix, fields and endpoints.

---

## Design

---

<a id="01M7BF6H9RDYSET9QTAKJWTX27"></a>

### Project configuration

| Type | Design |
| --- | --- |
| ID | DES-PROJECT-CONFIGURATION |
| Status | Accepted |
| Satisfies | [REQ-ONE-PROJECT-PER-OPERATION](#01M7BF6H8ND7YSXFMWCM0KXAAY) |

- Its presence marks the project root.
- It declares every input: document include patterns, the schema, Soufflé programs, and code languages with their extensions, comment prefixes and grammar pin (version and checksum).
- An unknown setting is an error.

---

<a id="01M7BF6HA0KG3R3AATWM1P2X1G"></a>

### Schema contents

| Type | Design |
| --- | --- |
| ID | DES-SCHEMA-MODEL |
| Status | Accepted |
| Satisfies | [REQ-SCHEMA-DEFINES-VOCABULARY](#01M7BF6H97RDY6XY07F5SC4777) |
| Satisfies | [REQ-SCHEMA-INSPECTION](#01M7BF6H9F8E7RB3XBSE0EKAR3) |

| Declaration | Holds |
| --- | --- |
| Type | Name, ID prefix, description, fields |
| Field | Name, value kind, allowed values, required, repeatable |
| Relation | Forward name, inverse name, description, allowed target types per source type, same-type constraint, acyclic constraint |

"Type" replaces the term "flavour" used before v0.4.0.

---

## Open questions

- File syntax and location of the configuration and the schema. Version 0.3.0 used `.mara/project.toml` and `.mara/schema.yaml`.
- Whether `Status` values are declared per type or once per schema.
- Which template `project init` uses when none is named.
