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

A one-shot converter rewrites a project's `:::mara` documents into the current format and keeps every MID, ID, field value and relation. No other operation reads `:::mara` syntax.

---

## Dropped capabilities

| 0.3.0 capability | In v0.4.0 |
| --- | --- |
| `trace matrix` | A coverage matrix is a view over a program's output ([DES-RULE-CONSUMERS](rules.mara.md#01M7BF6HE529B61Q2ZNXFQ1JXE)) |
| `minimal` template; the 0.3.0 `engineering` vocabulary | `empty`, and `engineering` with a new vocabulary ([DES-ENGINEERING-VOCABULARY](vocabulary.mara.md#01M7BF6HH8CA2B9TQJ9DT32WDG)) |
| Symmetric relations; relation targets that are not items | None |
| `[[ID]]` mentions, inline typed relations, authoring a relation from its target | Links and rows on the source item ([DES-RELATION-LINKS](identity.mara.md#01M7BF6H29JGHG65SV91K6D11V)) |
| YAML rule files | Soufflé programs ([DES-RULE-CONSUMERS](rules.mara.md#01M7BF6HE529B61Q2ZNXFQ1JXE)) |
| SCIP indexers | Comment markers ([DES-CODE-MARKERS](code.mara.md#01M7BF6GQ1G370S0G7HPN4HGY1)) |
| Mutation journal and `project transaction rollback` | Ordered writes ([DES-WRITE-PROCEDURE](writes.mara.md#01M7BF6HJA1E3SE8WJH77FGP17)) |
| `project mid backfill` | Fix ([REQ-ONE-FIX-OPERATION](authoring.mara.md#01M7BF6GNPRRHNP2TRX7MKZRRH)) |
| Standalone agent skill | `mara prime` and the MCP server instructions ([DES-AGENT-GUIDE](agents.mara.md#01M7BGRHWQHQJJHC6XCXVDWDM8)) |

---

## Open questions

- How the converter maps `[[ID]]` mentions and inline typed relations.
- Whether the converter also rewrites a 0.3.0 schema and configuration.
