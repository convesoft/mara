# Engineering template

Follow the [intended engineering workflow](engineering-workflow.mara.md) from
intent through implementation, candidate assessment and production verification.

:::mara design DES-ENGINEERING-PROFILE
:mid: 01M3FNS3QT63PBXC7HB76A0S4S
:title: Develop engineering knowledge incrementally with explicit coverage checks
:satisfies: REQ-ENGINEERING-TEMPLATE
:satisfies: REQ-ENGINEERING-TRACEABILITY

The optional engineering template installs an editable project profile:
`.mara/schema.yaml`, enabled `.mara/engineering-rules.yaml`, and request-local
`.mara/engineering-checks.yaml`, plus project configuration format 2. The
[template sources](../templates/engineering-schema.yaml) own exact enum values,
endpoints and inverse names; [policies](../templates/engineering-rules.yaml)
and [checks](../templates/engineering-checks.yaml) own executable constraints.
Existing projects require an explicit reviewed migration; initialization never
rewrites them. No starter items, language packs or report evidence are copied.

## Lifecycle and classifications

Keep eleven flavours: term, actor, goal, scenario, requirement, design, decision,
risk, verification, evidence and artifact. Supporting explanation stays narrative.
Delivery tasks and releases stay in the delivery tracker.

Every item has a required `status`: `draft`, `accepted` or `retired`.
Drafts may have incomplete classifications and coverage; their authored fields,
references and graph constraints must still be valid. Accepted means the knowledge
record is agreed and current. It does not mean the described behavior is
implemented or that a verification passed. Retired records remain addressable
history but do not qualify as accepted coverage. Supersession does not silently
change either endpoint's status.

| Flavour | Required when accepted | Optional classification |
|---|---|---|
| requirement | `kind`: functional, quality, constraint | Repeatable `concern` strings, such as security or performance |
| design | `kind`: structure, behavior, interface, data, deployment | Repeatable `concern` strings |
| verification | `method`: test, inspection, analysis, demonstration | `level`: unit, integration, system, acceptance, only with method test |
| evidence | `result`: passed, failed, inconclusive; nonblank `captured_at` and `subject_revision` | External report through `reported_at` |
| risk | `treatment`: open, mitigated, tolerated | `impact` and `likelihood`: low, medium, high |
| Other flavours | No additional fields | None initially |

Record capture time as an ISO 8601 timestamp and the actual tested revision or
immutable artifact identity as `subject_revision`. The current rules enforce
presence and nonblank provenance, not timestamp parsing, Git resolution,
authenticity or freshness. An accepted failed evidence record is valid knowledge.

## Accepted knowledge policies

| Item | Required connection |
|---|---|
| scenario | Contributes to an accepted goal; add involved actors where their identity matters |
| requirement | Has an accepted origin or parent through contributes_to, derives_from or refines, or an external sourced_from authority; has an accepted verification definition or a direct code check |
| design | Satisfies an accepted requirement or refines an accepted design |
| decision | Justifies an accepted requirement, design or risk treatment |
| verification | Verifies an accepted requirement/design or validates an accepted goal/scenario |
| evidence | Evidences an accepted verification |
| risk | Affects accepted knowledge; mitigated treatment has an accepted mitigation, tolerated treatment has an accepted justifying decision |

Classifications become mandatory only on acceptance. Accepted requirements do not
require implementation, passing evidence, or a dedicated design item. Ordinary
automated tests can connect directly through `checks`; give a verification its
own identity when the repeatable method needs independent traceability.
`refines`, `derives_from` and `supersedes` are acyclic; no blanket cycle ban
applies to `depends_on`. There is no obligation to invent missing items merely
to fill a diagram.

## Selected-scope assessments

Run project validation before assessing a selected scope. These request-local
checks do not become always-on project policy.

| Shape IRI suffix after urn:mara:rule: | Roots | Qualifying connection |
|---|---|---|
| intent | Accepted requirements | Accepted origin/parent or external source |
| realization | Accepted requirements/designs | Direct code implementation or accepted realizing artifact; a requirement may use one accepted satisfying design with such realization |
| verification | Accepted requirements/designs | Accepted verification definition or direct code check |
| validation | Accepted goals/scenarios | Accepted validation method |

Each check asserts its supported root flavour and accepted status. Draft or
retired roots fail an explicitly requested assessment; select the intended scope
with `--field status=accepted` when appropriate. Related draft or retired items
never satisfy an accepted-item minimum. File-level code endpoints require no
language adapter; symbol endpoints require project-configured language packs.

For example, assess one requirement's implementation coverage:

```sh
mara trace matrix --id REQ-EXPORT \
  --check-file .mara/engineering-checks.yaml \
  --shape urn:mara:rule:realization
```

Execution assessment is a separate question: whether a selected verification has
accepted passing evidence for the caller's specified subject revision. Its
reusable parameterized check depends on the matrix-parameter contract being
implemented separately. Do not substitute an unrestricted historical pass or
hardcode a moving revision into the reusable template. A direct code check or
implementation link never counts as execution evidence.
:::
