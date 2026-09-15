# Traceability research

Reviewed official documentation and selected repository sources on 2026-09-13.
This was a documentation/source comparison, not an installation benchmark or
verification of each released build. Upstream `main` and documentation may
describe work newer than a published package. Accepted Mara obligations belong
in [traceability](traceability.mara.md), not in this comparison. The separately
dated [[EVD-SHACL-CEL-SPIKE]] records an executed language integration experiment.

## Meaning and limits

[NASA requirements management](https://www.nasa.gov/reference/6-2-requirements-management/)
describes associations among requirements, system elements and verification,
with navigation back to origins and forward to allocated work. It also calls
for assessing whether linked work actually fulfills its parent. Automated link
checks alone do not establish that semantic judgment.

For Mara, distinguish origin, realization, verification, declared coverage, and
freshness. A matrix presents selected relationships. A coverage result checks
declared expectations. A verification definition describes a method; a result
records an execution. A changed target creates a reason to review its links,
not proof that they are wrong.

## Implementations and useful distinctions

| System and primary source | Observed mechanism | Implication for Mara |
|---|---|---|
| [StrictDoc user guide](https://github.com/strictdoc-project/strictdoc/blob/main/docs/strictdoc_01_user_guide.sdoc) | Configurable node grammar; Parent/Child roles and reverse display labels; code markers or requirement-side file references; language-aware parsing for supported languages. Test-report imports connect requirements, tests and results; the guide still labels JUnit integration experimental. | Reverse display names do not by themselves provide inverse authoring. Code links and execution evidence need distinct meanings. |
| [Doorstop item model](https://doorstop.readthedocs.io/en/latest/reference/item.html) and [validation](https://doorstop.readthedocs.io/en/stable/cli/validation.html) | YAML items link up a document hierarchy. Fingerprints track reviewed item content and parent content acknowledged by a link. Parent changes mark links suspect. | Item review and relationship review are separate; evaluate fingerprints for 0.4. |
| [Sphinx-Needs schema validation](https://sphinx-needs.readthedocs.io/en/stable/schema/) | Select items, check local properties and resolved incoming/outgoing targets, apply counts and nested checks, and report severities. Counting qualifying targets differs from counting raw links; requiring some targets differs from requiring all. | Reference for declarative current-state rules; do not copy the complete JSON Schema vocabulary merely for flexibility. |
| [Sphinx-Needs external needs](https://sphinx-needs.readthedocs.io/en/stable/configuration.html#needs-external-needs) | Load external item data from JSON and use it for linking and evaluation. | Imported external state has a larger contract than an opaque address and needs provenance/freshness decisions. |
| [OpenFastTrace user guide](https://github.com/itsallcode/openfasttrace/blob/main/doc/user_guide/user_guide.md) | `Needs` declares expected artifact types; `Covers` provides coverage. Deep coverage requires covering artifacts to have their own coverage. Explicit revisions invalidate outdated links. | Coverage follows declared expectations; assess revision-based freshness alongside automatic fingerprints in 0.4. |
| [ReqView traceability](https://www.reqview.com/doc/requirements-traceability-links/) and [change management](https://www.reqview.com/doc/change-management/) | Link types have forward/reverse labels. Linking can begin from either endpoint. Selected target-attribute changes mark links suspect, requiring review and acknowledgement. | Natural endpoint-oriented authoring and explicit change review are useful precedents. |

## Design consequences

Use explicit relationship paths and qualifying targets to explain failures.
Avoid interpreting arbitrary reachability as coverage or a linked test as a
passing result. Normalize alternate authoring into one semantic relationship
while retaining its source occurrences. Keep external addresses distinct from
imported items. Carry current-state policy into 0.3 and baseline comparison,
suspect-link review and lifecycle transitions into 0.4.

These are design conclusions from the comparison, not claims that an upstream
tool implements Mara's intended alias, symmetry or mutation semantics. No
upstream rule language, graph persistence layer, or lifecycle is adopted here.

:::mara evidence EVD-SHACL-CEL-SPIKE
:mid: 01M2JZNCW49RH3Y4M99DEZGC0D
:title: SHACL and CEL integration experiment

Executed on 2026-09-15 outside the product repository, using Mara 0.2.0,
Python 3.14.7, pySHACL 0.40.1, RDFLib 7.6.0 and Rust crate cel 0.14.5.
Eight Python unittest tests passed. This establishes reference-engine
feasibility for [[DES-TRACE-RULE-GRAMMAR]], not a production backend choice
or completion of [[VER-TRACEABILITY-WORKFLOW]].

## Bridge and observed results

The real Mara CLI created and validated a four-item engineering fixture.
Items became RDF nodes keyed by generated MIDs. Canonical relationships became
RDF triples; a separate map retained their owning item source locations.
SHACL inverse paths selected verifications and evidence. A SHACL-SPARQL
constraint called a registered RDFLib function, which evaluated a local CEL
expression through a Rust subprocess. The expression was
`has(node.status) && node.status == "approved"`.

| Check | Observed result |
|---|---|
| Qualified minimum of one; only draft verification linked | Complete failure, mapped to the requirement's actual source and coverage shape. |
| Draft and approved verifications linked | Pass. |
| Insert the same projected RDF edge again; require two qualifying targets | Failure; duplicate assertion did not increase the count. |
| Every target must qualify; one draft target | Failure. |
| Empty target set with every | Pass without a minimum; failure with minimum one. |
| Qualifying verification needs passed evidence on a second inverse hop | Failure without evidence; pass with evidence. |
| Division by zero in one CEL target while another qualifies | Raw SHACL conformance was true; the adapter's separate error ledger returned invalid and incomplete. |
| Invalid CEL syntax, non-Boolean result, unguarded absent field, or exhausted callback budget | Invalid and incomplete. |

The fixture's Markdown hash was unchanged by validation. Source mapping was
verified at item granularity; exact relationship occurrence spans, inverse
authoring and inline relationship parsing were not exercised.

## Integration limits

The error test deliberately returned false to SHACL after recording a CEL
error. That false could otherwise be treated as a nonqualifying target and
hidden by another qualifying target. Preserve evaluation failures separately
from ordinary predicate failures; do not infer Mara validity from the SHACL
conformance flag alone. The callback budget test counts invocations only: it
does not prove the accepted graph/CEL work budget, cancellation or memory bounds.

A separate native Rust probe using shacl_validation 0.2.12 and shacl_rdf 0.2.9
failed before execution. Fresh dependency resolution mixed incompatible
`iri_s::IriS` and `rudof_iri::IriS` types. Pinning rudof_rdf, prefixmap and
sparql_service to 0.2.9 still failed through mie 0.2.20. No native SHACL runtime
result or Rust-only integration is claimed.

## Consequence for the design

Combining SHACL relationship shapes with CEL local predicates is feasible.
The tested custom-function bridge requires a host extension; it is not a
portable standard SHACL-CEL language. Before replacing the current grammar,
define the persisted syntax, typed field bindings and missing-value behavior,
and demonstrate a compatible Rust engine with enforceable work limits.
CEL Boolean error masking and SHACL severity/conformance also need explicit
mapping to [[DES-TRACE-DIAGNOSTIC-INTERFACE]]. This experiment does not
supersede [[ADR-DECLARATIVE-TRACE-BASELINE]] or add product dependencies.

Primary references: [SHACL](https://www.w3.org/TR/shacl/),
[CEL language definition](https://github.com/cel-expr/cel-spec/blob/master/doc/langdef.md),
[pySHACL](https://github.com/RDFLib/pySHACL),
[cel-rust](https://github.com/cel-rust/cel-rust) and
[shacl_validation 0.2.12](https://crates.io/crates/shacl_validation/0.2.12).
:::
