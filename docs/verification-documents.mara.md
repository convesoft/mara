# Document verification

---

<a id="01M7D3NAQQ599DWC895A7TG3TC"></a>

### Check documents against their GitHub rendering

| Type | Verification |
| --- | --- |
| ID | VER-GITHUB-RENDERING |
| Status | Accepted |
| Methods | Test |
| Methods | Analysis |
| Methods | Inspection |
| Verifies | [REQ-SOURCE-IS-PUBLISHED-FORM](format.mara.md#01M7BF6GYWE2DT7WSXQH2QPNPA) |

Rust integration tests run the real CLI on disposable projects.

| Case | Method | Expectation |
| --- | --- | --- |
| A document with `***` and `___` breaks and a `---` line inside a fenced code block | Test | Regions break at the `***` and `___` lines only |
| A document that starts with YAML front matter | Test | No diagnostic |
| Same-file, cross-directory and table-cell relation links and mentions in a project that validates without diagnostics | Test | Each link's path, resolved from the linking file, names the file that holds the anchor of its MID |
| GitHub's handling of those links | Analysis | They have the forms that [EVD-GITHUB-FRAGMENT-MATCHING](identity.mara.md#01M7BF6H2MHAVK1Z5HCXR1V289) found GitHub to navigate to their target |
| The CLI's operations | Inspection | None writes documents in another format |

---

<a id="01M7D3NAQYGXQ5YG1YMRV12SKS"></a>

### Resolve hand-edited items by MID

| Type | Verification |
| --- | --- |
| ID | VER-MID-RESOLUTION |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-STABLE-IDENTITY](identity.mara.md#01M7BF6H1AN6G9GP3DV62342WJ) |

Rust integration tests run the real CLI on a disposable project in which another file links to an item by a relation and by a mention. The tests then change the item's title and ID and move it to another file by hand, leaving the links as they were.

| Case | Expectation |
| --- | --- |
| `get` of the MID | The item, with its new title, ID and location |
| `relation list` of the item | The incoming relation and the mention, at their unchanged source locations |

---

<a id="01M7D3NAR43MYRDF7YKMBDSJ10"></a>

### Select the project among nested projects

| Type | Verification |
| --- | --- |
| ID | VER-NESTED-PROJECTS |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-ONE-PROJECT-PER-OPERATION](project.mara.md#01M7BF6H8ND7YSXFMWCM0KXAAY) |

Rust integration tests run each CLI operation on a disposable project that holds a second project in a subdirectory. The include patterns of both match every `*.mara.md` file below their roots, so the outer project's patterns also match the inner project's documents. An operation uses only a project when no result holds an item, narrative unit or diagnostic from another project's files, and the file-read counter ([DES-PERFORMANCE-HARNESS](performance.mara.md#01M7BF6H4HKH0B0TXQZM3VXASA)) counts only that project's inputs.

| Case | Expectation |
| --- | --- |
| Run from a directory inside the inner project | Uses only the inner project: none of the outer project's files is read |
| Run from the outer project, outside the inner one | Uses only the outer project: none of the inner project's files is read |
| `--project` naming one project, run from inside the other | Uses only the named project |
| Run from the outer project after the inner project's `.mara/config.toml` is replaced by `.mara/project.toml` | None of the inner project's documents is read |
| Run from the outer project when a subdirectory holds a `config.toml` outside any `.mara/` directory | The subdirectory's documents are inputs of the outer project |
| Run outside every project | Fails with a diagnostic and parses no document |
| A directory with `.mara/project.toml` and no `.mara/config.toml` | Refused with a diagnostic that names the conversion |

---

<a id="01M7D3NARATT16K40DZCCFCX3H"></a>

### Validate against the project's own schema

| Type | Verification |
| --- | --- |
| ID | VER-CUSTOM-SCHEMA |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-SCHEMA-DEFINES-VOCABULARY](project.mara.md#01M7BF6H97RDY6XY07F5SC4777) |

Rust integration tests run the real CLI on a disposable project whose schema declares types, fields, values, statuses and relations of its own, and none of the `engineering` template's.

| Case | Expectation |
| --- | --- |
| Items that use only the declared vocabulary | Valid |
| An item of type `Requirement`; a row keyed by an `engineering` field or relation; a field value or `Status` outside the declared lists | An error each |
| An item of a type without fields, with only `Type`, `ID` and `Status` rows | Valid |
| A schema edit that adds the rejected field value to its list | The same document becomes valid |

---

<a id="01M7D3NARGT43PEVX6SWPN60CZ"></a>

### Report every seeded problem

| Type | Verification |
| --- | --- |
| ID | VER-SEEDED-PROBLEMS |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-REPORT-EVERY-PROBLEM](validation.mara.md#01M7BF6HEVWZ2490K493GDP8KC) |
| Verifies | [REQ-VALIDITY-COVERS-WHOLE-PROJECT](validation.mara.md#01M7BF6HF8R0Q5BK349ESZ2PS7) |

Rust integration tests run the real CLI on a disposable project seeded, several to a file, with one instance of each failure the built-in checks define ([DES-SCHEMA-CHECKS](validation.mara.md#01M7BF6HG7AQZ9FM38CMT4BTHR)), an unknown configuration setting and an invalid schema declaration.

| Case | Expectation |
| --- | --- |
| `project validate` | Each seeded problem exactly once, with its code, severity, path and line, and nothing else |
| `schema validate` | The schema's problem, and no document's |
| `item validate` of an item whose ID another file repeats and whose relation link has a stale path | Both problems, and none of another item |
| `project validate` with a page of one diagnostic, or filtered to a path without errors | Invalid, with the project's total error count and a non-zero exit status |
| The same project with every problem removed | Valid, no diagnostic and exit status 0 |

---

<a id="01M7D3NARPYEP9XHY86H015Y4Z"></a>

### Read an item with invalid rows

| Type | Verification |
| --- | --- |
| ID | VER-INVALID-ROWS |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-INVALID-ROWS-REPORTED](validation.mara.md#01M7BF6HFZVTK5PC7W60FPND94) |

Rust integration tests run the real CLI on a disposable project with an item that has an undeclared key, a value outside its field's allowed set and a row that repeats another.

| Case | Expectation |
| --- | --- |
| `project validate` | One diagnostic for each of the three rows |
| `get` of the item | The item with all its rows in source order, the invalid ones included |

---

<a id="01M7D3NARW1657P2G2P9BWXSWR"></a>

### Run every read on a broken project

| Type | Verification |
| --- | --- |
| ID | VER-UNCHANGED-SOURCES |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-READS-NEVER-WRITE](validation.mara.md#01M7BF6HFM0MQ2NH7RA7S00B8K) |

Rust integration tests run every validation and read operation of the CLI, failing requests included, on a disposable project with a missing anchor, a bare-ID relation value, stale link paths and text, invalid rows and CRLF line endings. After each run, no file in the project tree has been created, deleted or modified: names, bytes and mtimes are unchanged.
