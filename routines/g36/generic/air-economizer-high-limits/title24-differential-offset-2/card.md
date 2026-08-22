# Title 24 zones 6/8/9 differential dry-bulb economizer high limit

| Field | Value |
|---|---|
| Routine ID | `G36-GEN-AEHL__title24-differential-offset-2` |
| Status | `source_evidenced` |
| Evidence | E3 |
| Runtime profile | `HostTick-v1` |

## Purpose

This fixed specialization computes the outdoor-air economizer temperature
cutoff for California Title 24 climate zones 6, 8, and 9 with differential
dry-bulb control. The fixture selects zone 6 as the representative fixed
configuration.

Fixed parameters:

- `eneStd = Buildings.Controls.OBC.ASHRAE.G36.Types.EnergyStandard.California_Title_24`
- `ecoHigLimCon = Buildings.Controls.OBC.ASHRAE.G36.Types.ControlEconomizer.DifferentialDryBulb`
- `tit24CliZon = Buildings.Controls.OBC.ASHRAE.G36.Types.Title24ClimateZone.Zone_6`

## Interface and behavior

| Connector | Direction | Type | Unit | Quantity |
|---|---|---|---|---|
| `TRet` | input | Real scalar | K | ThermodynamicTemperature |
| `TCut` | output | Real scalar | K | ThermodynamicTemperature |

For this specialization:

```text
TCut = TRet - 2 K
```

The fixture represents the selected zone 6/8/9 source branch with
`Buildings.Controls.OBC.CDL.Reals.AddParameter` instance `addPar1` and
`p=-2.0`. It has no state, optional connectors, or host services beyond the tick
call.

![Signal flow](diagram.svg)

## Replay and evidence

`vectors.json` replays all four donor reference rows with zero absolute
tolerance: 289.25 → 287.25 K at 0 s, 293.15 → 291.15 K at 1 s, 297.5 →
295.5 K at 2 s, and 301.75 → 299.75 K at 3 s. The graph and files under
`golden/` are preserved from the donor and hash-locked in `provenance.json`.

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
other AirEconomizerHighLimits variants are outside this bundle. E4/E5, Modelica
execution, and EnergyPlus execution are not claimed.

## References

- Modelica Buildings, `Buildings/Controls/OBC/ASHRAE/G36/Generic/AirEconomizerHighLimits.mo`, at `SOURCE_PIN`.
- Open Control Engine fixture and independent oracle files at `DONOR_PIN`.
- Redistribution terms: `LICENSE-BUILDINGS.html` and `THIRD_PARTY_NOTICES.md`.
