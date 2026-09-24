# Relationship authoring and mutation

Accepted 0.3 contracts extending [traceability](traceability.mara.md).
The current checkout implements internal and external metadata and typed inline
relationships, inverse aliases, symmetric relations, semantic navigation,
bounded occurrence inspection, whole-edge or selected occurrence removal,
and structural cardinality and cycle checks declared in the schema.
Schema format 3, discovery format 2 and relationship format 1 are active.
Existing declarations remain directed unless explicitly changed. Published 0.2
uses the previous formats.
Examples require a project declaring the illustrated flavours and relations.

:::mara design DES-RELATION-AUTHORING
:mid: 01M2GC27RD21J5BRYB0438RSEE
:title: Declare inverse, symmetric, inline and external relationships
:satisfies: REQ-INVERSE-RELATION-AUTHORING
:satisfies: REQ-SYMMETRIC-RELATIONS
:satisfies: REQ-TYPED-INLINE-RELATIONS
:satisfies: REQ-TYPED-EXTERNAL-TARGETS

Schema format 3 extends existing relation declarations with optional
`inverse` (one alias string), `symmetric` (boolean, default false), and
`external` (boolean, default false). Existing `description`, `source`,
`target` and `same_flavour` retain their meanings for internal endpoints.

```yaml
format_version: 3
# Flavour declarations omitted from this relation excerpt.
relations:
  verifies:
    description: The source defines a check of the target.
    source: [verification]
    target: [requirement, design]
    inverse: verified_by
  associated_with:
    description: The endpoints have a nondirectional association.
    source: [requirement, design]
    target: [requirement, design]
    symmetric: true
  tracked_by:
    description: The source has an external delivery reference.
    source: [requirement]
    target: []
    external: true
```

An inverse alias belongs to its canonical declaration; it is not a second
relation. Resolve it by exchanging authored endpoints before applying source,
target and same-flavour constraints. In the example, `VER-A verifies REQ-B`
and `REQ-B verified_by VER-A` are valid equivalent assertions;
`REQ-B verifies VER-A` is invalid. Alias lookup returns the canonical
declaration and identifies the requested alias.

For symmetric relations, source and target must name the same nonempty set
of flavours, ignoring list order. `same_flavour: true` may further restrict
pairs. Reject an inverse alias on a symmetric relation. A self-association
counts once and has no incoming/outgoing presentation.

Canonical names and aliases use the existing lowercase snake-case grammar.
Reject aliases equal to their canonical name, duplicate aliases, and any
collision with another canonical name or alias. Neither canonical names nor
aliases may be structural metadata names or custom-field names on their
eligible authored-source flavours; alias checks use the canonical target
flavours. Existing builtin/schema collisions remain explicitly namespaced in
queries: `schema:mentions` versus `builtin:mentions`; ambiguous short names
are errors. Metadata and typed inline syntax always select the schema namespace.

## Authored spelling

Metadata retains `:relation: target`, with one target per line. An item body
accepts `[[relation:target]]`; split at the first colon only. The relation is
a declared canonical name or inverse alias. Internal targets are exact human
IDs or canonical MIDs. No whitespace, display-label syntax or nested markup
is accepted inside the token. Metadata scalar trimming remains unchanged.

```markdown
:verified_by: VER-A

Validated by [[verified_by:VER-A]] during recovery.
See [[VER-A]] for background.
Tracked by [[tracked_by:external:https://linear.app/example/issue/ENG-7]].
```

Typed inline references are recognized only inside item bodies, including
supported nested Markdown containers. Follow the code, raw-context and escape
exclusions of the existing mention parser. Typed-looking tokens outside items
are ordinary narrative, with no typed edge or implicit item owner. A malformed
typed token or unknown relation/invalid target inside a supported context is a
source-located validation error, not a bare mention. A valid typed token creates
only its typed occurrence; it does not also create a builtin mention.
Bare mentions and ordinary Markdown links keep their 0.2 meaning.

## External addresses

