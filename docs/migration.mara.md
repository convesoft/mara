# Migration from 0.3.0

## Requirements

---

<a id="01M7BF6H30RYJTH5Z2YERBJ690"></a>

### Convert a `:::mara` corpus once

| Type | Requirement |
| --- | --- |
| ID | REQ-CONVERT-DIRECTIVE-CORPUS |
| Status | Accepted |
| Satisfies | [NEED-READABLE-SOURCE](product.mara.md#01M7BF6H5VABHYDECYVCQ2A9Y8) |

A one-shot converter rewrites a 0.3.0 project (its `:::mara` documents, schema and configuration) into the current formats. It keeps every MID, ID and field value and every relation the current format can express, and reports whatever it leaves out. No other operation reads 0.3.0 syntax.

---

## Design

---

<a id="01M7CMW71TXDY6VCTJS7XNSQRM"></a>

### Conversion mapping

| Type | Design |
| --- | --- |
| ID | DES-CONVERSION-MAPPING |
| Status | Accepted |
| Satisfies | [REQ-CONVERT-DIRECTIVE-CORPUS](#01M7BF6H30RYJTH5Z2YERBJ690) |

| 0.3.0 source | Becomes |
| --- | --- |
| `[[ID]]` or `[[MID]]` mention | Mention link ([DES-RELATION-LINKS](identity.mara.md#01M7BF6H29JGHG65SV91K6D11V)) |
| Relation assertion in an item: a metadata row or an inline typed relation, under the forward or the inverse name | One row on the source item of its directed edge, written once; an inline form also leaves a mention link in place |
| Inline typed relation in narrative outside an item | Mention link; reported |
| Symmetric relation, or relation whose target is not an item | No row; an inline occurrence keeps its text; reported |
| Thematic break inside an item body | `<hr>` |
| Setext heading inside an item body | ATX heading of the same level |
| Item with an empty body | Converted as it is; reported for the author to complete |
| Flavour | Type |
| `status` fields of the flavours | The schema's status list: the union of their values |
| Field of type `string` | Field of kind `text` |
| Field of type `integer`, `number` or `boolean` | Field of kind `text`; reported |
| Relation with source and target lists | Every source type allows every listed target type; with `same_flavour`, each source type that is also a target allows only itself |
| Relation without an inverse name | Inverse name `inverse_of_<name>`; reported |
| `code_source` relation | Source `code` with the listed targets |
| `acyclic` of severity `error`; of severity `warning`, or `cardinality` | `acyclic = true`; left out and reported |
| `[[ID]]` or `[[MID]]` that names no item; relation row whose target names no item | Kept as text and reported; a row with the bare value, or, under the inverse name, left out and reported |
| YAML rule files, SCIP commands, schema guidance other than `description` | Left out; reported |
| Include pattern with brace alternatives or a character class | Kept as written, where they match themselves ([DES-PROJECT-CONFIGURATION](project.mara.md#01M7BF6H9RDYSET9QTAKJWTX27)); reported |

- An item without a status gets the status the user names for the run, `project convert --status <status>`, a status name or label. It joins the status list when no flavour has it. Without it, items without a status, or flavours with no status value, refuse the run with `invalid-argument` and a diagnostic at each such item.
- Each `:::mara` block becomes an item in its place, with the item writer's anchor, heading, table and separators ([DES-ITEM-WRITER](authoring.mara.md#01M7BF6GP2Z6R1VCQMQ6ZZB218)). Its heading is one level below the last narrative heading before it, at most 6, or level 2 when none precedes it. Its title is its `:title:`, or its ID when it has none. A block without a valid `:mid:` gets a new MID. Both are reported.
- A metadata key that the item's flavour declares as a field is a field, even when another flavour's relation has that name.
- Rows follow `Type` and `ID` in metadata order, a missing status after `ID`, then the rows that inline relations and other items' inverse assertions give it, in path and line order.
- A `<hr>` that text follows directly gets a blank line after it, so that it opens no HTML block that takes the text in.
- Narrative keeps its bytes but for its mentions: a setext heading there stays, and validation reports it.
- Documents are found as 0.3.0 found them: the include patterns read with `globset`, brace alternatives and character classes included, and `.gitignore` files applied. `.git`, symbolic links and other projects' roots are skipped, as in the input inventory ([DES-INPUTS-AND-KEYS](engine.mara.md#01M7BF6GYCNTZQ0VXCMGJYPZHY)). The YAML schema is read with `granit-parser`, the parser 0.3.0 used. Flags take the spellings 0.3.0 took: `true`, `yes`, `y`, `on` and their opposites, in any case. A value that cannot be read as what its key needs is left out and reported. A YAML merge key `<<` is not applied: what it would bring in is left out, and the merge key is reported.
- Values that become no schema name refuse the run like two values that become one label.
- The converted schema is `.mara/schema.toml`. The 0.3.0 configuration, schema and rule files stay where they are.
- Each converted document is parsed once before anything is written, and the run is refused with `unreadable-edit` unless the document reads exactly the items written, with their rows ([ADR-READ-BACK-EVERY-EDIT](authoring.mara.md#01M7FH6HEWNYQ66F5KE912BQSM)). The check compares each item's MID, its rows' keys and values and whether its body holds anything, and that no other item appears; unlike the writer's read-back, it does not compare narrative or references.
- A document that holds items and no `:::mara` block is already converted. A rerun changes in it only the rows that other documents' assertions give its items, and computes no entry from it, since its lines are no longer those the report names.
- The report entry of a generated MID names it. A rerun gives the block, found by its document and ID, the MID that a recorded entry names, so that the links converted documents already hold keep resolving.
- `project convert` of a project that is not a 0.3.0 project fails with `not-a-legacy-project`.
- When two values of one field, or two statuses, become the same label ([DES-METADATA-TABLE](format.mara.md#01M7BF6GZNS6VM3T94VS9A5HTD)), the converter writes nothing and reports each pair.
- A valid 0.3.0 project converts into a valid project, except for the items reported for the author to complete.
- The converter writes each file as [DES-WRITE-PROCEDURE](writes.mara.md#01M7BF6HJA1E3SE8WJH77FGP17) does: the report file first, then the documents, and the configuration last. An interrupted run leaves a 0.3.0 project that every other operation refuses with `legacy-project`; a rerun keeps the documents already converted and writes the files an uninterrupted run writes ([ADR-CONVERSION-RERUN](#01M7F9N0J8E9GJPCEATS4MQX7T)).
- The report file, `.mara/conversion-report.json`, holds the whole report before any document is written and stays as the record of the conversion ([ADR-CONVERSION-REPORT-FILE](#01M7FCAD62YSR955FCEE8AEDR7)). It is a JSON object of `format_version` (1) and `entries`, in path, line and message order, one entry per line. An entry is the `path` of the 0.3.0 file it concerns, relative to the project root, a `line` there and a `message`. The result is `format_version` (1), the `files` written, in order, and the same `entries`.
- A rerun keeps every entry already in the report file, adds each entry it computes that the file does not hold with the same path, line and message, writes the whole report to the file and returns it. No recorded entry is ever dropped: one whose line no longer matches its file stays as recorded, and when the rerun finds the same omission at another line, the report holds both.
- An existing report file that is not a regular file, cannot be read as that object, or has another `format_version` refuses the conversion with a diagnostic at that file, and nothing is written.

---

## Decisions

---

<a id="01M7F9N0J8E9GJPCEATS4MQX7T"></a>

### Complete an interrupted conversion by rerunning it

| Type | Decision |
| --- | --- |
| ID | ADR-CONVERSION-RERUN |
| Status | Accepted |
| Justifies | [REQ-INTERRUPTED-EDIT-LOSES-NOTHING](writes.mara.md#01M7BF6HJ3758Y06V61901ZSKK) |
| Justifies | [DES-CONVERSION-MAPPING](#01M7CMW71TXDY6VCTJS7XNSQRM) |

- Chosen by the maintainer on 2026-11-15: the documents first and the configuration last, so an interrupted conversion leaves a 0.3.0 project that rerunning the conversion completes, and that leftover joins those of [REQ-INTERRUPTED-EDIT-LOSES-NOTHING](writes.mara.md#01M7BF6HJ3758Y06V61901ZSKK). No user data is lost.
- Rejected: holding the conversion to the leftovers of the other edits. A partly converted project is none of them.
- Rejected: the configuration first. A 0.4.0 project would read the unconverted `:::mara` blocks as narrative, so their items would drop out without a diagnostic.

---

<a id="01M7FCAD62YSR955FCEE8AEDR7"></a>

### Keep the conversion report in a file written before the documents

| Type | Decision |
| --- | --- |
| ID | ADR-CONVERSION-REPORT-FILE |
| Status | Accepted |
| Justifies | [REQ-CONVERT-DIRECTIVE-CORPUS](#01M7BF6H30RYJTH5Z2YERBJ690) |
| Justifies | [DES-CONVERSION-MAPPING](#01M7CMW71TXDY6VCTJS7XNSQRM) |

- Chosen by the maintainer on 2026-11-15: before writing any document, the converter writes the whole report to a file in the project with its own format version. The file stays as the record of the conversion, and a rerun keeps the entries of documents already converted and returns the whole report. It is complete on every surface and survives a power loss, at the cost of one persisted file in every converted project.
- Rejected: printing the report before the first write. With `--format json`, through MCP or with a lost terminal, an interrupted run would still lose the omissions of the documents it had converted.
- Rejected: keeping the dropped text in the converted document as an HTML comment. Every conversion's output would carry it.

---

## Dropped capabilities

| 0.3.0 capability | In v0.4.0 |
| --- | --- |
| `trace matrix` | A coverage matrix is a view over a program's output ([DES-RULE-CONSUMERS](rules.mara.md#01M7BF6HE529B61Q2ZNXFQ1JXE)) |
| `related` and `relation get` | `relation list` ([REQ-DIRECT-NEIGHBOURS](retrieval.mara.md#01M7BF6HBCR1H0P6S8KWBV3DAS)) |
| `minimal` template; the 0.3.0 `engineering` vocabulary | `empty`, and `engineering` with a new vocabulary ([DES-ENGINEERING-VOCABULARY](vocabulary.mara.md#01M7BF6HH8CA2B9TQJ9DT32WDG)) |
| Symmetric relations; relation targets that are not items | None |
| `[[ID]]` mentions, inline typed relations, authoring a relation from its target | Links and rows on the source item ([DES-RELATION-LINKS](identity.mara.md#01M7BF6H29JGHG65SV91K6D11V)) |
| YAML rule files | Soufflé programs ([DES-RULE-CONSUMERS](rules.mara.md#01M7BF6HE529B61Q2ZNXFQ1JXE)) |
| SCIP indexers | Comment markers ([DES-CODE-MARKERS](code.mara.md#01M7BF6GQ1G370S0G7HPN4HGY1)) |
| Brace alternatives and character classes in include patterns | `**`, `*` and `?` ([DES-PROJECT-CONFIGURATION](project.mara.md#01M7BF6H9RDYSET9QTAKJWTX27)) |
| Mutation journal and `project transaction rollback` | Ordered writes ([DES-WRITE-PROCEDURE](writes.mara.md#01M7BF6HJA1E3SE8WJH77FGP17)) |
| `project mid backfill` | Fix ([REQ-ONE-FIX-OPERATION](authoring.mara.md#01M7BF6GNPRRHNP2TRX7MKZRRH)) |
| Standalone agent skill | `mara prime` and the MCP server instructions ([DES-AGENT-GUIDE](agents.mara.md#01M7BGRHWQHQJJHC6XCXVDWDM8)) |
