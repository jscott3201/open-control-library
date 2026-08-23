# cxf-library Schema

Normative contract for this library's layout and file formats. Contract versions
are per artifact: fault contracts remain v1 where named, and routine catalog
artifacts use the identifiers in their section. Changes to any contract in this
file require bumping the affected identifier.

## Layout

```
cxf-library/
├── SCHEMA.md                    # this contract
├── points/<equip>.points.json   # canonical point dictionary per equipment family
├── faults/<equip>/<FAULT-ID>/   # one folder per fault rule
│   ├── card.md                  # fault card: YAML frontmatter (machine) + prose (human)
│   ├── rule.cxf.jsonld          # detection logic, hand-authored CXF JSON-LD
│   ├── vectors.json             # executable test scenarios
│   └── diagram.svg              # block-graph figure referenced from card.md
├── faults/<equip>/README.md     # chapter index and status table
├── playbooks/<slug>.md          # remediation playbooks (shared across faults)
├── clusters/clusters.json       # fault clusters (syndromes with shared root cause)
├── routines/                    # planned control-routine catalog
│   ├── registry.json            # canonical class inventory
│   ├── generated-registry.json  # executable deployment inventory
│   ├── ontology/                # immutable ontology pins and local vocabulary
│   ├── schemas/                 # six governed routine contract schemas
│   └── g36/                     # G36 pins, source inventory, scope, and coverage
├── tools/verify/                # Rust harness: loads each rule into the engine, runs vectors
```

Equipment family keys: `ahu`, `vav`, `fpb`, `rtu`, `hp`, `fcu`, `chw`, `hw`, `hx`,
`erv`, `pmp`, `vfd`, `sys`, `tower`. Fault IDs live in a general namespace:
`{EQUIP}-{NNNN}` — uppercase family key, four digits, contiguous from `0001`
per family in authoring order. The folder name is the fault ID. The number
carries no semantic meaning; provenance lives in each card's `source:` list.
IDs are stable identifiers and are never reused; renames are exceptional (the
CXF does not embed the fault ID, so a rename never churns `content_id`, but it
does break external links and every cross-reference).

**`faults/registry.json`** (`cxf-library/registry/v1`) is the library-wide
fault-code map: one row per rule — `id`, `family`, `name`, `method`, `status`,
and `legacy_id` (the rule's pre-2026-08-18 `{EQUIP}-FC-{NNN}` code, for
continuity with older references). The registry is orchestrator-maintained
like `clusters/clusters.json`, and `tools/lint/registry.py` enforces in CI
that it stays a bijection with the fault dirs, that IDs match the format and
their folder, and that names/statuses match the cards. When a card is added,
renamed, or changes status, the registry row moves with it in the same PR.

Reserved-but-unauthored IDs (a planned rule a README or card already names)
are allowed: they appear in prose and index tables marked planned/deferred,
never in the registry, and the next authored rule in that family takes the
next free number, honoring any reservation.

## Routine catalog

Routine contracts are independent of fault contracts. Nothing in this section
changes a fault schema identifier or fault behavior. The routine catalog is
schema-defined and non-executable. The current contract defines future
canonical class, interface, specialization, semantic-profile, and derivation
shapes without adding a production class, source mapping, semantic profile,
derivation manifest, specialization, or executable deployment.

Pin ownership is split by purpose:

- Root `ENGINE_PIN` is the runtime evaluator revision.
- `routines/g36/SOURCE_RELEASE_PIN` is the stable Modelica Buildings release
  baseline.
- `routines/g36/SOURCE_DEVELOPMENT_PIN` is the reviewed development source
  baseline for material absent from the release.

Each source pin file contains exactly one lowercase 40-hex Git commit followed
by a newline, and the commits MUST differ. The pin files are the authoritative
source identities. `routines/g36/DONOR_PIN` and `routines/g36/SOURCE_PIN` are
retired and MUST be absent.

### Catalog inventories

`routines/registry.json` is the canonical class inventory. Its top-level object
has exactly `schema` and `routines`; `schema` is
`cxf-library/routine-registry/v2`. `routines` MUST be an array and remains empty
until production class rows are implemented. This registry remains the sole
catalog inventory; the schemas below do not replace it. A scope anchor is not a
canonical class. The source inventory defined below records Git blobs; it does
not identify Modelica classes or subsequences.

`routines/generated-registry.json` is the only inventory that may eventually
drive routine execution. Its top-level object has exactly `schema` and
`deployments`; `schema` is
`cxf-library/generated-routine-registry/v1`. `deployments` MUST be an array and
remains empty until generated deployments are implemented. The verifier's
`--routines` mode reads this file, accepts the empty array, and rejects nonempty
arrays until that contract is implemented.

Canonical IDs MUST NOT encode fixed parameter values. Generated deployment IDs
and row schemas are not defined by this version.

### Canonical routine schema resources

This schema set governs six JSON Schema Draft 2020-12 resources:

