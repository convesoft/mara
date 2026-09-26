# Product intent and workflows

Mara keeps project knowledge in readable, Git-tracked Markdown and exposes
structured operations through CLI and MCP. These retained goals and scenarios
keep their baseline IDs and MIDs. Accepted knowledge states intended behavior;
implementation and execution evidence are assessed separately.

:::mara goal GOAL-UNIFIED-PROJECT-KNOWLEDGE
:mid: 01M1PXP2KGBN9S4G5PC0SAE5P6
:title: Keep project knowledge in one structured source of truth
:status: accepted

Mara combines ordinary Markdown narrative with identifiable, typed, related,
and validated items. Requirements, design, decisions, and supporting knowledge
remain readable files rather than parallel documentation systems.
:::

:::mara goal GOAL-BOUNDED-AGENT-CONTEXT
:mid: 01M1PXP2KGMDJHQGF2MCTP7TED
:title: Let agents retrieve relevant knowledge without loading the full corpus
:status: accepted

Agents discover, search, inspect, and traverse project knowledge incrementally.
Derived query results never become a second authoring authority.
:::

:::mara scenario SCN-START-STRUCTURED-PROJECT
:mid: 01M1PXP2KGMG0M2HVW68FS0EJW
:title: Start a project with structured documentation
:status: accepted
:contributes_to: GOAL-UNIFIED-PROJECT-KNOWLEDGE

The user initializes Mara in a new project, inspects its effective schema,
creates and relates requirements, validates the corpus, and retrieves the same
knowledge while implementing the project. This advances
[[GOAL-UNIFIED-PROJECT-KNOWLEDGE]].
:::

:::mara scenario SCN-AUTHOR-ITEM-FLEXIBLY
:mid: 01M1PXP2KGMN7M1S7ND2AGM8ZK
:title: Author an item through the most convenient path
:status: accepted
:contributes_to: GOAL-UNIFIED-PROJECT-KNOWLEDGE

An author creates a complete item through Mara, creates a scaffold and fills its
body manually, or writes an item directly and validates it. Tool-authored and
manually authored items follow the same schema and source format.
:::

:::mara scenario SCN-RETRIEVE-BOUNDED-KNOWLEDGE
:mid: 01M1PXP2KGTZWMSGFSKY6CCKW4
:title: Retrieve bounded project knowledge
:status: accepted
:contributes_to: GOAL-BOUNDED-AGENT-CONTEXT

An author or agent searches with filters, fetches a selected item, and inspects
compact related-item summaries before retrieving any additional full bodies.
This advances [[GOAL-BOUNDED-AGENT-CONTEXT]].
:::

:::mara scenario SCN-ONBOARD-MARA-AGENT
:mid: 01M1PXP2KGEBFRGNFRZ89V2JMS
:title: Give an agent access to Mara project knowledge
:status: accepted
:contributes_to: GOAL-BOUNDED-AGENT-CONTEXT

A user connects an installed Mara executable to the agent as an MCP server and
installs its skill separately. A compatible client may instead install the
optional complete Agent Plugin. The agent initializes an explicit project when
needed, inspects its schema, and performs bounded operations against one
selected project through MCP. This advances [[GOAL-BOUNDED-AGENT-CONTEXT]].
:::

:::mara scenario SCN-START-ENGINEERING-KNOWLEDGE
:mid: 01M1XSJZVEZKVRBMCWK9GFQ3XC
:title: Start a project with an engineering vocabulary
:status: accepted
:contributes_to: GOAL-UNIFIED-PROJECT-KNOWLEDGE

An author selects an engineering template, initializes a project, and creates
requirements, designs, and verification knowledge using the supplied vocabulary.
The resulting schema is ordinary project-owned data. No Mara product documents,
existing item identities, or placeholder trace chains are copied into the project.
:::

:::mara scenario SCN-CHOOSE-KNOWLEDGE-FLAVOUR
:mid: 01M1XSJZVN9VWH6TP58DX6WH54
:title: Choose a flavour from project guidance
:status: accepted
:contributes_to: GOAL-UNIFIED-PROJECT-KNOWLEDGE

An author or agent inspects the selected project's schema guidance before
creating an item. The guidance explains each flavour's purpose, when to use or
avoid it, and how it differs from confusable flavours. The author can select a
flavour without relying on knowledge of Mara's own repository taxonomy.
:::

:::mara scenario SCN-READ-DOCUMENT-CONTEXT
:mid: 01M1XSJZVV5V98210YBNJ3XYY4
:title: Discover narrative and follow its connections through Mara
:status: accepted
:contributes_to: GOAL-BOUNDED-AGENT-CONTEXT

An author or agent searches the selected project's canonical documentation and finds an item, section, or ordinary Markdown block, including content in documents without items. From a narrative mention, the reader inspects the referenced requirement and follows its direct connections to designs or decisions, choosing each subsequent step.

