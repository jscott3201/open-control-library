# Routine catalog

Status: **six executable routines**, source-evidenced at E3.

`registry.json` is the executable routine inventory. `g36/coverage.json`
records aggregate scope without duplicating that inventory. The current entries
are six scalar specializations of G36 Generic AirEconomizerHighLimits; they do
not establish class, family, donor-set, or guideline completeness.

Revision ownership is explicit:

- Root `ENGINE_PIN` selects the runtime evaluator.
- `g36/DONOR_PIN` selects the open-control-engine donor fixture and golden
  revision.
- `g36/SOURCE_PIN` selects the upstream Modelica Buildings source revision.

The pin files are authoritative. See the routine catalog section in
[`SCHEMA.md`](../SCHEMA.md) for row identities, allowed values, and path rules.
Validate and replay the catalog from the repository root:

```sh
python3 tools/lint/routines.py
cargo run --manifest-path tools/verify/Cargo.toml -- --routines
```

Pass `--donor-root <open-control-checkout>` to compare copied artifacts
byte-for-byte with that checkout. CI first verifies the checkout at `DONOR_PIN`.

Arrays, member lists, enum domains, optional connectors, packages, controller
traces, and E4/E5 evidence remain deferred.
