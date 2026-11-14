# Decisions

---

<a id="01M7BF6GTYJB4MCBXQSHWFXMYZ"></a>

### Write items as ordinary Markdown

| Type | Decision |
| --- | --- |
| ID | ADR-MARKDOWN-NATIVE-ITEMS |
| Status | Accepted |
| Justifies | [DES-ITEM-STRUCTURE](format.mara.md#01M7BF6GZDTNNW0Z0MX7FEG6ZF) |

- Chosen: an item is an HTML anchor, an ATX heading and a GFM table.
- Rejected: `:::mara` directive blocks. GitHub shows them as raw text, without highlighting, rendering or link navigation.

---

<a id="01M7BF6GVAEPTJR91QAR6QRW24"></a>

### Store the MID in an uppercase HTML anchor

| Type | Decision |
| --- | --- |
| ID | ADR-MID-IN-HTML-ANCHOR |
| Status | Accepted |
| Justifies | [DES-MID-ANCHOR](identity.mara.md#01M7BF6H1MB5RR3RS43B3CJHAR) |

- Chosen: `<a id="<MID>"></a>` is the only place a MID is stored; it is the identity and the link target at once.
- Rejected: heading slugs as link targets. They change with the title.
- Rejected: lowercase MIDs in anchors and links. GitHub already matches fragments case-insensitively ([EVD-GITHUB-FRAGMENT-MATCHING](identity.mara.md#01M7BF6H2MHAVK1Z5HCXR1V289)).

---

<a id="01M7BF6GVK3EZ2G1CYXSEB9XC3"></a>

### Author typed relations only in the source item's table

| Type | Decision |
| --- | --- |
| ID | ADR-RELATIONS-ONLY-IN-TABLE |
| Status | Accepted |
| Justifies | [DES-RELATION-LINKS](identity.mara.md#01M7BF6H29JGHG65SV91K6D11V) |
| Justifies | [REQ-EDIT-RELATIONS](authoring.mara.md#01M7BF6GN8DJ66NPBMQK83H9PW) |

- Chosen: one row on the source item, under the forward relation name, holding a Markdown link.
- Rejected: `[[ID]]` and `[[relation:target]]` syntax. It is not Markdown and does not navigate on GitHub.
- Rejected: rows under the inverse name on the target item. One edge would have two spellings.
- Rejected: an inverse name in `relation add`, written as the forward row on the other item. The operation would edit an item it was not pointed at.

---

<a id="01M7BF6GW5PH28XAS4VF6EJF8G"></a>

### Order multi-file writes instead of journaling them

| Type | Decision |
| --- | --- |
| ID | ADR-NO-MUTATION-JOURNAL |
| Status | Accepted |
| Justifies | [DES-WRITE-PROCEDURE](writes.mara.md#01M7BF6HJA1E3SE8WJH77FGP17) |

- Chosen: order the writes so that an interruption leaves only states validation reports. The MID in each link fragment keeps identity intact meanwhile.
- Rejected: a journal of preimages with an explicit rollback command, as in 0.3.0. It adds transaction state that blocks writers, and recovery code.
- Rejected: fix removing one copy after an interrupted move. Fix would delete an item and guess between the copies.

---

<a id="01M7CMW6Y0WFD4W6951906MXQY"></a>

### Keep configuration and schema as TOML files under `.mara/`

| Type | Decision |
| --- | --- |
| ID | ADR-TOML-PROJECT-FILES |
| Status | Accepted |
| Justifies | [DES-PROJECT-CONFIGURATION](project.mara.md#01M7BF6H9RDYSET9QTAKJWTX27) |

- Chosen: `.mara/config.toml` and `.mara/schema.toml`, each with its own format version starting at 1.
- Rejected: the 0.3.0 names, `.mara/project.toml` and `.mara/schema.yaml`. YAML needs a second parser, and the format versions would continue at 4.
- Rejected: one `mara.toml` at the project root. A schema edit would change the configuration hash, and `project init` could not write the schema as its own file.

---

<a id="01M7BF6GWEX6QCDC1YGR54P23Z"></a>

### Distribute prebuilt binaries as npm packages

| Type | Decision |
| --- | --- |
| ID | ADR-SCRIPT-FREE-NPM-PACKAGES |
| Status | Accepted |
| Justifies | [DES-PACKAGES](distribution.mara.md#01M7BF6GWZ72PGJNA3VQNZMDC8) |

- Chosen: a dispatcher package with one optional dependency per platform.
- Rejected: an install-time downloader. It depends on lifecycle scripts, which restricted environments disable.

---

<a id="01M7CMW6YNXSQAS7H0ATKG4GH0"></a>

### Keep the 0.3.0 platforms and Node version

| Type | Decision |
| --- | --- |
| ID | ADR-KEEP-PLATFORMS-AND-NODE |
| Status | Accepted |
| Justifies | [DES-PACKAGES](distribution.mara.md#01M7BF6GWZ72PGJNA3VQNZMDC8) |

- Chosen: Linux x64 and arm64 (glibc), macOS arm64 and Node 24, as in 0.3.0.
- Rejected until requested: Windows. It needs a Soufflé MSVC build, a patch for explanations (the explain console throws `No explain/explore provenance on Windows`) and named-pipe IPC for the daemon.
- Rejected until requested: Intel macOS and musl Linux.
- Rejected: Node 22. It reaches end of life in April 2027.

---

<a id="01M7CQ57SQY6M97NEDTA27K5WE"></a>

### Embed Mara in Node through a native addon

| Type | Decision |
| --- | --- |
| ID | ADR-NODE-NATIVE-ADDON |
| Status | Accepted |
| Justifies | [DES-PACKAGES](distribution.mara.md#01M7BF6GWZ72PGJNA3VQNZMDC8) |

- Chosen: an N-API addon over the same Rust host code as the CLI, shipped in the native packages.
- Rejected: the WASM engine with a host written in JavaScript. A second host would have to match the Rust one in file access, hashing, caching and running Soufflé.

---

<a id="01M7CQ57SDRGXWJPC1CAK82Z98"></a>

### Name every operation `<group> <verb>`

| Type | Decision |
| --- | --- |
| ID | ADR-GROUP-VERB-COMMANDS |
| Status | Accepted |
| Justifies | [DES-OPERATIONS](hosts.mara.md#01M7BF6H124JRJAQSYVQ21Q2JS) |

- Chosen: `<group> <verb>` for every operation. Only `search` and `get` stay top-level, because they return items, narrative and code alike.
- Rejected: a top-level `related`, as in 0.3.0. It lists relations from outside the `relation` group.
- Rejected: `relation get`, as in 0.3.0. A relation is one row, which `relation list` already returns with its location.

---

<a id="01M7BGRHX3NVNZS0NGQ0BKPSH1"></a>

### Deliver agent guidance from the binary, not a skill

| Type | Decision |
| --- | --- |
| ID | ADR-GUIDE-IN-BINARY |
| Status | Accepted |
| Justifies | [REQ-AGENT-GUIDE](agents.mara.md#01M7BGRHWB2MGXEY5G41YP8E0G) |

- Chosen: the guide is compiled into the binary and delivered by `mara prime` and the MCP server instructions, as [zban](https://github.com/convesoft/zban) does.
- Rejected: a standalone skill, as in 0.3.0. It was installed separately and had to be matched to the binary's version by hand.

---

<a id="01M7BH0XEYXZXY3ZD8NNYGS0GM"></a>

### Without a project, advertise only initialization

| Type | Decision |
| --- | --- |
| ID | ADR-INIT-NOTE-WITHOUT-PROJECT |
| Status | Accepted |
| Justifies | [DES-AGENT-GUIDE](agents.mara.md#01M7BGRHWQHQJJHC6XCXVDWDM8) |

- Chosen: outside a project, `mara prime` and the MCP instructions say how to initialize one, and nothing more.
- Rejected: the full guide everywhere. It fills the context of projects that do not rely on Mara.
- Rejected: no output at all. Nothing would tell a user or agent how to start.

---

<a id="01M7CMW6YBVK2FYK2AQA7XAT6Y"></a>

### Keep the agent guide independent of the project's schema

| Type | Decision |
| --- | --- |
| ID | ADR-STATIC-AGENT-GUIDE |
| Status | Accepted |
| Justifies | [DES-AGENT-GUIDE](agents.mara.md#01M7BGRHWQHQJJHC6XCXVDWDM8) |

- Chosen: one guide per binary, which sends the agent to schema inspection.
- Rejected: listing the project's types and ID prefixes. MCP instructions are sent once per connection, so the list goes stale after a schema edit, and a large schema exceeds the size limit.
