---
schema: cxf-library/fault-card/v1
id: TOWER-0006
name: Cooling-tower basin freeze-protection failure
equipment: tower
status: verified
phase: 2
method: rule
severity: 2
category: PROTECTIVE
confidence: MEDIUM
estimation_method: QUALITATIVE_ONLY
source:
  - "EVAPCO, Cooling Towers Operation and Maintenance Instructions (2017), p.21 — remote sump is the most effective idle freeze strategy; basin heaters protect basin water but not external piping and are for idle/no-flow operation"
  - "SPX Cooling Technologies, Cooling Tower Fundamentals, 2nd ed., p.90 — basin heater systems use a thermostat and low-water protection; the example maintains at least 40 °F and warns of fire or heater burnout without water"
  - "SPX Cooling Technologies, Basin Heater System Engineering Data TECH-BH-19 — packaged basin heater control includes temperature control and low-water-level protection"
  - "Library-authored watchdog thresholds/timers; OEM/site freeze plan remains authoritative"
g36: null
clusters: []
suppresses: []
suppressed_by: []
related: []
playbooks: [cooling-tower-performance]
operating_states: "wet cooling tower with water intentionally present in its basin/sump and a monitored heater or equivalent freeze-protection device under automatic control"
preconditions: "Adopt only where the site/OEM freeze plan intentionally retains basin water and makes this monitored heater or equivalent device responsible for protection. Exclude drained-down towers, remote-sump strategies with an empty outdoor basin, dry coolers/fluid coolers, seasonal shutdown, maintenance, manual operation, and any interval in which basin level is below the heater's safe operating requirement. Never use this diagnostic to bypass low-water cutoff, thermostat, over-temperature, fire, electrical, or OEM safeties, and never energize a heater manually from an alarm. Bind tower_basin_heater_cmd to the final post-safety request and status to independent current, power, contactor, or thermal proof rather than a command echo. Basin temperature must represent the bulk water, away from the local heater plume; OAT must represent the tower exposure. Points must be fresh and time-aligned. Configure both minimum_basin_temp and thermal_response_time from the actual OEM/site plan before evaluation; both shipped values are NO_PORTABLE_DEFAULT and otherwise require NO_EVAL. Basin heaters do not protect external piping, pumps, or heat exchangers."
points:
  - oat
  - tower_basin_temp
  - tower_basin_heater_cmd
  - tower_basin_heater_status
outputs:
  - name: yFault
    description: True while either the independent heater-proof lane or the freeze-exposed low-basin-temperature lane has matured
  - name: yHeaterFailToRun
    description: True after a final heater command lacks independent proof for heater_proof_time
  - name: yLowBasinTemp
    description: True after representative basin water remains below its configured minimum during freeze exposure for thermal_response_time
  - name: yFreezeExposure
    description: True while OAT is strictly below freeze_exposure_oat; a diagnostic subcondition only, not a rule-wide data-quality flag (false does not mean NO_EVAL)
params:
  freeze_exposure_oat:
    default: 2.0
    unit: "°C"
    description: "Outdoor exposure threshold. ADOPTED_TUNABLE: review against site climate, sensor bias, wind exposure, and OEM freeze strategy."
    cxf: freezeExposure.t
  minimum_basin_temp:
    default: 4.0
    unit: "°C"
    description: "Minimum representative basin temperature during exposure. NO_PORTABLE_DEFAULT: 4 °C is an adoption-blocking placeholder; replace it with the OEM/site freeze-plan value before evaluation."
    cxf: basinLow.t
  heater_proof_time:
    default: 120.0
    unit: s
    description: "Allowed independent electrical/operating proof delay. ADOPTED_TUNABLE: exceed contactor/current sensing and telemetry latency."
    cxf: heaterProof.delayTime
  thermal_response_time:
    default: 1800.0
    unit: s
    description: "Continuous low bulk-water temperature during exposure required for the thermal alarm. NO_PORTABLE_DEFAULT: 1800 s is an adoption-blocking placeholder; configure for basin volume, heater capacity, circulation, sensor location, and OEM response requirements."
    cxf: thermalProof.delayTime
energy_impact:
  affected_subsystem: Cooling-tower basin, heater circuit, condenser-water plant, and freeze protection
  savings_range: "Not estimated; this is an asset-protection and life-safety-adjacent diagnostic, not an energy-conservation claim"
  climate_sensitivity: cold-climate
  runtime_estimation: "None in-rule — QUALITATIVE_ONLY. Command false/status true is an uncovered protective hazard; a future rule may separately quantify its energy direction."
