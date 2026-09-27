# Engineering profile

Use [the workflow](engineering-workflow.mara.md) to connect intent, implementation
and genuine execution results for a selected change.

:::mara design DES-ENGINEERING-PROFILE
:mid: 01M3HJFYSTNV84CRRZF0YPXY76
:title: Develop engineering knowledge with explicit coverage and execution checks
:status: accepted
:kind: data
:satisfies: REQ-ENGINEERING-TEMPLATE

The optional engineering profile is editable project-owned vocabulary and policy. Initialization assets and persisted formats follow [[REQ-ENGINEERING-TEMPLATE]]. The [bundled schema](../templates/engineering-schema.yaml) owns flavour guidance, fields, exact relation meanings, endpoints and inverse names. The generic engine evaluates these declarations; it does not hardcode engineering flavour or relation names.

Every flavour requires draft, accepted or retired status. Draft knowledge can have incomplete classifications and coverage, but authored fields, references and graph constraints must still be valid. Accepted means an agreed current knowledge record; it does not establish implementation or successful execution. Retired records remain addressable and cannot satisfy accepted-knowledge coverage. Supersession does not change either item's status automatically.

The [accepted-knowledge policies](../templates/engineering-rules.yaml) require classifications and meaningful connections when knowledge becomes accepted: scenarios contribute to goals; requirements identify an origin and a verification method or direct code check; designs satisfy requirements or refine designs; decisions justify knowledge; verifications check requirements/designs or validate goals/scenarios; evidence records a verification; risks identify affected knowledge and their treatment. Acceptance does not require implementation, a dedicated design, or passing execution evidence. Author only meaningful relationships. Use a verification item when the repeatable method needs its own identity; ordinary tests may use checks directly.

The [coverage checks](../templates/engineering-checks.yaml) assess selected accepted roots: intent for requirements, realization and verification for requirements/designs, validation for goals/scenarios. Draft and retired roots fail an explicitly requested accepted-scope check. Related draft/retired knowledge cannot satisfy its accepted-item minimum. Select only the intended scope; these checks are request-local and are not always-on acceptance policy.

The [execution check](../templates/engineering-execution.yaml) selects accepted verification methods with accepted passing evidence whose subject_revision exactly matches the supplied text. Record actual capture time and the checked revision or immutable artifact identity. Policy enforces required nonblank provenance, not timestamp parsing, Git resolution, authenticity or freshness. An accepted failed evidence record is valid knowledge. Implementation links and test definitions never count as execution. Inspect conflicting results, scope and environment alongside the matrix under [[DES-TRACE-CHECK-BINDING]].

The [engineering workflow](engineering-workflow.mara.md) applies these distinctions to one bounded change. [[implemented_by:code:templates/engineering-schema.yaml]] [[implemented_by:code:templates/engineering-rules.yaml]] [[implemented_by:code:templates/engineering-checks.yaml]] [[implemented_by:code:templates/engineering-execution.yaml]]
:::