| Path | `$id` |
|---|---|
| `routines/schemas/common.schema.json` | `https://open-control-library.example/schemas/routine-common-v1.json` |
| `routines/schemas/class-manifest.schema.json` | `https://open-control-library.example/schemas/routine-class-manifest-v1.json` |
| `routines/schemas/interface.schema.json` | `https://open-control-library.example/schemas/routine-interface-v2.json` |
| `routines/schemas/specialization.schema.json` | `https://open-control-library.example/schemas/routine-specialization-v1.json` |
| `routines/schemas/routine-semantic-profile.schema.json` | `https://open-control-library.example/schemas/routine-semantic-profile-v1.json` |
| `routines/schemas/routine-derivation-manifest.schema.json` | `https://open-control-library.example/schemas/routine-derivation-manifest-v1.json` |

Each resource declares
`https://json-schema.org/draft/2020-12/schema`. References use same-resource
fragments or the six absolute IDs above and resolve from an in-memory registry.
Validation performs no network or filesystem retrieval for schema references.
Objects are closed unless stated otherwise. Semantic-only definitions belong to
the semantic-profile resource; `routine-common-v1` remains the existing routine
class/interface contract.

Canonical class IDs have the form
`G36-05-(01..22)-<UPPERCASE-HYPHENATED-CLASS-SLUG>`. Scope IDs are invalid
canonical IDs. A canonical ID identifies a parameterized engineering class; it
MUST be independent of source paths and revisions, fixed parameter values,
ordering, hashes, and future generated content IDs. IDs are immutable and MUST
NOT be reused. Immutability and reuse are authoring and review invariants; the
checker has no historical registry against which to prove them. A positive
integer `revision` records contract changes separately from identity.

Parameter, connector, type, and dimension IDs use bounded lower-case
snake_case. Stable repeated-member IDs use bounded lower-case hyphenated text
beginning with a letter; a dense numeric index is not a stable member ID.
Type, dimension, parameter, and connector IDs MUST be unique within their
respective lists. Enum member IDs and symbols MUST be unique within their enum.

#### Class manifests (`cxf-library/routine-class-manifest/v1`)

A future class manifest has exactly `schema`, `id`, `revision`, `section`,
`source`, and `artifacts`. `section` is `5.1` through `5.22`; its number MUST
agree with the section encoded in `id`.

`source` is a closed union selected by `kind`:

- `upstream` records `snapshot` (`release` or `development`), an exact
  lower-case 40-hex Git revision, a Modelica class path, and one or more file
  locators. Each locator contains a safe path below
  `Buildings/Controls/OBC/ASHRAE/G36/` and a `sha1:<40 lowercase hex>` Git blob
  ID.
- `independent` records one or more safe repository-relative source paths.

Duplicate source paths, absolute paths, backslashes, control characters, and
empty, `.`, or `..` segments are invalid. `artifacts` has exactly `interface`,
`specialization_schema`, and `specialization_config`. Their safe relative paths
share one non-root class directory and end in `interface.json`,
`specialization.schema.json`, and `specialization.json`, respectively. This is
an artifact-location contract, not a production source-to-class mapping.

#### Interfaces (`cxf-library/routine-interface/v2`)

An interface has exactly `schema`, `canonical_id`, `revision`, `types`,
`dimensions`, `parameters`, and `connectors`. Types and enums are local to that
interface; this contract defines no global type catalog.

The primitive symbols are exactly `real`, `integer`, and `boolean`. String and
runtime object types are excluded. A named alias selects one primitive and may
record nonempty trimmed `quantity`, `unit`, and `display_unit` strings.
These strings assert no QUDT, Brick, or ASHRAE 223 semantics. An enum declares
a nonempty ordered list of unique stable member IDs and unique symbols. Enum
values use the stable member IDs; no integer lowering code is assigned.

A type use is either primitive or a reference to a local named type. A shape is
either scalar or an array with an ordered list of one or two dimension IDs.
Dimensions have unique IDs. Their extent is either a positive fixed integer or
a reference to a scalar Integer parameter. Rank greater than two, zero extents,
ragged matrices, and arithmetic dimension expressions are invalid.

Parameters have unique IDs, a type use, shape, `fixed` or `configurable`
configurability, an optional typed default, and optional numeric minimum and
maximum constraints. A fixed parameter MUST have a default and cannot be
assigned by specialization. A configurable parameter without a default MUST be
assigned by specialization.

Connectors have unique IDs, `input` or `output` direction, a type use, shape,
and explicit presence. Presence is `always` or `when` with a closed guard AST.
Guards support `and`, `or`, `not`, and `eq`, `ne`, `lt`, `lte`, `gt`, or `gte`
comparisons. Operands are scalar parameter references or typed scalar literals.
Ordering comparisons require numeric operands; Integer and Real operands are
compatible. Runtime signals, connectors, time, point IDs, operating states,
and host or fault logic cannot appear in guards. The checker validates guard
structure, references, and operand compatibility but does not evaluate a guard
or resolve optional branches.