An external target is the literal prefix `external:` followed by an absolute
HTTP(S) URL with a nonempty host. Reject credentials, whitespace, control
characters and unescaped `[`, `]`, `<` or `>`; encode those characters in the URL.
Preserve the address after metadata trimming, including query and fragment;
do not case-fold, decode, follow redirects or otherwise normalize it for
identity. Two different address strings remain different external targets.
A raw URL without `external:` is not an internal handle and is rejected.
The target discriminator, never a failed internal lookup, selects external
interpretation.

`external: true` permits external targets in addition to any listed internal
target flavours. An empty `target` is valid only with `external: true`.
External-capable declarations cannot declare `inverse`, `symmetric: true`
or `same_flavour: true`: an external address cannot author an assertion or
supply a flavour. `source` always requires internal item flavours.

The canonical relation declares the external reference's meaning; for example,
`tracked_by` can satisfy a local ticket-reference obligation. It does not
verify the URL's service or remote object type. External endpoints have an
address only, with no MID, source document, fields or outgoing graph. Rules
may test the presence/count of such edges, but attempts to inspect external
status or traverse beyond the address are configuration errors. No network
request, credential or external-system integration participates in validation.
:::

:::mara design DES-RELATION-MUTATION
:mid: 01M2GC38XWX7TS7GJXVWC5FE8Y
:title: Mutate semantic edges and individual source occurrences safely
:satisfies: REQ-INVERSE-RELATION-AUTHORING
:satisfies: REQ-SYMMETRIC-RELATIONS
:satisfies: REQ-TYPED-INLINE-RELATIONS
:satisfies: REQ-TYPED-EXTERNAL-TARGETS

Mutations resolve the forms in [[DES-RELATION-AUTHORING]] to the semantic
identity in [[DES-CANONICAL-TRACE-RELATIONS]]. The `source` argument always
identifies the internal item in whose context the caller expresses the relation;
it need not be the canonical directed source when using an inverse alias.

## Add and duplicate assertions

`relation add` writes one metadata occurrence on the supplied source item,
using the requested canonical name or alias and the target's current human ID,
or the exact external target spelling. It does not insert prose. Author inline
occurrences through direct Markdown or item body creation/update.

Reject an add if the semantic edge already exists anywhere in the project,
including inverse, symmetric, MID/ID and inline equivalents. Report the
canonical edge and occurrence count; its locations remain available through
relation inspection. Do not add another assertion or turn the rejection into
success. Direct Markdown and body edits may deliberately contain repeated
assertions: they are valid, retain all occurrences, and count once.

Initial `item create` relations follow the same inputs and normalization.
Reject equivalent entries within the initial relation list or an initial
metadata edge already asserted in the new body. Repeated inline assertions
within the body alone remain valid. Validate and publish creation atomically.

## Remove scope and prose

`relation remove` without an occurrence selector removes every authored
assertion of the resolved semantic edge, across all included project documents.
Remove matching metadata lines; demote each matching internal inline token to
a bare mention, retaining its authored target spelling. Demote an external
inline token to an ordinary Markdown autolink of its address. Preserve all
bytes outside the selected lines/tokens, including surrounding prose, unrelated
relations, code examples and existing mentions.

An explicit `occurrence` selects exactly one assertion by an opaque token from
relation inspection. It must belong to the requested edge and current project
snapshot. Apply the same metadata deletion or inline demotion to that occurrence
only. Reject a stale or mismatched token before writing; never select a nearby
occurrence by its old position. A missing edge is an error, not successful
removal. Results state the number of changed occurrences, the remaining count,
and whether the semantic edge still exists. No "remove text" mode is implied.

Before, with two documents contributing three occurrences:

```markdown
<!-- checks.mara.md: VER-A metadata -->
:verifies: REQ-B

<!-- requirements.mara.md: REQ-B metadata and body -->
:verified_by: VER-A

Validated by [[verified_by:VER-A]] during recovery. See [[VER-A]].
```

After removing only the inline occurrence, both metadata lines remain and the
body is:

```markdown
Validated by [[VER-A]] during recovery. See [[VER-A]].
```

The result is one changed occurrence, two remaining, edge present. Removing the
whole edge from the original state removes both metadata lines and produces
the same body: three changed occurrences, zero remaining, edge absent.
Demotion preserves navigation, not an assertion of typed coverage.

External before/after:

