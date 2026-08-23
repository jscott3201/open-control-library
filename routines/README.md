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
- `schemas/` defines future class manifests, typed interfaces, and
  specialization inputs. It does not contain production class instances.

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

`requirements-routine-schemas.txt` pins `jsonschema==4.26.0` and
`referencing==0.37.0`.

```sh
python3 -m venv /tmp/cxf-routine-schemas
/tmp/cxf-routine-schemas/bin/python -m pip install \
  --requirement tools/lint/requirements-routine-schemas.txt
/tmp/cxf-routine-schemas/bin/python -m unittest \
  tools.lint.tests.test_routine_schemas -v
/tmp/cxf-routine-schemas/bin/python tools/lint/routine_schemas.py
/tmp/cxf-routine-schemas/bin/python -m unittest discover \
  -s tools/lint/tests -v
/tmp/cxf-routine-schemas/bin/python tools/lint/routines.py
cargo run --manifest-path tools/verify/Cargo.toml -- --routines
```

Canonical IDs name parameterized engineering classes, never fixed parameter
variants or source locations. Local types and enums belong to one interface.
The schemas cover scalar and rank-one/rank-two typed values,
parameter-controlled dimensions, stable repeated-member IDs, and
parameter-only optional-connector guards. They do not evaluate guards or
define point semantics, connector bindings, source mapping instances,
production specializations, generated deployments, or executable CXF.