#### Specialization inputs (`cxf-library/routine-specialization/v1`)

A specialization input has exactly `schema`, `canonical_id`, `revision`,
`parameters`, and `members`. `parameters` is an ordered list of unique parameter
IDs and JSON values. `members` binds each parameter-driven dimension ID to a
nonempty ordered list of globally unique stable member IDs.

The interface and specialization canonical ID and revision MUST agree with the
class manifest. Specialization checks parameter existence, fixed-parameter
override rejection, required configurable assignments, primitive and enum
value compatibility, numeric bounds, concrete dimension extents, rectangular
rank-one and rank-two values, and stable-member count. All numeric values MUST
be finite. A parameter-driven dimension resolves only from a positive Integer
effective value.

Specialization is input only. It contains no connector bindings, point IDs,
resolved connector set, source map, generated CXF, runtime state, engine
identity, or deployment identity.

#### Ontology identities and local vocabulary

`routines/ontology/ontology-pins.json` is the sole product ontology-pin record.
It has the closed identifier `cxf-library/ontology-pins/v1` and records these
immutable authorities:

| Authority | Identity |
|---|---|
| Brick | namespace `https://brickschema.org/schema/Brick#`; `BrickSchema/Brick` release `v1.4.4`; commit `4b5be60d27f9b4d96fe477f45513fa71afebe684`; release `Brick.ttl` SHA-256 `b65720b7b9b64c646745c689777e6138c0d59ce0088df0aeb78fbd444d04d8e7` |
| ASHRAE 223 compatibility | core namespace `http://data.ashrae.org/standard223#`; G36 extension `http://data.ashrae.org/standard223/1.0/extensions/g36#`; version `1.0.0-ppr.2.1`; `open223/open223.info` commit `97656845cab16183e64e9611c94f40a6fad95226`; blob `c2ee998a1e0f5cc3e496ff9c20c30e01019ff250`; artifact SHA-256 `1f156f9938c0be430d2216e01e31bb183c438ba318d8d4a23d2f074ebcd6f573` |
| QUDT | quantity-kind namespace `http://qudt.org/vocab/quantitykind/`; unit namespace `http://qudt.org/vocab/unit/`; `qudt/qudt-public-repo` release `v3.1.4`; tag object `e6cba51f5769691a926e000cbeb044d4d5cd754e`; commit `5a19ef66a5b8d8c404f469244304afc7d9f83eaa`; exact quantity-kind and unit paths, blobs, and SHA-256 values in the pin record |
| OCL | namespace `urn:open-control-library:ontology:`; version `0.1.0-draft`; checked-in path `routines/ontology/ocl-vocabulary.ttl`; byte hash in the pin record |

The S223 artifact imports `<http://qudt.org/3.1.8/shacl/qudt-all>`. The pin
record keeps that as a compatibility observation. It does not replace the
Library's QUDT 3.1.4 authority, and the S223 artifact is not represented as the
final published standard.

`ocl-vocabulary.ttl` contains only Library-owned profile, connector-binding,
software-signal, derived-signal, aggregate, derivation, and policy terms used by
the governed fixtures. It has no imports. Its SHA-256 is part of the pin record;
changing the Turtle bytes requires updating that hash in the same change.

#### Routine semantic profiles (`cxf-library/routine-semantic-profile/v1`)

A semantic profile has a stable JSON-LD `@id`, type
`ocl:RoutineSemanticProfile`, canonical class ID and revision, the exact
`routines/ontology/ontology-pins.json` reference, and one or more connector
roles. Its context is one closed embedded object. String, list, nested, remote,
or imported contexts are invalid.

Connector role IDs use the interface connector-ID syntax and are unique. Each
role has a bounded nonempty `semantic_role`, a `mapping_status` of `verified` or
`provisional`, and a closed list of bounded topology requirements. `verified`
means the author reviewed the mapping against the named pin evidence;
`provisional` marks a mapping that still needs review. Neither value certifies a
building instance. A physical role requires at least one location, topology, or
ownership obligation; software and derived roles may use an empty list.

Direction is connector dataflow (`input` or `output`) and does not determine the
S223 property class. For example, an active setpoint may be an input while its
property remains actuatable. Requirement is `R`, `A`, `O`, `N`, `S`, `D`, or
`P`; cardinality records integer `minimum` and `maximum` values with minimum not
greater than maximum. Bindings are a closed union:

- `physical-or-bms-point` carries one
  `points/<family>.points.json#<point_key>` reference plus closed Brick and S223
  mappings;
- `software-signal` carries an OCL class and no physical ontology mapping; and
- `derived-signal` carries an OCL class, output ID, and local derivation-manifest
  reference.