```markdown
Tracked by [[tracked_by:external:https://linear.app/example/issue/ENG-7]].
Tracked by <https://linear.app/example/issue/ENG-7>.
```

Removing `A associated_with B` also removes assertions authored on B.
Removing `verifies` never removes a different relation between the same items.

## Reference-safe publication

Plan all affected source edits against one loaded snapshot; validate candidate
source and reference targets before recoverable publication using the existing
mutation transaction boundary. A stale source, invalid selector, parse failure
or broken/retargeted surviving reference rejects the operation without a
partial source edit. Existing pending-transaction recovery remains applicable.
Do not introduce a policy-validation gate requiring an otherwise incomplete
project to meet every lifecycle/coverage rule before removing a relationship;
ordinary validation reports obligations made unmet by removal.

Rename changes internal target IDs in canonical/alias metadata and typed inline
tokens throughout the project, along with existing supported mentions. Preserve
MIDs, external addresses, literal examples and unrelated prose. For example,
renaming `REQ-B` to `REQ-C` changes `:verifies: REQ-B` and
`[[verifies:REQ-B]]` to `:verifies: REQ-C` and `[[verifies:REQ-C]]`.
References authored using the target MID stay unchanged.

Move preserves item identity and all typed assertions. Internal targets are
project identities, so moving an item does not change those handles. Apply the
existing relative Markdown-link and anchor preflight to the proposed location;
reject a move that breaks or retargets a surviving link.

Delete checks semantic incident edges and occurrences on surviving items,
including inverse assertions stored at the endpoint opposite the canonical
source. Reject deletion while another item's typed token, metadata relation,
bare mention or supported Markdown link would become unresolved. Assertions
wholly removed with the deleted item do not block it. Demoting a typed token
leaves a mention that can still block deletion; removing a relationship is not
permission to delete its target or erase prose.

Create/update also recognize typed tokens and reject newly invalid targets.
The reference preflight applies to relation mutations too: demotion inside a
heading can change its generated anchor, so reject the edit if a surviving
link would break or retarget. Do not silently repair those links.
:::

:::mara design DES-RELATION-INTERFACES
:mid: 01M2GC4PXK0MMANAW6AKGXAW7S
:title: Expose canonical relationships and bounded occurrence inspection
:satisfies: REQ-SURFACE-PARITY
:satisfies: REQ-INVERSE-RELATION-AUTHORING
:satisfies: REQ-SYMMETRIC-RELATIONS
:satisfies: REQ-TYPED-INLINE-RELATIONS
:satisfies: REQ-TYPED-EXTERNAL-TARGETS

CLI JSON and MCP share these relationship inputs and domain results.
Project selection retains the current absolute-root convention; examples omit
it and use `--format json` on the CLI. Required strings cannot be empty;
optional `occurrence`, `cursor` and `limit` may be omitted or null in MCP.
Unknown parameters are rejected.

| CLI after `mara --format json` | MCP tool and operation parameters |
|---|---|
| `schema get relation verified_by` | `schema_get {kind:"relation", name:"verified_by"}` |
| `relation add VER-A verifies REQ-B` | `relation_add {source:"VER-A", relation:"verifies", target:"REQ-B"}` |
| `relation add REQ-B verified_by VER-A` | `relation_add {source:"REQ-B", relation:"verified_by", target:"VER-A"}` |
| `relation add REQ-B tracked_by external:https://linear.app/example/issue/ENG-7` | `relation_add {source:"REQ-B", relation:"tracked_by", target:"external:https://linear.app/example/issue/ENG-7"}` |
| `relation get REQ-B verified_by VER-A --limit 20` | `relation_get {source:"REQ-B", relation:"verified_by", target:"VER-A", limit:20}` |
| `relation remove REQ-B verified_by VER-A` | `relation_remove {source:"REQ-B", relation:"verified_by", target:"VER-A"}` |
| `relation remove REQ-B verified_by VER-A --occurrence TOKEN` | `relation_remove {source:"REQ-B", relation:"verified_by", target:"VER-A", occurrence:"TOKEN"}` |
| `related REQ-B --relation verified_by --direction incoming` | `related {reference:"REQ-B", relations:["verified_by"], direction:"incoming"}` |

