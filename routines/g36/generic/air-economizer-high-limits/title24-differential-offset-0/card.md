# Title 24 zero-offset differential dry-bulb economizer high limit

| Field | Value |
|---|---|
| Routine ID | `G36-GEN-AEHL__title24-differential-offset-0` |
| Status | `source_evidenced` |
| Evidence | E3 |
| Runtime profile | `HostTick-v1` |

## Purpose

This fixed specialization computes the outdoor-air economizer temperature
cutoff for the California Title 24 differential dry-bulb zero-offset bucket.
The source groups climate zones 1, 3, 5, and 11 through 16 in this bucket. The
fixture selects zone 1 as its representative fixed configuration.

Fixed parameters:

- `eneStd = Buildings.Controls.OBC.ASHRAE.G36.Types.EnergyStandard.California_Title_24`
- `ecoHigLimCon = Buildings.Controls.OBC.ASHRAE.G36.Types.ControlEconomizer.DifferentialDryBulb`
- `tit24CliZon = Buildings.Controls.OBC.ASHRAE.G36.Types.Title24ClimateZone.Zone_1`

## Interface and behavior

| Connector | Direction | Type | Unit | Quantity |
|---|---|---|---|---|
| `TRet` | input | Real scalar | K | ThermodynamicTemperature |
| `TCut` | output | Real scalar | K | ThermodynamicTemperature |

For this specialization:

```text
TCut = TRet
```

The upstream zero-offset bucket selects the direct `TRet` branch. The donor
fixture makes the reduced signal path explicit with
`Buildings.Controls.OBC.CDL.Reals.AddParameter(p=0)`. That identity block is
fixture-local; it is not part of the upstream branch topology. The routine has
no state, optional connectors, or host services beyond the tick call.

![Signal flow](diagram.svg)

## Replay and evidence

`vectors.json` replays all four donor reference rows with zero absolute
tolerance. At 0, 1, 2, and 3 seconds, `TRet` and `TCut` are equal: 289.25,
293.15, 297.5, and 301.75 K. The graph and files under `golden/` are preserved
from the donor and hash-locked in `provenance.json`.

The stable routine ID names the human contract. The evaluator-derived content
ID is recorded separately in `provenance.json`.

## Completeness

- Donor configuration: complete for this fixed variant.
- Canonical class: partial; 6 of 12 donor variants are present.
- Family package: not applicable to this leaf.
- Declared guideline profile: partial.

This card does not claim complete AirEconomizerHighLimits, generic-family,
donor-set, or G36 coverage.

## Exclusions

Arrays, member lists, enum-domain inputs, optional or package connectors,
state, controller traces, other climate-zone offsets, and fixed dry-bulb or
other AirEconomizerHighLimits variants are outside this bundle. Although the
observable mapping equals the ASHRAE differential variant, the fixed
parameters and source branch differ; the two routines are not aliases. E4/E5,
Modelica execution, and EnergyPlus execution are not claimed.

## References

- Modelica Buildings, `Buildings/Controls/OBC/ASHRAE/G36/Generic/AirEconomizerHighLimits.mo`, at `SOURCE_PIN`.
- Open Control Engine fixture and independent oracle files at `DONOR_PIN`.
- Redistribution terms: `LICENSE-BUILDINGS.html` and `THIRD_PARTY_NOTICES.md`.
