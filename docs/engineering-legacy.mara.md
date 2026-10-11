# Engineering legacy profile

Use `mara project init --template engineering-legacy` for the previous profile.
Existing initialized projects keep their own files; they are not migrated.

The [legacy schema](../templates/engineering-legacy-schema.yaml) owns the old
flavours, fields, relations and guidance. Its installed path remains
`.mara/schema.yaml`. The three other installed paths also remain unchanged:
`.mara/engineering-rules.yaml`, `.mara/engineering-checks.yaml` and
`.mara/engineering-execution.yaml`. Project configuration uses format 2.

## Knowledge and coverage

Draft, accepted and retired describe knowledge authority, not task progress.
The [legacy acceptance policies](../templates/engineering-legacy-rules.yaml)
require classifications and meaningful connections: scenarios contribute to
goals; requirements identify origin and a verification or direct code check;
designs satisfy requirements or refine designs; decisions justify knowledge;
verifications check requirements/designs or validate goals/scenarios; evidence
records verification; risks identify affected knowledge and treatment.
Acceptance does not require implementation, a dedicated design or a passing run.

The [legacy coverage checks](../templates/engineering-legacy-checks.yaml)
evaluate accepted selected roots: intent for requirements, realization and
verification for requirements/designs, validation for goals/scenarios. Draft or
retired roots fail an explicitly requested accepted-scope check; related drafts
or retired items cannot meet accepted-item minima. These checks are request-local.

For a selected accepted requirement or design:

```sh
mara trace matrix --id REQ-EXPORT \
  --check-file .mara/engineering-checks.yaml \
  --shape urn:mara:rule:realization
```

A requirement can qualify directly or through an accepted satisfying design.
A design can qualify directly. Mixed requirement/design selections are supported.

## Execution

The [legacy execution check](../templates/engineering-legacy-execution.yaml)
requires accepted passing evidence whose subject_revision exactly matches the
supplied text. An accepted failed evidence record is valid knowledge. Record
actual capture time, tested identity, scope, environment and limitations.
Provenance presence does not authenticate the report, parse timestamps or resolve
Git identities.

```sh
mara trace matrix --id VER-EXPORT \
  --check-file .mara/engineering-execution.yaml \
  --shape urn:mara:rule:execution \
  --param 'subject_revision=<actual-immutable-tested-identity>'
```

Example IDs are illustrative, not repository selections. Consume all pages.
Matrices do not execute checks, deploy, choose release obligations or prove
outcome validation. Apply [[DES-TRACE-CHECK-BINDING]] when interpreting conflicting
or historical results. Do not use legacy vocabulary to silently substitute for
the [current model](engineering-template.mara.md).
