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

- Chosen: one row on the source item, under the forward relation name, holding a Markdown link.
- Rejected: `[[ID]]` and `[[relation:target]]` syntax. It is not Markdown and does not navigate on GitHub.
- Rejected: rows under the inverse name on the target item. One edge would have two spellings.

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