The add rows illustrate alternative requests, not sequential successful adds.
Creation's repeated `--relation NAME=TARGET` and MCP `relations` entries
accept the same target strings and names; inline creation/update uses the body.

## Canonical edge and occurrence inspection

A schema edge is represented as:

```json
{
  "relation": "verifies",
  "symmetric": false,
  "source": {"kind": "item", "id": "VER-A", "mid": "<actual VER-A MID>"},
  "target": {"kind": "item", "id": "REQ-B", "mid": "<actual REQ-B MID>"}
}
```

MID placeholders here denote generated identities, never literal fixture MIDs.
An external target instead has exactly `{kind:"external", address:"https://…"}`.
The relation is always canonical; internal identity uses MIDs rather than
mutable IDs. For symmetric serialization only, order endpoints by MID byte
order. That stable order conveys no semantic direction.

`relation get` / `relation_get` inspects one existing semantic edge without
changing source. Return `format_version:1`, `edge`, `occurrence_count`,
`occurrences`, `has_more` and `next_cursor`. Each occurrence contains:

| Field | Meaning |
|---|---|
| `reference` | Opaque occurrence selector bound to project, schema and corpus snapshot; not persisted identity. |
| `kind` | `metadata` or `inline`. |
| `source` | Existing source-location shape: relative path, end-exclusive UTF-8 byte range, one-based start/end lines; spans the authored metadata entry or complete inline token. |
| `author` | Internal endpoint descriptor for the item containing the assertion. |
| `relation`, `target` | Authored relation spelling and target scalar, retaining alias and ID/MID/external choice. |

Sort occurrences by document path and start byte. Default `limit` is 20,
accepted range 1–100. Repeat the same request with `--cursor` / `cursor`
until `has_more:false`; `occurrence_count` always states the total.
Apply the discovery 65,536-byte result budget and no-silent-skip behavior.
Oversized mandatory occurrence fields produce an actionable bounded-read error.
Reject stale cursors/selectors after schema or corpus changes; re-inspect
rather than attempting to recover an old byte position. Missing edges are errors.

## Navigation and filters

`related` emits each semantic schema edge once at the selected endpoint,
regardless of occurrence count. A schema connection contains `relation`,
`label`, `direction`, `neighbour`, `edge` and `occurrence_count`.
Replace the former singular `source` for schema edges with inspection through
`relation get`. Builtin connections retain their existing `source` shape
and do not acquire typed-edge counts.

`relation` identifies the canonical schema relation, namespaced when needed;
`label` is the inverse alias at an incoming endpoint when declared, otherwise
the canonical name. Symmetric connections use their canonical label and
`direction:"symmetric"`. Internal neighbours retain discovery node summaries;
external neighbours contain only `kind:"external"` and `address`. They are
terminal: `get` and `related` do not accept them as source references.

Human-facing renderers follow [[REQ-INVERSE-RELATION-AUTHORING]]. From
`VER-A`, display `verifies → REQ-B`; from `REQ-B`, display
`verified_by → VER-A`, without an incoming prefix. If `verifies` has no
inverse alias, the latter falls back to `incoming verifies → VER-A`.
Render the chosen endpoint's label without rewriting canonical `relation` or
`direction` in structured results. This presentation rule does not rewrite
verbatim authored source shown in an excerpt or specification.

Canonical and alias relation filters select the same relation kind. Direction
filters always use the canonical edge relative to the selected item; an alias
filter does not reverse the explicit direction. Thus the last request in the
table returns the incoming `verifies` edge labelled `verified_by`.
Omitting direction includes incoming, outgoing and symmetric connections;
`direction:"symmetric"` selects symmetric only, and incoming/outgoing exclude
symmetric edges.

A directed self-edge has one connection, with this deterministic orientation:

| Direction filter | Result |
|---|---|
| Omitted | Once as `outgoing`, with the canonical label. |
| `outgoing` | Once as `outgoing`, with the canonical label. |
| `incoming` | Once as `incoming`, with the inverse alias when declared, otherwise the canonical label. |
| `symmetric` | Excluded. |

A symmetric self-edge appears once as `symmetric` when direction is omitted or
`symmetric`, and is excluded by incoming/outgoing filters. Deduplicate semantic
schema edges and choose self-edge orientation before sorting and pagination.
Each included self-edge consumes one connection slot and cannot reappear in
another orientation on a later page. This presentation choice does not change
its one-edge semantic count.

