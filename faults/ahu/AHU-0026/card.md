---
schema: cxf-library/fault-card/v1
id: AHU-0026
name: Outdoor air damper not closed during unoccupied periods
equipment: ahu
status: verified
phase: 2
method: rule
severity: 3
category: CRITICAL_WASTE
confidence: HIGH
estimation_method: DIRECT_MEASUREMENT
source:
  - "HVAC FDD Reference v1.0 §9, AHU-0026"
  - "PNNL RetuningOpps A11"
  - "PNNL-25985 EEM-06"
g36: null
clusters: [CLU-04]
suppresses: []
suppressed_by: []
related: [AHU-0018, AHU-0017]
playbooks: [after-hours-operation]
operating_states: all
preconditions: "Occupancy schedule data available and current; the host evaluates the schedule (time zone, calendar, holidays) into the boolean occ_schedule point. When schedule provenance is unknown or stale, the verdict is NO_EVAL, not healthy."
points:
  - sf_status
  - occ_schedule
  - oa_dmpr_cmd
outputs:
  - name: yFault
    description: True while the supply fan has run unoccupied with the OA damper commanded above oa_closed_threshold for at least alarm_delay
params:
  oa_closed_threshold:
    default: 5.0
    unit: "%"
    description: Damper command at or below which the outdoor air damper counts as closed
    cxf: dmprOpen.t
  alarm_delay:
    default: 900.0
    unit: s
    description: Continuous fault persistence required before the alarm asserts
    cxf: persist.delayTime
energy_impact:
  affected_subsystem: AHU ventilation (outdoor-air conditioning) energy during unoccupied operation
  savings_range: 3-10% of off-hours energy; 100% of ventilation energy while the fault is active
  climate_sensitivity: heating-dominant
  runtime_estimation: "waste_kw = (oa_dmpr_cmd/100) × design_oa_flow × cp × |OAT − RAT|"
emissions:
  scope: "1+2"
  method: DIRECT_EMISSIONS
verified:
  engine_rev: 41e997f
  content_id: "cxf:fnv1a128:162a0ed14faa4a16fc9bb66a7c1eed88"
  date: 2026-10-05
---

## Description

The air handler runs outside the occupancy schedule with the outdoor air
damper still commanded open. Unoccupied operation — night setback, morning
warmup, a tenant override — needs no ventilation: there is nobody to ventilate
for, so every cubic metre of outdoor air drawn in is heated or cooled to no
purpose. A damper left at its occupied minimum (typically 15–30%) through a
winter night puts the full outdoor-to-return enthalpy difference on the coils
for the entire run. Prevalence is about 10%, and the fix is usually a single
line in the unoccupied sequence. This is the ventilation half of the after-hours
pair: AHU-0018 asks whether the fan should be running at all, this one asks
whether it is dragging in outdoor air while it runs.

## Detection Logic

```
yFault = sf_status
     AND NOT occ_schedule
     AND oa_dmpr_cmd > oa_closed_threshold
     sustained continuously for alarm_delay
```

Block graph (`rule.cxf.jsonld`):

![AHU-0026 block graph](diagram.svg)

`unoccRun` establishes that the fan is running while the schedule says
unoccupied; `dmprOpen` tests the damper command against the closed threshold
with a strict comparison, so a damper parked at exactly 5% (leakage band, or a
minimum-position parameter never zeroed but within tolerance) does not alarm.
`persist` requires the combination to hold for `alarm_delay`, which rides out
the damper stroke at a scheduled occupied→unoccupied transition. Any fan stop,
return to occupancy, or damper close resets the timer, and a damper that reopens
restarts the full 15 min; `delayOnInit = true` holds the window across a
controller restart.

## Possible Diagnoses

1. Minimum OA position not set to 0% in the unoccupied sequence — the damper
   holds its occupied minimum around the clock
2. OA damper stuck open (seized linkage, failed actuator, disconnected crank
   arm)
3. Economizer logic overriding unoccupied damper control — the economizer
   sees a favorable outdoor condition and modulates the damper open without
   consulting occupancy

## Energy Impact

CRITICAL_WASTE, HIGH confidence, DIRECT_MEASUREMENT. The waste is the
conditioning load on unnecessary outdoor air: `waste_kw = (oa_dmpr_cmd/100) ×
design_oa_flow × cp × |OAT − RAT|`. With the damper open, 100% of ventilation
energy during the unoccupied run is waste; measured against total off-hours
energy the correction typically returns 3–10% (PNNL-25985 EEM-06, OA damper
faults; PNNL RetuningOpps A11). Heating-dominant: the winter night is when
|OAT − RAT| is largest.

## Emissions Impact

Scope 1 + 2, DIRECT_EMISSIONS, HIGH confidence; typical 1,000–8,000 kg
CO₂e/yr for the ventilation energy alone. The split follows the heating
source — scope 1 for the gas burned to temper unnecessary outdoor air, scope 2
for the electric heat or cooling. Because the fault runs overnight, the
electric share should be valued at the marginal rather than average grid rate.
Avoided-emissions basis: MOER.

## Deviations

- No grace period, matching the reference, where AHU-0018 grants 30 minutes.
  The difference is deliberate: legitimate unoccupied fan operation — setback,
  warmup, an authorized override — should still run with the OA damper closed,
  so there is no state this rule needs to forgive.
- The reference's logic uses a schedule-evaluation predicate over an occupancy
  schedule object; as in AHU-0018 we consume the host-evaluated boolean
  `occ_schedule` point, leaving time zone, calendar, and holiday
  interpretation to the host (see `points/ahu.points.json` notes on
  `occ_schedule`).
- Severity 3 (warning), per the reference's chapter 9 card — its only severity
  statement for this fault, since the §5.8.1 index carries no severity column.
  This chapter's README previously mistranscribed it as 2, corrected alongside
  this card.
- The reference tags this fault for both AHU and RTU. This is the AHU-family
  instance; an RTU sibling would reuse the same block graph against the RTU
  point dictionary.
- `delayOnInit = true` on `persist`: a controller restart mid-condition still
  waits out the full persistence window (same startup-alarm rationale as
  AHU-0016 and AHU-0018).

## Notes

The remote fix is a sequence edit — zero the minimum OA position in unoccupied
mode and gate the economizer on occupancy — Step 2 of the after-hours-operation
playbook. Firing alongside AHU-0018 (the CLU-04 trigger), fix the schedule
first: restoring the unoccupied period often closes the damper as a side effect.
Firing *without* FC-052, the schedule is right and the damper sequence or the
actuator is wrong.
