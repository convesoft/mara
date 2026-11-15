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

`project init` writes a project configuration and the schema and programs of a template into the current or a named directory: `engineering` unless `empty` is named. It refuses to overwrite an existing file and reports every file it created.

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

- The configuration is `.mara/config.toml`. The directory that contains `.mara/` is the project root.
- It declares every input by a path or pattern relative to the project root: document include patterns, the schema, Soufflé programs, and code languages with their extensions, comment prefixes and grammar pin (version and checksum).
- A directory below the project root whose `.mara/` holds `config.toml` or `project.toml` is the root of another project. Nothing below it is an input, even where a pattern matches. A `config.toml` outside a `.mara/` directory makes no project.
- `project init` writes a template's configuration, schema and programs ([DES-PROJECT-INIT](init.mara.md#01M7FVWHVXW71CWC0HJ52NTEQJ)).
- The configuration and the schema are TOML. Each carries its own `format_version`, starting at 1. The configuration also holds the version of the document syntax, `[documents] format_version`.
- An unknown setting is an error.
- `.mara/project.toml` without `.mara/config.toml` marks a 0.3.0 project. Every operation except the conversion ([REQ-CONVERT-DIRECTIVE-CORPUS](migration.mara.md#01M7BF6H30RYJTH5Z2YERBJ690)) refuses it and names the conversion.
- Selection starts at the working directory and stops at the nearest directory, itself included, whose `.mara/` holds an entry named `config.toml` or `project.toml`. `--project` names that directory itself, relative to the working directory, and is not searched from. A directory that cannot be examined stops the search with an error, at line 1 of its configuration when its `.mara` is the cause. The project root is the selected directory with symlinks resolved.
- Every setting of the example is required, and a host test loads the example from this page. Until 0.4.0, a setting the binary does not implement yet is unknown; adding it keeps `format_version` 1.
- A path or pattern is relative to the project root, with `/` between non-empty parts and no `.` or `..` part. Loading checks nothing else of a pattern. In a pattern, a part `**` matches any number of path parts, none included; in any other part, `*` matches any run of characters, `?` one character, and every other character itself, so no pattern is invalid. Unlike 0.3.0, braces and character classes are no syntax: `{`, `}`, `[` and `]` match themselves.
- Mara follows no symbolic link in a project's own files. Loading refuses `.mara`, `.mara/config.toml` or a component of the schema path that is a symbolic link, with `symbolic-link` at line 1 of the configuration for the first two and at the line that names the schema otherwise. The ancestor search, `--project` and the check below refuse a `.mara` that is a symbolic link the same way; none follows or skips it. The rule is checked on the path as it is when Mara reads it; a process that changes the project's files during a load is outside what it covers.
- A schema path, as written, with a component that is another project's root is `outside-project` at the line that names it, and the schema is not read.
- Every operation that reads documents or the schema, except validation, refuses a project whose configuration or schema has an error and reports those diagnostics. `prime`, `project init` and `project convert` are unaffected.
- A `format_version` other than 1 is an error, and nothing else in its file, or in `[documents]`, is read. A missing `format_version` is an error, and the rest of its file or table is read as version 1. Any other problem leaves out only the setting it concerns.
- A configuration or schema that cannot be read is reported at line 1 of the configuration, or at the line that names the schema. Problems of selection, configuration and schema, and of the input inventory, have these codes ([DES-DIAGNOSTICS](validation.mara.md#01M7CMW71GXWZS8HPVVTYPENDK)). The inventory reports at the path of the input concerned, at the line [DES-INPUTS-AND-KEYS](engine.mara.md#01M7BF6GYCNTZQ0VXCMGJYPZHY) gives:

| Code | Problem | Reported by |
| --- | --- | --- |
| `no-project` | No project root at or above the working directory | Selection |
| `not-a-project` | `--project` names a directory whose `.mara/` holds no `config.toml` | Selection |
| `legacy-project` | The selected directory is the root of a 0.3.0 project | Selection |
| `io-error` | A file or directory cannot be read; in the inventory, also an input or `.gitignore` that is not a regular file, or a name that is not UTF-8; in a write, a file that cannot be written | Selection, loading, inventory, writes |
| `outside-project` | The schema path enters another project's root | Loading |
| `symbolic-link` | `.mara`, the configuration or a component of the schema path is a symbolic link; in the inventory, a matched document, a `.gitignore` or a subdirectory's `.mara`; in a write, a file it writes or a directory above it ([DES-WRITE-PROCEDURE](writes.mara.md#01M7BF6HJA1E3SE8WJH77FGP17)) | Selection, loading, inventory, writes |
| `unsupported-gitignore` | A `.gitignore` the `ignore` crate cannot take: a line that is not UTF-8, or patterns it cannot compile | Inventory |
| `toml-syntax` | The configuration or schema is not UTF-8 TOML | Loading |
| `unsupported-format-version` | A `format_version` other than 1 | Loading |
| `unknown-key` | A key the format does not define | Loading |
| `missing-key` | A required key is absent | Loading |
| `invalid-value` | A value of the wrong type or form | Loading |
| `duplicate-name` | A name repeated where it must be unique | Loading |
| `undeclared-type` | A relation names a type the schema does not declare | Loading |
| `reserved-name` | A type named `code`, or a field or relation named `type`, `id` or `status` | Loading |

```toml
format_version = 1
schema = ".mara/schema.toml"

[documents]
format_version = 1
include = ["docs/**/*.mara.md"]
```

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
| Schema | Format version; the `Status` values, one list for every type |
| Type | Name, ID prefix, description, fields |
| Field | Name, value kind (`text` or `enum`), allowed values, required, repeatable |
| Relation | Forward name, inverse name, description, allowed target types per source type, same-type constraint, acyclic constraint |

- The source `code` in a relation's targets stands for code markers ([DES-CODE-MARKERS](code.mara.md#01M7BF6GQ1G370S0G7HPN4HGY1)). No type may be named `code`.
- "Type" replaces the term "flavour" used before v0.4.0.
- `types`, `relations`, `fields` and the flags `required`, `repeatable`, `same_type` and `acyclic` are optional; a flag defaults to `false`. Every other key of the example is required, and a host test loads the example from this page. A description is not empty.
- Every name, statuses and values included, is lowercase ASCII letters and digits in parts joined by single `_` or `-`. An ID prefix is uppercase letters and digits in parts that each end with `-` ([DES-ITEM-IDS](identity.mara.md#01M7BF6H1ZBTKWTF9ZXEJ8EBGS)).
- No field or relation name, forward or inverse, is `type`, `id` or `status`. Forward and inverse names are all distinct, and no field has the name of a relation its type is a source of.
- `statuses`, the `values` of a field and the targets of a source each list at least one name, none twice, and `targets` names at least one source. A repeated entry is reported at its later occurrence, whether or not it is otherwise valid. Only an `enum` field has `values`.
- Every target is a declared type. With `same_type`, a source lists only its own type, and `code` is no source.
- A schema with errors still loads: each invalid declaration, or invalid part of one, is left out once its other parts are checked.
- Declared names are collected as written before declarations are checked, so a collision is reported whether or not its declarations are kept, at its later occurrence in the file. An inverse name that repeats a relation name or an earlier inverse name is left out, and so is a field with the name of a relation its type is a source of.

```toml
format_version = 1
statuses = ["draft", "accepted", "retired"]

[types.need]
id_prefix = "NEED-"
description = "A stakeholder capability, condition or result."

[types.requirement]
id_prefix = "REQ-"
description = "One precise, necessary, verifiable obligation."

[types.risk]
id_prefix = "RISK-"
description = "An uncertain cause with an adverse consequence."

[types.risk.fields.likelihood]
kind = "enum"
values = ["rare", "unlikely", "possible", "likely", "almost-certain"]

[types.term]
id_prefix = "TERM-"
description = "A canonical definition whose ambiguity affects engineering work."

[types.term.fields.aliases]
kind = "text"
repeatable = true

[relations.satisfies]
inverse = "satisfied_by"
description = "The source is intended to fulfil the target."
targets = { requirement = ["need"] }

[relations.implements]
inverse = "implemented_by"
description = "Code realizes the target."
targets = { code = ["requirement"] }

[relations.supersedes]
inverse = "superseded_by"
description = "The source replaces the target."
targets = { need = ["need"], requirement = ["requirement"] }
same_type = true
acyclic = true
```
