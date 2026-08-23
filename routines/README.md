# Routine catalog

Status: **planned and non-executable**.

The L0 catalog separates three inventories:

- `g36/scope.json` records 22 Section 5 planning anchors and their intended
  destinations. Scope IDs are not canonical class IDs, and the destinations do
  not imply implemented classes or directories.
- `registry.json` is the canonical class inventory. It remains empty until the
  source inventory identifies actual classes and subsequences.
- `generated-registry.json` is the only future executable deployment inventory.
  It remains empty until the deployment bundle contract and specializer exist.

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
Validate the catalog boundary from the repository root:

```sh
python3 -m unittest discover -s tools/lint/tests -v
python3 tools/lint/routines.py
cargo run --manifest-path tools/verify/Cargo.toml -- --routines
```

Canonical typed artifact schemas, generated deployment bundles, semantic
sidecars, source inventory, specialization, and executable deployments remain
deferred.
