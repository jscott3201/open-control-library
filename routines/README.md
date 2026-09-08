# Routine catalog

Status: **two canonical reference subsequences; no qualified deployments**.

The first review pass adds independently authored **ASHRAE Guideline 36-2018**
classes. They are concrete, engine-replayable reference logic, not complete
controllers, Modelica-translated classes, or production deployment bundles.

| Reference | Included behavior | Source baseline |
|---|---|---|
| [Thermal-zone state](g36/zones/thermal/zone-state/card.md) | Classify existing heating/cooling loop outputs, with a separate conflict diagnostic | 2018 §5.3.5 |
| [Cooling-only airflow setpoint](g36/terminal-units/cooling-only/airflow-setpoint/card.md) | Group-mode airflow limits, linear cooling reset, and warm-supply inhibition | 2018 Table 5.5.4 and §5.5.5 |

The supplied standard was the 2018 edition. No addenda were silently applied.
The existing 2021 planning anchors and source inventory remain unchanged and
carry no new coverage claims. A newer source edition requires an explicit
behavior review, not a metadata relabel.

The catalog separates planning, source evidence, schema contracts, and future
routine inventories:

- `g36/scope.json` records 22 Section 5 planning anchors and their intended
  destinations. Scope IDs are not canonical class IDs, and the destinations do
  not imply implemented classes or directories.
- `g36/source-inventory.json` records every regular Git blob below the pinned
  upstream G36 source root in separate release and development snapshots.
  `g36/LICENSE-BUILDINGS.html` retains the legal notice shared by both pins.
- `registry.json` is the sole canonical class inventory. Its v3 rows register
  review-only reference bundles with IDs, names, status, and class directories.
  Fixed engineering values do not appear in class IDs.
- `generated-registry.json` is the only future executable deployment inventory.
  It remains empty until the deployment bundle contract and specializer exist.
- `schemas/` contains six governed schemas for future class manifests, typed
  interfaces, specialization inputs, semantic profiles, and derivation
  manifests. The reference classes reuse the existing manifest, interface, and
  specialization shapes; semantic profiles and derivations remain deferred.
- `ontology/ontology-pins.json` fixes the Brick 1.4.4, ASHRAE 223
  1.0.0-ppr.2.1 compatibility, QUDT 3.1.4, and local OCL identities.
  `ontology/ocl-vocabulary.ttl` is the hashed Library-owned vocabulary for
  software and derivation concepts. `ontology/shacl/` contains the SHACL Core
  graph used against the synthetic fixtures' RDF projections.

`g36/coverage.json` references the scope manifest, remains `planned`, and makes
no implementation or completeness claims.

Revision ownership is explicit:

- Root `ENGINE_PIN` selects the runtime evaluator.
- `g36/SOURCE_RELEASE_PIN` selects the stable Modelica Buildings release
  baseline.
- `g36/SOURCE_DEVELOPMENT_PIN` selects the reviewed development baseline.

The pin files are authoritative source identities. There is no donor pin in the
catalog contract. See the routine catalog section in
[`SCHEMA.md`](../SCHEMA.md) for exact shapes, scope identities, and path rules.

The source inventory hashes Git object bytes, not working-tree files. It does
not parse `package.order` or Modelica declarations and makes no claim about
classes, package members, dependencies outside the source root, source-family
mapping, or executable coverage.

Check the inventory against separate upstream checkouts whose HEADs match the
pin files:

```sh
python3 tools/lint/g36_source.py --check \
  --release-root /path/to/modelica-buildings-release \
  --development-root /path/to/modelica-buildings-development
```

Run the remaining catalog gates from the repository root:

`requirements-routine-schemas.txt` pins `jsonschema==4.26.0`,
`pyshacl==0.31.0`, `referencing==0.37.0`, and `rdflib==7.1.4`.

```sh
python3 -m venv /tmp/cxf-routine-schemas
/tmp/cxf-routine-schemas/bin/python -m pip install \
  --requirement tools/lint/requirements-routine-schemas.txt PyYAML==6.0.3
/tmp/cxf-routine-schemas/bin/python -m unittest \
  tools.lint.tests.test_routine_schemas -v
/tmp/cxf-routine-schemas/bin/python -m unittest \
  tools.lint.tests.test_routine_semantics -v
/tmp/cxf-routine-schemas/bin/python tools/lint/routine_schemas.py
/tmp/cxf-routine-schemas/bin/python tools/lint/routine_semantics.py
/tmp/cxf-routine-schemas/bin/python -m unittest discover \
  -s tools/lint/tests -v
/tmp/cxf-routine-schemas/bin/python tools/lint/routines.py
cargo run --manifest-path tools/verify/Cargo.toml -- --routines
```