emissions:
  scope: "2"
  method: QUALITATIVE_EMISSIONS
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:6a926c5a2ff88dc03e2cfebe65d131c0"
  date: 2026-10-05
---

## Description

This watchdog exposes two different failures without pretending either is a
complete freeze-control system. The electrical lane finds a final heater request
that lacks independent proof. The thermal lane finds bulk basin water that
remains below a site/OEM minimum while outdoor conditions meet the configured
freeze-exposure threshold. Either is a prompt to follow the approved freeze
plan—not permission to defeat safeties or energize equipment manually.

## Detection Logic

```
freeze_exposure = oat < freeze_exposure_oat
heater_mismatch = tower_basin_heater_cmd AND NOT tower_basin_heater_status
low_condition   = freeze_exposure AND tower_basin_temp < minimum_basin_temp

yHeaterFailToRun = heater_mismatch sustained for heater_proof_time
yLowBasinTemp     = low_condition sustained for thermal_response_time
yFreezeExposure   = freeze_exposure
yFault            = yHeaterFailToRun OR yLowBasinTemp
```

![TOWER-0006 block graph](diagram.svg)

Both comparators are strict: exactly 2 °C OAT is not exposure and exactly 4 °C
basin temperature is not low under the shipped graph. Each lane has an
independent `TrueDelay(delayOnInit=true)` and clears immediately when its own
condition clears.

## Possible Diagnoses

`yHeaterFailToRun`:

1. Open disconnect, breaker/fuse, contactor, control transformer, or heater element
2. Low-water, thermostat, over-temperature, or OEM safety correctly blocking operation
3. Failed current/power proof or status mapped to the wrong heater circuit
4. Final command is not actually downstream of local thermostat/safety logic

`yLowBasinTemp`:

5. Heater is undersized, failed, staged incorrectly, or not receiving voltage
6. Basin temperature sensor is biased, poorly located, or in a stagnant pocket
7. Basin level, wind exposure, leakage, or unintended circulation exceeds the design basis
8. Site intended drain-down/remote-sump operation but the host failed to suppress evaluation
9. The configured minimum or exposure threshold does not match the OEM freeze plan

## Energy Impact

No energy savings estimate. This is a protective finding whose value is avoided
freeze damage, loss of heat rejection, water release, and unsafe inspection
conditions. The opposite electrical direction—heater proven on with command
off—is deliberately not included and may become a future energy rule.

## Emissions Impact

Scope 2, qualitative only. Avoided repair and refrigerant/water consequences
are real but outside the point set and cannot be defensibly converted to
operational emissions here.

## Deviations

- **`minimum_basin_temp = 4 °C` is not portable.** OEM examples commonly
  describe about 40 °F protection, but strategy, equipment, ambient design,
  circulation, glycol, sensor location, and basin geometry differ. The shipped
  value is an adoption-blocking placeholder, not a universal safety setpoint.
- **The 2 °C exposure threshold and 120 s electrical timer are adopted.** The
  1800 s thermal response is `NO_PORTABLE_DEFAULT` because basin volume, heater
  capacity, circulation, and sensor placement dominate it. No cited source
  publishes this complete watchdog algorithm or these limits.
- **The graph does not encode water level.** Low-water cutoff is a mandatory
  independent safety and a host precondition. Never infer that `yFault=false`
  makes heater operation safe.
- **A proven heater does not suppress low basin temperature.** Electrical proof
  is not thermal adequacy, which is why the two lanes remain independent.
- **Command false/status true is intentionally silent in this graph, not safe.**
  It may indicate stale proof, a stuck contactor, or unintended heater operation
  with property/fire consequences—especially without water. Route it to the
  OEM/site safety workflow immediately; a future rule may separately quantify
  the excess-energy direction.
- **Basin protection is not plant protection.** OEM literature explicitly
  warns that a basin heater does not protect external piping, pumps, or heat
  exchangers; the full site drain-down/circulation plan remains authoritative.
- **No EnergyPlus validation is claimed.** Simulated basin-heater electricity
  is neither an independent command/proof pair nor a validation of the physical
  safety strategy.

## Notes

Follow the site/OEM freeze procedure before field inspection. Confirm basin
level and electrical isolation from a safe location. Do not bypass low-water or
other safeties, and do not manually energize a heater based on this diagnostic.