Physical mappings allow only the reviewed directional S223 property classes:
`s223:QuantifiableObservableProperty`,
`s223:QuantifiableActuatableProperty`,
`s223:EnumeratedObservableProperty`, and
`s223:EnumeratedActuatableProperty`. Quantifiable mappings require a QUDT
quantity kind and unit. Enumerated mappings require an enumeration kind. Both
mapping variants record the S223 medium as a CURIE or explicit `null`. Allowed
aspects are `s223:Aspect-Setpoint`, `s223:Aspect-Delta`, and
`s223:Aspect-Maximum`; `s223:EnumeratedProperty` is invalid. Topology strings
are structural authoring obligations, not topology instances or SHACL results.

#### Derivation manifests (`cxf-library/routine-derivation-manifest/v1`)

A derivation manifest identifies one `ocl:DerivedSignal` or
`ocl:DerivedAggregate` output. It records the canonical class revision, an
exact `routines/ontology/ontology-pins.json` reference, an immutable function ID
and version, ordered typed inputs with stable source IDs, stable members,
exclusions, data-quality handling, freshness and alignment limits in seconds,
readiness and in-domain policy, output unit and conversion policy, output scope,
and reset behavior.

Member-linked inputs, exclusions, member output scopes, and source-triggered
resets must resolve inside the manifest. IDs are unique. Data-quality and ready
minimums cannot exceed the member population and must agree. A profile's
derived output ID and manifest fragment must equal the manifest output ID; a
manifest output must be referenced by exactly one derived connector role in the
synthetic fixture pair.

#### Schema validation boundary

The schemas enforce required and closed shapes, discriminators, ID patterns,
primitive JSON types, and array-rank bounds. `tools/lint/routine_schemas.py`
adds deterministic cross-document checks for uniqueness, section coherence,
reference existence and kind, finite and compatible values, dimensions,
rectangular arrays, guards, and specialization completeness. It rejects
duplicate JSON keys and non-finite numbers before schema validation, checks all
six schema resources with `Draft202012Validator.check_schema`, and reports
sorted errors without a traceback for expected failures.

The linter validates one coherent fixture set under
`tools/lint/tests/fixtures/routine_schemas/`. Those documents are synthetic,
test-only contract evidence. They MUST NOT appear below `routines/g36/` or be
added to a registry, coverage claim, source inventory, book, or production
catalog destination.

`tools/lint/routine_semantics.py` checks the closed pin record, recomputes the
local-vocabulary hash, parses the Turtle from local bytes, applies the same
six-resource in-memory schema registry, rejects unsafe JSON-LD constructs before
RDFLib parsing, and validates semantic and derivation cross-document rules. Its
two fixtures under `tools/lint/tests/fixtures/routine_semantics/` are synthetic.
Their point references are syntax examples and are not resolved against
production dictionaries or routine interfaces.

No external ontology is vendored or fetched. Brick, S223, and QUDT CURIE checks
therefore prove closed syntax and selected S223 class and aspect policy, not that
every external term exists in its pinned ontology. Connector semantic-role and
topology requirements are authoring evidence, not building-instance evidence.
Production profiles must later be paired with typed interfaces, canonical point
dictionaries, ontology-term evidence, and building-instance validation before
any semantic-conformance claim.

### `routines/g36/source-inventory.json` (`cxf-library/g36-source-inventory/v1`)

The source inventory records two independent Git-tree snapshots from
`https://github.com/lbl-srg/modelica-buildings.git`. Its top-level object has
exactly these keys in order: `schema`, `repository`, `source_root`,
`inventory_scope`, `dependency_closure`, `license`, and `snapshots`.

- `schema` is `cxf-library/g36-source-inventory/v1`;
- `repository` is `https://github.com/lbl-srg/modelica-buildings.git`;
- `source_root` is `Buildings/Controls/OBC/ASHRAE/G36`;
- `inventory_scope` is `source-root-regular-files`; and
- `dependency_closure` is `not-inventoried`.

`license` has exactly `upstream_path`, `retained_path`, `git_blob_sha1`, and
`sha256`, in that order. `upstream_path` is `Buildings/legal.html` and
`retained_path` is `routines/g36/LICENSE-BUILDINGS.html`. The retained file MUST
equal the Git blob bytes at both pins. `git_blob_sha1` uses
`sha1:<40 lowercase hex>` and `sha256` uses `sha256:<64 lowercase hex>`.

`snapshots` contains exactly two rows, ordered `release` then `development`.
Each row has exactly these keys in order: `role`, `revision`,
`root_tree_sha1`, `file_count`, `total_bytes`, `modelica_file_count`,
`package_order_count`, and `files`. `revision` MUST equal the corresponding pin
file. `root_tree_sha1` is the Git tree ID for `source_root`, encoded as
`sha1:<40 lowercase hex>`.

Each `files` row has exactly `path`, `mode`, `bytes`, `git_blob_sha1`, and
`sha256`, in that order. Paths are full upstream repository-relative POSIX
paths below `source_root`, sorted lexicographically and unique. Empty, `.`, and
`..` path segments, absolute paths, backslashes, and control characters are
invalid. Version 1 supports only regular `100644` Git blobs; other modes and
object types MUST be rejected. `bytes` and both hashes are calculated from Git
object bytes rather than working-tree files.