Canonical IDs name parameterized engineering classes, never fixed parameter
variants or source locations. Local types and enums belong to one interface.
The schemas cover scalar and rank-one/rank-two typed values,
fixed and parameter-controlled dimensions, stable repeated-member IDs, and
parameter-only optional-connector guards. A fixed dimension owns an ordered
canonical member list in interface v3 whose count equals its extent. A
parameter-driven dimension has no canonical member list; specialization v1 owns
its ordered members. Member IDs are authored stable identities rather than
array ordinals and are unique across all dimensions in an interface and
specialization pair. The schemas do not evaluate guards or define production
connector bindings, source mapping instances, source-compiled specializations or generated deployments. The two independent
reference graphs are authored separately from that unfinished source pipeline.

The semantic and derivation schemas are exercised only by synthetic fixtures
under `tools/lint/tests/fixtures/routine_semantics/`. Validation is local and
network-free. JSON Schema and Python checks own closed JSON syntax, policy,
uniqueness, references, and the profile-to-derivation relationship. The SHACL
Core graph validates each RDF projection independently: expected local classes
and predicates, nested node/cardinality/datatype/class structure, and closed
current entities. It does not mirror every JSON lexical or cross-document rule.

Connector dataflow does not reclassify an S223 property as observable or
actuatable. Topology strings are authoring requirements, not building-instance
certification. Validation does not certify external ontology term existence,
resolve fixture point references against production dictionaries, compare a
profile to a production interface, or validate a building instance. Production
semantic profiles, derivation manifests, point migrations, building-instance
SHACL certification, and deployable routine classes remain deferred.

## Reference replay

From the repository root, with the normal engine checkout/build prerequisites:

```sh
python3 tools/authoring/g36_reference.py --check
python3 tools/routines/reference.py
python3 -m unittest discover -s tools/routines/tests -v
cargo build --locked --manifest-path tools/verify/Cargo.toml
python3 tools/routines/reference.py \
  --verifier tools/verify/target/debug/cxf-verify \
  --report target/g36-reference-results.json
python3 tools/book/generate.py
```

`--check` compares CXF against the small independent graph author. It is not a
Modelica compilation check. The reference runner validates manifests, interfaces,
card/source consistency, scalar graph wiring and input contracts, then replays
both authored vectors and an independently expressed boundary matrix through the
actual engine binary. Every declared output needs exactly one expectation at
every tick; empty suites, missing initial inputs, ambiguous windows, malformed
values, reversed airflow limits, and unknown modes fail before replay.

The default command without `--verifier` performs static checks and explicitly
reports engine replay as **not-run**. `cxf-verify --routines` still checks the
empty deployment inventory; it does not verify these reference cases and must
not be reported as executable sequence coverage.

Each class folder has `card.md`, `source.md`, `class-manifest.json`,
`interface.json`, `specialization.schema.json`, `specialization.json`,
`reference.json`, `reference.cxf.jsonld`, `vectors.json`, `overview.svg`, and
`diagram.svg`. The empty specialization is intentional: these two scalar classes
have no structural choices, and their engineering software values remain runtime
inputs. The reference contract carries the explicit scalar ABI, units, accepted
ranges, and cross-input ordering checks. It is not a semantic binding profile or
an activation authorization.

## Before any live deployment

These graphs contain no device writes. Offline input admission is not a live
host implementation. A consuming host still needs accepted point bindings,
unit conversion, input freshness/quality, coherent frames, loop and timer
lifecycle, interlocks, command arbitration, watchdogs, and site-specific fallback.
There is no universally safe rule that invalid data should close every damper or
stop every fan. Do not put these reference rows into `generated-registry.json`
until its deployment contract and the required evidence actually exist.

Open Control Studio's current 1.0 scope remains read-only acquisition and fault
detection. Browsing a routine card does not enable control commands. See
[`Handoff.md`](../Handoff.md) for the reviewed boundaries and the next bounded pass.
