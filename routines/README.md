# Routine catalog

Status: **schema-defined and non-executable**.

The catalog separates planning, source evidence, schema contracts, and future
routine inventories:

- `g36/scope.json` records 22 Section 5 planning anchors and their intended
  destinations. Scope IDs are not canonical class IDs, and the destinations do
  not imply implemented classes or directories.
- `g36/source-inventory.json` records every regular Git blob below the pinned
  upstream G36 source root in separate release and development snapshots.
  `g36/LICENSE-BUILDINGS.html` retains the legal notice shared by both pins.
- `registry.json` is the canonical class inventory. It remains empty until the
  first production class rows are implemented.
- `generated-registry.json` is the only future executable deployment inventory.
  It remains empty until the deployment bundle contract and specializer exist.
- `schemas/` contains six governed schemas for future class manifests, typed
  interfaces, specialization inputs, semantic profiles, and derivation
  manifests. It contains no production instances.
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
  --requirement tools/lint/requirements-routine-schemas.txt
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
connector bindings, source mapping instances, specializations, generated
deployments, or executable CXF.

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
SHACL certification, and routine classes remain deferred.