Neighbour-flavour filters exclude external endpoints. Existing item-list and
search relation filters also resolve aliases to canonical kinds, preserving
their existing item-selection meaning. Schema inspection lists canonical
declarations with their aliases; lookup by alias returns
`{kind:"relation", name:"verifies", requested_name:"verified_by",
inverse:true, definition:{…}}`. Canonical lookup returns the same fields with
`requested_name` equal to `name` and `inverse:false`.

Preserve discovery ordering by orientation (outgoing, incoming, symmetric),
then internal neighbour source order; external neighbours sort by exact address
after internal neighbours. Break remaining ties by canonical relation name.
Page limits count returned connections, not source occurrences.
Relationship inspection supplies the locations independently of this ordering.

## Mutation results

Successful add/remove returns a relationship result with `format_version:1`,
`action` (`added` or `removed`), `scope` (`relationship` or
`occurrence`), `edge`, `changed_occurrences`, `remaining_occurrences`
and `edge_exists`. These counts are exact and not paginated. Add has
relationship scope, one changed and one remaining occurrence. Removal uses
occurrence scope only when the selector is supplied. The edge descriptor
identifies the affected relationship even after its final assertion is removed.

For the three-occurrence example in [[DES-RELATION-MUTATION]], removing the
inline selector returns this excerpt on both surfaces:

```json
{
  "format_version": 1,
  "action": "removed",
  "scope": "occurrence",
  "changed_occurrences": 1,
  "remaining_occurrences": 2,
  "edge_exists": true
}
```

The full response also includes `edge` as defined above. Whole-edge removal
from the original state has `scope:"relationship"`, three changed,
zero remaining and `edge_exists:false`.

Relationship operation failures return the same `format_version:1` domain
error object on both surfaces: `error:{code,message}`, with optional `edge`
and `occurrence_count` when resolved. Duplicate add uses `relation_exists`;
absent edge uses `relation_not_found`; stale or mismatched selectors use
`stale_occurrence` or `occurrence_mismatch`. Include the inspection request
in duplicate diagnostics so all existing locations can be read through
`relation get`. CLI failures exit nonzero; MCP failures set `isError:true`.
Global validation diagnostic codes and policy severities remain a separate
design area; these operation errors do not define that taxonomy.
:::

:::mara design DES-RELATION-COMPATIBILITY
:mid: 01M2GC5THM24P2J4KMGB9E99VC
:title: Version relationship syntax and public results explicitly
:satisfies: REQ-SCHEMA-EVOLUTION
:satisfies: REQ-SURFACE-PARITY

The contracts in [[DES-RELATION-AUTHORING]], [[DES-RELATION-MUTATION]] and
[[DES-RELATION-INTERFACES]] are the planned 0.3 relationship baseline.
Version each affected persisted or public surface independently:

| Surface | 0.3 relationship contract |
|---|---|
| Schema | `format_version: 3`; reject format 2 with an actionable migration reference instead of silently enabling new syntax. |
| Documents | Retain the existing item delimiters, metadata boundary and MIDs; schema format 3 selects the new relationship grammar. No per-document version marker is added. |
| Project configuration | Remains format 1; no new project setting is required. |
| Discovery JSON | `format_version: 2` for search/get/related as one response family. Search/get retain their shapes; related gains semantic-edge summaries, symmetric direction and external neighbours. |
| Relationship JSON | `format_version: 1` starts a separately versioned family for relation get/add/remove and their errors. Replaces the previously unversioned single-source mutation result. |
| MCP transport | Keep the existing MCP transport negotiation. Tool input/output schemas reflect the same domain changes as CLI JSON; transport version does not replace domain format versions. |

Do not offer an implicit format-1 discovery fallback that invents one source
occurrence or disguises a symmetric/external edge. Clients upgrade their result
parsers and discard all old discovery cursors and structural handles. MIDs stay
valid. Relation operation argument names remain `source`, `relation` and
`target`; the optional occurrence selector and bounded inspection operation
are additions, while whole-edge removal can now affect several files.

