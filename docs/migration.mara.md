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
| Inline typed relation in an item body | Relation row on that item, and a mention link in place |
| Inline typed relation in narrative outside an item | Mention link; reported |
| Symmetric relation, or relation whose target is not an item | No row; an inline occurrence keeps its text; reported |
| Flavour | Type |
| `status` fields of the flavours | The schema's status list: the union of their values |
| Field of type `string` | Field of kind `text` |
| Relation with source and target lists | Every source type allows every listed target type |
| YAML rule files, SCIP commands, schema guidance other than `description` | Left out; reported |

- Items of a flavour without a `status` field get the status the user names for the run.
- A valid 0.3.0 project converts into a valid project.

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
| Mutation journal and `project transaction rollback` | Ordered writes ([DES-WRITE-PROCEDURE](writes.mara.md#01M7BF6HJA1E3SE8WJH77FGP17)) |
| `project mid backfill` | Fix ([REQ-ONE-FIX-OPERATION](authoring.mara.md#01M7BF6GNPRRHNP2TRX7MKZRRH)) |
| Standalone agent skill | `mara prime` and the MCP server instructions ([DES-AGENT-GUIDE](agents.mara.md#01M7BGRHWQHQJJHC6XCXVDWDM8)) |
