# Engineering templates

`engineering` starts the current model. `engineering-legacy` preserves the
previous profile. Neither choice migrates an existing project. See the
[workflow](engineering-workflow.mara.md) and [legacy usage](engineering-legacy.mara.md).

:::mara design DES-ENGINEERING-PROFILE
:mid: 01M3HJFYSTNV84CRRZF0YPXY76
:title: Separate engineering knowledge, facts and contextual readiness
:status: accepted
:kind: data
:satisfies: REQ-ENGINEERING-TEMPLATE

The user selected the Engineering process approach supplied in `.temp` as the basis for a new template, while retaining the prior template as engineering-legacy. The initial existing-engine-only delivery was followed by the user's authorization of a bounded native code-fact bridge to unblock implementation-dependent gates. Other unsupported evaluations remain bounded below. It does not remove trace matrix, install the controller, migrate existing projects or change the self-hosted ontology. Installation follows [[REQ-ENGINEERING-TEMPLATE]].

The [schema](../templates/engineering-schema.yaml) owns eleven flavours, fields, guidance and relation declarations. Need and validation are distinct flavours; actor and artifact are absent. Every item requires draft, accepted or retired knowledge status and substantive Markdown. Only risk likelihood/consequence, term aliases and verification/validation methods add domain fields. Routine runs remain external; evidence records durable conclusions, not CI executions.

The [structural rules](../templates/engineering-rules.yaml) refine satisfies to requirement→need and design→requirement, since schema endpoint lists alone admit their cross-product. Accepted verifications/validations need methods and accepted targets; accepted risks need assessment dimensions and an accepted threatened target. The [query policies](../templates/engineering-policy.yaml) and [SHACL shapes](../templates/engineering-shapes.ttl) require an accepted requirement basis and incoming accepted verification, an accepted design basis, accepted-term collision freedom and relation lifecycle compatibility. Goals, needs, scenarios, decisions and evidence need no mandatory graph coverage. Acceptance is knowledge authority, not implementation, execution success or semantic adequacy.

Strict relations from accepted sources target accepted knowledge. Justifies and supports retain developmental and historical targets. Supersession is same-flavour and acyclic; an accepted replacement targets retired knowledge. Historical retired-to-retired links remain valid after later replacements, as selected in [[ADR-ENGINEERING-HISTORICAL-SUPERSESSION]]. This checks present lineage, not historical event authority; supersession never changes statuses automatically. Risk and evidence cannot supersede.

The [query catalog](../templates/engineering-queries.json) supplies the ten domain projections and policy/gate helpers. Basis and relevant-risk queries retain nonqualifying statuses. Risk scope follows only outgoing requirement decomposition/derivation ancestry, then satisfied needs and contributed-to goals; it never adds siblings to workload. Returned ancestor, target and relation support explain context; the existing engine does not reconstruct every transitive derivation path.

The [gates](../templates/engineering-gates.yaml) separate implementation_ready, verified, release_ready and explicitly required validation. Empty or wrong-flavour scope cannot pass. HIGH accepted relevant risk blocks unless an accepted decision accepts it; mitigation does not lower the current assessment. Every accepted verification must have an applicable PASS, not just one. Providers establish applicability against the requested candidate/context and current definition revision; the engine does not authenticate that claim. Required pre-release validations are selected externally and evaluated individually in their own contexts, then ANDed with the requirement gate. An empty required-validation list adds no validation obligation and does not claim outcome validation or deployment authority.

The [factual view definitions](../templates/engineering-views.json) supply the seven proposed presentation compositions. They retain roots with absent facts. Knowledge validity and delivery readiness tables are factual explanations, not computed policy verdicts; use query reports for actual rule/gate outcomes. Change impact is deliberately direct/on-demand navigation, not automatic workload expansion. Guarded view-input definitions accommodate the existing engine's same-root dependency signatures; they do not widen engineering scope.

[[implemented_by:code:templates/engineering-schema.yaml]] [[implemented_by:code:templates/engineering-rules.yaml]] [[implemented_by:code:templates/engineering-policy.yaml]] [[implemented_by:code:templates/engineering-queries.json]] [[implemented_by:code:templates/engineering-shapes.ttl]] [[implemented_by:code:templates/engineering-gates.yaml]] [[implemented_by:code:templates/engineering-risk-matrix.json]] [[implemented_by:code:templates/engineering-views.json]]
:::

## Installed-runtime limits

The user authorized only the native code-fact bridge beyond the initial existing
engine. These limits do not claim the complete replacement architecture:

- Native queries, gates, policies and views reuse the existing code index and
  canonical code relationships when a reachable definition requires code.
  Complete coverage with no implementation links is known absence; indexing or
  referenced-input failures remain unavailable. Code snapshots and referenced
  file content bind identity and continuation. This establishes declared code
  coverage, not code correctness. Document-only/WASM hosts still cannot supply
  native code facts; their implementation-dependent evaluations remain unavailable.
- Fact inputs support one selected record per key, not arbitrary provider
  enumeration. Verification and validation status expose each linked definition
  with the supplied result, definition revision and applicability. Providers must
  reconcile multiple runs and establish applicability before supplying that
  record; conflicting histories and expected revisions are not independently
  reconciled by this template. No results means PENDING only within explicit
  complete coverage; absent/out-of-coverage snapshots are unavailable.