Migration is deliberate and reviewable:

1. Work on a recoverable copy or Git checkpoint. Inventory current custom
   declarations, item ID/MID pairs and source files.
2. Review item bodies for tokens newly meaningful as `[[relation:target]]`.
   Escape literal examples before enabling the grammar. Check proposed aliases
   against canonical names, metadata and eligible custom fields. Do not
   automatically reinterpret an existing separately declared reverse relation
   as an alias.
3. Change schema `format_version: 2` to `3` in place. Existing declarations
   without the new keys remain directed and internal with no inverse alias.
   Add new declarations only when intended; do not add statuses or policy,
   reinitialize the project, regenerate MIDs or rewrite existing relation
   direction/meaning.
4. Review the complete diff, then run schema and project validation with the
   0.3 executable. Compare identities, custom fields, declarations, prose and
   links; only reviewed changes may differ. Inspect representative relations
   and their occurrence counts through CLI and MCP.

A customized schema with `verifies`, custom fields and ordinary internal
metadata can migrate by changing the version alone if its bodies contain no
newly meaningful typed tokens. Adding `inverse: verified_by` then allows the
reverse spelling without rewriting existing assertions. A schema already
declaring a separate `verified_by` relation must resolve the collision and
explicitly review its meaning and occurrences before adopting the alias;
migration must never merge them merely because their names appear reciprocal.

An invalid candidate leaves the recoverable baseline available and must not be
reported as a completed migration. Automation and further vocabulary migration operations remain under
[[REQ-SCHEMA-EVOLUTION]]. Rule grammar, validation/view formats and policy
adoption follow [[DES-TRACE-CONTRACT-COMPATIBILITY]]; this relationship
contract does not introduce a migration command. The current implementation
adopts these relationship formats and external targets. This increment does
not complete all 0.3 trace views or schema migration work.
:::

:::mara decision ADR-RELATION-ASSERTION-REMOVAL
:mid: 01M2GC5TK1ABKQ0V57CJGMG1AC
:title: Remove typed assertions while preserving prose navigation
:justifies: DES-RELATION-MUTATION
:justifies: DES-CANONICAL-TRACE-RELATIONS

Treat relation removal as removal of typed assertions under
[[DES-RELATION-MUTATION]]. Default to the whole semantic edge, with an explicit
selector for an individual occurrence; retain [[DES-CANONICAL-TRACE-RELATIONS]]
as the identity/counting authority.

Removing just the caller's metadata could report success while an inverse or
inline assertion still supplies coverage. Whole-edge removal eliminates that
ambiguity. Occurrence selection supports a deliberate local cleanup without
claiming the relationship disappeared. Exact remaining counts make both results
observable.

Demote inline assertions to mentions or ordinary external links instead of
deleting tokens or rewriting sentences. Removing text could damage surrounding
prose; inferring a new sentence would invent authored meaning. The retained
navigation is explicit and continues to participate in reference-safe deletion.
Duplicate add remains an error so retries do not manufacture redundant source
assertions; deliberate repeated assertions remain possible through body edits.
:::

:::mara decision ADR-EXPLICIT-RELATION-SYNTAX
:mid: 01M2GC5TMAH83K8F6X5T4BAC24
:title: Use explicit typed spelling behind a versioned schema boundary
:justifies: DES-RELATION-AUTHORING
:justifies: DES-RELATION-COMPATIBILITY

Adopt [[DES-RELATION-AUTHORING]] and [[DES-RELATION-COMPATIBILITY]]:
keep canonical metadata names, declare inverse aliases on their owning relation,
mark symmetry explicitly, use colon-delimited typed inline tokens, and mark
external targets with an explicit prefix.

These forms extend existing metadata and wiki-reference authoring without
turning ordinary mentions into typed obligations or guessing external identity
from failed internal lookup. Explicit symmetry avoids manufacturing a preferred
direction. Exact external addresses provide local traceability without remote
identity resolution or lifecycle imports.

Choose schema format 3 and an explicit discovery response revision because
typed-looking text and relation result shapes can acquire different meanings.
Silently accepting the new semantics as format 2 would hide migration work
from authors and existing clients. Keep project configuration unchanged because
this change adds no project-level setting.
:::