`file_count` is the number of file rows, and `total_bytes` is the sum of their
`bytes` values. `modelica_file_count` counts paths ending in `.mo`.
`package_order_count` counts paths ending in `/package.order`; `package.order`
content is not parsed or validated. All files remain in the inventory whether
or not another source file names them.

The release and development snapshots MUST remain separate. The inventory does
not choose a snapshot for a future canonical class and MUST NOT use one as a
fallback for the other. It inventories no dependency outside `source_root` and
makes no claim about Modelica declarations, classes, package members,
references, imports, inheritance, source-family mapping, or executable
coverage.

`tools/lint/g36_source.py --write` regenerates the inventory and retained legal
notice from two supplied checkouts. `--check` verifies each checkout origin and
exact HEAD, derives all records through Git object commands, and compares both
tracked artifacts byte-for-byte without rewriting them. JSON uses two-space
indentation, the field order above, and one final newline. It contains no
timestamp, branch name, checkout path, or moving source identity. A source pin
change requires regenerating both snapshots; generation fails if the pinned
legal blobs or bytes differ.

### `routines/g36/scope.json` (`cxf-library/g36-scope/v1`)

The scope manifest makes the Section 5 planning boundary discoverable without
claiming canonical classes. Its top-level object has exactly `schema`,
`profile`, `status`, and `sections`:

- `schema` is `cxf-library/g36-scope/v1`;
- `profile` is `ASHRAE Guideline 36-2021 Section 5`;
- `status` is `planned`; and
- `sections` is an array of exactly 22 rows, ordered from `5.1` through `5.22`.

Each row has exactly `id`, `section`, `name`, `status`,
`source_disposition`, and `destination`. IDs, sections, and destinations MUST
each be unique. `name` is an independently written, nonempty display name and
`status` is `planned`. Scope IDs identify planning anchors only; they MUST NOT
be used as canonical class IDs.

The IDs, destinations, and reviewed planning dispositions are:

| Section | Scope ID | Destination | Source disposition |
|---|---|---|---|
| 5.1 | `G36-SCOPE-05-01` | `g36/shared/general` | `mixed` |
| 5.2 | `G36-SCOPE-05-02` | `g36/zones/ventilation` | `upstream-partial` |
| 5.3 | `G36-SCOPE-05-03` | `g36/zones/thermal` | `upstream-broad` |
| 5.4 | `G36-SCOPE-05-04` | `g36/zone-groups` | `upstream-broad` |
| 5.5 | `G36-SCOPE-05-05` | `g36/terminal-units/cooling-only` | `upstream-broad` |
| 5.6 | `G36-SCOPE-05-06` | `g36/terminal-units/reheat` | `upstream-broad` |
| 5.7 | `G36-SCOPE-05-07` | `g36/terminal-units/parallel-fan-cv` | `upstream-broad` |
| 5.8 | `G36-SCOPE-05-08` | `g36/terminal-units/parallel-fan-vv` | `upstream-broad` |
| 5.9 | `G36-SCOPE-05-09` | `g36/terminal-units/series-fan-cv` | `upstream-broad` |
| 5.10 | `G36-SCOPE-05-10` | `g36/terminal-units/series-fan-vv` | `upstream-broad` |
| 5.11 | `G36-SCOPE-05-11` | `g36/terminal-units/dual-duct-snap` | `upstream-broad` |
| 5.12 | `G36-SCOPE-05-12` | `g36/terminal-units/dual-duct-mix-inlet` | `upstream-broad` |
| 5.13 | `G36-SCOPE-05-13` | `g36/terminal-units/dual-duct-mix-discharge` | `upstream-broad` |
| 5.14 | `G36-SCOPE-05-14` | `g36/terminal-units/dual-duct-cold-min` | `upstream-broad` |
| 5.15 | `G36-SCOPE-05-15` | `g36/ahus/system-modes` | `upstream-embedded` |
| 5.16 | `G36-SCOPE-05-16` | `g36/ahus/multizone-vav` | `upstream-broad` |
| 5.17 | `G36-SCOPE-05-17` | `g36/ahus/dual-fan-dual-duct` | `independent-authoring` |
| 5.18 | `G36-SCOPE-05-18` | `g36/ahus/single-zone-vav` | `upstream-broad` |
| 5.19 | `G36-SCOPE-05-19` | `g36/exhaust-fans/constant-speed` | `independent-authoring` |
| 5.20 | `G36-SCOPE-05-20` | `g36/plants/chilled-water` | `development-source` |
| 5.21 | `G36-SCOPE-05-21` | `g36/plants/hot-water` | `independent-authoring` |
| 5.22 | `G36-SCOPE-05-22` | `g36/fan-coil-units` | `upstream-partial` |

