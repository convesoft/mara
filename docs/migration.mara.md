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
| Relation with source and target lists | Every source type allows every listed target type |
| YAML rule files, SCIP commands, schema guidance other than `description` | Left out; reported |
| Include pattern with brace alternatives or a character class | Kept as written, where they match themselves ([DES-PROJECT-CONFIGURATION](project.mara.md#01M7BF6H9RDYSET9QTAKJWTX27)); reported |

- An item without a status gets the status the user names for the run.
- When two values of one field, or two statuses, become the same label ([DES-METADATA-TABLE](format.mara.md#01M7BF6GZNS6VM3T94VS9A5HTD)), the converter writes nothing and reports each pair.
- A valid 0.3.0 project converts into a valid project, except for the items reported for the author to complete.

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
