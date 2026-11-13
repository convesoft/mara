# Product

## Goals

---

<a id="01M7BF6H538WCKRHK34YZGH46D"></a>

### One source for knowledge and documentation

| Type | Goal |
| --- | --- |
| ID | GOAL-ONE-SOURCE |
| Status | Accepted |

Project knowledge lives in Git-tracked Markdown files that are both the editable source and the published documentation. No database, index, export or tool response is authoritative.

---

<a id="01M7BF6H5B5Q7V0GB9CFXF1KJ8"></a>

### Agents work from the relevant knowledge, not the whole corpus

| Type | Goal |
| --- | --- |
| ID | GOAL-BOUNDED-CONTEXT |
| Status | Accepted |

An agent finds, reads and follows the items relevant to its task in bounded steps, without loading the corpus.

---

<a id="01M7BF6H5JHS6GZ8CMB98MBTMM"></a>

### Operations stay fast as projects grow

| Type | Goal |
| --- | --- |
| ID | GOAL-FAST-AT-SCALE |
| Status | Accepted |

The cost of an operation follows what changed and what the operation reads, not the size of the repository.

---

## Needs

---

<a id="01M7BF6H5VABHYDECYVCQ2A9Y8"></a>

### Read the source as documentation

| Type | Need |
| --- | --- |
| ID | NEED-READABLE-SOURCE |
| Status | Accepted |
| Contributes to | [GOAL-ONE-SOURCE](#01M7BF6H538WCKRHK34YZGH46D) |

Readers open a Mara document on GitHub or in an editor and get rendered documentation with working links, without an export step.

---

<a id="01M7BF6H61ST5R4855YZKCNJHC"></a>

### Record knowledge as typed, related items

| Type | Need |
| --- | --- |
| ID | NEED-STRUCTURED-KNOWLEDGE |
| Status | Accepted |
| Contributes to | [GOAL-ONE-SOURCE](#01M7BF6H538WCKRHK34YZGH46D) |

Authors record each unit of knowledge as an item with a type, a stable identity, fields and typed relations, all defined by a schema the project owns.

---

<a id="01M7BF6H67YSV2DQ3QYBVQCN9V"></a>

### Revise knowledge without breaking it

| Type | Need |
| --- | --- |
| ID | NEED-SAFE-AUTHORING |
| Status | Accepted |
| Contributes to | [GOAL-ONE-SOURCE](#01M7BF6H538WCKRHK34YZGH46D) |

Authors and agents create, edit, move, rename and delete items, by tool or by hand, without losing identity, links or unrelated text.

---

<a id="01M7BF6H6GBKEHK5BQ8NDX7JG5"></a>

### Find and read knowledge in bounded portions

| Type | Need |
| --- | --- |
| ID | NEED-BOUNDED-RETRIEVAL |
| Status | Accepted |
| Contributes to | [GOAL-BOUNDED-CONTEXT](#01M7BF6H5B5Q7V0GB9CFXF1KJ8) |

Agents and authors search items and narrative, read a selected node and follow its direct connections, each in a bounded response.

---

<a id="01M7BF6H6S3P42Y9TQ30T38WG6"></a>

### Learn where the corpus is wrong

| Type | Need |
| --- | --- |
| ID | NEED-CONFORMANCE-FEEDBACK |
| Status | Accepted |
| Contributes to | [GOAL-ONE-SOURCE](#01M7BF6H538WCKRHK34YZGH46D) |

Authors learn every place where documents break the format, the schema or the project's rules, with file and line.

---

<a id="01M7BF6H74D5GX045CWVY9106D"></a>

### Define project-specific rules and questions

| Type | Need |
| --- | --- |
| ID | NEED-PROJECT-LOGIC |
| Status | Accepted |
| Contributes to | [GOAL-ONE-SOURCE](#01M7BF6H538WCKRHK34YZGH46D) |

Project owners write their own invariants, queries, gates and views over the knowledge graph, without changing Mara.

---

<a id="01M7BF6H7D3K8R07DTSRVVGX0G"></a>

### Connect code to knowledge

| Type | Need |
| --- | --- |
| ID | NEED-CODE-LINKS |
| Status | Accepted |
| Contributes to | [GOAL-ONE-SOURCE](#01M7BF6H538WCKRHK34YZGH46D) |

Developers mark the code that implements an item and navigate between the two.

---

<a id="01M7BF6H7Q0YP8SPVYWYJFB1MF"></a>

### Same behaviour from every entry point

| Type | Need |
| --- | --- |
| ID | NEED-ONE-BEHAVIOUR-EVERYWHERE |
| Status | Accepted |
| Contributes to | [GOAL-ONE-SOURCE](#01M7BF6H538WCKRHK34YZGH46D) |

Users get the same operations and results from the CLI, an MCP server, a Node library and a browser.

---

<a id="01M7BF6H83C0SM1C8RHDHMBXZ2"></a>

### Answers at interactive speed

| Type | Need |
| --- | --- |
| ID | NEED-INTERACTIVE-SPEED |
| Status | Accepted |
| Contributes to | [GOAL-FAST-AT-SCALE](#01M7BF6H5JHS6GZ8CMB98MBTMM) |

Users and agents get answers at interactive speed on projects of thousands of items, including right after an edit.

---

<a id="01M7BF6H8C2X09EX518DVSWDCE"></a>

### Run without a toolchain

| Type | Need |
| --- | --- |
| ID | NEED-TOOLCHAIN-FREE-INSTALL |
| Status | Accepted |

Users and agents run Mara from a package manager without installing compilers or running install scripts.

---

<a id="01M7BGRHXED9E91E4E7P0T5WT5"></a>

### Agents learn Mara from Mara

| Type | Need |
| --- | --- |
| ID | NEED-AGENT-ONBOARDING |
| Status | Accepted |
| Contributes to | [GOAL-BOUNDED-CONTEXT](#01M7BF6H5B5Q7V0GB9CFXF1KJ8) |

An agent working in a Mara project learns the workflow from Mara itself, with nothing else to install and at most one line in the project's agent instructions.