These dispositions classify the source plan; they do not identify a source
class or prove implementation. Destinations are safe repository-relative POSIX
paths below `g36/`. Absolute paths, backslashes, empty segments, `.` segments,
and `..` segments are invalid. The manifest reserves destinations without
requiring placeholder directories.

### `routines/g36/coverage.json` (`cxf-library/g36-coverage/v2`)

Coverage has exactly `schema`, `profile`, `status`, `scope`, and `claims`.
`schema` is `cxf-library/g36-coverage/v2`; `profile` and `status` MUST equal
`scope.json`; `scope` is `scope.json`; and `claims` remains empty until
production coverage claims are implemented. Coverage does not repeat scope rows
or inventory and makes no completeness, implementation, or evidence claim.

No `routine.cxf.jsonld` may appear below `routines/g36/` until generated
deployments are implemented. The retired
`routines/g36/generic/air-economizer-high-limits` fixed-variant path MUST be
absent.

### Deferred routine contracts

Production class manifests, interfaces, specialization inputs, semantic
profiles, and derivation manifests remain deferred, as do source-to-family and
class mapping instances, point migrations, a specializer, resolved connectors,
generated deployment bundle schemas and rows, source maps, vectors, generated
CXF, building-instance conformance, and execution.

## Design stance (why the pieces split this way)

- **A fault rule is a CDL composite block**: canonical point inputs → elementary
  comparison/logic/timing blocks → boolean fault output(s). Stored as CXF the
  open-control engine loads directly.
- **The rule computes the fault condition given valid data.** Data quality
  (NO_EVAL), operating-state gating, suppression, and energy accumulation are
  host/runtime concerns declared in the card frontmatter, never encoded in the
  block graph. The engine is deliberately status-blind; hosts enforce
  `preconditions` and `operating_states` before trusting `yFault`.
- **Semantics live in the point dictionary** (Brick + ASHRAE 223P), keyed by
  canonical point name. CXF documents stay semantics-free in v1; a generator may
  later inject annotations from the dictionary (the engine preserves unknown
  keys losslessly).

## `card.md` contract (`cxf-library/fault-card/v1`)

YAML frontmatter followed by Markdown prose. Frontmatter fields:

| Field | Type | Req | Meaning |
|---|---|---|---|
| `schema` | string | ✓ | `cxf-library/fault-card/v1` |
| `id` | string | ✓ | Fault ID, equals folder name |
| `name` | string | ✓ | Short human name |
| `equipment` | string | ✓ | Equipment family key |
| `status` | enum | ✓ | `draft` \| `verified` \| `adopted` \| `deprecated` |
| `phase` | int | ✓ | Rollout phase (1–4) |
| `method` | enum | ✓ | `rule` \| `statistical` \| `ml` \| `meta` |
| `severity` | int | ✓ | 1 Critical · 2 High · 3 Warning · 4 Info |
| `category` | enum | ✓ | `CRITICAL_WASTE` \| `EFFICIENCY_LOSS` \| `EXCESS_CONSUMPTION` \| `COMFORT_ENERGY` \| `PROTECTIVE` |
| `confidence` | enum | ✓ | `HIGH` \| `MEDIUM` \| `LOW` (evidence strength) |
| `estimation_method` | enum | ✓ | `DIRECT_MEASUREMENT` \| `BASELINE_COMPARISON` \| `PROXY_ESTIMATION` \| `QUALITATIVE_ONLY` |
| `source` | list | ✓ | Provenance refs (reference §, PNNL report, G36 §) |
| `g36` | string\|null | ✓ | G36 clause for 001-range rules, else null |
| `clusters` | list | | Cluster IDs this rule participates in |
| `suppresses` | list | | Rule IDs silenced while this fault is active |
| `suppressed_by` | list | | Rule IDs that silence this rule when active |
| `adjudicates` | map | | Sensor-health rules only: `{points: [...], verdict: invalid_while_active \| ambiguous}` — the canonical point(s) whose data validity this rule judges. Hosts derive the NO_EVAL fan-out from downstream cards' `points` lists (point-keyed, so it stays complete as rules are added — never a hand-written rule list). `invalid_while_active`: treat the point as invalid for all consumers while this fault asserts; `ambiguous`: a redundancy-pair rule that cannot name which member drifted. |
| `related` | list | | Co-occurring rules (informational) |
| `playbooks` | list | ✓ | Playbook slugs in `playbooks/` |
| `operating_states` | string | ✓ | Applicable states (`all` or list, prose ok) |
| `preconditions` | string | ✓ | Host-enforced evaluation gate, prose |
| `points` | list | ✓ | Canonical point names consumed (see below) |
| `outputs` | list | ✓ | `{name, description}` — boundary outputs |
| `params` | map | ✓ | name → `{default, unit, description, cxf}` |
| `energy_impact` | map | ✓ | `{affected_subsystem, savings_range, climate_sensitivity, runtime_estimation}` |
| `emissions` | map | ✓ | `{scope, method}` |
| `verified` | map | ✓ | `{engine_rev, content_id, date}` — all null until verified |