- Release validations with different contexts require separate gate requests.
  The existing gate engine cannot consume a per-release list of context-bound
  validations in one call. `release_ready` covers the requirement-side conditions;
  it is never the complete release claim without the externally selected checks.
- Exact NFC/default Unicode casefold detects collisions between accepted terms
  and aliases, and aliases repeating their own title. RDF set semantics do not
  detect duplicate repeated aliases within one item; authors must keep aliases
  unique. Semantic review remains necessary for definitions, authority and body
  quality. Validation status retains every definition and result alongside an
  accepted-definition summary: FAIL, ERROR, INCONCLUSIVE, PENDING, then PASS in
  descending precedence. PASS requires every accepted validation to be applicable
  and passing; no accepted definitions yields PENDING, not validated. Counts are
  unavailable when evaluation is incomplete.

No unavailable or incomplete result is a fallback pass. None of these checks
runs tests, fetches provider facts, accepts risks, releases or deploys software.

## Tailored default risk policy

[[ADR-ENGINEERING-RISK-MATRIX]] records the user's choice of a NASA-informed
default rather than leaving all 25 cells unconfigured. [NASA/SP-2011-3422, §4.3.2.6, pp. 143–145](https://ntrs.nasa.gov/api/citations/20120000033/downloads/20120000033.pdf)
describes matrix limitations and context-dependent tailoring. This mapping is
Mara's chosen default, **not a verbatim NASA matrix or NASA compliance claim**.

Likelihood is assessed over an explicitly described operating context/window:
rare (exceptional), unlikely (not expected), possible (credible), likely
(expected), almost-certain (near certainty). Consequence describes impact on the
threatened outcome: minimal (negligible), minor (limited reversible disruption),
moderate (material degradation), major (primary outcome blocked), severe
(irreversible or catastrophic harm). These are qualitative interpretations, not
universal probability, cost or safety thresholds. Record assumptions in the risk
body and tailor the policy for the project before treating it as organizational
risk authority.

| Likelihood \ Consequence | minimal | minor | moderate | major | severe |
| --- | --- | --- | --- | --- | --- |
| rare | LOW | LOW | LOW | MEDIUM | MEDIUM |
| unlikely | LOW | LOW | MEDIUM | MEDIUM | HIGH |
| possible | LOW | MEDIUM | MEDIUM | HIGH | HIGH |
| likely | MEDIUM | MEDIUM | HIGH | HIGH | HIGH |
| almost-certain | MEDIUM | HIGH | HIGH | HIGH | HIGH |

[The installed mapping](../templates/engineering-risk-matrix.json) contains all
25 explicit cells and no fallback. Pass it with `--mapping-file`, or supply a
project-tailored replacement using the same typed key names. Missing dimensions,
mapping or cells remain unavailable. Never store derived level or treatment
state on the risk item. Separate risks, uncertainty and mitigation effectiveness
still need engineering judgment; a matrix is not an aggregate risk model.

:::mara decision ADR-ENGINEERING-RISK-MATRIX
:mid: 01M4MZAM8A513PT6QY1MGNKRWZ
:title: Ship a tailored NASA-informed default risk matrix
:status: accepted
:justifies: DES-ENGINEERING-PROFILE

Origin: the user explicitly requested a researched, invented NASA-informed matrix rather than leaving the 25 cells unconfigured.

Ship the explicit default in [engineering-risk-matrix.json](../templates/engineering-risk-matrix.json), with qualitative scale interpretations and project override documented in [the profile](engineering-template.mara.md#tailored-default-risk-policy). This is a Mara-tailored default, not NASA's universal matrix or a compliance claim. NASA/SP-2011-3422 §4.3.2.6 explains context-dependent tailoring and matrix limitations; it does not supply authority for these particular cells.

The alternative was no adopted default, making risk-dependent checks unavailable until callers supplied all cells. The selected default makes initial use practical while preserving explicit inputs: missing mappings, dimensions or cells never imply LOW or pass. Projects must adopt or tailor the mapping for their own risk context. No numerical probability or cost thresholds, automatic mitigation credit or risk acceptance are introduced.
:::

:::mara decision ADR-ENGINEERING-HISTORICAL-SUPERSESSION
:mid: 01M4MZC3F6K90PT1MVDHJKWKN1
:title: Preserve retired historical replacement links
:status: accepted
:justifies: DES-ENGINEERING-PROFILE

Origin: the user selected keeping historical links valid when C replaces B after B replaced A.

An accepted replacement targets retired knowledge. Retired-to-retired supersedes links remain valid history; the replaced event's authority is not invalidated when its source is later retired. Draft links can express proposed replacements. Same-flavour and acyclic lineage remain structural requirements, and statuses are never changed automatically.

Rejecting every retired source would invalidate legitimate B→A history after C→B. The existing source graph has no replacement-event ledger, so validation checks present status compatibility, not past event authorization. Semantic review establishes that the source was authoritative at the replacement event. Do not require a currently accepted successor merely to retain valid retired history.
:::
