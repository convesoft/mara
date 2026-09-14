# Traceability research for 0.3

Reviewed official documentation and selected repository sources on 2026-09-13.
This was a documentation/source comparison, not an installation benchmark or
verification of each released build. Upstream `main` and documentation may
describe work newer than a published package. Accepted Mara obligations belong
in [traceability](traceability.mara.md), not in this comparison.

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