Conventions:
- **Every entry in `points` is a canonical name from
  `points/<equipment>.points.json`, and the CXF boundary input connector for it
  has exactly that local name.** This single convention makes point binding
  mechanical for every consumer.
- `params.*.cxf` is the parameter's CXF path relative to the root block
  (`<instance>.<param>`, e.g. `persist.delayTime`) so hosts can retune deployed
  rules via `set_param` without re-authoring. It may be a list of paths when
  one card parameter binds several block parameters (e.g. an evaluation
  window driving sampler periods and dwell times); hosts must set every
  listed path together.
- Signal units are those declared in the point dictionary (°C, Pa, %, bool).
  Hosts must feed those units; rules do no unit conversion in v1.
- `verified.engine_rev` is the open-control git rev the vectors last passed
  against; `verified.content_id` is the engine's exported `cxf:fnv1a128:…`
  diagnostic identity. Git history is the integrity record for the bytes.

Prose sections (in order): `## Description`, `## Detection Logic`,
`## Possible Diagnoses`, `## Energy Impact`, `## Emissions Impact`,
`## Deviations` (differences from the source reference and why — required, may
be "None"), `## Notes` (optional).

`## Detection Logic` contains the equation in a fenced block plus the block
graph as `![…](diagram.svg)` (a standalone SVG file — GitHub renders linked
SVG but strips inline SVG). Diagram conventions: boundary inputs as blue pills
on the left, fault outputs as red pills on the right, elementary blocks as
rounded rectangles labeled `instance` over `Class · key params`, signal flow
left to right.

**`validation:` (optional frontmatter block, adopted 2026-08-18).** Records
empirical validation runs against the card's rule. A list; each entry:
`kind` (`simulation_fpr` | `simulation_tpr` — for `simulation_tpr`, `failures` counts MISSED detections), `harness` (e.g.
`simharness/v1`), `date`, `fleet` (one-line description of buildings ×
climates × period), `scenarios` (count), `failures` (count), optional
`notes` (one line, e.g. the finding a failure represents). Results are
facts about a specific fleet and gating configuration, not guarantees;
the harness README documents mapping proxies and gating. Cards without
the block simply have not been swept yet.

**Card style (conciseness contract, adopted 2026-08-18; exemplar:
`faults/ahu/AHU-0016/card.md`).** Cards are clear, concise, and outlay the
conditions — they are specifications, not design journals. Targets:
Description ≤ ~10 lines; Detection Logic prose ≤ ~15 lines beyond the
equation and diagram (timing semantics, strictness, deployer must-knows
only — no block-by-block narration of the diagram); Energy/Emissions ≤ ~8
lines each; Notes ≤ ~8 lines or omitted. Deviations keeps EVERY engineering
decision but each as one bullet of 2–5 lines (decision + one-sentence why +
citation) — no alternatives-considered essays. Never narrate vector
scenarios in the card (vectors.json is that record), except a sentence
naming a deliberately-pinned engine behavior. Typical full card:
~140–220 lines (statistical cards may run ~250).

## `rule.cxf.jsonld` contract

Target dialect: the open-control engine's composite subset
(`open-control/docs/cxf-composite-subset.md`), matching its G36 fixture style:

- `@context`: `{"S231": "http://data.ashrae.org/S231P#", "base": "<fault ns>"}`
  where the fault namespace is `urn:cxf-library:<fault-id-lowercase>#`.
- Flat `@graph`, full IRIs written out. Root block:
  `<ns><fault_id_snake>` with `@type S231:Block`, `S231:label`,
  `S231:containsBlock`, `S231:hasInput` (boundary points), `S231:hasOutput`.
- Child instances: `<root>.<label>`; `@type` is
  `<ns>Buildings.Controls.OBC.CDL.<ClassPath>` (the engine resolves the class
  from the IRI fragment). Only registry-supported classes.
- Ports `<instance>.<portName>` typed `S231:RealInput`/`S231:BooleanInput`/…
  with `S231:isOfDataType`; connections via `S231:isConnectedTo` on the source
  (output) node.
- Parameters: `<instance>.<param>` nodes carrying `S231:value` typed literals;
  referenced from `S231:hasParameter`. Set only non-default values.
- Fault outputs are `BooleanOutput`s; primary output is named `yFault` (true
  while the fault condition persists). Additional outputs allowed (e.g.
  sub-condition flags) and must be listed in the card's `outputs`. When the
  reference semantics include an in-rule evaluability condition (a NO_EVAL
  test vector), expose it as an additional boolean output (`y…` name); the
  card documents that false means NO_EVAL — the host must consult it before
  interpreting `yFault`. Secondary outputs come in TWO kinds and the card's
  `outputs` prose must say which: **evaluability flags** (`y…Ok` — false
  means NO_EVAL) and **sub-condition/direction flags** (e.g. SYS-0006's
  `yBias`/`yNoise`, SYS-0008's direction flags — diagnostic detail only;
  false never means NO_EVAL). Hosts must not treat every non-`yFault`
  boolean as an evaluability gate.
