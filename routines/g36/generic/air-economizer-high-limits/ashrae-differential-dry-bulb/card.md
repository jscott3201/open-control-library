# ASHRAE differential dry-bulb economizer high limit

| Field | Value |
|---|---|
| Routine ID | `G36-GEN-AEHL__ashrae-differential-dry-bulb` |
| Status | `source_evidenced` |
| Evidence | E3 |
| Runtime profile | `HostTick-v1` |

## Purpose

This fixed specialization computes the outdoor-air economizer temperature
cutoff for the ASHRAE 90.1 differential dry-bulb branch. It is one executable
variant of `Buildings.Controls.OBC.ASHRAE.G36.Generic.AirEconomizerHighLimits`.

Fixed parameters:

- `eneStd = Buildings.Controls.OBC.ASHRAE.G36.Types.EnergyStandard.ASHRAE90_1`
- `ecoHigLimCon = Buildings.Controls.OBC.ASHRAE.G36.Types.ControlEconomizer.DifferentialDryBulb`
- `ashCliZon = Buildings.Controls.OBC.ASHRAE.G36.Types.ASHRAEClimateZone.Zone_5A`

## Interface and behavior

| Connector | Direction | Type | Unit | Quantity |
|---|---|---|---|---|
| `TRet` | input | Real scalar | K | ThermodynamicTemperature |
| `TCut` | output | Real scalar | K | ThermodynamicTemperature |

For this specialization, `TCut = TRet`. The fixture represents that selected
branch with `Buildings.Controls.OBC.CDL.Reals.AddParameter(p=0)`. It has no
state, optional connectors, or host services beyond the tick call.

![Signal flow](diagram.svg)

## Replay and evidence

`vectors.json` replays the four donor reference rows at 0, 1, 2, and 3 seconds.
At each time, `TRet` and `TCut` are equal: 289.25, 293.15, 297.5, and 301.75 K,
with zero absolute tolerance. The graph and files under `golden/` are preserved
from the donor and hash-locked in `provenance.json`.

The stable routine ID names the human contract. The evaluator-derived content
ID is recorded separately in `provenance.json`.

## Completeness

- Donor configuration: complete for this fixed variant.
- Canonical class: partial; 8 of 12 donor variants are present.
- Family package: not applicable to this leaf.
- Declared guideline profile: partial.

This card does not claim complete AirEconomizerHighLimits, generic-family,
donor-set, or G36 coverage.

## Exclusions

Arrays, member lists, enum-domain inputs, packages, controller traces, and
other AirEconomizerHighLimits variants are outside this bundle.

## References

- Modelica Buildings, `Buildings/Controls/OBC/ASHRAE/G36/Generic/AirEconomizerHighLimits.mo`, at `SOURCE_PIN`.
- Open Control Engine fixture and independent oracle files at `DONOR_PIN`.
- Redistribution terms: `LICENSE-BUILDINGS.html` and `THIRD_PARTY_NOTICES.md`.
