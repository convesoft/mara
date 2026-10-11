# Engineering workflow

Initialize a new project with `mara project init --template engineering`.
For the previous profile, use [engineering-legacy](engineering-legacy.mara.md).
[The profile contract](engineering-template.mara.md) defines acceptance, risk
policy and installed-runtime limits. There is no mandatory chain of item types
or fixed sequence of phases.

## Author proportionately

Inspect `mara schema get` before choosing flavours, fields or relations.

- Keep stakeholder outcomes in goals/needs and operational context in scenarios.
  Actors belong in prose, not a separate flavour.
- Give each obligation an actual basis. Prepare requirement and verification
  drafts together: acceptance requires accepted basis and incoming verification.
- Define a design only where durable realization meaning is useful. A design
  may satisfy a requirement, decompose a design or contribute directly to a goal.
- Preserve consequential choices and their authority in decisions. Record risk
  assessment and threatened knowledge; mitigation and authorized acceptance use
  relations, not a treatment/status field.
- Separate verification definitions from validation definitions. Keep routine
  execution records with their providers. Evidence items preserve significant
  durable conclusions with conditions and provenance in Markdown.

Run `mara schema validate` and `mara project validate`; consume every page.
A complete policy pass does not establish semantic adequacy, implementation or
successful execution. Resolve material concerns before the requested commitment.

## Query facts and explain policy

The installed `.mara/engineering-queries.json` contains the domain catalog:

| Projection | Explicit roots / context |
| --- | --- |
| requirement_basis | requirements |
| design_basis | designs |
| term_collisions | terms |
| relation_lifecycle | selected knowledge |
| implemented | requirements/designs/verifications; native code inputs, unavailable on document-only hosts |
| risk_level | risks; risk_matrix mapping |
| risk_scope | selected requirements |
| relevant_risks | selected requirements |
| verification_status | requirements; candidate and verification_results |
| validation_status | needs/goals; context and validation_results |

Basis candidates and relevant risks retain draft/retired status for explanation.
Caller-selected workload is never silently expanded into siblings.

Example IDs below are illustrative. Select actual project IDs:

```sh
mara query execute requirement_basis --id REQ-EXPORT \
  --definition-file .mara/engineering-queries.json --max-rows 10000

mara query gate implementation_ready --id REQ-EXPORT \
  --definition-file .mara/engineering-queries.json \
  --gate-file .mara/engineering-gates.yaml \
  --shape-file .mara/engineering-shapes.ttl \
  --mapping-file .mara/engineering-risk-matrix.json --max-rows 10000
```

The risk mapping is an explicit input. An absent matrix, missing assessment or
missing cell cannot produce a safe fallback. The default is template-tailored,
not an unmodified NASA or organization-wide policy.

Use `query report rules` for enabled-policy explanations. For gate explanations,
substitute `query report gate` for `query gate` with the same inputs.

## Supply applicable execution facts

The host supplies one reconciled result per check/candidate or check/context key.
The provider must compare the actual candidate/context and current definition
revision before asserting applicable. Mara does not run checks, fetch records or
authenticate provider statements.

A verification fact file has this shape (substitute actual identities):

```json
{
  "input": "verification_results",
  "identity": "provider-snapshot-identity",
  "coverage": [
    {
      "verification": {"type": "item", "value": "VER-EXPORT"},
      "candidate": {"type": "text", "value": "candidate-identity"}
    }
  ],
  "records": [
    {
      "keys": {
        "verification": {"type": "item", "value": "VER-EXPORT"},
        "candidate": {"type": "text", "value": "candidate-identity"}
      },
      "fields": {
        "result": {"type": "text", "value": "PASS"},
        "definition_revision": {"type": "text", "value": "checked-definition-identity"},
        "applicable": {"type": "boolean", "value": true}
      }
    }
  ]
}
```

Results are PASS, FAIL, INCONCLUSIVE or ERROR. Within explicit complete coverage,
no record means PENDING. Absent/partial/out-of-coverage inputs are unavailable.
For validation, use input validation_results and keys validation/context.
validation_status retains each linked definition and reports an accepted-definition
summary and count. Only an applicable PASS for every accepted definition supports
validated; no accepted definitions reports PENDING. Mixed nonpassing results use
FAIL, ERROR, INCONCLUSIVE, PENDING precedence while preserving individual records.

```sh
mara query execute verification_status --id REQ-EXPORT \
  --definition-file .mara/engineering-queries.json \
  --param 'candidate={"type":"text","value":"candidate-identity"}' \
  --fact-file verification-results.json --max-rows 10000
```

The verified gate requires native implementation coverage and an applicable PASS
for every accepted verification. Native hosts reuse project-configured code
indexing and resolved file links; code failures are unavailable, not empty
successful coverage. The same implementation condition applies to release_ready.
Document-only/WASM hosts still report required native code facts unavailable. Evaluate each
explicitly required pre-release validation with required_validation, selecting
its validation ID, supplying its context and validation fact file. Combine all
required outcomes externally; no required-validation list does not claim that
needs or goals are validated. Readiness is not deployment authorization.

## Factual views

Load both definition files for the seven `view_...` compositions:

```sh
mara query view view_engineering_basis_and_rationale --id REQ-EXPORT \
  --definition-file .mara/engineering-queries.json \
  --definition-file .mara/engineering-views.json --max-rows 10000
```

The catalog includes delivery_readiness,
requirement_implementation_and_verification, outcome_coverage_and_validation,
risk_relevance_and_treatment, engineering_basis_and_rationale, knowledge_validity
and change_impact. Prefix each with `view_`. Supply candidate/context, fact
snapshots and the risk mapping when the selected view needs them.

Views retain absent facts and selected roots; they are not acceptance or readiness
verdicts. Implementation-dependent views require native code inputs and remain
incomplete on document-only hosts. Change impact is
direct adjacency for on-demand exploration, not a complete transitive impact
analysis. Use query reports for actual configured policy outcomes and ordinary
navigation for further exploration. Stop at the user's authorized commitment.