- No semantic annotations in v1 (see design stance). No `oce.*` class aliases.

## `vectors.json` contract (`cxf-library/vectors/v1`)

```json
{
  "schema": "cxf-library/vectors/v1",
  "clock": { "step_s": 60, "horizon_s": 1800 },
  "scenarios": [
    {
      "name": "snake_case_name",
      "description": "optional",
      "inputs": {
        "htg_vlv_cmd": 20.0,
        "clg_vlv_cmd": [ { "t": 0, "value": 30.0 }, { "t": 600, "value": 0.0 } ]
      },
      "expect": [
        { "output": "yFault", "from_s": 0, "to_s": 840, "equals": false }
      ]
    }
  ]
}
```

- `inputs`: canonical point name → constant (number/bool) or a step list
  `[{t, value}…]` (piecewise-constant; each value staged before the first tick
  with model time ≥ its `t`).
- `expect`: windowed assertions on boundary outputs, inclusive of both ends,
  checked at every tick whose time falls in the window. Reals compare with
  optional `tolerance` (default 1e-9).
- Scenarios are independent: each runs on a freshly loaded engine.
- Windows must respect timing parameters (e.g. leave ≥ one step of margin
  around an `alarm_delay` edge rather than asserting the exact boundary tick).
- Scenarios must cover at minimum: the reference card's published test vectors,
  one threshold-edge case, and one transient case exercising delay/reset
  behavior where the rule has timing state.

## `points/<equip>.points.json` contract (`cxf-library/points/v1`)

```json
{
  "schema": "cxf-library/points/v1",
  "equipment": "ahu",
  "points": [
    {
      "name": "htg_vlv_cmd",
      "description": "Heating coil valve command (0 = closed, 100 = full open)",
      "kind": "real",
      "unit": "%",
      "qudt_unit": "PERCENT",
      "brick": null,
      "s223": null,
      "provisional": true
    }
  ]
}
```

- `name` is the library-wide canonical identifier (snake_case; suffixes: none =
  measured, `_sp` setpoint, `_cmd` command, `_status` status, `_fbk` feedback).
- **Role points** (documented exception, `points/sys.points.json` only): the
  cross-equipment sensor-health rules bind role names (`sensor_value`,
  `sensor_value_a/b`, `equip_active`) rather than canonical points, because
  the same graph deploys against many real points. Role entries carry
  `brick: null, s223: null`; the host's instance configuration records each
  binding, and that record is also what resolves the rule's `adjudicates`
  target and drives its NO_EVAL fan-out. The reference's own SYS-0005 card
  uses the same role form ("varies by application").
- `derived: true` marks host-computed points rather than physical ones — both
  aggregates (a max or fraction across zones) and physical transforms (e.g.
  saturation temperatures from pressure via a refrigerant P-T lookup). The
  entry's `notes` must state the derivation and its site-specific inputs
  (which refrigerant, which underlying points); rules consume derived points
  exactly like physical ones, and the derivation itself never appears in a
  rule graph.
- A top-level `namespaces` map records the exact ontology IRIs and the versions
  the terms were verified against.
- `brick`: verified Brick class local name (namespace
  `https://brickschema.org/schema/Brick#`).
- `s223`: object `{pattern, property_class, quantitykind, unit, medium,
  aspects, enumerationkind?}` using verified ASHRAE 223P terms
  (`enumerationkind` for enumerated properties). See
  the internal 223P point-modeling note (local-only, not distributed) for the modeling pattern.
- Every term must be verified against the published ontology files — never
  from memory. `provisional: true` additionally marks entries with genuine
  ambiguity (class-choice judgment calls, unit conflicts, or patterns
  unattested in the standard's reference models); the per-point `notes` field
  records the specifics. All s223 entries also await confirmation against the
  formal ASHRAE 223 standard text once obtained.

## `clusters/clusters.json` (`cxf-library/clusters/v1`)

Array of `{id, name, trigger, members, playbook, prevalence, energy_impact}`.
`trigger` fires first; fixing it should clear `members` within 24–48 h.

## Playbooks

`playbooks/<slug>.md`: prose with a header block (Applies To, Fix Complexity,
Typical Time, Typical Cost, Energy Impact) and the four-step workflow:
Verify → Remote fix → On-site service → Confirm resolution. Faults reference
playbooks by slug; playbooks list the fault IDs they apply to.

## Verification

`tools/verify` loads each fault's `rule.cxf.jsonld` into the open-control
engine (path dependency, in-process), replays every `vectors.json` scenario
tick by tick, and checks the assertion windows. A fault may be marked
`status: verified` only when all scenarios pass; record the engine rev,
exported content ID, and date in `verified`. Re-verify (and re-record) after
any engine pin bump or rule edit.
