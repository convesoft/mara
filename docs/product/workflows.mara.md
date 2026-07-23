# Human and agent workflows

Mara workflows distinguish durable engineering outcomes from temporary delivery
work. A story remains meaningful after implementation issues and pull requests
have closed. External work items may link to Mara items, but they do not own the
semantic content.

## Durable stories

:::story m_01KY7Y9R3JMCNEJ9HMSQ1CRKHD
:id: STORY-AUTHOR-KNOWLEDGE
:title: Author structured knowledge in readable documents
:status: accepted
:priority: must
:involves: ACT-AUTHOR
:derives_from: GOAL-READABLE-SOURCE
:derives_from: GOAL-SCHEMA-GENERIC

As an engineering knowledge author, I want to place typed items inside readable
Markdown documents so that structured requirements do not lose their narrative
context.
:::

:::story m_01KY7Y9R3K9P12NXDMF47DCZD7
:id: STORY-VALIDATE-PROJECT
:title: Validate the complete project before review
:status: accepted
:priority: must
:involves: ACT-AUTHOR
:involves: ACT-CI
:derives_from: GOAL-TRACEABILITY
:derives_from: GOAL-AUDITABLE

As an author or CI service, I want one deterministic check to find syntax,
schema, identity, reference, and traceability problems before changes are
accepted.
:::

:::story m_01KY7Y9R3MS9RBB0AC2RSY563P
:id: STORY-NAVIGATE-TRACE
:title: Navigate from an item to its engineering context
:status: accepted
:priority: must
:involves: ACT-AUTHOR
:involves: ACT-REVIEWER
:derives_from: GOAL-TRACEABILITY

As an author or reviewer, I want to inspect an item and traverse its incoming and
outgoing relations so that I can understand rationale, dependencies,
implementation, and verification.
:::

:::story m_01KY7Y9R3NG7CW8629W6FDPRAD
:id: STORY-RENAME-DISPLAY-ID
:title: Resolve a display-ID collision safely
:status: accepted
:priority: must
:involves: ACT-AUTHOR
:derives_from: GOAL-GIT-CANONICAL
:derives_from: GOAL-READABLE-SOURCE

As an author resolving parallel-branch changes, I want to rename one human-
readable ID and update every internal reference without changing the item's MID
or producing broad formatting changes.
:::

:::story m_01KY7Y9R3P5YZ59K3NPABT70QC
:id: STORY-AGENT-CONTEXT
:title: Give an agent bounded traceable context
:status: accepted
:priority: should
:involves: ACT-AGENT
:derives_from: GOAL-AGENT-READY

As an engineering agent, I want a deterministic context pack for a selected item
or change so that I can act within relevant requirements, designs, decisions,
risks, tests, and vocabulary without reading an entire repository.
:::

:::story m_01KY7Y9R3Q21J51B4E1D878DGP
:id: STORY-LINK-DELIVERY
:title: Link durable semantics to delivery work
:status: accepted
:priority: should
:involves: ACT-INTEGRATION
:derives_from: GOAL-GIT-CANONICAL
:derives_from: GOAL-TRACEABILITY

As an external delivery integration, I want to associate issues and pull
requests with Mara items so that execution can be coordinated without copying
or taking ownership of requirement and design content.
:::

:::story m_01KY7Y9R3R708TEC7395RRKVMN
:id: STORY-WEB-AUTHORING
:title: Browse and edit Mara through a web interface
:status: accepted
:priority: could
:involves: ACT-WEB-USER
:derives_from: GOAL-READABLE-SOURCE
:derives_from: GOAL-GIT-CANONICAL

As a web interface user, I want to browse complete documents and make structured
edits through a branch so that non-CLI users can participate without creating a
second source of truth.
:::

## Scenarios

:::scenario m_01KY7Y9R3STY8G13PTPR5YWW2K
:id: SCN-INITIALIZE-PROJECT
:title: Initialize an empty Mara project
:status: accepted
:kind: user_flow
:involves: ACT-AUTHOR
:derives_from: STORY-AUTHOR-KNOWLEDGE

The author runs `mara init` in a directory. Mara creates strict project and
schema files without inventing business flavours or overwriting existing
content. The author then defines the project taxonomy and begins adding
`.mara.md` documents.
:::

:::scenario m_01KY7Y9R3TXZAV1TF7YMM307PJ
:id: SCN-CHECK-WORKTREE
:title: Check a working tree before review
:status: accepted
:kind: system_flow
:involves: ACT-AUTHOR
:involves: ACT-CI
:derives_from: STORY-VALIDATE-PROJECT

Mara discovers the project, loads the strict configuration and schema, parses
all selected documents, normalizes references and relations, evaluates project
rules, and reports every independently discoverable issue in deterministic
order.
:::

:::scenario m_01KY7Y9R3VGNT4BPR94D2GGV40
:id: SCN-EXPLORE-ITEM
:title: Inspect and trace an item
:status: accepted
:kind: user_flow
:involves: ACT-AUTHOR
:involves: ACT-REVIEWER
:derives_from: STORY-NAVIGATE-TRACE

The user resolves an MID or display ID, views the complete item with provenance,
and traverses incoming, outgoing, or bidirectional edges to a bounded depth.
:::

:::scenario m_01KY7Y9R3WC2GE2BK8Q4D15HBG
:id: SCN-RENAME-DISPLAY-ID
:title: Rename a colliding display ID
:status: accepted
:kind: failure_flow
:involves: ACT-AUTHOR
:derives_from: STORY-RENAME-DISPLAY-ID

Mara verifies the replacement ID, resolves every occurrence of the old display
ID, plans exact source patches, stages a recoverable multi-file transaction, and
applies the rename without changing MID references or unrelated formatting.
:::

:::scenario m_01KY7Y9R3XJ2Z06RJ1AEYYRE1D
:id: SCN-AGENT-CONSUMES-CONTEXT
:title: Agent consumes a deterministic context pack
:status: accepted
:kind: agent_workflow
:involves: ACT-AGENT
:derives_from: STORY-AGENT-CONTEXT

An external agent requests a named context profile for a focus item. Mara emits
the selected narrative and graph neighbourhood with source and Git provenance.
The agent uses that context under the repository's normal review and validation
rules.
:::

:::scenario m_01KY7Y9R3Y6936VD4ETTK03QXN
:id: SCN-LINK-EXTERNAL-WORK
:title: Link an external issue without copying semantics
:status: accepted
:kind: system_flow
:involves: ACT-INTEGRATION
:derives_from: STORY-LINK-DELIVERY

A Mara item records a typed external URI such as `linear://MARA-123`. An adapter
may synchronize delivery state or attach completion provenance, but the item
body and semantic lifecycle remain owned by the Git corpus.
:::

:::scenario m_01KY7Y9R3Z15DYAN8N5H0GVKGH
:id: SCN-WEB-EDIT-BRANCH
:title: Edit an item through an isolated web branch
:status: accepted
:kind: user_flow
:involves: ACT-WEB-USER
:derives_from: STORY-WEB-AUTHORING
:derives_from: GOAL-GIT-CANONICAL
:uses_term: TERM-DERIVED-PROJECTION

The web service opens an isolated Git branch or worktree, applies source-span-
aware patches, validates the result, and creates a commit for review while using
the graph database only as a derived read view.
:::