Search and neighbour results expose source locations and connection meaning. The reader retrieves each selected node through Mara, continuing bounded reads until the needed content is complete. CLI and MCP expose equivalent behavior without requiring client filesystem access or turning narrative into items.
:::

:::mara scenario SCN-AUTHOR-TRACE-CONNECTION
:mid: 01M2FX4BQMS5A6W5KM4T0WJ7MD
:title: Author one relationship from either endpoint
:status: accepted
:contributes_to: GOAL-UNIFIED-PROJECT-KNOWLEDGE

An author connects a verification to a requirement from either item's context,
using the relation name appropriate to that endpoint. Another author expresses
the same relationship in supported inline syntax. Navigation shows one semantic
connection and lets the reader inspect each authored occurrence. Symmetric
associations can be authored from either endpoint without inventing an upstream
or downstream meaning.
:::

:::mara scenario SCN-CHECK-TRACE-OBLIGATIONS
:mid: 01M2FX4BQV6MJZRBSAGGEB0AP6
:title: Find unmet project obligations
:status: accepted
:contributes_to: GOAL-UNIFIED-PROJECT-KNOWLEDGE

A project owner requires approved requirements to have an approved verification,
accepted designs to reference a requirement, and mitigated risks to have incoming
mitigation. An author runs validation after ordinary Markdown edits. Mara names
the applicable rule, affected item and unmet condition, distinguishing a missing
link from linked items that do not qualify. A declared chain can reveal a gap
beyond the first hop. Fixing the actual knowledge makes the corresponding check
pass; unrelated links cannot conceal the gap.
:::

:::mara scenario SCN-READ-TRACE-VIEW
:mid: 01M2FX4BR2F15YVXCD89RTGDKQ
:title: Inspect trace coverage in a matrix
:status: accepted
:contributes_to: GOAL-UNIFIED-PROJECT-KNOWLEDGE

An author selects root items and a declared rule or request-local check for a
traceability matrix. A reader can inspect coverage states, relationship paths,
source evidence and unmet obligations. A requirement linked to an external
delivery ticket remains distinguishable from one with verification evidence.
Regenerating the matrix after source changes reflects current knowledge without
making its output another authoring authority.
:::

:::mara scenario SCN-INSTALL-DISTRIBUTED-MARA
:mid: 01M1PXP2KG35JD2VV6SBSXPDQW
:title: Run Mara without a Rust toolchain
:status: accepted
:contributes_to: GOAL-UNIFIED-PROJECT-KNOWLEDGE

On a supported host, a user runs an exact `@convesoft/mara` version through
`npx`. The package runs the native Mara binary with inherited standard streams,
so the same command serves the CLI and long-running stdio MCP workflows. This
advances [[GOAL-UNIFIED-PROJECT-KNOWLEDGE]] and
[[GOAL-BOUNDED-AGENT-CONTEXT]].
:::

:::mara scenario SCN-EDIT-CONNECTED-KNOWLEDGE
:mid: 01M3FVHD0805AQM358NW6GC5A1
:title: Revise connected knowledge without losing identity
:status: accepted
:contributes_to: GOAL-UNIFIED-PROJECT-KNOWLEDGE

An author updates an item's body or fields, renames its human-readable ID, or moves it to another selected document. The MID remains unchanged, and surviving references continue to identify their intended targets. A rejected edit leaves source data unchanged. When deleting an item, the author resolves reported incoming references before retrying; successful deletion removes only the selected item.
:::

:::mara scenario SCN-RECOVER-INTERRUPTED-EDIT
:mid: 01M3FVHH34M7R06DHSR1SMXY48
:title: Recover an interrupted multi-file edit
:status: accepted
:contributes_to: GOAL-UNIFIED-PROJECT-KNOWLEDGE

A move or rename is interrupted after a recoverable transaction starts. Mara blocks further content mutations and identifies the pending state while reads and validation remain available. The author requests explicit rollback, restoring the original sources. If later manual changes conflict with recovery, Mara reports the conflict without overwriting those changes.
:::

:::mara scenario SCN-TRACE-IMPLEMENTATION-AND-CHECKS
:mid: 01M3FVHM32VDHD2XFC6PBK3MW9
:title: Connect knowledge to real implementation and checks
:status: accepted
:contributes_to: GOAL-UNIFIED-PROJECT-KNOWLEDGE

An author declares code relations and configures language adapters when symbol resolution is needed, then associates a source file or declaration with a requirement, design, or verification method through a code comment or item-authored inverse link. Navigation resolves the source location; validation identifies missing, ambiguous, or unsupported targets. A coverage matrix distinguishes implementation and check definitions from separately recorded execution evidence.
:::
